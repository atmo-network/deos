//! Full Executive block fixtures for production-Wasm replay.
//!
//! Unlike the Actor-only capacity harness, this includes consensus context,
//! all runtime hooks, inherent order, signed extrinsics, roots and finalization.

use super::{
  actors_integration_tests,
  common::{ALICE, ASSET_A},
};
use crate::{
  Actors, Assets, Balances, Block, Executive, Header, InherentDataExt, Runtime, RuntimeCall,
  RuntimeEvent, RuntimeGenesisConfig, RuntimeOrigin, System, configs::BlockResourceBudgetValue,
};
use codec::Encode;
use cumulus_primitives_parachain_inherent::{INHERENT_IDENTIFIER, ParachainInherentData};
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use pallet_deos_actors::{
  ActorId, ActorType, AmountResolution, CloseReason, ContractSteps, CycleResult, CycleState, Event,
  InputLimit, Mutability, ProcessResidence, ScheduleWindow, StepErrorPolicy, StepOf, Task, Trigger,
  TriggerFamily, WakeupClock, WakeupKey, WeightInfo,
};
use polkadot_sdk::frame_support::{
  BoundedVec, assert_ok,
  dispatch::GetDispatchInfo,
  traits::{StorageInfo, StorageInfoTrait},
  weights::Weight,
};
use polkadot_sdk::{
  cumulus_primitives_core::PersistedValidationData,
  polkadot_parachain_primitives::primitives::HeadData,
  sp_core::{
    H256, Pair, sr25519,
    storage::{Storage, well_known_keys},
  },
  sp_inherents::InherentData,
  sp_io::TestExternalities,
  sp_runtime::{
    BuildStorage, Digest, DigestItem, MultiAddress, Perbill,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
  },
  sp_trie::{
    StorageProof,
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
  sp_weights::WeightToFee,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(not(feature = "runtime-benchmarks"))]
#[path = "../../examples/actor_retention_fixture/wide_contract.rs"]
mod retention_wide_contract;

#[cfg(not(feature = "runtime-benchmarks"))]
#[path = "../../examples/actor_retention_fixture/pvf.rs"]
mod pvf_fixture;
#[cfg(not(feature = "runtime-benchmarks"))]
use crate as pvf_runtime;

const PREPARED_W1_ACTORS: u32 = 128;
const W1_TARGET_CYCLES: u32 = 10_000;
const W1_TARGET_BLOCKS: u32 = 100;
const W3_REPETITIONS_PER_CELL: u32 = 14;
const W3_BLOCK_LIMIT: u32 = 100;
const W4_TRANSFER_ACTORS: u32 = 9_485;
const W4_SWAP_OUT_ACTORS: u32 = 400;
const W4_STOP_CYCLE_ACTORS: u32 = 100;
const W5_BLOCK_LIMIT: u32 = 10;
const W6_BLOCK_LIMIT: u32 = 16;
const W6_RANDOM_SEED: u64 = 0x0066_c10c_5eed;
const CURRENT_W7_SLEEPERS: u32 = 1_024;
const CURRENT_W7_DEADLINE_KEYS: u32 = 32;
const CURRENT_W7_MEMBERS_PER_KEY: u32 = 32;
const CURRENT_W7_SETUP_BATCH: u32 = 16;
const CURRENT_W7_SETUP_BLOCKS: u32 = CURRENT_W7_SLEEPERS / CURRENT_W7_SETUP_BATCH;
const CURRENT_W7_FIRST_DUE_BLOCK: u32 = CURRENT_W7_SETUP_BLOCKS + 2;
const CURRENT_W7_ARRIVAL_BLOCKS: u32 = 32;
const CURRENT_W7_DRAIN_BLOCKS: u32 = 32;
const W7_DUE_ACTORS: u32 = 100;
const W7_FUTURE_WORKLOAD_ACTORS: u32 = 4_943;
const W7_UNSIGNALED_WORKLOAD_ACTORS: u32 = 4_942;
const W7_FUTURE_BLOCK: u32 = 100;
const W7_BLOCK_LIMIT: u32 = 10;
const W8_TOMBSTONE_PREFIXES: [u32; 7] = [1, 4, 8, 16, 32, 64, 128];
const W8_DUE_ACTORS: u32 = 100;
const W9_DUE_ACTORS: u32 = 100;
const CONTROL_ATTRIBUTION_ACTORS: u32 = 100;
const CONTROL_ATTRIBUTION_BLOCKS: u32 = 9;
/// SHA-256 of the current-tree production-profile Wasm rebuilt by `scripts/03-build-runtime.sh`.
/// Exact-Wasm profiles fail closed on other bytes. This identity is not a certificate of
/// complete Weight/resource, host or release acceptance.
const ACCEPTED_PRODUCTION_WASM_SHA256: [u8; 32] = [
  0x8a, 0xd6, 0x6f, 0x3a, 0x63, 0xf0, 0xec, 0x30, 0xcc, 0xd1, 0xfe, 0x8b, 0x4c, 0x35, 0x14, 0x15,
  0x90, 0x3f, 0xa3, 0x38, 0x10, 0x75, 0xe5, 0x6a, 0x7b, 0xbe, 0x01, 0x72, 0xed, 0x5c, 0xb9, 0xb6,
];
const REFERENCE_ACTIVE_SYSTEM_ACTORS: u32 = 3;
const REFERENCE_SYSTEM_ACTOR_IDENTITIES: u32 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UserDemand {
  ActorOnly,
  ContinuousValid,
  ContinuousShared,
  RefTimeHeavy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkloadSchedule {
  ManualOnly,
  CadencedOnly,
  MixedManualCadenced,
}

impl UserDemand {
  fn label(self) -> &'static str {
    match self {
      Self::ActorOnly => "actor-only",
      Self::ContinuousValid => "continuous-valid-user-demand",
      Self::ContinuousShared => "continuous-shared-capacity-user-demand",
      Self::RefTimeHeavy => "ref-time-heavy-valid-user-demand",
    }
  }
}

/// Retained TOP cuts exercise full-length permanent signed births and recorder-adjusted Drain.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires exact retained TOP seed inputs and diagnostic Wasm"]
fn retained_parked_permanent_birth_recorder_frame_and_drain() {
  use codec::Decode;
  use pallet_deos_actors::{
    ActorContract, DependencyRegistrationHeaders, DependencyRevisions,
    DependencyScanSourceListState, DependencyScanSourceNodes, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceRule,
  };
  use primitives::AssetKind;
  fn bytes(value: &serde_json::Value) -> Vec<u8> {
    let encoded = value.as_str().unwrap();
    assert!(encoded.len().is_multiple_of(2));
    (0..encoded.len())
      .step_by(2)
      .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
      .collect()
  }
  let seeds: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::env::var("DEOS_PARKED_SEED_INPUTS").unwrap()).unwrap(),
  )
  .unwrap();
  assert_eq!(seeds.as_array().unwrap().len(), 4);
  let alice = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(alice.public());
  let mut observations = Vec::new();
  let contract = ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![
      StepOf::<Runtime> {
        precondition: None,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      };
      12
    ]
    .try_into()
    .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        (1..=4)
          .map(|asset| ParkedBalanceRule {
            asset: AssetKind::Local(0x7000_0000 + asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  let housekeeping = <Runtime as pallet_deos_actors::Config>::WeightInfo::scheduler_on_idle_base()
    .saturating_add(<Runtime as pallet_deos_actors::Config>::WeightInfo::block_resource_finalize());
  let selector =
    <Runtime as pallet_deos_actors::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(
        <Runtime as pallet_deos_actors::Config>::WeightInfo::service_round_probe_eligible(),
      );
  let required = housekeeping.saturating_add(selector);
  for seed in seeds.as_array().unwrap() {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    let digest = polkadot_sdk::sp_io::hashing::sha2_256(&raw)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    assert_eq!(digest, seed["sha256"].as_str().unwrap());
    let snapshot: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let encoded_header = bytes(&snapshot["header"]);
    let mut encoded = encoded_header.as_slice();
    let mut parent = Header::decode(&mut encoded).unwrap();
    assert!(encoded.is_empty());
    let code = storage.top[well_known_keys::CODE].clone();
    assert_eq!(
      code,
      std::fs::read(std::env::var("DEOS_GOVERNANCE_PVF_WASM").unwrap()).unwrap()
    );
    let start = *parent.number();
    for offset in 1..=6 {
      let number = start + offset;
      let owner_slot = if offset == 1 { 5 } else { 6 };
      let calls = if offset == 1 || offset == 6 {
        vec![RuntimeCall::Actors(
          pallet_deos_actors::Call::create_user_actor_at_slot {
            owner_slot,
            mutability: Mutability::Mutable,
            contract: Some(contract.clone()),
          },
        )]
      } else {
        Vec::new()
      };
      let mut before = externalities_with_bound_code(storage.clone());
      assert_construction_parent_matches_state(&mut before, &parent);
      let (selected, next_source, scan, frontier, nonce, actor_id, identities, accounts) = before
        .execute_with(|| {
          let selected = DependencyScanSourceListState::<Runtime>::get()
            .cursor
            .unwrap();
          (
            selected,
            DependencyScanSourceNodes::<Runtime>::get(selected)
              .unwrap()
              .next,
            DependencyRevisions::<Runtime>::get(selected),
            DependencyRegistrationHeaders::<Runtime>::get(selected).next_index,
            System::account_nonce(&owner),
            Actors::next_actor_id(),
            Actors::actor_identity_count(),
            polkadot_sdk::frame_system::Account::<Runtime>::iter_keys().collect::<BTreeSet<_>>(),
          )
        });
      assert!(accounts.len() <= 71);
      let script = if calls.is_empty() {
        Vec::new()
      } else {
        vec![(&alice, nonce, &calls[0])]
      };
      let authored = author_complete_block_after_with_context(
        storage,
        &code,
        &parent,
        number,
        UserDemand::ActorOnly,
        &alice,
        nonce,
        &BTreeMap::new(),
        &script,
        inherent_data_for(&parent, number),
      );
      let recorded_proof_bytes = authored.execution_recorded_proof_bytes.unwrap();
      assert!(recorded_proof_bytes > 0);
      assert!(!authored.execution_backend_keys.is_empty());
      let frame_remaining = authored
        .metrics
        .pre_idle_remaining
        .checked_sub(&crate::XcmpQueue::on_idle_weight())
        .unwrap();
      assert!(
        required.all_lte(frame_remaining),
        "recorder-adjusted FRAME must admit housekeeping and source selection"
      );
      let checked = assert_complete_block_replays_with_aura(&authored);
      let mut after = externalities_with_bound_code(authored.post_state.clone());
      assert_construction_parent_matches_state(&mut after, &checked);
      let (normal, registered, fees, hold, account_count) = after.execute_with(|| {
        assert_eq!(
          System::account_nonce(&owner),
          nonce + u32::try_from(calls.len()).unwrap()
        );
        assert_eq!(
          Actors::next_actor_id(),
          actor_id + u64::try_from(calls.len()).unwrap()
        );
        assert_eq!(
          Actors::actor_identity_count(),
          identities + u32::try_from(calls.len()).unwrap()
        );
        let current_accounts =
          polkadot_sdk::frame_system::Account::<Runtime>::iter_keys().collect::<BTreeSet<_>>();
        assert!(accounts.is_subset(&current_accounts));
        let new_accounts = current_accounts
          .difference(&accounts)
          .cloned()
          .collect::<Vec<_>>();
        assert!(
          new_accounts.is_empty() || new_accounts == vec![crate::CollatorSelection::account_id()]
        );
        assert!(current_accounts.len() <= 71);
        let mut hold = 0u128;
        if !calls.is_empty() {
          let sovereign = Actors::sovereign_account_id(&owner, owner_slot);
          let identity = Actors::actor_identity(actor_id).unwrap();
          assert_eq!(identity.owner, owner);
          assert_eq!(identity.sovereign_account, sovereign);
          assert_eq!(identity.mutability, Mutability::Mutable);
          assert_eq!(identity.cycle_nonce, 0);
          assert_eq!(Actors::actor_contract(actor_id).unwrap(), contract);
          assert_eq!(
            Actors::actor_hot(actor_id).unwrap().cycle_state,
            CycleState::Idle
          );
          match Actors::actor_semantic_states(actor_id).unwrap() {
            pallet_deos_actors::ActorSemanticState::Active(record) => {
              assert_eq!(record.generation, 1)
            }
            _ => panic!("new contract must have active semantic authority"),
          }
          assert!(!polkadot_sdk::frame_system::Account::<Runtime>::contains_key(&sovereign));
          assert_eq!(System::providers(&sovereign), 0);
          assert_eq!(Balances::free_balance(&sovereign), 0);
          let record = Actors::actor_state_hold(actor_id).unwrap();
          assert_eq!(record.owner, owner);
          let b = record.breakdown;
          hold = b.identity + b.contract_head + b.contract_body + b.detector + b.run;
          assert_eq!(hold, 5_366_000_000);
        }
        let fees = System::events()
          .iter()
          .filter_map(|record| match &record.event {
            RuntimeEvent::TransactionPayment(
              polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
                who,
                actual_fee,
                tip,
              },
            ) => {
              assert_eq!(who, &owner);
              assert_eq!(*tip, 0);
              assert!(*actual_fee > 0);
              Some(*actual_fee)
            }
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(fees.len(), calls.len());
        assert!(Actors::block_resource_state().is_none());
        let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
        assert!(!telemetry.optional_actor_work_halted());
        assert_eq!(
          DependencyScanSourceListState::<Runtime>::get().cursor,
          Some(next_source)
        );
        let current = DependencyRevisions::<Runtime>::get(selected);
        assert_eq!(
          current.scan_cursor,
          if scan.scan_cursor < scan.scan_end {
            scan.scan_cursor + 1
          } else {
            0
          }
        );
        if scan.scan_cursor == scan.scan_end {
          assert_eq!(current.scan_target, Some(current.revision));
          assert!(current.scan_end >= frontier);
        }
        let normal =
          *System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal);
        if calls.is_empty() {
          assert_eq!(normal, Weight::zero());
        }
        (
          normal,
          System::block_weight().total(),
          fees,
          hold,
          current_accounts.len(),
        )
      });
      assert!(registered.all_lte(crate::MAXIMUM_BLOCK_WEIGHT));
      println!(
        "Permanent birth recorder Drain: preset={} phase={} block={number} offset={offset} signed_calls={} recorded_proof={} backend_keys={} pre_idle_ref={} pre_idle_proof={} normal_ref={} normal_proof={} final_ref={} final_proof={} required_ref={} required_proof={} source_advanced=true author_import_equal=true",
        seed["preset"].as_str().unwrap(),
        seed["phase"].as_str().unwrap(),
        calls.len(),
        recorded_proof_bytes,
        authored.execution_backend_keys.len(),
        authored.metrics.pre_idle_remaining.ref_time(),
        authored.metrics.pre_idle_remaining.proof_size(),
        normal.ref_time(),
        normal.proof_size(),
        registered.ref_time(),
        registered.proof_size(),
        required.ref_time(),
        required.proof_size()
      );
      let signed_length = if calls.is_empty() {
        0
      } else {
        authored.block.extrinsics.last().unwrap().encoded_size()
      };
      let call_weight = calls
        .first()
        .map(|call| call.get_dispatch_info().call_weight)
        .unwrap_or_else(Weight::zero);
      observations.push(serde_json::json!({
        "preset": seed["preset"], "phase": seed["phase"], "block": number, "offset": offset,
        "signedCalls": calls.len(), "ownerSlot": if calls.is_empty() { None } else { Some(owner_slot) },
        "actorId": if calls.is_empty() { None } else { Some(actor_id) },
        "contractSteps": if calls.is_empty() { 0 } else { 12 }, "watchCount": if calls.is_empty() { 0 } else { 4 },
        "signedLength": signed_length, "declaredCallWeight": [call_weight.ref_time(), call_weight.proof_size()],
        "actualTransactionFees": fees, "actorStateHold": hold,
        "systemAccountCount": account_count, "sovereignNativeAccountAbsent": true,
        "recordedProof": recorded_proof_bytes, "backendKeyCount": authored.execution_backend_keys.len(),
        "preIdleRemaining": [authored.metrics.pre_idle_remaining.ref_time(), authored.metrics.pre_idle_remaining.proof_size()],
        "normalWeight": [normal.ref_time(), normal.proof_size()], "finalWeight": [registered.ref_time(), registered.proof_size()],
        "sourceAdvanced": true, "scanAdvanced": true, "authorImportEqual": true,
      }));
      parent = checked;
      storage = authored.post_state;
    }
  }
  std::fs::write(
    std::env::var("DEOS_PERMANENT_BIRTH_OUTPUT").unwrap(),
    serde_json::to_vec_pretty(&observations).unwrap(),
  )
  .unwrap();
}

impl WorkloadSchedule {
  fn uses_manual_trigger(self, index: u32) -> bool {
    match self {
      Self::ManualOnly => true,
      Self::CadencedOnly => false,
      Self::MixedManualCadenced => index.is_multiple_of(2),
    }
  }

  fn label(self) -> &'static str {
    match self {
      Self::ManualOnly => "manual-only",
      Self::CadencedOnly => "cadenced-only",
      Self::MixedManualCadenced => "mixed-manual-cadenced",
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkloadTask {
  Transfer,
  RouterSwapOut,
  StopCycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkloadActorProfile {
  step_count: u32,
  opening_predicates_per_step: u32,
  task: Option<WorkloadTask>,
}

struct PreparedActorFixture {
  storage: Storage,
  actor_ids: Vec<ActorId>,
  actor_profiles: BTreeMap<ActorId, WorkloadActorProfile>,
  signer: sr25519::Pair,
  next_control_maximum: Weight,
  next_effect_maximum: Weight,
}

struct PreparedW5Fixture {
  actors: PreparedActorFixture,
  zero_step: ActorId,
  running: ActorId,
  retry_prefix: ActorId,
  productive_cleanup: ActorId,
  apoptosis: ActorId,
  retry_sovereign: crate::AccountId,
  retry_balance_before: u128,
}

struct PreparedW6Fixture {
  actors: PreparedActorFixture,
  dense_manual: Vec<ActorId>,
  sparse_block: Vec<ActorId>,
  sparse_tick: Vec<ActorId>,
  randomized_cadenced: Vec<ActorId>,
  randomized_periods: Vec<u64>,
  paused_actor: ActorId,
  closed_actor: ActorId,
}

struct PreparedW7Fixture {
  actors: PreparedActorFixture,
  due: Vec<ActorId>,
  future: Vec<ActorId>,
  unsignaled: Vec<ActorId>,
  reference_identities: u32,
}

struct PreparedW8Fixture {
  actors: PreparedActorFixture,
  due: Vec<ActorId>,
}

#[derive(Debug)]
struct W7CampaignResult {
  steps_per_block: Vec<u64>,
  control_ref_time: Vec<u64>,
  control_proof_size: Vec<u64>,
  effect_ref_time: Vec<u64>,
  effect_proof_size: Vec<u64>,
  execution_storage_proof: Vec<u64>,
  execution_compact_proof: Vec<u64>,
  verification_storage_proof: Vec<u64>,
  verification_compact_proof: Vec<u64>,
}

#[derive(Debug)]
struct CurrentW7DeadlineSnapshot {
  handles: BTreeMap<ActorId, u32>,
  service: BTreeSet<ActorId>,
  suspended: BTreeSet<ActorId>,
  closed: BTreeSet<ActorId>,
  block_headers: BTreeMap<u32, u32>,
  block_index_len: u32,
}

#[derive(Clone, Debug)]
struct FullExecutiveBlockMetrics {
  actor_steps: u32,
  distinct_actors: u32,
  progressed_steps: Vec<(ActorId, u32)>,
  opening_steps: u32,
  middle_steps: u32,
  final_steps: u32,
  transfer_steps: u32,
  swap_out_steps: u32,
  stop_cycle_steps: u32,
  non_successful_steps: u32,
  completed_cycles: u32,
  completed_cycle_actors: Vec<ActorId>,
  failed_cycle_actors: Vec<ActorId>,
  suspended_steps: Vec<(ActorId, u32)>,
  continued_steps: Vec<(ActorId, u32)>,
  closed_actors: Vec<(ActorId, CloseReason)>,
  paused_actors: Vec<ActorId>,
  resumed_actors: Vec<ActorId>,
  trigger_occurrences: Vec<(ActorId, TriggerFamily)>,
  user_calls: u32,
  next_user_weight: Option<Weight>,
  user_admission_refusal: Option<TransactionValidityError>,
  prepass_steps: u32,
  prepass_trigger_occurrences: u32,
  prepass_actor_control: Weight,
  prepass_actor_effect: Weight,
  pre_idle_remaining: Weight,
  actor_control: Weight,
  actor_effect: Weight,
  user_dispatch: Weight,
  queue_head: u64,
  queue_tail: u64,
}

#[derive(Clone, Debug)]
struct WasmProofMetrics {
  execution_storage_proof_bytes: u64,
  execution_compact_proof_bytes: u64,
  verification_storage_proof_bytes: u64,
  verification_compact_proof_bytes: u64,
  execution_recorded_keys: BTreeSet<(H256, Vec<u8>)>,
  remaining_residual_node_bytes: usize,
}

struct AuthoredBlock {
  pre_state: Storage,
  post_state: Storage,
  block: Block,
  inherent_data: InherentData,
  metrics: FullExecutiveBlockMetrics,
  execution_backend_keys: BTreeSet<(H256, Vec<u8>)>,
  execution_overlay_keys: BTreeSet<(Option<Vec<u8>>, Vec<u8>)>,
  #[cfg(not(feature = "runtime-benchmarks"))]
  execution_recorded_proof_bytes: Option<u64>,
}

#[derive(Debug)]
struct ControlAttributionResult {
  steps: Vec<u32>,
  prepass_steps: Vec<u32>,
  trigger_occurrences: Vec<u32>,
  prepass_control: Vec<Weight>,
  final_control: Vec<Weight>,
  wasm_proofs: Vec<WasmProofMetrics>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DatabaseIo {
  reads: u64,
  writes: u64,
}

impl DatabaseIo {
  const fn new(reads: u64, writes: u64) -> Self {
    Self { reads, writes }
  }

  const fn saturating_add(self, other: Self) -> Self {
    Self {
      reads: self.reads.saturating_add(other.reads),
      writes: self.writes.saturating_add(other.writes),
    }
  }

  const fn saturating_mul(self, multiplier: u64) -> Self {
    Self {
      reads: self.reads.saturating_mul(multiplier),
      writes: self.writes.saturating_mul(multiplier),
    }
  }
}

fn merge_genesis_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
  match (target, patch) {
    (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
      for (key, value) in patch {
        merge_genesis_patch(target.entry(key).or_insert(serde_json::Value::Null), value);
      }
    }
    (target, patch) => *target = patch,
  }
}

#[test]
fn declared_presets_disable_user_creation_without_freezing_system_topology() {
  for preset in crate::genesis_config_presets::preset_names() {
    TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
      assert!(!Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      assert!(Actors::active_actor_count() > 0);
      System::set_block_number(1);
      let before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      polkadot_sdk::frame_support::assert_noop!(
        Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, None),
        pallet_deos_actors::Error::<Runtime>::UserCreationDisabled
      );
      polkadot_sdk::frame_support::assert_noop!(
        Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(ALICE),
          0,
          Mutability::Mutable,
          None
        ),
        pallet_deos_actors::Error::<Runtime>::UserCreationDisabled
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        before
      );
      let next_id = Actors::next_actor_id();
      let active_count = Actors::active_actor_count();
      for frozen in [false, true] {
        polkadot_sdk::frame_support::assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          frozen
        ));
        for enabled in [false, true, false] {
          polkadot_sdk::frame_support::assert_ok!(Actors::set_user_creation_enabled(
            RuntimeOrigin::root(),
            enabled
          ));
          assert_eq!(Actors::user_creation_enabled(), enabled);
          assert_eq!(Actors::global_circuit_breaker(), frozen);
          System::assert_last_event(RuntimeEvent::Actors(Event::UserCreationEnabledSet {
            enabled,
          }));
          assert_eq!(Actors::next_actor_id(), next_id);
          assert_eq!(Actors::active_actor_count(), active_count);
        }
      }
    });
  }
}

#[test]
fn session_authority_bound_covers_declared_genesis_presets() {
  use polkadot_sdk::frame_support::traits::Get;

  let maximum: u32 = <Runtime as polkadot_sdk::pallet_aura::Config>::MaxAuthorities::get();
  let roster_maximum: u32 =
    <Runtime as polkadot_sdk::pallet_collator_selection::Config>::MaxInvulnerables::get();
  assert_eq!(maximum, roster_maximum);
  for preset in crate::genesis_config_presets::preset_names() {
    TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
      let validators = polkadot_sdk::pallet_session::Validators::<Runtime>::get();
      let queued = polkadot_sdk::pallet_session::QueuedKeys::<Runtime>::get();
      let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
      assert!(!validators.is_empty());
      assert!(validators.len() <= maximum as usize);
      assert_eq!(queued.len(), validators.len());
      assert_eq!(authorities.len(), validators.len());
      assert_eq!(
        authorities.into_inner(),
        queued
          .into_iter()
          .map(|(_, keys)| keys.aura)
          .collect::<Vec<_>>(),
      );
    });
  }
}

#[test]
fn trusted_genesis_session_callbacks_preserve_empty_security_obligations() {
  use crate::{Session, SessionRotation, Staking};
  use polkadot_sdk::frame_support::{assert_noop, traits::Hooks};

  for preset in crate::genesis_config_presets::preset_names() {
    TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
      let assert_empty = || {
        assert!(
          pallet_staking::NativeSecurityRewardPots::<Runtime>::iter_keys()
            .next()
            .is_none()
        );
        assert!(
          pallet_staking::NativeSecurityEpochSnapshots::<Runtime>::iter_keys()
            .next()
            .is_none()
        );
        assert!(Staking::active_native_security_epoch_snapshot().is_none());
        assert_eq!(Staking::native_security_reward_liability(), 0);
      };
      assert_eq!(
        Staking::native_security_mode(),
        pallet_staking::NativeSecurityMode::TrustedSet
      );
      assert_empty();
      assert_noop!(
        Staking::open_native_security_epoch(0, &[]),
        pallet_staking::Error::<Runtime>::NativeSecurityModeInactive,
      );
      assert_noop!(
        Staking::activate_native_security_epoch(0),
        pallet_staking::Error::<Runtime>::NativeSecurityModeInactive,
      );
      assert_noop!(
        Staking::fund_native_security_reward(RuntimeOrigin::root(), 1),
        pallet_staking::Error::<Runtime>::NativeSecurityModeInactive,
      );
      let source = crate::configs::staking_config::SecurityRewardFundingSource::get();
      assert_noop!(
        Staking::certify_native_security_reward_funding(&source, 0, 1),
        pallet_staking::Error::<Runtime>::NativeSecurityModeInactive,
      );
      let reward_account = Staking::native_security_reward_account();
      let balance_before = Balances::free_balance(&reward_account);
      assert_ok!(Balances::transfer_keep_alive(
        RuntimeOrigin::signed(polkadot_sdk::sp_keyring::Sr25519Keyring::Alice.to_account_id()),
        reward_account.clone().into(),
        crate::EXISTENTIAL_DEPOSIT,
      ));
      assert_empty();
      let rotation_prefixes =
        [b"System".as_slice(), b"Session", b"Aura"].map(polkadot_sdk::sp_io::hashing::twox_128);
      let preserved_state = || {
        let mut storage = current_top_storage();
        storage.top.retain(|key, _| {
          !rotation_prefixes
            .iter()
            .any(|prefix| key.starts_with(prefix))
        });
        (
          storage.top,
          polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
        )
      };
      let state_before = preserved_state();
      let validators = polkadot_sdk::pallet_session::Validators::<Runtime>::get();
      let queued = polkadot_sdk::pallet_session::QueuedKeys::<Runtime>::get();
      let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
      let horizon = crate::configs::staking_config::SecurityRewardClaimHorizon::get();
      for epoch in 1..=horizon + 2 {
        let block = crate::configs::Period::get() * epoch;
        System::set_block_number(block);
        assert_ne!(SessionRotation::on_initialize(block), Weight::zero());
        assert_eq!(Session::current_index(), epoch);
        assert_empty();
        assert_eq!(preserved_state(), state_before);
        assert_eq!(
          polkadot_sdk::pallet_session::Validators::<Runtime>::get(),
          validators
        );
        assert_eq!(
          polkadot_sdk::pallet_session::QueuedKeys::<Runtime>::get(),
          queued
        );
        assert_eq!(
          polkadot_sdk::pallet_aura::Authorities::<Runtime>::get(),
          authorities
        );
      }
      assert_eq!(
        Balances::free_balance(&reward_account),
        balance_before + crate::EXISTENTIAL_DEPOSIT
      );
    });
  }
}

#[test]
fn authorship_rewards_preserve_construction_custody_on_declared_presets() {
  use pallet_deos_actors::adapters::SovereignAccountPolicy;
  use polkadot_sdk::frame_support::traits::Hooks;
  use polkadot_sdk::sp_keyring::Sr25519Keyring;

  let mut owners = Sr25519Keyring::well_known()
    .map(|key| key.to_account_id())
    .collect::<Vec<_>>();
  assert_eq!(owners.len(), 12);
  owners
    .extend((0..28).map(|index| crate::AccountId::from(Sr25519Keyring::numeric(index).public())));
  assert_eq!(owners.iter().collect::<BTreeSet<_>>().len(), 40);
  for preset in crate::genesis_config_presets::preset_names() {
    TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
      let pot = crate::CollatorSelection::account_id();
      let validators = polkadot_sdk::pallet_session::Validators::<Runtime>::get();
      let reward_accounts = validators.iter().cloned().chain(std::iter::once(pot.clone())).collect::<BTreeSet<_>>();
      assert!(!validators.is_empty());
      assert!(!validators.contains(&pot));
      let slots = <Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get();
      let owner_accounts = owners.iter().cloned().collect::<BTreeSet<_>>();
      let mut user_sovereigns = BTreeSet::new();
      for owner in &owners {
        assert!(owner.encode().iter().any(|byte| *byte != 0));
        assert_ne!(owner, &pot);
        for slot in 0..slots {
          let custody = Actors::sovereign_account_id(owner, u8::try_from(slot).unwrap());
          assert!(!reward_accounts.contains(&custody));
          assert!(!owner_accounts.contains(&custody));
          assert!(!crate::configs::actor_config::DeosSovereignAccountPolicy::is_reserved(
            &custody
          ));
          assert!(user_sovereigns.insert(custody));
        }
      }
      assert_eq!(user_sovereigns.len(), owners.len() * usize::from(slots));
      for actor in 0..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID {
        let custody = Actors::sovereign_account_id_system(actor);
        assert!(!reward_accounts.contains(&custody));
        assert!(!owner_accounts.contains(&custody));
        assert!(!user_sovereigns.contains(&custody));
      }
      let pool = crate::Staking::pool_account_for(
        <Runtime as pallet_staking::Config>::NativeStakingAssetId::get(),
      );
      assert!(!owner_accounts.contains(&pool));
      assert!(!user_sovereigns.contains(&pool));
      let allowed_prefixes = [b"System".as_slice(), b"Authorship", b"CollatorSelection"]
        .map(polkadot_sdk::sp_io::hashing::twox_128);
      let preserved_state = || {
        let mut storage = current_top_storage();
        storage
          .top
          .retain(|key, _| !allowed_prefixes.iter().any(|prefix| key.starts_with(prefix)));
        storage.top
      };
      let mut block = 0;
      for top_up in [0, 7 * crate::EXISTENTIAL_DEPOSIT + 1] {
        if top_up > 0 {
          assert_ok!(Balances::transfer_keep_alive(
            RuntimeOrigin::signed(owners[0].clone()),
            pot.clone().into(),
            top_up,
          ));
        }
        for _ in &validators {
          block += 1;
          System::initialize(
            &block,
            &Default::default(),
            &Digest {
              logs: vec![DigestItem::PreRuntime(
                polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
                u64::from(block).encode(),
              )],
            },
          );
          let author = validators[block as usize % validators.len()].clone();
          let reward = Balances::free_balance(&pot)
            .saturating_sub(crate::EXISTENTIAL_DEPOSIT)
            / 2;
          let mut expected_accounts =
            polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>();
          if reward > 0 {
            expected_accounts.get_mut(&pot).unwrap().data.free -= reward;
            expected_accounts.get_mut(&author).unwrap().data.free += reward;
          }
          let state_before = preserved_state();
          crate::Authorship::on_initialize(block);
          assert_eq!(crate::Authorship::author(), Some(author.clone()));
          assert_eq!(
            polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::get(&author),
            block,
          );
          assert_eq!(preserved_state(), state_before);
          assert_eq!(
            polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
            expected_accounts,
          );
          crate::Authorship::on_finalize(block);
        }
      }
      println!(
        "Construction authorship: preset={preset} owners={} slots={slots} unique_custody={} wallets_disjoint=true system_pool_disjoint=true validators={} callbacks={block} reserved_policy_clear=true split_recipients_disjoint=true owners_nonzero=true",
        owners.len(),
        user_sovereigns.len(),
        validators.len(),
      );
    });
  }
}

/// Negative review uses the fixed custody/watch episode across creator and body length.
/// Read-only observers stay outside both native executions and their key recording.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn parked_negative_reviews_preserve_creator_and_body_projection_in_complete_native_blocks() {
  use pallet_deos_actors::{
    ActorContract, ActorContractTailChunks, ActorProcesses, BalanceDependencySources,
    DependencyRegistrationPositions, DependencyRegistrations, DependencyRevisions,
    DependencyTimedReviews, FundingSourcePolicy, OnUnfunded, ParkedBalanceActivationOf,
    ParkedBalanceEpisodes, ParkedBalanceRule, ServiceNodes,
  };
  use primitives::AssetKind;

  let alice = sr25519::Pair::from_string("//Alice", None).unwrap();
  let numeric = polkadot_sdk::sp_keyring::Sr25519Keyring::numeric(0);
  let owners = [&alice, &numeric].map(|pair| crate::AccountId::from(pair.public()));
  let watches = <Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get();
  assert_eq!(watches, 1);
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let mut storage = ext.execute_with(current_top_storage);
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let mut block = 0;
    let mut advance = |calls: &[(&sr25519::Pair, RuntimeCall)]| {
      block += 1;
      let (alice_nonce, nonces) = TestExternalities::new(storage.clone()).execute_with(|| {
        let mut next = BTreeMap::new();
        let nonces = calls
          .iter()
          .map(|(pair, _)| {
            let account = crate::AccountId::from(pair.public());
            let nonce = next
              .entry(account.clone())
              .or_insert_with(|| System::account_nonce(&account));
            let current = *nonce;
            *nonce = current.checked_add(1).unwrap();
            current
          })
          .collect::<Vec<_>>();
        (System::account_nonce(&owners[0]), nonces)
      });
      let script = calls
        .iter()
        .zip(nonces)
        .map(|((pair, call), nonce)| (*pair, nonce, call))
        .collect::<Vec<_>>();
      let authored = author_complete_block_after_with_context(
        core::mem::take(&mut storage),
        &[],
        &parent,
        block,
        UserDemand::ActorOnly,
        &alice,
        alice_nonce,
        &BTreeMap::new(),
        &script,
        inherent_data_for(&parent, block),
      );
      assert_complete_block_replays_natively(&authored, &[]);
      #[cfg(feature = "try-runtime")]
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(block).unwrap();
      });
      parent = authored.block.header.clone();
      storage = authored.post_state.clone();
      authored
    };

    advance(&[(
      &alice,
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(owners[1].clone()),
        value: 30_000 * crate::UNIT,
      }),
    )]);
    let mut assets = Vec::new();
    let mut paired = AssetKind::Local(0);
    for _ in 0..watches {
      let authored = advance(&[
        (
          &alice,
          RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
            asset_a: AssetKind::Native,
            asset_b: paired,
          }),
        ),
        (
          &alice,
          RuntimeCall::AssetConversion(
            polkadot_sdk::pallet_asset_conversion::Call::add_liquidity {
              asset1: Box::new(AssetKind::Native),
              asset2: Box::new(paired),
              amount1_desired: crate::UNIT,
              amount2_desired: crate::UNIT / 4,
              amount1_min: 1,
              amount2_min: 1,
              mint_to: owners[0].clone(),
            },
          ),
        ),
      ]);
      let lp = TestExternalities::new(authored.post_state).execute_with(|| {
        polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((AssetKind::Native, paired))
          .unwrap()
          .lp_token
      });
      assets.push(lp);
      paired = AssetKind::Local(lp);
    }
    let contracts = [
      1,
      <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get(),
    ]
    .map(|steps| ActorContract {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
      window: None,
      steps: vec![
        StepOf::<Runtime> {
          precondition: None,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        };
        steps as usize
      ]
      .try_into()
      .unwrap(),
      on_unfunded: OnUnfunded::Close,
      close_when: None,
      parked_balance_activation: Some(
        ParkedBalanceActivationOf::<Runtime>::try_from_rules(
          assets
            .iter()
            .map(|asset| ParkedBalanceRule {
              asset: AssetKind::Local(*asset),
              authored_min_delta: 100,
            })
            .collect(),
        )
        .unwrap(),
      ),
      funding: FundingSourcePolicy::OwnerOnly,
      auto_close_at_cycle_nonce: None,
    });
    let calls = [&alice, &numeric]
      .into_iter()
      .flat_map(|pair| {
        contracts.iter().enumerate().map(move |(slot, contract)| {
          (
            pair,
            RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor_at_slot {
              owner_slot: slot as u8,
              mutability: Mutability::Mutable,
              contract: Some(contract.clone()),
            }),
          )
        })
      })
      .collect::<Vec<_>>();
    let created = advance(&calls);
    let baseline = TestExternalities::new(created.post_state).execute_with(|| {
      (15..19)
        .map(|actor| {
          let identity = Actors::actor_identity(actor).unwrap();
          let episode = ParkedBalanceEpisodes::<Runtime>::get(actor).unwrap();
          assert_eq!(identity.owner, owners[(actor - 15) as usize / 2]);
          assert_eq!(
            Actors::actor_contract(actor).unwrap(),
            contracts[(actor - 15) as usize % 2]
          );
          assert_eq!(episode.watches.len(), watches as usize);
          assert!(episode.watches.iter().all(|watch| watch.anchor == 0
            && watch.certified_minimum_balance == 1
            && watch.authored_min_delta == 100));
          (
            actor,
            identity,
            episode,
            Actors::actor_state_hold(actor).unwrap(),
          )
        })
        .collect::<Vec<_>>()
    });
    let mut reviews = BTreeMap::<ActorId, u32>::new();
    let mut previous_due = BTreeMap::new();
    for round in 0..16 {
      // Move existing LP between non-Actor accounts: source publication changes,
      // while each compared Actor's fixed zero anchor and custody stay unchanged.
      let calls = assets
        .iter()
        .map(|asset| {
          (
            &alice,
            RuntimeCall::Assets(polkadot_sdk::pallet_assets::Call::transfer {
              id: *asset,
              target: MultiAddress::Id(owners[1].clone()),
              amount: 1,
            }),
          )
        })
        .collect::<Vec<_>>();
      let authored = advance(&calls);
      for (_, key) in &authored.execution_backend_keys {
        for (actor, _, _, _) in &baseline {
          for chunk in 0..3 {
            assert_ne!(
              key,
              &ActorContractTailChunks::<Runtime>::hashed_key_for(actor, chunk),
              "negative review must not load unreachable body tails"
            );
          }
        }
      }
      TestExternalities::new(authored.post_state).execute_with(|| {
        assert!(
          !Actors::finalized_block_resource_telemetry()
            .unwrap()
            .optional_actor_work_halted()
        );
        for (actor, identity, episode, hold) in &baseline {
          assert_eq!(Actors::actor_identity(*actor), Some(identity.clone()));
          assert_eq!(
            Actors::actor_contract(*actor).unwrap(),
            contracts[(*actor - 15) as usize % 2]
          );
          assert_eq!(
            ParkedBalanceEpisodes::<Runtime>::get(actor)
              .unwrap()
              .encode(),
            episode.encode()
          );
          assert_eq!(Actors::actor_state_hold(*actor), Some(hold.clone()));
          assert_eq!(Balances::free_balance(&identity.sovereign_account), 0);
          assert!(
            assets
              .iter()
              .all(|asset| Assets::balance(*asset, &identity.sovereign_account) == 0)
          );
          assert!(!ServiceNodes::<Runtime>::contains_key(actor));
          assert!(Actors::actor_run_state(*actor).is_none());
          assert!(matches!(
            ActorProcesses::<Runtime>::get(actor).unwrap().residence,
            Some(ProcessResidence::Parked(_))
          ));
          let due = DependencyTimedReviews::<Runtime>::get(actor)
            .unwrap()
            .deadline;
          if previous_due
            .insert(*actor, due)
            .is_some_and(|previous| previous != due)
          {
            *reviews.entry(*actor).or_default() += 1;
          }
          for (index, asset) in assets.iter().enumerate() {
            let source =
              BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)).unwrap();
            assert!(!DependencyRevisions::<Runtime>::get(source).exhausted);
            assert_eq!(
              DependencyRegistrationPositions::<Runtime>::get(source, actor).unwrap(),
              pallet_deos_actors::DependencyRegistrationPosition {
                page: 0,
                slot: (*actor - 15) as u8
              }
            );
            let handle = DependencyRegistrations::<Runtime>::get(source, actor).unwrap();
            assert_eq!(handle.actor, episode.owner.actor);
            assert_eq!(handle.plan_revision, episode.owner.plan_revision);
            assert_eq!(episode.watches[index].asset, AssetKind::Local(*asset));
          }
        }
      });
      if round == 15 {
        assert_eq!(reviews.len(), 4);
        assert!(reviews.values().all(|count| *count >= 2));
      }
    }
    println!(
      "Parked negative projection: preset={preset} complete_blocks={block} creators=2 bodies=1/12 watched_assets={watches} publications={} repeated_reviews={reviews:?}; identity/contract/hold/anchor/custody preserved; unreachable tails absent",
      16 * watches,
    );
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn empty_message_hooks_preserve_construction_domain_through_complete_native_blocks() {
  use crate::configs::AssetKind;
  use polkadot_sdk::frame_support::traits::fungibles::Inspect;
  use polkadot_sdk::sp_keyring::Sr25519Keyring;
  use polkadot_sdk::{cumulus_pallet_parachain_system as parachain, pallet_message_queue};

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let prefixes = [
    b"MessageQueue".as_slice(),
    b"XcmpQueue",
    b"PolkadotXcm",
    b"CumulusXcm",
  ]
  .map(polkadot_sdk::sp_io::hashing::twox_128);
  let message_state = || {
    let mut storage = current_top_storage();
    storage
      .top
      .retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix)));
    assert!(pallet_message_queue::ServiceHead::<Runtime>::get().is_none());
    assert!(
      pallet_message_queue::BookStateFor::<Runtime>::iter_keys()
        .next()
        .is_none()
    );
    assert!(
      pallet_message_queue::Pages::<Runtime>::iter_keys()
        .next()
        .is_none()
    );
    for (pallet, item) in [
      (
        b"PolkadotXcm".as_slice(),
        b"VersionDiscoveryQueue".as_slice(),
      ),
      (b"XcmpQueue".as_slice(), b"OutboundXcmpStatus".as_slice()),
    ] {
      let key = [
        polkadot_sdk::sp_io::hashing::twox_128(pallet),
        polkadot_sdk::sp_io::hashing::twox_128(item),
      ]
      .concat();
      assert!(polkadot_sdk::sp_io::storage::get(&key).is_none_or(|value| value.as_ref() == [0]));
    }
    for (pallet, item) in [
      (b"PolkadotXcm".as_slice(), b"CurrentMigration".as_slice()),
      (b"XcmpQueue".as_slice(), b"InboundXcmpStatus".as_slice()),
    ] {
      let key = [
        polkadot_sdk::sp_io::hashing::twox_128(pallet),
        polkadot_sdk::sp_io::hashing::twox_128(item),
      ]
      .concat();
      assert!(polkadot_sdk::sp_io::storage::get(&key).is_none());
    }
    assert!(parachain::PendingUpwardMessages::<Runtime>::get().is_empty());
    assert!(parachain::PendingUpwardSignals::<Runtime>::get().is_empty());
    assert!(parachain::PendingApprovedPeer::<Runtime>::get().is_none());
    storage.top
  };
  let creators = Sr25519Keyring::well_known()
    .map(|key| key.to_account_id())
    .chain((0..28).map(|index| crate::AccountId::from(Sr25519Keyring::numeric(index).public())))
    .collect::<Vec<_>>();
  let system_state = || {
    (0..u64::from(REFERENCE_SYSTEM_ACTOR_IDENTITIES))
      .map(|actor| {
        let mut identity = Actors::actor_identity(actor).unwrap();
        identity.cycle_nonce = 0;
        let contract = Actors::active_actor_state(actor).map(|state| {
          assert_eq!(state.hot.unsuccessful_attempt_streak, 0);
          assert!(state.hot.terminal_at.is_none());
          assert_ne!(state.hot.cycle_state, CycleState::Suspended);
          assert!(state.contract.close_when.is_none());
          assert!(state.contract.auto_close_at_cycle_nonce.is_none());
          assert!(state.contract.parked_balance_activation.is_none());
          state.contract
        });
        (actor, (identity, contract))
      })
      .collect::<BTreeMap<_, _>>()
  };
  for preset in crate::genesis_config_presets::preset_names() {
    for burn_intake in [false, true] {
      let mut storage = preset_genesis_storage(&[], &preset);
      let systems_before = TestExternalities::new(storage.clone()).execute_with(|| {
        message_state();
        let endpoints = (0..u64::from(REFERENCE_SYSTEM_ACTOR_IDENTITIES))
          .map(Actors::sovereign_account_id_system)
          .chain(std::iter::once(crate::Staking::pool_account_for(0)))
          .collect::<BTreeSet<_>>();
        assert_eq!(
          endpoints.len(),
          REFERENCE_SYSTEM_ACTOR_IDENTITIES as usize + 1
        );
        for creator in &creators {
          assert!(!endpoints.contains(creator));
          for slot in 0..<Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get() {
            assert!(!endpoints.contains(&Actors::sovereign_account_id(creator, slot)));
          }
        }
        system_state()
      });
      let fee_sink = Actors::sovereign_account_id_system(1);
      let liquidity_recipient = Actors::sovereign_account_id_system(14);
      let staking_pool = crate::Staking::pool_account_for(0);
      let (initial_native, initial_local) =
        TestExternalities::new(storage.clone()).execute_with(|| {
          assert_eq!(
            Balances::free_balance(&fee_sink),
            crate::EXISTENTIAL_DEPOSIT
          );
          assert_eq!(
            Balances::free_balance(&staking_pool),
            crate::EXISTENTIAL_DEPOSIT
          );
          assert_eq!(
            Balances::free_balance(&liquidity_recipient),
            crate::EXISTENTIAL_DEPOSIT
          );
          assert_eq!(Assets::balance(0, &liquidity_recipient), 0);
          (Balances::total_issuance(), Assets::total_issuance(0))
        });
      let initial_backing = initial_native.checked_add(initial_local).unwrap();
      let mut parent = parent_header_for(storage.clone(), &[], 0);
      let recipient = Actors::sovereign_account_id_system(if burn_intake { 0 } else { 1 });
      let mut fixed_point = None;
      let mut burns = 0;
      let mut allocations = 0;
      let mut completed_fee_cycles = 0;
      let mut burned = 0u128;
      let mut bridged = 0u128;
      let mut staking_allocated = 0u128;
      let mut expected_fee_sink = crate::EXISTENTIAL_DEPOSIT;
      for block in 1..=32 {
        let nonce =
          TestExternalities::new(storage.clone()).execute_with(|| System::account_nonce(&owner));
        let call =
          RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
            dest: recipient.clone().into(),
            value: if burn_intake {
              primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD
                .checked_add(crate::EXISTENTIAL_DEPOSIT)
                .unwrap()
            } else {
              crate::EXISTENTIAL_DEPOSIT
            },
          });
        let authored = author_complete_block_after_with_calls(
          storage,
          &[],
          &parent,
          block,
          UserDemand::ActorOnly,
          &signer,
          nonce,
          &BTreeMap::new(),
          &[call],
        );
        assert_complete_block_replays_natively(&authored, &[]);
        let observed = TestExternalities::new(authored.post_state.clone()).execute_with(|| {
          assert_eq!(system_state(), systems_before);
          assert_eq!(Actors::active_actor_count(), REFERENCE_ACTIVE_SYSTEM_ACTORS);
          assert_eq!(
            Actors::actor_identity_count(),
            REFERENCE_SYSTEM_ACTOR_IDENTITIES
          );
          assert!(
            pallet_deos_actors::BalanceDependencySources::<Runtime>::iter_keys()
              .next()
              .is_none()
          );
          assert!(Actors::active_actor_state(14).is_none());
          assert!(!Assets::asset_exists(
            primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID
          ));
          for endpoint in [
            Actors::sovereign_account_id_system(1),
            Actors::sovereign_account_id_system(14),
            crate::Staking::pool_account_for(0),
          ] {
            assert!(Balances::free_balance(endpoint) >= crate::EXISTENTIAL_DEPOSIT);
          }
          if !burn_intake {
            assert_eq!(
              pallet_deos_actors::DeadlineIndexLen::<Runtime>::get(WakeupClock::Block),
              0
            );
            assert!(pallet_deos_actors::DeadlineIndexLen::<Runtime>::get(WakeupClock::Tick) <= 1);
            assert_eq!(Actors::actor_identity(0).unwrap().cycle_nonce, 0);
            assert_eq!(Actors::actor_identity(10).unwrap().cycle_nonce, 0);
          }
          assert_eq!(System::account_nonce(&owner), nonce + 1);
          assert_eq!(parachain::ProcessedDownwardMessages::<Runtime>::get(), 0);
          assert!(parachain::UpwardMessages::<Runtime>::get().is_empty());
          assert!(parachain::HrmpOutboundMessages::<Runtime>::get().is_empty());
          let credit_key = [
            polkadot_sdk::sp_io::hashing::twox_128(b"TransactionPayment"),
            polkadot_sdk::sp_io::hashing::twox_128(b"TxPaymentCredit"),
          ]
          .concat();
          assert!(polkadot_sdk::sp_io::storage::get(&credit_key).is_none());
          let mut actual_fee = None;
          let mut block_distributed = 0u128;
          for record in System::events() {
            if let RuntimeEvent::TransactionPayment(
              polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
                who,
                actual_fee: paid,
                tip,
              },
            ) = &record.event
            {
              assert_eq!(who, &owner);
              assert_eq!(*tip, 0);
              assert!(actual_fee.replace(*paid).is_none());
            }
            assert!(!matches!(
              record.event,
              RuntimeEvent::Actors(
                Event::StepFailed { .. } | Event::ActorClosed { .. } | Event::ActorActivated { .. }
              )
            ));
            assert!(!matches!(
              record.event,
              RuntimeEvent::MessageQueue(_) | RuntimeEvent::PolkadotXcm(_)
            ));
            if let RuntimeEvent::Actors(Event::BurnExecuted {
              actor_id, asset, amount, ..
            }) = record.event
            {
              assert_eq!(actor_id, 0);
              assert_eq!(asset, AssetKind::Native);
              burned = burned.checked_add(amount).unwrap();
              burns += 1;
            }
            if let RuntimeEvent::Actors(Event::CycleSummary {
              actor_id, result, ..
            }) = record.event
            {
              assert_eq!(result, CycleResult::Completed);
              completed_fee_cycles += u32::from(actor_id == 1);
            }
            if let RuntimeEvent::Actors(Event::TransferExecuted {
              actor_id: 1, asset, to, amount, ..
            }) = record.event
            {
              assert_eq!(asset, AssetKind::Native);
              assert!(amount >= crate::EXISTENTIAL_DEPOSIT);
              if to == staking_pool {
                staking_allocated = staking_allocated.checked_add(amount).unwrap();
              } else {
                assert_eq!(to, liquidity_recipient);
                bridged = bridged.checked_add(amount).unwrap();
                allocations += 1;
              }
              block_distributed = block_distributed.checked_add(amount).unwrap();
            }
          }
          let paid = actual_fee.expect("one signed transfer pays one corrected fee");
          let direct_intake = if burn_intake { 0 } else { crate::EXISTENTIAL_DEPOSIT };
          expected_fee_sink = expected_fee_sink
            .checked_add(paid)
            .unwrap()
            .checked_add(direct_intake)
            .unwrap()
            .checked_sub(block_distributed)
            .unwrap();
          assert_eq!(Balances::free_balance(&fee_sink), expected_fee_sink);
          assert!(expected_fee_sink >= crate::EXISTENTIAL_DEPOSIT);
          assert_eq!(Balances::free_balance(&liquidity_recipient), crate::EXISTENTIAL_DEPOSIT);
          assert_eq!(Assets::balance(0, &liquidity_recipient), bridged);
          assert_eq!(Assets::total_issuance(0), initial_local.checked_add(bridged).unwrap());
          assert_eq!(Balances::free_balance(&staking_pool), crate::EXISTENTIAL_DEPOSIT.checked_add(staking_allocated).unwrap());
          let native = Balances::total_issuance();
          let local = Assets::total_issuance(0);
          assert_eq!(native.checked_add(local).unwrap().checked_add(burned).unwrap(), initial_backing);
          println!(
            "System ledger conservation: preset={preset} burn_intake={burn_intake} block={block} initial_native={initial_native} initial_local={initial_local} native={native} local={local} burned={burned} bridged={bridged} paid_fee={paid} direct_intake={direct_intake} distributed={block_distributed} fee_sink={expected_fee_sink} staking_pool={} liquidity_native={} liquidity_local={} conserved=true dormant_recipient=true",
            Balances::free_balance(&staking_pool), Balances::free_balance(&liquidity_recipient), Assets::balance(0, &liquidity_recipient)
          );
          message_state()
        });
        // Empty discovery/outbound values become materialized during the first complete block.
        if let Some(expected) = &fixed_point {
          assert_eq!(&observed, expected);
        } else {
          fixed_point = Some(observed);
        }
        parent = authored.block.header;
        storage = authored.post_state;
      }
      assert_eq!(burns > 0, burn_intake);
      assert!(allocations > 0);
      assert!(completed_fee_cycles >= allocations);
      TestExternalities::new(storage).execute_with(|| {
        assert!(Assets::balance(0, Actors::sovereign_account_id_system(14)) > 0);
      });
      println!(
        "Construction systems/messages: preset={preset} burn_intake={burn_intake} complete_blocks=32 signed_transfers=32 burns={burns} allocations={allocations} completed_fee_cycles={completed_fee_cycles} fixed_point_keys={}",
        fixed_point.unwrap().len()
      );
    }
  }
}

#[test]
fn xcm_discovery_failed_prefix_then_ump_success_has_distinct_storage_owners() {
  use codec::Decode;
  use polkadot_sdk::frame_support::{assert_noop, traits::Hooks};
  use polkadot_sdk::staging_xcm::{
    VersionedLocation, VersionedXcm, WrapVersion, latest::prelude::*,
  };

  TestExternalities::new(reference_genesis_storage(&[])).execute_with(|| {
    System::set_block_number(1);
    polkadot_sdk::cumulus_pallet_parachain_system::HostConfiguration::<Runtime>::put(
      RelayStateSproofBuilder::default().host_config,
    );
    let before = System::block_weight();
    let empty_return = crate::PolkadotXcm::on_initialize(1);
    assert_eq!(empty_return, <Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get().reads_writes(1, 1));
    let capacity = <Runtime as polkadot_sdk::pallet_xcm::Config>::VERSION_DISCOVERY_QUEUE_SIZE;
    let parent = Location::parent();
    for destination in core::iter::once(parent.clone()).chain(
      (1..capacity).map(|index| Location::new(0, [GeneralIndex(u128::from(index))])),
    ) {
      assert_ok!(<crate::PolkadotXcm as WrapVersion>::wrap_version::<RuntimeCall>(
        &destination,
        Xcm::<RuntimeCall>(Vec::new()),
      ));
    }
    let queue_key = polkadot_sdk::frame_support::storage::storage_prefix(
      b"PolkadotXcm", b"VersionDiscoveryQueue",
    );
    let queue_before = Vec::<(VersionedLocation, u32)>::decode(
      &mut &polkadot_sdk::sp_io::storage::get(&queue_key).expect("version wrapping enqueued destinations")[..],
    ).expect("SDK discovery queue decodes");
    assert_eq!(queue_before.len(), capacity as usize);
    assert_eq!(queue_before.first(), Some(&(VersionedLocation::from(parent.clone()), 1)));
    let returned = crate::PolkadotXcm::on_initialize(1);
    assert_eq!(returned, <Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get().reads_writes(2, 2));
    assert_eq!(System::block_weight(), before);
    let messages = polkadot_sdk::cumulus_pallet_parachain_system::PendingUpwardMessages::<Runtime>::get();
    assert_eq!(messages.len(), 1);
    let message = VersionedXcm::<()>::decode(&mut &messages[0][..]).expect("UMP contains versioned XCM");
    let message: Xcm<()> = message.try_into().expect("UMP subscription converts to current XCM");
    let query_id = u64::from(capacity - 1);
    assert!(matches!(message.0.first(), Some(SubscribeVersion { query_id: actual, .. }) if *actual == query_id));
    assert!(matches!(
      crate::PolkadotXcm::query(&query_id),
      Some(polkadot_sdk::pallet_xcm::QueryStatus::VersionNotifier { origin, is_active: false })
        if origin == VersionedLocation::from(parent.clone()),
    ));
    assert_noop!(crate::PolkadotXcm::request_version_notify(parent), polkadot_sdk::staging_xcm::latest::Error::InvalidLocation);
    let remaining = Vec::<(VersionedLocation, u32)>::decode(
      &mut &polkadot_sdk::sp_io::storage::get(&queue_key).expect("hook rewrote discovery queue")[..],
    ).expect("rewritten discovery queue decodes");
    assert!(remaining.is_empty());
    println!("XCM discovery: {} rejected routes then UMP success; empty return={empty_return:?}; successful return={returned:?}; one pending UMP message and retained notifier query", capacity - 1);
  });
}

#[test]
fn ump_relay_capacity_is_not_a_local_pending_queue_bound() {
  use polkadot_sdk::sp_runtime::traits::Dispatchable;
  use polkadot_sdk::staging_xcm::{VersionedLocation, VersionedXcm, latest::prelude::*};

  TestExternalities::new(reference_genesis_storage(&[])).execute_with(|| {
    System::set_block_number(1);
    let host = RelayStateSproofBuilder::default().host_config;
    let count_limit = host.max_upward_queue_count;
    let byte_limit = host.max_upward_queue_size;
    let message_limit = host.max_upward_message_size;
    polkadot_sdk::cumulus_pallet_parachain_system::HostConfiguration::<Runtime>::put(host);
    let sender = polkadot_sdk::sp_keyring::Sr25519Keyring::Alice.to_account_id();
    let balance_before = Balances::free_balance(&sender);
    let message = Xcm::<()>(vec![ClearOrigin; (message_limit / 4) as usize]);
    for _ in 0..=count_limit {
      assert_ok!(RuntimeCall::PolkadotXcm(polkadot_sdk::pallet_xcm::Call::send {
        dest: Box::new(VersionedLocation::from(Location::parent())),
        message: Box::new(VersionedXcm::from(message.clone())),
      }).dispatch(RuntimeOrigin::signed(sender.clone())));
    }
    let pending = polkadot_sdk::cumulus_pallet_parachain_system::PendingUpwardMessages::<Runtime>::get();
    let total_bytes = pending.iter().map(Vec::len).sum::<usize>();
    assert_eq!(pending.len(), count_limit as usize + 1);
    assert!(total_bytes > byte_limit as usize);
    assert!(pending.iter().all(|message| message.len() <= message_limit as usize));
    assert!(Balances::free_balance(&sender) < balance_before);
    println!(
      "UMP accepted signed sends: pending_count={} > relay_count={count_limit}; pending_bytes={total_bytes} > relay_bytes={byte_limit}; individual_limit={message_limit}; delivery_fees_paid={}",
      pending.len(),
      balance_before - Balances::free_balance(&sender),
    );
  });
}

/// Ordinary Signed Root-debit attempts remain valid fee-paying extrinsics but fail
/// dispatch. The declared genesis, consensus context and full import are unchanged.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_fee_sink_root_debits_fail_through_complete_native_blocks() {
  use polkadot_sdk::sp_runtime::DispatchError;

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let sink = Actors::sovereign_account_id_system(sink_id);
  let recipient = super::common::BOB;
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    for block_number in 1..=2 {
      let (nonce, payer_before, sink_before, recipient_before, issuance_before) =
        TestExternalities::new(storage.clone()).execute_with(|| {
          assert!(!Actors::user_creation_enabled());
          assert!(!Actors::global_circuit_breaker());
          (
            System::account_nonce(&owner),
            Balances::free_balance(&owner),
            Balances::free_balance(&sink),
            Balances::free_balance(&recipient),
            Balances::total_issuance(),
          )
        });
      assert!(sink_before >= crate::EXISTENTIAL_DEPOSIT);
      let calls = [
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::force_set_balance {
          who: MultiAddress::Id(sink.clone()),
          new_free: 0,
        }),
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::force_transfer {
          source: MultiAddress::Id(sink.clone()),
          dest: MultiAddress::Id(recipient.clone()),
          value: crate::EXISTENTIAL_DEPOSIT,
        }),
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
          dest: MultiAddress::Id(sink.clone()),
          value: crate::EXISTENTIAL_DEPOSIT,
        }),
      ];
      let expected = [
        Err(DispatchError::BadOrigin),
        Err(DispatchError::BadOrigin),
        Ok(()),
      ];
      let signed = calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
          (
            &signer,
            nonce.checked_add(u32::try_from(index).unwrap()).unwrap(),
            call,
          )
        })
        .collect::<Vec<_>>();
      let authored = author_complete_block_after_with_expected_dispatch(
        storage,
        &[],
        &parent,
        block_number,
        UserDemand::ActorOnly,
        &signer,
        nonce,
        &BTreeMap::new(),
        &signed,
        inherent_data_for(&parent, block_number),
        Some(&expected),
      );
      assert_eq!(authored.metrics.user_calls, 3);
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        let events = System::events();
        let failures = events.iter().filter_map(|record| {
          match (&record.phase, &record.event) {
            (polkadot_sdk::frame_system::Phase::ApplyExtrinsic(index),
             RuntimeEvent::System(polkadot_sdk::frame_system::Event::ExtrinsicFailed {
               dispatch_error, ..
             })) => Some((*index, *dispatch_error)),
            _ => None,
          }
        }).collect::<Vec<_>>();
        let signed_start = u32::try_from(authored.block.extrinsics.len() - calls.len()).unwrap();
        assert_eq!(failures, vec![
          (signed_start, DispatchError::BadOrigin),
          (signed_start + 1, DispatchError::BadOrigin),
        ]);
        let fees = events.iter().filter_map(|record| {
          match &record.event {
            RuntimeEvent::TransactionPayment(polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
              who, actual_fee, tip,
            }) => {
              assert_eq!(who, &owner);
              assert_eq!(*tip, 0);
              assert!(*actual_fee > 0);
              Some(*actual_fee)
            },
            _ => None,
          }
        }).collect::<Vec<_>>();
        assert_eq!(fees.len(), calls.len(), "failed dispatch still pays its actual transaction fee");
        let total_fee = fees.iter().try_fold(0u128, |total, fee| total.checked_add(*fee)).unwrap();
        assert_eq!(System::account_nonce(&owner), nonce + 3);
        assert_eq!(Balances::free_balance(&owner), payer_before - total_fee - crate::EXISTENTIAL_DEPOSIT);
        assert_eq!(Balances::free_balance(&sink), sink_before + total_fee + crate::EXISTENTIAL_DEPOSIT);
        assert_eq!(Balances::free_balance(&recipient), recipient_before);
        assert_eq!(Balances::total_issuance(), issuance_before);
        assert!(!Actors::user_creation_enabled());
        assert_eq!(Actors::active_actor_state(sink_id).unwrap().identity.cycle_nonce, 0);
        assert!(!Actors::pending_signal(sink_id));
        assert!(Actors::actor_run_state(sink_id).is_none());
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(block_number).unwrap();
        println!("Signed Fee Sink anchor: preset={preset} block={block_number} failures={} nonce={} actual_fees={fees:?} sink_before={sink_before} sink_after={} author_import_root_equal=true", failures.len(), nonce + 3, Balances::free_balance(&sink));
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
  }
}

/// Third-party asset deposits may credit Fee Sink's Native reserve, but must not
/// consume its free anchor. All mutations and refusals use actual signed blocks.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_asset_deposits_preserve_fee_sink_free_anchor() {
  use crate::configs::assets_config::MetadataDepositBase;
  use crate::configs::assets_config::MetadataDepositPerByte;
  use polkadot_sdk::{pallet_assets, sp_runtime::DispatchError};

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let sink = Actors::sovereign_account_id_system(sink_id);
  let asset = 0;
  let metadata_deposit = MetadataDepositBase::get() + 2 * MetadataDepositPerByte::get();
  assert_ne!(owner, sink);
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let (payer_initial, sink_initial, issuance_initial) = TestExternalities::new(storage.clone())
      .execute_with(|| {
        let details = pallet_assets::Asset::<Runtime>::get(asset).unwrap();
        assert_eq!(details.owner, owner);
        assert_eq!(details.deposit, 0);
        assert_eq!(pallet_assets::Metadata::<Runtime>::get(asset).deposit, 0);
        assert!(!pallet_assets::Account::<Runtime>::contains_key(
          asset, &sink
        ));
        assert_eq!(Balances::reserved_balance(&sink), 0);
        (
          Balances::free_balance(&owner),
          Balances::free_balance(&sink),
          Balances::total_issuance(),
        )
      });
    assert!(sink_initial >= crate::EXISTENTIAL_DEPOSIT);
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let mut total_fees = 0u128;
    let mut nonce = 0;
    for block_number in 1..=2 {
      let (calls, expected) = if block_number == 1 {
        (
          vec![
            RuntimeCall::Assets(pallet_assets::Call::set_metadata {
              id: asset,
              name: vec![b'a'],
              symbol: vec![b'a'],
              decimals: 12,
            }),
            RuntimeCall::Assets(pallet_assets::Call::approve_transfer {
              id: asset,
              delegate: MultiAddress::Id(sink.clone()),
              amount: 1,
            }),
            RuntimeCall::Assets(pallet_assets::Call::touch_other {
              id: asset,
              who: MultiAddress::Id(sink.clone()),
            }),
            RuntimeCall::Assets(pallet_assets::Call::cancel_approval {
              id: asset,
              delegate: MultiAddress::Id(sink.clone()),
            }),
            RuntimeCall::Assets(pallet_assets::Call::refund_other {
              id: asset,
              who: MultiAddress::Id(sink.clone()),
            }),
            RuntimeCall::Assets(pallet_assets::Call::transfer_ownership {
              id: asset,
              owner: MultiAddress::Id(sink.clone()),
            }),
          ],
          vec![Ok(()); 6],
        )
      } else {
        (
          vec![
            RuntimeCall::Assets(pallet_assets::Call::set_metadata {
              id: asset,
              name: vec![b'b'; 2],
              symbol: vec![b'b'; 2],
              decimals: 12,
            }),
            RuntimeCall::Assets(pallet_assets::Call::transfer_ownership {
              id: asset,
              owner: MultiAddress::Id(owner.clone()),
            }),
            RuntimeCall::Assets(pallet_assets::Call::create {
              id: 42,
              admin: MultiAddress::Id(owner.clone()),
              min_balance: 1,
            }),
          ],
          vec![
            Err(pallet_assets::Error::<Runtime>::NoPermission.into()),
            Err(pallet_assets::Error::<Runtime>::NoPermission.into()),
            Err(DispatchError::BadOrigin),
          ],
        )
      };
      let signed = calls
        .iter()
        .enumerate()
        .map(|(index, call)| (&signer, nonce + u32::try_from(index).unwrap(), call))
        .collect::<Vec<_>>();
      let authored = author_complete_block_after_with_expected_dispatch(
        storage,
        &[],
        &parent,
        block_number,
        UserDemand::ActorOnly,
        &signer,
        nonce,
        &BTreeMap::new(),
        &signed,
        inherent_data_for(&parent, block_number),
        Some(&expected),
      );
      assert_eq!(authored.metrics.user_calls as usize, calls.len());
      assert_complete_block_replays_natively(&authored, &[]);
      nonce += u32::try_from(calls.len()).unwrap();
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        let fees = System::events()
          .iter()
          .filter_map(|record| match &record.event {
            RuntimeEvent::TransactionPayment(
              polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
                who,
                actual_fee,
                tip,
              },
            ) => {
              assert_eq!(who, &owner);
              assert_eq!(*tip, 0);
              assert!(*actual_fee > 0);
              Some(*actual_fee)
            }
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(fees.len(), calls.len());
        for fee in &fees {
          total_fees = total_fees.checked_add(*fee).unwrap();
        }
        assert_eq!(System::account_nonce(&owner), nonce);
        assert_eq!(Balances::free_balance(&owner), payer_initial - metadata_deposit - total_fees);
        assert_eq!(Balances::free_balance(&sink), sink_initial + total_fees);
        assert_eq!(Balances::reserved_balance(&sink), metadata_deposit);
        assert_eq!(Balances::total_issuance(), issuance_initial);
        assert_eq!(pallet_assets::Asset::<Runtime>::get(asset).unwrap().owner, sink);
        assert_eq!(pallet_assets::Metadata::<Runtime>::get(asset).deposit, metadata_deposit);
        assert!(!pallet_assets::Account::<Runtime>::contains_key(asset, &sink));
        assert!(!pallet_assets::Approvals::<Runtime>::contains_key((asset, &owner, &sink)));
        assert_eq!(Actors::active_actor_state(sink_id).unwrap().identity.cycle_nonce, 0);
        assert!(!Actors::pending_signal(sink_id));
        assert!(Actors::actor_run_state(sink_id).is_none());
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(block_number).unwrap();
        println!("Signed asset deposits: preset={preset} block={block_number} nonce={nonce} sink_free={} sink_reserved={} total_fees={total_fees} author_import_root_equal=true", Balances::free_balance(&sink), Balances::reserved_balance(&sink));
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
  }
}

/// Pool account deposits bind the authenticated creator or touch caller, even
/// when Fee Sink owns the traded asset. Its own plan remains owner-controlled.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_pool_deposit_payers_preserve_fee_sink_free_anchor() {
  use crate::configs::{AssetKind, assets_config::DeosPoolLocator};
  use polkadot_sdk::pallet_asset_conversion::PoolLocator;
  use polkadot_sdk::{pallet_asset_conversion, pallet_assets};

  let alice_signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let bob_signer = sr25519::Pair::from_string("//Bob", None).unwrap();
  let alice = crate::AccountId::from(alice_signer.public());
  let bob = crate::AccountId::from(bob_signer.public());
  let sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let sink = Actors::sovereign_account_id_system(sink_id);
  let asset_id = 0xF000_0001;
  let assets = (AssetKind::Native, AssetKind::Foreign(asset_id));
  let pool_id = DeosPoolLocator::pool_id(&assets.0, &assets.1).unwrap();
  let pool_account = DeosPoolLocator::address(&pool_id).unwrap();
  assert_ne!(alice, sink);
  assert_ne!(bob, sink);
  assert_ne!(pool_account, sink);
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let (initial_alice, initial_bob, initial_sink, issuance, lp_id, sink_contract) =
      TestExternalities::new(storage.clone()).execute_with(|| {
        assert!(
          pallet_asset_conversion::Pools::<Runtime>::iter()
            .next()
            .is_none()
        );
        assert_eq!(
          pallet_assets::Asset::<Runtime>::get(asset_id)
            .unwrap()
            .owner,
          alice
        );
        assert_eq!(Balances::reserved_balance(&alice), 0);
        assert_eq!(Balances::reserved_balance(&bob), 0);
        assert_eq!(Balances::reserved_balance(&sink), 0);
        let state = Actors::active_actor_state(sink_id).unwrap();
        assert_ne!(state.identity.owner, alice);
        assert_ne!(state.identity.owner, bob);
        assert_eq!(state.contract.steps.len(), 2);
        assert!(
          state
            .contract
            .steps
            .iter()
            .all(|step| matches!(step.task, Task::Transfer { .. }))
        );
        (
          Balances::free_balance(&alice),
          Balances::free_balance(&bob),
          Balances::free_balance(&sink),
          Balances::total_issuance(),
          pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap(),
          state.contract,
        )
      });
    assert!(initial_sink >= crate::EXISTENTIAL_DEPOSIT);
    let mut replacement = sink_contract.clone();
    replacement.steps = vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::AddLiquidity {
        asset_a: assets.0,
        asset_b: assets.1,
        amount_a: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
        amount_b: AmountResolution::Fixed(1),
        min_lp_out: 1,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap();
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let mut alice_fees = 0u128;
    let mut bob_fees = 0u128;
    for block_number in 1..=2 {
      let calls = if block_number == 1 {
        vec![
          RuntimeCall::Assets(pallet_assets::Call::transfer_ownership {
            id: asset_id,
            owner: MultiAddress::Id(sink.clone()),
          }),
          RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
            asset_a: assets.0,
            asset_b: assets.1,
          }),
        ]
      } else {
        vec![
          RuntimeCall::Assets(pallet_assets::Call::refund_other {
            id: lp_id,
            who: MultiAddress::Id(pool_account.clone()),
          }),
          RuntimeCall::AssetConversion(pallet_asset_conversion::Call::touch {
            asset1: Box::new(assets.0),
            asset2: Box::new(assets.1),
          }),
          RuntimeCall::Actors(pallet_deos_actors::Call::update_contract {
            actor_id: sink_id,
            contract: replacement.clone(),
          }),
        ]
      };
      let expected = if block_number == 1 {
        vec![Ok(()); 2]
      } else {
        vec![
          Ok(()),
          Ok(()),
          Err(pallet_deos_actors::Error::<Runtime>::NotOwner.into()),
        ]
      };
      let signed = if block_number == 1 {
        vec![(&alice_signer, 0, &calls[0]), (&alice_signer, 1, &calls[1])]
      } else {
        vec![
          (&alice_signer, 2, &calls[0]),
          (&bob_signer, 0, &calls[1]),
          (&alice_signer, 3, &calls[2]),
        ]
      };
      let authored = author_complete_block_after_with_expected_dispatch(
        storage,
        &[],
        &parent,
        block_number,
        UserDemand::ActorOnly,
        &alice_signer,
        if block_number == 1 { 0 } else { 2 },
        &BTreeMap::new(),
        &signed,
        inherent_data_for(&parent, block_number),
        Some(&expected),
      );
      assert_eq!(authored.metrics.user_calls as usize, calls.len());
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        let mut fee_count = 0;
        for record in System::events() {
          if let RuntimeEvent::TransactionPayment(
            polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
              who, actual_fee, tip,
            },
          ) = record.event {
            assert_eq!(tip, 0);
            assert!(actual_fee > 0);
            if who == alice {
              alice_fees = alice_fees.checked_add(actual_fee).unwrap();
            } else {
              assert_eq!(who, bob);
              bob_fees = bob_fees.checked_add(actual_fee).unwrap();
            }
            fee_count += 1;
          }
        }
        assert_eq!(fee_count, calls.len());
        let (depositor, alice_reserve, bob_reserve, alice_nonce, bob_nonce) = if block_number == 1 {
          (&alice, crate::EXISTENTIAL_DEPOSIT, 0, 2, 0)
        } else {
          (&bob, 0, crate::EXISTENTIAL_DEPOSIT, 4, 1)
        };
        assert_eq!(System::account_nonce(&alice), alice_nonce);
        assert_eq!(System::account_nonce(&bob), bob_nonce);
        assert_eq!(Balances::reserved_balance(&alice), alice_reserve);
        assert_eq!(Balances::reserved_balance(&bob), bob_reserve);
        assert_eq!(Balances::free_balance(&alice), initial_alice - alice_fees - alice_reserve);
        assert_eq!(Balances::free_balance(&bob), initial_bob - bob_fees - bob_reserve);
        assert_eq!(Balances::free_balance(&sink), initial_sink + alice_fees + bob_fees);
        assert_eq!(Balances::reserved_balance(&sink), 0);
        assert_eq!(Balances::total_issuance(), issuance);
        assert_eq!(pallet_assets::Asset::<Runtime>::get(asset_id).unwrap().owner, sink);
        assert_eq!(pallet_asset_conversion::Pools::<Runtime>::get(pool_id).unwrap().lp_token, lp_id);
        assert_eq!(pallet_assets::Account::<Runtime>::get(lp_id, &pool_account).unwrap().reason,
          pallet_assets::ExistenceReason::DepositFrom(depositor.clone(), crate::EXISTENTIAL_DEPOSIT));
        let state = Actors::active_actor_state(sink_id).unwrap();
        assert_eq!(state.contract, sink_contract);
        assert_eq!(state.identity.cycle_nonce, 0);
        assert!(!Actors::pending_signal(sink_id));
        assert!(Actors::actor_run_state(sink_id).is_none());
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(block_number).unwrap();
        println!("Signed pool deposits: preset={preset} block={block_number} depositor={depositor:?} alice_fees={alice_fees} bob_fees={bob_fees} sink_free={} author_import_root_equal=true", Balances::free_balance(&sink));
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
  }
}

// Benchmark builds replace the production MessageQueue processor with a no-op.
#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn parent_dmp_partial_withdrawal_preserves_native_issuance() {
  use crate::configs::{NativeLocation, XcmOriginToTransactDispatchOrigin};
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system as parachain_system,
    cumulus_primitives_core::{AggregateMessageOrigin, InboundDownwardMessage},
    frame_support::traits::OriginTrait,
    staging_xcm::{VersionedXcm, latest::prelude::*},
    staging_xcm_executor::traits::{ConvertOrigin, WeightBounds},
  };

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let parent_account =
    <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
      Location::parent(),
      OriginKind::SovereignAccount,
    )
    .unwrap()
    .into_signer()
    .unwrap();
  let sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let sink = Actors::sovereign_account_id_system(sink_id);
  let anchor = crate::EXISTENTIAL_DEPOSIT;
  let ledger_sum = || {
    polkadot_sdk::frame_system::Account::<Runtime>::iter()
      .try_fold(0u128, |total, (_, account)| {
        total
          .checked_add(account.data.free)?
          .checked_add(account.data.reserved)
      })
      .unwrap()
  };
  let mut results = Vec::new();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let initial_issuance = TestExternalities::new(storage.clone()).execute_with(|| {
      assert_eq!(ledger_sum(), Balances::total_issuance());
      assert_eq!(Balances::free_balance(&parent_account), 0);
      Balances::total_issuance()
    });
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let funding = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
      dest: MultiAddress::Id(parent_account.clone()),
      value: 2 * anchor,
    });
    let mut dmq = MessageQueueChain::default();
    for number in 1..=3 {
      let mut data = inherent_data_for(&parent, number);
      if number >= 2 {
        let mut context = data
          .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
          .unwrap()
          .unwrap();
        if number == 2 {
          let assets: polkadot_sdk::staging_xcm::latest::Assets = vec![
            Asset {
              id: AssetId(NativeLocation::get()),
              fun: Fungible(anchor),
            },
            Asset {
              id: AssetId(Location::new(1, [GeneralIndex(u128::MAX)])),
              fun: Fungible(1),
            },
          ]
          .into();
          assert_eq!(assets.get(0).unwrap().id, AssetId(NativeLocation::get()));
          let mut program = Xcm::<RuntimeCall>(vec![
            UnpaidExecution {
              weight_limit: Unlimited,
              check_origin: None,
            },
            WithdrawAsset(assets),
          ]);
          assert_ok!(
            <Runtime as polkadot_sdk::pallet_xcm::Config>::Weigher::weight(
              &mut program,
              crate::configs::MessageQueueServiceWeight::get(),
            )
          );
          let message = InboundDownwardMessage {
            sent_at: number,
            msg: VersionedXcm::from(program).encode(),
          };
          dmq.extend_downward(&message);
          context.downward_messages = vec![message];
        }
        let proof_builder = RelayStateSproofBuilder {
          para_id: crate::PARACHAIN_ID.into(),
          current_slot: u64::from(number).into(),
          included_para_head: Some(context.validation_data.parent_head.clone()),
          dmq_mqc_head: Some(dmq.head()),
          ..Default::default()
        };
        let (root, proof) = proof_builder.into_state_root_and_proof();
        context.validation_data.relay_parent_storage_root = root;
        context.relay_chain_state = proof;
        data.replace_data(INHERENT_IDENTIFIER, &context);
      }
      let calls = if number == 1 {
        vec![(&signer, 0, &funding)]
      } else {
        Vec::new()
      };
      let authored = author_complete_block_after_with_context(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        u32::from(number != 1),
        &BTreeMap::new(),
        &calls,
        data,
      );
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert_eq!(Balances::free_balance(&parent_account), 2 * anchor);
        assert_eq!(System::account_nonce(&owner), 1);
        assert!(Balances::free_balance(&sink) >= anchor);
        assert_eq!(Actors::active_actor_state(sink_id).unwrap().identity.cycle_nonce, 0);
        assert!(!Actors::pending_signal(sink_id));
        assert!(Actors::actor_run_state(sink_id).is_none());
        assert_eq!(ledger_sum(), initial_issuance);
        let mut processed = Vec::new();
        let mut errors = Vec::new();
        for record in System::events() {
          match record.event {
            RuntimeEvent::MessageQueue(polkadot_sdk::pallet_message_queue::Event::Processed {
              origin, success, ..
            }) => {
              assert_eq!(origin, AggregateMessageOrigin::Parent);
              processed.push(success);
            },
            RuntimeEvent::MessageQueue(polkadot_sdk::pallet_message_queue::Event::ProcessingFailed { error, .. }) => {
              panic!("DMP was not executed: {error:?}");
            },
            RuntimeEvent::PolkadotXcm(polkadot_sdk::pallet_xcm::Event::ProcessXcmError { origin, error, .. }) => {
              assert_eq!(origin, Location::parent());
              errors.push(error);
            },
            _ => {},
          }
        }
        assert_eq!(processed, if number == 3 { vec![false] } else { vec![] });
        if number == 3 {
          assert!(matches!(errors.as_slice(), [polkadot_sdk::staging_xcm::latest::Error::FailedToTransactAsset(_)]));
          let actual = Balances::total_issuance();
          println!("Partial DMP withdrawal: preset={preset} parent_free={} ledger_sum={} initial_issuance={initial_issuance} actual_issuance={actual} deficit={} author_import_root_equal=true", Balances::free_balance(&parent_account), ledger_sum(), initial_issuance - actual);
          results.push((preset.clone(), actual, initial_issuance));
        } else {
          assert!(errors.is_empty());
          assert_eq!(Balances::total_issuance(), initial_issuance);
        }
        assert_eq!(parachain_system::LastDmqMqcHead::<Runtime>::get().head(), dmq.head());
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
  }
  for (preset, actual, expected) in results {
    assert_eq!(
      actual, expected,
      "partial withdrawal must preserve issuance for {preset}"
    );
  }
}

// Benchmark builds replace the production MessageQueue processor with a no-op.
#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn parent_dmp_subscriptions_retain_ump_backlog_through_full_executive() {
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system as parachain_system,
    cumulus_primitives_core::{AggregateMessageOrigin, InboundDownwardMessage},
    staging_xcm::{VersionedXcm, latest::prelude::*},
    staging_xcm_executor::traits::{VersionChangeNotifier, WeightBounds},
  };

  let host = RelayStateSproofBuilder::default().host_config;
  let burst = 2 * host.max_upward_queue_count + 1;
  let signer = sr25519::Pair::from_seed(&[0; 32]);
  let profiles = BTreeMap::new();
  let mut storage = reference_genesis_storage(&[]);
  let mut parent = parent_header_for(storage.clone(), &[], 0);
  let mut dmq = MessageQueueChain::default();
  let mut previous_pending: Vec<Vec<u8>> = Vec::new();
  let mut previous_subscriptions = 0;
  for (index, subscriptions) in [1, burst, burst, 0].into_iter().enumerate() {
    let number = index as u32 + 1;
    let mut data = inherent_data_for(&parent, number);
    let mut context = data
      .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
      .expect("fixture context decodes")
      .expect("fixture supplies context");
    if subscriptions != 0 {
      let mut instructions = vec![UnpaidExecution {
        weight_limit: Unlimited,
        check_origin: None,
      }];
      for query in 0..subscriptions {
        instructions.push(SubscribeVersion {
          query_id: u64::from(number * burst + query),
          max_response_weight: Weight::zero(),
        });
        instructions.push(UnsubscribeVersion);
      }
      let mut program = Xcm::<RuntimeCall>(instructions);
      assert_ok!(
        <Runtime as polkadot_sdk::pallet_xcm::Config>::Weigher::weight(
          &mut program,
          crate::configs::MessageQueueServiceWeight::get(),
        )
      );
      let message = InboundDownwardMessage {
        sent_at: number,
        msg: VersionedXcm::from(program).encode(),
      };
      dmq.extend_downward(&message);
      context.downward_messages = vec![message];
    }
    let proof_builder = RelayStateSproofBuilder {
      para_id: crate::PARACHAIN_ID.into(),
      current_slot: u64::from(number).into(),
      included_para_head: Some(context.validation_data.parent_head.clone()),
      dmq_mqc_head: Some(dmq.head()),
      ..Default::default()
    };
    let (root, proof) = proof_builder.into_state_root_and_proof();
    context.validation_data.relay_parent_storage_root = root;
    context.relay_chain_state = proof;
    data.replace_data(INHERENT_IDENTIFIER, &context);
    let authored = author_complete_block_after_with_context(
      storage,
      &[],
      &parent,
      number,
      UserDemand::ActorOnly,
      &signer,
      0,
      &profiles,
      &[],
      data,
    );
    assert_complete_block_replays_natively(&authored, &[]);
    let (pending, exported, sent, started, processed) =
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert_eq!(
          parachain_system::LastDmqMqcHead::<Runtime>::get().head(),
          dmq.head()
        );
        assert!(!<crate::PolkadotXcm as VersionChangeNotifier>::is_subscribed(&Location::parent()));
        let (mut sent, mut started, mut processed) = (0, 0, 0);
        for record in System::events() {
          match record.event {
            RuntimeEvent::ParachainSystem(parachain_system::Event::UpwardMessageSent {
              ..
            }) => sent += 1,
            RuntimeEvent::PolkadotXcm(polkadot_sdk::pallet_xcm::Event::VersionNotifyStarted {
              destination,
              ..
            }) => {
              assert_eq!(destination, Location::parent());
              started += 1;
            }
            RuntimeEvent::MessageQueue(polkadot_sdk::pallet_message_queue::Event::Processed {
              origin,
              success,
              ..
            }) => {
              assert_eq!(origin, AggregateMessageOrigin::Parent);
              assert!(success);
              processed += 1;
            }
            RuntimeEvent::MessageQueue(
              polkadot_sdk::pallet_message_queue::Event::ProcessingFailed { error, .. },
            ) => {
              panic!("DMP processing failed: {error:?}");
            }
            _ => {}
          }
        }
        (
          parachain_system::PendingUpwardMessages::<Runtime>::get(),
          parachain_system::UpwardMessages::<Runtime>::get(),
          sent,
          started,
          processed,
        )
      });
    assert_eq!(started, previous_subscriptions);
    assert_eq!(processed, u32::from(previous_subscriptions != 0));
    assert_eq!(
      previous_pending.len() + sent,
      exported.len() + pending.len()
    );
    assert!(exported.len() <= host.max_upward_message_num_per_candidate as usize);
    assert!(exported.iter().map(Vec::len).sum::<usize>() <= host.max_upward_queue_size as usize);
    assert!(
      pending
        .iter()
        .chain(&exported)
        .all(|message| message.len() <= host.max_upward_message_size as usize)
    );
    let previous_exported = previous_pending.len().min(exported.len());
    assert_eq!(
      exported[..previous_exported],
      previous_pending[..previous_exported]
    );
    assert!(pending.starts_with(&previous_pending[previous_exported..]));
    println!(
      "DMP full Executive block={number}: subscriptions={started}, processed={processed}, sent={sent}, exported={}, pending={}, pending_bytes={}",
      exported.len(),
      pending.len(),
      pending.iter().map(Vec::len).sum::<usize>(),
    );
    previous_pending = pending;
    previous_subscriptions = subscriptions;
    parent = authored.block.header;
    storage = authored.post_state;
  }
  assert!(previous_pending.len() > host.max_upward_queue_count as usize);
  assert!(
    previous_pending.iter().map(Vec::len).sum::<usize>() > host.max_upward_queue_size as usize
  );
}

// Benchmark builds replace the production MessageQueue processor with a no-op.
#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn parent_dmp_origin_rewrites_cannot_withdraw_fee_sink_anchor() {
  use crate::configs::{NativeLocation, XcmOriginToTransactDispatchOrigin};
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system as parachain_system,
    cumulus_primitives_core::{AggregateMessageOrigin, InboundDownwardMessage},
    frame_support::traits::{
      Get, OriginTrait,
      fungible::{Inspect, Mutate},
    },
    staging_xcm::{VersionedXcm, latest::prelude::*},
    staging_xcm_executor::traits::{ConvertOrigin, WeightBounds},
  };

  let ledger_identity = |location: Location| {
    <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
      location,
      OriginKind::SovereignAccount,
    )
    .ok()
    .and_then(|origin| origin.into_signer())
  };
  let transact = |origin_kind, call: RuntimeCall| Transact {
    origin_kind,
    fallback_max_weight: None,
    call: call.encode().into(),
  };
  let raw_key = vec![0xfe; 713];
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let sink = crate::configs::actor_config::ActorFeeRecipient::get();
    let parent_account =
      ledger_identity(Location::parent()).expect("Parent has its own ledger identity");
    let local_sink = Location::new(
      0,
      [AccountId32 {
        network: None,
        id: sink.clone().into(),
      }],
    );
    let beneficiary = Location::new(
      0,
      [AccountId32 {
        network: None,
        id: ALICE.into(),
      }],
    );
    let anchor = crate::EXISTENTIAL_DEPOSIT;
    assert_ne!(parent_account, sink);
    // Every Sibling ledger identity encodes `sibl` plus u32 and trailing zeroes.
    let sink_bytes: [u8; 32] = sink.clone().into();
    assert!(sink_bytes[8..].iter().any(|byte| *byte != 0));
    assert_eq!(ledger_identity(local_sink.clone()), Some(sink.clone()));
    assert_eq!(
      ledger_identity(Location::new(
        1,
        [AccountId32 {
          network: None,
          id: sink_bytes
        }]
      )),
      None
    );
    let mut setup = TestExternalities::new(storage);
    let issuance = setup.execute_with(|| {
      assert_ok!(<Balances as Mutate<crate::AccountId>>::mint_into(
        &parent_account,
        2 * anchor
      ));
      assert_eq!(Balances::free_balance(&sink), anchor);
      <Balances as Inspect<crate::AccountId>>::total_issuance()
    });
    storage = setup.execute_with(current_top_storage);
    let (initial_alice, initial_bob) = TestExternalities::new(storage.clone()).execute_with(|| {
      (
        Balances::free_balance(ALICE),
        Balances::free_balance(super::common::BOB),
      )
    });
    let rewrites: Vec<(&str, Vec<Instruction<RuntimeCall>>)> = vec![
      (
        "descendant",
        vec![DescendOrigin(
          [AccountId32 {
            network: None,
            id: sink_bytes,
          }]
          .into(),
        )],
      ),
      ("alias", vec![AliasOrigin(local_sink)]),
      (
        "universal",
        vec![UniversalOrigin(GlobalConsensus(NetworkId::Polkadot))],
      ),
      ("clear", vec![ClearOrigin]),
      (
        "transact-signed-parent-control",
        vec![transact(
          OriginKind::SovereignAccount,
          RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
            dest: super::common::BOB.into(),
            value: anchor,
          }),
        )],
      ),
      (
        "transact-superuser-fee-sink",
        vec![transact(
          OriginKind::Superuser,
          RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::force_set_balance {
            who: sink.clone().into(),
            new_free: 0,
          }),
        )],
      ),
      (
        "transact-superuser-raw-storage",
        vec![transact(
          OriginKind::Superuser,
          RuntimeCall::System(polkadot_sdk::frame_system::Call::set_storage {
            items: vec![(raw_key.clone(), vec![7])],
          }),
        )],
      ),
      ("own-parent-control", vec![]),
    ];
    let signer = sr25519::Pair::from_seed(&[0; 32]);
    let profiles = BTreeMap::new();
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let mut dmq = MessageQueueChain::default();
    for index in 0..=rewrites.len() {
      let number = index as u32 + 1;
      let mut data = inherent_data_for(&parent, number);
      let mut context = data
        .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
        .expect("fixture context decodes")
        .expect("fixture supplies context");
      if let Some((_, rewrite)) = rewrites.get(index) {
        let mut instructions = vec![UnpaidExecution {
          weight_limit: Unlimited,
          check_origin: None,
        }];
        instructions.extend(rewrite.clone());
        instructions.push(WithdrawAsset(
          Asset {
            id: AssetId(NativeLocation::get()),
            fun: Fungible(anchor),
          }
          .into(),
        ));
        instructions.push(DepositAsset {
          assets: All.into(),
          beneficiary: beneficiary.clone(),
        });
        let mut program = Xcm::<RuntimeCall>(instructions);
        assert_ok!(
          <Runtime as polkadot_sdk::pallet_xcm::Config>::Weigher::weight(
            &mut program,
            crate::configs::MessageQueueServiceWeight::get()
          )
        );
        let message = InboundDownwardMessage {
          sent_at: number,
          msg: VersionedXcm::from(program).encode(),
        };
        dmq.extend_downward(&message);
        context.downward_messages = vec![message];
      }
      let proof_builder = RelayStateSproofBuilder {
        para_id: crate::PARACHAIN_ID.into(),
        current_slot: u64::from(number).into(),
        included_para_head: Some(context.validation_data.parent_head.clone()),
        dmq_mqc_head: Some(dmq.head()),
        ..Default::default()
      };
      let (root, proof) = proof_builder.into_state_root_and_proof();
      context.validation_data.relay_parent_storage_root = root;
      context.relay_chain_state = proof;
      data.replace_data(INHERENT_IDENTIFIER, &context);
      let authored = author_complete_block_after_with_context(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        0,
        &profiles,
        &[],
        data,
      );
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert_eq!(Balances::free_balance(&sink), anchor);
        let fee_actor =
          Actors::active_actor_state(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
            .expect("Fee Sink retains its active identity");
        assert_eq!(fee_actor.identity.cycle_nonce, 0);
        assert!(!fee_actor.hot.pending_signal);
        assert_eq!(
          <Balances as Inspect<crate::AccountId>>::total_issuance(),
          issuance
        );
        assert_eq!(
          parachain_system::LastDmqMqcHead::<Runtime>::get().head(),
          dmq.head()
        );
        let mut processed = Vec::new();
        let mut process_errors = Vec::new();
        for record in System::events() {
          match record.event {
            RuntimeEvent::MessageQueue(polkadot_sdk::pallet_message_queue::Event::Processed {
              origin,
              success,
              ..
            }) => {
              assert_eq!(origin, AggregateMessageOrigin::Parent);
              processed.push(success);
            }
            RuntimeEvent::MessageQueue(
              polkadot_sdk::pallet_message_queue::Event::ProcessingFailed { error, .. },
            ) => panic!("DMP was not executed: {error:?}"),
            RuntimeEvent::PolkadotXcm(polkadot_sdk::pallet_xcm::Event::ProcessXcmError {
              origin,
              error,
              ..
            }) => {
              assert_eq!(origin, Location::parent());
              process_errors.push(error);
            }
            _ => {}
          }
        }
        assert_eq!(
          processed,
          if index == 0 {
            vec![]
          } else {
            vec![index == rewrites.len()]
          }
        );
        if index > 0 && rewrites[index - 1].0.starts_with("transact-") {
          assert_eq!(
            process_errors,
            vec![polkadot_sdk::staging_xcm::latest::Error::NoPermission]
          );
        }
        let paid = if index == rewrites.len() { anchor } else { 0 };
        assert_eq!(Balances::free_balance(&parent_account), 2 * anchor - paid);
        assert_eq!(Balances::free_balance(ALICE), initial_alice + paid);
        assert_eq!(Balances::free_balance(super::common::BOB), initial_bob);
        assert!(!polkadot_sdk::sp_io::storage::exists(&raw_key));
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number)
          .expect("DMP origin probe preserves Actor invariants");
      });
      println!(
        "Fee Sink XCM authority probe: preset={preset}, block={number}, processed={}",
        if index == 0 {
          "none"
        } else {
          rewrites[index - 1].0
        }
      );
      storage = authored.post_state;
      parent = authored.block.header;
    }
  }
}

fn reference_genesis_storage(wasm: &[u8]) -> Storage {
  preset_genesis_storage(
    wasm,
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  )
}

pub(super) fn preset_genesis_storage(
  wasm: &[u8],
  id: &polkadot_sdk::sp_genesis_builder::PresetId,
) -> Storage {
  let preset = crate::genesis_config_presets::get_preset(id)
    .expect("requested preset is declared by the runtime");
  let mut config = serde_json::to_value(RuntimeGenesisConfig::default())
    .expect("default runtime genesis serializes");
  merge_genesis_patch(
    &mut config,
    serde_json::from_slice(&preset).expect("runtime-owned preset is JSON"),
  );
  let config: RuntimeGenesisConfig =
    serde_json::from_value(config).expect("complete runtime preset deserializes");
  let mut storage = config.build_storage().expect("reference genesis builds");
  storage
    .top
    .insert(well_known_keys::CODE.to_vec(), wasm.to_vec());
  storage
}

pub(super) fn current_top_storage() -> Storage {
  let mut storage = Storage::default();
  let mut cursor = Vec::new();
  while let Some(key) = polkadot_sdk::sp_io::storage::next_key(&cursor) {
    let value = polkadot_sdk::sp_io::storage::get(&key)
      .unwrap_or_else(|| panic!("enumerated storage key disappeared: {key:?}"));
    cursor = key.clone();
    storage.top.insert(key, value.to_vec());
  }
  storage
}

fn prepare_actor_fixture(
  wasm: &[u8],
  actor_count: u32,
  workload_schedule: WorkloadSchedule,
) -> PreparedActorFixture {
  let steps = actors_integration_tests::transfer_contract_steps(
    super::common::BOB,
    primitives::AssetKind::Native,
    crate::EXISTENTIAL_DEPOSIT,
  );
  let (next_control_maximum, _) = actors_integration_tests::one_step_fifo_attempt_maxima(&steps);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (actor_ids, actor_profiles) = ext.execute_with(|| {
    System::set_block_number(1);
    let initial_active = pallet_deos_actors::ActiveActorCount::<Runtime>::get();
    let maximum = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
    assert!(
      initial_active.saturating_add(actor_count) <= maximum,
      "prepared W1 population must fit the reference active bound"
    );
    let mut actor_ids = Vec::with_capacity(actor_count as usize);
    let mut actor_profiles = BTreeMap::new();
    for index in 0..actor_count {
      let manually_triggered = workload_schedule.uses_manual_trigger(index);
      let schedule = if manually_triggered {
        actors_integration_tests::manual_schedule()
      } else {
        actors_integration_tests::cadenced_schedule(1)
      };
      let actor_id = actors_integration_tests::create_system(ALICE, schedule, None, steps.clone());
      actors_integration_tests::fund_native(
        actor_id,
        1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
      );
      if manually_triggered {
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
      }
      actor_ids.push(actor_id);
      actor_profiles.insert(
        actor_id,
        WorkloadActorProfile {
          step_count: 1,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      );
    }
    assert_eq!(
      pallet_deos_actors::ActiveActorCount::<Runtime>::get(),
      initial_active.saturating_add(actor_count)
    );
    let signer_account = crate::AccountId::from(signer.public());
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(signer_account),
      u128::MAX / 4,
    ));
    (actor_ids, actor_profiles)
  });
  ext
    .commit_all()
    .expect("prepared Actor fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedActorFixture {
    storage,
    actor_ids,
    actor_profiles,
    signer,
    next_control_maximum,
    next_effect_maximum: actors_integration_tests::one_step_fifo_attempt_maxima(&steps).1,
  }
}

fn prepare_w3_fixture(wasm: &[u8]) -> PreparedActorFixture {
  let actor_count = W3_REPETITIONS_PER_CELL.saturating_mul(9);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (actor_ids, actor_profiles, next_control_maximum) = ext.execute_with(|| {
    System::set_block_number(1);
    let initial_active = pallet_deos_actors::ActiveActorCount::<Runtime>::get();
    assert_eq!(initial_active, REFERENCE_ACTIVE_SYSTEM_ACTORS);
    assert!(
      initial_active.saturating_add(actor_count)
        <= <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get()
    );
    let mut actor_ids = Vec::with_capacity(actor_count as usize);
    let mut actor_profiles = BTreeMap::new();
    let mut next_control_maximum = Weight::zero();
    for predicates_per_step in [0, 2, 4] {
      for step_count in [1, 2, 3] {
        let steps = actors_integration_tests::transfer_contract_steps_with_opening_predicates(
          super::common::BOB,
          primitives::AssetKind::Native,
          crate::EXISTENTIAL_DEPOSIT,
          step_count,
          predicates_per_step,
        );
        let (control, _) = if step_count == 1 && predicates_per_step == 0 {
          actors_integration_tests::one_step_fifo_attempt_maxima(&steps)
        } else {
          (
            Actors::contract_steps_admission_weight_upper(
              pallet_deos_actors::ActorType::System,
              &steps,
            ),
            Weight::zero(),
          )
        };
        next_control_maximum = next_control_maximum.max(control);
        for _ in 0..W3_REPETITIONS_PER_CELL {
          let actor_id = actors_integration_tests::create_system(
            ALICE,
            actors_integration_tests::manual_schedule(),
            None,
            steps.clone(),
          );
          actors_integration_tests::fund_native(
            actor_id,
            1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
          );
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
          actor_ids.push(actor_id);
          actor_profiles.insert(
            actor_id,
            WorkloadActorProfile {
              step_count,
              opening_predicates_per_step: predicates_per_step,
              task: Some(WorkloadTask::Transfer),
            },
          );
        }
      }
    }
    assert_eq!(actor_ids.len() as u32, actor_count);
    assert_eq!(actor_profiles.len() as u32, actor_count);
    (actor_ids, actor_profiles, next_control_maximum)
  });
  ext
    .commit_all()
    .expect("prepared W3 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedActorFixture {
    storage,
    actor_ids,
    actor_profiles,
    signer,
    next_control_maximum,
    next_effect_maximum: Weight::zero(),
  }
}

fn w4_contract_steps(task: WorkloadTask, initial_balance: u128) -> ContractSteps<Runtime> {
  let task = match task {
    WorkloadTask::Transfer => Task::Transfer {
      to: super::common::BOB,
      asset: primitives::AssetKind::Native,
      amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
    },
    WorkloadTask::RouterSwapOut => Task::SwapOut {
      asset_out: primitives::AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
      asset_in: primitives::AssetKind::Native,
      input_limit: InputLimit::Absolute(initial_balance),
      slippage_tolerance: Perbill::from_percent(5),
    },
    WorkloadTask::StopCycle => Task::StopCycle,
  };
  BoundedVec::try_from(vec![StepOf::<Runtime> {
    precondition: None,
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }])
  .expect("one W4 Step fits")
}

fn prepare_w4_fixture(wasm: &[u8]) -> PreparedActorFixture {
  let actor_count = W4_TRANSFER_ACTORS
    .saturating_add(W4_SWAP_OUT_ACTORS)
    .saturating_add(W4_STOP_CYCLE_ACTORS);
  let initial_balance = 10_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (actor_ids, actor_profiles, next_control_maximum, next_effect_maximum) =
    ext.execute_with(|| {
      System::set_block_number(1);
      assert_eq!(
        pallet_deos_actors::ActorIdentityCount::<Runtime>::get(),
        REFERENCE_SYSTEM_ACTOR_IDENTITIES
      );
      assert_eq!(
        actor_count.saturating_add(REFERENCE_SYSTEM_ACTOR_IDENTITIES),
        <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get(),
        "W4 fills production identity capacity without replacing reference identities"
      );

      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(ALICE),
        u128::MAX / 4,
      ));
      assert_ok!(super::common::create_test_asset(ASSET_A, &ALICE));
      assert_ok!(Assets::set_team(
        RuntimeOrigin::signed(ALICE),
        ASSET_A,
        ALICE.into(),
        ALICE.into(),
        ALICE.into(),
      ));
      assert_ok!(super::common::mint_tokens(
        ASSET_A,
        &ALICE,
        &ALICE,
        super::common::INITIAL_BALANCE,
      ));
      super::common::setup_deos_router_infrastructure()
        .expect("W4 canonical Router pool infrastructure initializes");

      let transfer_steps = w4_contract_steps(WorkloadTask::Transfer, initial_balance);
      let swap_steps = w4_contract_steps(WorkloadTask::RouterSwapOut, initial_balance);
      let stop_steps = w4_contract_steps(WorkloadTask::StopCycle, initial_balance);
      let mut next_control_maximum = Weight::zero();
      let mut next_effect_maximum = Weight::zero();
      for steps in [&transfer_steps, &swap_steps, &stop_steps] {
        let (control, effect) = actors_integration_tests::one_step_fifo_attempt_maxima(steps);
        next_control_maximum = next_control_maximum.max(control);
        next_effect_maximum = next_effect_maximum.max(effect);
      }

      let mut actor_ids = Vec::with_capacity(actor_count as usize);
      let mut actor_profiles = BTreeMap::new();
      let mut task_counts = BTreeMap::<&'static str, u32>::new();
      for index in 0..actor_count {
        // Every complete hundred contributes 95/4/1. The final 85 positions
        // contribute 80/4/1, yielding 9,485/400/100 after the retained 15
        // production System identities consume the remaining capacity slots.
        let task = match index % 100 {
          0..=3 => WorkloadTask::RouterSwapOut,
          4 => WorkloadTask::StopCycle,
          _ => WorkloadTask::Transfer,
        };
        let steps = match task {
          WorkloadTask::Transfer => transfer_steps.clone(),
          WorkloadTask::RouterSwapOut => swap_steps.clone(),
          WorkloadTask::StopCycle => stop_steps.clone(),
        };
        let schedule = if index.is_multiple_of(2) {
          actors_integration_tests::manual_schedule()
        } else {
          actors_integration_tests::cadenced_schedule(1)
        };
        let actor_id = actors_integration_tests::create_system(ALICE, schedule, None, steps);
        actors_integration_tests::fund_native(actor_id, initial_balance);
        if index.is_multiple_of(2) {
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
        }
        *task_counts
          .entry(match task {
            WorkloadTask::Transfer => "transfer",
            WorkloadTask::RouterSwapOut => "swap-out",
            WorkloadTask::StopCycle => "stop-cycle",
          })
          .or_default() += 1;
        actor_ids.push(actor_id);
        actor_profiles.insert(
          actor_id,
          WorkloadActorProfile {
            step_count: 1,
            opening_predicates_per_step: 0,
            task: Some(task),
          },
        );
      }
      assert_eq!(task_counts.get("transfer"), Some(&W4_TRANSFER_ACTORS));
      assert_eq!(task_counts.get("swap-out"), Some(&W4_SWAP_OUT_ACTORS));
      assert_eq!(task_counts.get("stop-cycle"), Some(&W4_STOP_CYCLE_ACTORS));
      let signer_account = crate::AccountId::from(signer.public());
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(signer_account),
        u128::MAX / 4,
      ));
      (
        actor_ids,
        actor_profiles,
        next_control_maximum,
        next_effect_maximum,
      )
    });
  ext
    .commit_all()
    .expect("prepared W4 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedActorFixture {
    storage,
    actor_ids,
    actor_profiles,
    signer,
    next_control_maximum,
    next_effect_maximum,
  }
}

fn prepare_w5_fixture(wasm: &[u8]) -> PreparedW5Fixture {
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (
    actor_ids,
    actor_profiles,
    zero_step,
    running,
    retry_prefix,
    productive_cleanup,
    apoptosis,
    retry_sovereign,
    retry_balance_before,
  ) = ext.execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(ALICE),
      u128::MAX / 4,
    ));
    assert_ok!(super::common::create_test_asset(ASSET_A, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    assert_ok!(super::common::mint_tokens(
      ASSET_A,
      &ALICE,
      &ALICE,
      super::common::INITIAL_BALANCE,
    ));
    super::common::setup_deos_router_infrastructure()
      .expect("W5 canonical Router pool infrastructure initializes");
    let transfer_step = || StepOf::<Runtime> {
      precondition: None,
      task: Task::Transfer {
        to: super::common::BOB,
        asset: primitives::AssetKind::Native,
        amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };

    let zero_step = actors_integration_tests::create_system(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      ContractSteps::<Runtime>::default(),
    );
    let running_steps =
      BoundedVec::try_from(vec![transfer_step(), transfer_step(), transfer_step()])
        .expect("W5 three-Step Running Contract fits");
    let running = actors_integration_tests::create_system(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      running_steps,
    );
    actors_integration_tests::fund_native(
      running,
      1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
    );

    let retry_steps = BoundedVec::try_from(vec![
      transfer_step(),
      StepOf::<Runtime> {
        precondition: None,
        task: Task::SwapOut {
          asset_out: primitives::AssetKind::Local(ASSET_A),
          amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
          asset_in: primitives::AssetKind::Native,
          input_limit: InputLimit::Absolute(1),
          slippage_tolerance: Perbill::zero(),
        },
        on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
      },
      transfer_step(),
    ])
    .expect("W5 committed-prefix retry Contract fits");
    let retry_prefix = actors_integration_tests::create_system(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      retry_steps,
    );
    let retry_balance_before = 10_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT);
    actors_integration_tests::fund_native(retry_prefix, retry_balance_before);
    let retry_sovereign = Actors::active_actor_state(retry_prefix)
      .expect("W5 retry Actor exists")
      .identity
      .sovereign_account;

    let cleanup_steps =
      BoundedVec::try_from(vec![transfer_step()]).expect("W5 productive-cleanup Contract fits");
    let productive_cleanup = actors_integration_tests::create_system_with_close_when(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      cleanup_steps,
      Some(crate::tests::common::close_when_action_and_normal_cycle()),
    );
    actors_integration_tests::fund_native(
      productive_cleanup,
      1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
    );

    let apoptosis_steps =
      BoundedVec::try_from(vec![transfer_step()]).expect("W5 apoptosis Contract fits");
    let apoptosis = actors_integration_tests::create_user(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      apoptosis_steps,
    );
    let apoptosis_account = Actors::active_actor_state(apoptosis)
      .expect("W5 apoptosis Actor exists")
      .identity
      .sovereign_account;
    let trigger_fee = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
      &<<Runtime as pallet_deos_actors::Config>::WeightInfo as WeightInfo>::manual_trigger(),
    );
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(apoptosis_account),
      crate::configs::actor_config::ActorMinUserBalance::get().saturating_add(trigger_fee),
    ));

    for actor_id in [zero_step, running, retry_prefix, productive_cleanup] {
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      apoptosis,
    ));

    let actor_ids = vec![
      zero_step,
      running,
      retry_prefix,
      productive_cleanup,
      apoptosis,
    ];
    let actor_profiles = BTreeMap::from([
      (
        zero_step,
        WorkloadActorProfile {
          step_count: 0,
          opening_predicates_per_step: 0,
          task: None,
        },
      ),
      (
        running,
        WorkloadActorProfile {
          step_count: 3,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      ),
      (
        retry_prefix,
        WorkloadActorProfile {
          step_count: 3,
          opening_predicates_per_step: 0,
          task: None,
        },
      ),
      (
        productive_cleanup,
        WorkloadActorProfile {
          step_count: 1,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      ),
      (
        apoptosis,
        WorkloadActorProfile {
          step_count: 1,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      ),
    ]);
    (
      actor_ids,
      actor_profiles,
      zero_step,
      running,
      retry_prefix,
      productive_cleanup,
      apoptosis,
      retry_sovereign,
      retry_balance_before,
    )
  });
  ext
    .commit_all()
    .expect("prepared W5 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedW5Fixture {
    actors: PreparedActorFixture {
      storage,
      actor_ids,
      actor_profiles,
      signer,
      next_control_maximum: Weight::zero(),
      next_effect_maximum: Weight::zero(),
    },
    zero_step,
    running,
    retry_prefix,
    productive_cleanup,
    apoptosis,
    retry_sovereign,
    retry_balance_before,
  }
}

fn assert_paid_window_deadline(actor_id: ActorId, window_start: u32) {
  let state = Actors::active_actor_state(actor_id).expect("future-window Actor stays active");
  assert_eq!(state.identity.cycle_nonce, 0);
  assert_eq!(state.hot.cycle_state, CycleState::Idle);
  assert!(state.hot.pending_signal);
  assert!(state.run_state.is_none());
  assert!(Actors::service_nodes(actor_id).is_none());
  assert!(Actors::trigger_deadline_handles(actor_id).is_none());
  let handle = Actors::deadline_handles(actor_id).expect("paid future window owns a Deadline");
  let process = Actors::actor_processes(actor_id).expect("future-window process exists");
  assert_eq!(handle.key, WakeupKey::Block(window_start));
  assert_eq!(handle.actor.actor_id, actor_id);
  assert_eq!(handle.actor.generation, process.generation);
  assert_eq!(process.status, pallet_deos_actors::ProcessStatus::Serving);
  assert_eq!(
    process.residence,
    Some(ProcessResidence::Deadline {
      key: handle.key,
      page: handle.page,
      slot: handle.slot,
    })
  );
  assert_eq!(
    Actors::deadline_pages(handle.key, handle.page)
      .expect("paid future window owns its Deadline page")
      .entries[usize::from(handle.slot)],
    Some(handle.actor)
  );
  assert!(state.hot.wakeup_pointer.is_none());
  assert!(state.hot.queue_ticket.is_none());
}

fn prepare_w6_fixture(wasm: &[u8]) -> PreparedW6Fixture {
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let owner = crate::AccountId::from(signer.public());
  let (
    actor_ids,
    actor_profiles,
    dense_manual,
    sparse_block,
    sparse_tick,
    randomized_cadenced,
    randomized_periods,
    paused_actor,
    closed_actor,
  ) = ext.execute_with(|| {
    System::set_block_number(1);
    assert!(!Actors::user_creation_enabled());
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
    assert!(Actors::user_creation_enabled());
    System::assert_last_event(RuntimeEvent::Actors(Event::UserCreationEnabledSet {
      enabled: true,
    }));
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(owner.clone()),
      u128::MAX / 4,
    ));
    let steps = actors_integration_tests::transfer_contract_steps(
      super::common::BOB,
      primitives::AssetKind::Native,
      crate::EXISTENTIAL_DEPOSIT,
    );
    let mut actor_ids = Vec::new();
    let mut actor_profiles = BTreeMap::new();
    let mut add_actor = |actor_type, schedule, window, trigger_manual| {
      let actor_id = match actor_type {
        ActorType::User => {
          actors_integration_tests::create_user(owner.clone(), schedule, window, steps.clone())
        }
        ActorType::System => {
          actors_integration_tests::create_system(owner.clone(), schedule, window, steps.clone())
        }
      };
      let identity = Actors::actor_identity(actor_id).expect("W6 created identity exists");
      assert_eq!(identity.actor_class.actor_type(), actor_type);
      assert_eq!(identity.mutability, Mutability::Mutable);
      actors_integration_tests::fund_native(
        actor_id,
        1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
      );
      if trigger_manual {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(owner.clone()),
          actor_id,
        ));
      }
      actor_ids.push(actor_id);
      actor_profiles.insert(
        actor_id,
        WorkloadActorProfile {
          step_count: 1,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      );
      actor_id
    };

    let dense_manual = (0..3)
      .map(|_| {
        add_actor(
          ActorType::User,
          actors_integration_tests::manual_schedule(),
          None,
          true,
        )
      })
      .collect::<Vec<_>>();
    let sparse_block = [3, 6, 9]
      .into_iter()
      .enumerate()
      .map(|(index, start)| {
        add_actor(
          if index == 1 {
            ActorType::System
          } else {
            ActorType::User
          },
          actors_integration_tests::manual_schedule(),
          Some(ScheduleWindow { start, end: 120 }),
          true,
        )
      })
      .collect::<Vec<_>>();
    let sparse_tick = [1, 4, 7]
      .into_iter()
      .map(|after_ticks| {
        add_actor(
          ActorType::User,
          actors_integration_tests::at_time_schedule(after_ticks),
          None,
          false,
        )
      })
      .collect::<Vec<_>>();
    let mut random_state = W6_RANDOM_SEED;
    let randomized_periods = (0..3)
      .map(|_| {
        random_state ^= random_state << 13;
        random_state ^= random_state >> 7;
        random_state ^= random_state << 17;
        1 + random_state % 7
      })
      .collect::<Vec<_>>();
    let randomized_cadenced = randomized_periods
      .iter()
      .map(|period| {
        add_actor(
          ActorType::User,
          actors_integration_tests::cadenced_schedule(*period),
          None,
          false,
        )
      })
      .collect::<Vec<_>>();
    let paused_actor = sparse_block[1];
    let closed_actor = sparse_block[2];
    assert_eq!(actor_ids.len(), 12);
    for actor_id in &actor_ids {
      assert_eq!(
        Actors::actor_identity(*actor_id)
          .expect("W6 identity exists")
          .actor_class
          .actor_type(),
        if *actor_id == paused_actor {
          ActorType::System
        } else {
          ActorType::User
        },
        "W6 has eleven fee-bearing Users and one fee-exempt pause-controlled System",
      );
    }
    for (actor_id, start) in sparse_block.iter().zip([3, 6, 9]) {
      assert_paid_window_deadline(*actor_id, start);
    }
    (
      actor_ids,
      actor_profiles,
      dense_manual,
      sparse_block,
      sparse_tick,
      randomized_cadenced,
      randomized_periods,
      paused_actor,
      closed_actor,
    )
  });
  ext
    .commit_all()
    .expect("prepared W6 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedW6Fixture {
    actors: PreparedActorFixture {
      storage,
      actor_ids,
      actor_profiles,
      signer,
      next_control_maximum: Weight::zero(),
      next_effect_maximum: Weight::zero(),
    },
    dense_manual,
    sparse_block,
    sparse_tick,
    randomized_cadenced,
    randomized_periods,
    paused_actor,
    closed_actor,
  }
}

fn prepare_w7_fixture(wasm: &[u8], maximum_population: bool) -> PreparedW7Fixture {
  let steps = actors_integration_tests::transfer_contract_steps(
    super::common::BOB,
    primitives::AssetKind::Native,
    crate::EXISTENTIAL_DEPOSIT,
  );
  let (next_control_maximum, next_effect_maximum) =
    actors_integration_tests::one_step_fifo_attempt_maxima(&steps);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (actor_ids, actor_profiles, due, future, unsignaled, reference_identities) = ext
    .execute_with(|| {
      System::set_block_number(1);
      let reference_identities = pallet_deos_actors::ActorIdentityCount::<Runtime>::get();
      assert_eq!(reference_identities, REFERENCE_SYSTEM_ACTOR_IDENTITIES);
      assert_eq!(
        pallet_deos_actors::ActiveActorCount::<Runtime>::get(),
        REFERENCE_ACTIVE_SYSTEM_ACTORS
      );
      let future_count = if maximum_population {
        W7_FUTURE_WORKLOAD_ACTORS
      } else {
        0
      };
      let unsignaled_count = if maximum_population {
        W7_UNSIGNALED_WORKLOAD_ACTORS
      } else {
        0
      };
      let added_count = future_count
        .saturating_add(unsignaled_count)
        .saturating_add(W7_DUE_ACTORS);
      if maximum_population {
        assert_eq!(
          added_count.saturating_add(reference_identities),
          <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get(),
          "W7 fills the exact production identity capacity"
        );
      }

      let mut actor_ids = Vec::with_capacity(added_count as usize);
      let mut actor_profiles = BTreeMap::new();
      let mut add_actor = |window, trigger_manual| {
        let actor_id = actors_integration_tests::create_system(
          ALICE,
          actors_integration_tests::manual_schedule(),
          window,
          steps.clone(),
        );
        actors_integration_tests::fund_native(
          actor_id,
          1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
        );
        if trigger_manual {
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
        }
        actor_ids.push(actor_id);
        actor_profiles.insert(
          actor_id,
          WorkloadActorProfile {
            step_count: 1,
            opening_predicates_per_step: 0,
            task: Some(WorkloadTask::Transfer),
          },
        );
        actor_id
      };
      let due = (0..W7_DUE_ACTORS)
        .map(|_| add_actor(None, true))
        .collect::<Vec<_>>();
      let future = (0..future_count)
        .map(|_| {
          add_actor(
            Some(ScheduleWindow {
              start: W7_FUTURE_BLOCK,
              end: W7_FUTURE_BLOCK.saturating_add(100),
            }),
            true,
          )
        })
        .collect::<Vec<_>>();
      let unsignaled = (0..unsignaled_count)
        .map(|_| add_actor(None, false))
        .collect::<Vec<_>>();
      if maximum_population {
        for actor_id in &future {
          assert_paid_window_deadline(*actor_id, W7_FUTURE_BLOCK);
        }
        assert_eq!(
          reference_identities
            .saturating_add(future.len() as u32)
            .saturating_add(unsignaled.len() as u32),
          9_900,
          "retained references and future/unsignaled workload form 9,900 non-due identities"
        );
      }
      assert_eq!(due.len() as u32, W7_DUE_ACTORS);
      assert_eq!(actor_ids.len() as u32, added_count);
      (
        actor_ids,
        actor_profiles,
        due,
        future,
        unsignaled,
        reference_identities,
      )
    });
  ext
    .commit_all()
    .expect("prepared W7 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedW7Fixture {
    actors: PreparedActorFixture {
      storage,
      actor_ids,
      actor_profiles,
      signer,
      next_control_maximum,
      next_effect_maximum,
    },
    due,
    future,
    unsignaled,
    reference_identities,
  }
}

fn prepare_w8_fixture(wasm: &[u8], tombstone_prefix: u32) -> PreparedW8Fixture {
  assert!(W8_TOMBSTONE_PREFIXES.contains(&tombstone_prefix));
  let steps = actors_integration_tests::transfer_contract_steps(
    super::common::BOB,
    primitives::AssetKind::Native,
    crate::EXISTENTIAL_DEPOSIT,
  );
  let (next_control_maximum, next_effect_maximum) =
    actors_integration_tests::one_step_fifo_attempt_maxima(&steps);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let (due, actor_profiles) = ext.execute_with(|| {
    System::set_block_number(1);
    assert_eq!(
      pallet_deos_actors::ActorIdentityCount::<Runtime>::get(),
      REFERENCE_SYSTEM_ACTOR_IDENTITIES
    );
    assert_eq!(
      pallet_deos_actors::ActiveActorCount::<Runtime>::get(),
      REFERENCE_ACTIVE_SYSTEM_ACTORS
    );

    let mut tombstones = Vec::with_capacity(tombstone_prefix as usize);
    for _ in 0..tombstone_prefix {
      let actor_id = actors_integration_tests::create_system(
        ALICE,
        actors_integration_tests::manual_schedule(),
        None,
        steps.clone(),
      );
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
      tombstones.push(actor_id);
    }
    for actor_id in &tombstones {
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), *actor_id));
    }
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert_eq!(Actors::service_header().count, 0);
    assert!(tombstones.iter().all(|actor_id| {
      pallet_deos_actors::ActorControlLocators::<Runtime>::get(actor_id).is_none()
        && Actors::active_actor_state(*actor_id).is_none()
        && Actors::service_nodes(*actor_id).is_none()
    }));

    let mut actor_profiles = BTreeMap::new();
    let due = (0..W8_DUE_ACTORS)
      .map(|_| {
        let actor_id = actors_integration_tests::create_system(
          ALICE,
          actors_integration_tests::manual_schedule(),
          None,
          steps.clone(),
        );
        actors_integration_tests::fund_native(
          actor_id,
          1_000u128.saturating_mul(crate::EXISTENTIAL_DEPOSIT),
        );
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
        actor_profiles.insert(
          actor_id,
          WorkloadActorProfile {
            step_count: 1,
            opening_predicates_per_step: 0,
            task: Some(WorkloadTask::Transfer),
          },
        );
        actor_id
      })
      .collect::<Vec<_>>();
    assert_eq!(due.len() as u32, W8_DUE_ACTORS);
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    let service = Actors::service_header();
    assert_eq!(service.count, W8_DUE_ACTORS);
    assert_eq!(
      service.cursor.map(|actor| actor.actor_id),
      due.first().copied()
    );
    assert!(due.iter().all(|actor_id| {
      Actors::service_nodes(*actor_id).is_some_and(|node| node.eligible_from == 2)
    }));
    assert_eq!(
      pallet_deos_actors::ActorIdentityCount::<Runtime>::get(),
      REFERENCE_SYSTEM_ACTOR_IDENTITIES.saturating_add(W8_DUE_ACTORS),
      "closed prefix identities do not consume retained production capacity"
    );
    (due, actor_profiles)
  });
  ext
    .commit_all()
    .expect("prepared W8 fixture commits before block authoring");
  let storage = ext.execute_with(current_top_storage);
  PreparedW8Fixture {
    actors: PreparedActorFixture {
      storage,
      actor_ids: due.clone(),
      actor_profiles,
      signer,
      next_control_maximum,
      next_effect_maximum,
    },
    due,
  }
}

fn prepare_w9_fixture(wasm: &[u8]) -> PreparedActorFixture {
  let mut fixture = prepare_w7_fixture(wasm, false).actors;
  assert_eq!(fixture.actor_ids.len() as u32, W9_DUE_ACTORS);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    fixture.storage.clone(),
    crate::VERSION.state_version(),
  );
  ext.execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(ALICE),
      u128::MAX / 4,
    ));
    assert_ok!(super::common::create_test_asset(ASSET_A, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    assert_ok!(super::common::mint_tokens(
      ASSET_A,
      &ALICE,
      &ALICE,
      super::common::INITIAL_BALANCE,
    ));
    super::common::setup_deos_router_infrastructure()
      .expect("W9 canonical Router pool infrastructure initializes");
    let signer_account = crate::AccountId::from(fixture.signer.public());
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      MultiAddress::Id(signer_account.clone()),
      u128::MAX / 4,
    ));
    assert_ok!(Assets::mint(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      MultiAddress::Id(signer_account),
      u128::MAX / 4,
    ));
  });
  ext
    .commit_all()
    .expect("prepared W9 fixture commits before block authoring");
  fixture.storage = ext.execute_with(current_top_storage);
  fixture
}

fn parent_header_for(storage: Storage, wasm: &[u8], parent_number: u32) -> Header {
  let mut ext =
    TestExternalities::new_with_code_and_state(wasm, storage, crate::VERSION.state_version());
  let state_root = *ext.as_backend().root();
  Header::new(
    parent_number,
    BlakeTwo256::ordered_trie_root(Vec::new(), crate::VERSION.extrinsics_root_state_version()),
    state_root,
    Default::default(),
    Digest::default(),
  )
}

fn inherent_data_for(parent: &Header, block_number: u32) -> InherentData {
  let parent_head = HeadData(parent.encode());
  let proof_builder = RelayStateSproofBuilder {
    para_id: crate::PARACHAIN_ID.into(),
    current_slot: u64::from(block_number).into(),
    included_para_head: Some(parent_head.clone()),
    ..Default::default()
  };
  let (relay_parent_storage_root, relay_chain_state) = proof_builder.into_state_root_and_proof();
  let mut data = InherentData::new();
  data
    .put_data(
      polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER,
      &u64::from(block_number).saturating_mul(crate::SLOT_DURATION),
    )
    .expect("timestamp inherent encodes");
  data
    .put_data(
      INHERENT_IDENTIFIER,
      &ParachainInherentData {
        validation_data: PersistedValidationData {
          parent_head,
          relay_parent_number: block_number,
          relay_parent_storage_root,
          max_pov_size: crate::MAXIMUM_BLOCK_WEIGHT.proof_size() as u32,
        },
        relay_chain_state,
        downward_messages: Default::default(),
        horizontal_messages: Default::default(),
        relay_parent_descendants: Default::default(),
        collator_peer_id: None,
      },
    )
    .expect("parachain inherent encodes");
  pallet_deos_actors::provide_actor_prepass_inherent_data(&mut data)
    .expect("Actor Prepass inherent encodes");
  data
}

fn saturate_user_dispatch(
  signer: &sr25519::Pair,
  block_number: u32,
  initial_nonce: crate::Nonce,
  extrinsics: &mut Vec<crate::UncheckedExtrinsic>,
  fill_shared: bool,
) -> (u32, Weight, Option<TransactionValidityError>) {
  let user_limit = BlockResourceBudgetValue::get().limits().user_base_turn();
  let mut nonce = initial_nonce;
  let mut calls = 0u32;
  loop {
    assert!(calls < 10_000, "valid user-demand fixture remains bounded");
    let call = RuntimeCall::System(polkadot_sdk::frame_system::Call::remark {
      remark: (block_number, nonce).encode(),
    });
    let extrinsic =
      actors_integration_tests::signed_extrinsic(signer, crate::Nonce::from(nonce), call);
    let next_weight = polkadot_sdk::frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
      &crate::configs::RuntimeBlockWeights::get(),
      &extrinsic.get_dispatch_info(),
      extrinsic.encoded_size(),
    );
    let state = Actors::block_resource_state().expect("Actor Prepass opens resource state");
    let remaining = user_limit
      .checked_sub(&state.usage().user_dispatch_used())
      .unwrap_or_else(Weight::zero);
    if !fill_shared && !next_weight.all_lte(remaining) {
      assert!(
        calls > 0,
        "continuous demand must admit at least one valid call"
      );
      return (calls, next_weight, None);
    }
    let result = if fill_shared {
      // Match block authoring: rejected candidates leave no state in the committed block.
      polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
        let result = Executive::apply_extrinsic(extrinsic.clone());
        if result.is_ok() {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
        } else {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
        }
      })
      .expect("one bounded authoring attempt fits the storage transaction limit")
    } else {
      Executive::apply_extrinsic(extrinsic.clone())
    };
    match result {
      Ok(result) => {
        result.unwrap_or_else(|error| panic!("valid user call failed dispatch: {error:?}"))
      }
      Err(error) if fill_shared => return (calls, next_weight, Some(error)),
      Err(error) => panic!("valid user call rejected: {error:?}"),
    }
    extrinsics.push(extrinsic);
    nonce = nonce.saturating_add(1);
    calls = calls.saturating_add(1);
  }
}

fn saturate_user_dispatch_with_router_swaps(
  signer: &sr25519::Pair,
  block_number: u32,
  initial_nonce: crate::Nonce,
  extrinsics: &mut Vec<crate::UncheckedExtrinsic>,
) -> (u32, Weight) {
  let signer_account = crate::AccountId::from(signer.public());
  let user_limit = BlockResourceBudgetValue::get().limits().user_base_turn();
  let mut nonce = initial_nonce;
  let mut calls = 0u32;
  loop {
    assert!(calls < 1_000, "W9 Router saturation remains bounded");
    let native_to_local = calls.is_multiple_of(2);
    let call = RuntimeCall::DeosRouter(pallet_deos_router::Call::swap {
      from: if native_to_local {
        primitives::AssetKind::Native
      } else {
        primitives::AssetKind::Local(ASSET_A)
      },
      to: if native_to_local {
        primitives::AssetKind::Local(ASSET_A)
      } else {
        primitives::AssetKind::Native
      },
      amount_in: super::common::SWAP_AMOUNT,
      min_amount_out: 1,
      recipient: signer_account.clone(),
      deadline: block_number.saturating_add(1),
    });
    let extrinsic =
      actors_integration_tests::signed_extrinsic(signer, crate::Nonce::from(nonce), call);
    let next_weight = polkadot_sdk::frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
      &crate::configs::RuntimeBlockWeights::get(),
      &extrinsic.get_dispatch_info(),
      extrinsic.encoded_size(),
    );
    let state = Actors::block_resource_state().expect("Actor Prepass opens resource state");
    let remaining = user_limit
      .checked_sub(&state.usage().user_dispatch_used())
      .unwrap_or_else(Weight::zero);
    if !next_weight.all_lte(remaining) {
      assert!(calls > 0, "W9 must admit at least one valid Router swap");
      return (calls, next_weight);
    }
    Executive::apply_extrinsic(extrinsic.clone())
      .unwrap_or_else(|error| panic!("valid W9 Router call rejected: {error:?}"))
      .unwrap_or_else(|error| panic!("valid W9 Router call failed dispatch: {error:?}"));
    extrinsics.push(extrinsic);
    nonce = nonce.saturating_add(1);
    calls = calls.saturating_add(1);
  }
}

fn authored_metrics(
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
  user_calls: u32,
  next_user_weight: Option<Weight>,
  user_admission_refusal: Option<TransactionValidityError>,
  prepass_steps: u32,
  prepass_trigger_occurrences: u32,
  prepass_actor_control: Weight,
  prepass_actor_effect: Weight,
  pre_idle_remaining: Weight,
) -> FullExecutiveBlockMetrics {
  let mut progressed_steps = Vec::new();
  let mut opening_steps = 0u32;
  let mut middle_steps = 0u32;
  let mut final_steps = 0u32;
  let mut transfer_steps = 0u32;
  let mut swap_out_steps = 0u32;
  let mut stop_cycle_steps = 0u32;
  let mut non_successful_steps = 0u32;
  let mut completed_cycles = 0u32;
  let mut completed_cycle_actors = Vec::new();
  let mut failed_cycle_actors = Vec::new();
  let mut suspended_steps = Vec::new();
  let mut continued_steps = Vec::new();
  let mut closed_actors = Vec::new();
  let mut paused_actors = Vec::new();
  let mut resumed_actors = Vec::new();
  let mut trigger_occurrences = Vec::new();
  for record in System::events() {
    let (actor_id, step_index, successful, observed_task) = match record.event {
      RuntimeEvent::Actors(Event::ActorPaused { actor_id })
        if workload_profiles.contains_key(&actor_id) =>
      {
        paused_actors.push(actor_id);
        continue;
      }
      RuntimeEvent::Actors(Event::ActorResumed { actor_id })
        if workload_profiles.contains_key(&actor_id) =>
      {
        resumed_actors.push(actor_id);
        continue;
      }
      RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
        actor_id,
        trigger_family,
        ..
      }) if workload_profiles.contains_key(&actor_id) => {
        trigger_occurrences.push((actor_id, trigger_family));
        continue;
      }
      RuntimeEvent::Actors(Event::TransferExecuted {
        actor_id,
        step_index,
        ..
      }) if workload_profiles.contains_key(&actor_id) => {
        transfer_steps = transfer_steps.saturating_add(1);
        (actor_id, step_index, true, Some(WorkloadTask::Transfer))
      }
      RuntimeEvent::Actors(Event::SwapExecuted {
        actor_id,
        step_index,
        ..
      }) if workload_profiles.contains_key(&actor_id) => {
        swap_out_steps = swap_out_steps.saturating_add(1);
        (
          actor_id,
          step_index,
          true,
          Some(WorkloadTask::RouterSwapOut),
        )
      }
      RuntimeEvent::Actors(Event::CycleStopped {
        actor_id,
        step_index,
        ..
      }) if workload_profiles.contains_key(&actor_id) => {
        stop_cycle_steps = stop_cycle_steps.saturating_add(1);
        (actor_id, step_index, true, Some(WorkloadTask::StopCycle))
      }
      RuntimeEvent::Actors(Event::StepSkipped {
        actor_id,
        step_index,
        ..
      }) if workload_profiles.contains_key(&actor_id) => (actor_id, step_index, false, None),
      RuntimeEvent::Actors(Event::StepFailed {
        actor_id,
        step_index,
        retry_class,
        ref error,
        ..
      }) if workload_profiles.contains_key(&actor_id) => {
        println!(
          "EXP_0066_WORKLOAD_STEP_FAILED actor={actor_id} step={step_index} retry={retry_class:?} error={error:?}"
        );
        (actor_id, step_index, false, None)
      }
      RuntimeEvent::Actors(Event::CycleSummary {
        actor_id, result, ..
      }) if workload_profiles.contains_key(&actor_id) => {
        match result {
          CycleResult::Completed => {
            completed_cycles = completed_cycles.saturating_add(1);
            completed_cycle_actors.push(actor_id);
          }
          CycleResult::Failed => failed_cycle_actors.push(actor_id),
          CycleResult::Cancelled => {}
        }
        continue;
      }
      RuntimeEvent::Actors(Event::CycleSuspended {
        actor_id, cursor, ..
      }) if workload_profiles.contains_key(&actor_id) => {
        suspended_steps.push((actor_id, cursor));
        continue;
      }
      RuntimeEvent::Actors(Event::CycleContinued {
        actor_id, cursor, ..
      }) if workload_profiles.contains_key(&actor_id) => {
        continued_steps.push((actor_id, cursor));
        continue;
      }
      RuntimeEvent::Actors(Event::ActorClosed {
        actor_id, reason, ..
      }) if workload_profiles.contains_key(&actor_id) => {
        closed_actors.push((actor_id, reason));
        continue;
      }
      _ => continue,
    };
    let profile = workload_profiles
      .get(&actor_id)
      .expect("matched workload Actor has a profile");
    if let (Some(task), Some(expected)) = (observed_task, profile.task) {
      assert_eq!(task, expected, "effect event matches its fixture task");
    }
    assert!(step_index < profile.step_count);
    if step_index == 0 {
      opening_steps = opening_steps.saturating_add(1);
    } else if step_index.saturating_add(1) == profile.step_count {
      final_steps = final_steps.saturating_add(1);
    } else {
      middle_steps = middle_steps.saturating_add(1);
    }
    if !successful {
      non_successful_steps = non_successful_steps.saturating_add(1);
    }
    progressed_steps.push((actor_id, step_index));
  }
  let mut actor_ids = progressed_steps
    .iter()
    .map(|(actor_id, _)| *actor_id)
    .collect::<Vec<_>>();
  actor_ids.sort_unstable();
  actor_ids.dedup();
  assert_eq!(
    progressed_steps.len(),
    actor_ids.len(),
    "full block must preserve Q1 for the workload population"
  );
  let actor_steps =
    u32::try_from(progressed_steps.len()).expect("bounded block Step count fits u32");
  assert_eq!(
    actor_steps,
    opening_steps
      .saturating_add(middle_steps)
      .saturating_add(final_steps)
  );
  assert_eq!(
    actor_steps,
    transfer_steps
      .saturating_add(swap_out_steps)
      .saturating_add(stop_cycle_steps)
      .saturating_add(non_successful_steps),
    "every workload Step has exactly one terminal task outcome"
  );
  let telemetry = Actors::finalized_block_resource_telemetry()
    .expect("full Executive finalization publishes Actor telemetry");
  let usage = telemetry.usage();
  FullExecutiveBlockMetrics {
    actor_steps,
    distinct_actors: actor_ids.len() as u32,
    progressed_steps,
    opening_steps,
    middle_steps,
    final_steps,
    transfer_steps,
    swap_out_steps,
    stop_cycle_steps,
    non_successful_steps,
    completed_cycles,
    completed_cycle_actors,
    failed_cycle_actors,
    suspended_steps,
    continued_steps,
    closed_actors,
    paused_actors,
    resumed_actors,
    trigger_occurrences,
    user_calls,
    next_user_weight,
    user_admission_refusal,
    prepass_steps,
    prepass_trigger_occurrences,
    prepass_actor_control,
    prepass_actor_effect,
    pre_idle_remaining,
    actor_control: usage.actor_control_used(),
    actor_effect: usage.actor_effect_used(),
    user_dispatch: usage.user_dispatch_used(),
    queue_head: Actors::queue_head(),
    queue_tail: Actors::queue_tail(),
  }
}

fn author_complete_block_after_with_calls(
  pre_state: Storage,
  wasm: &[u8],
  parent: &Header,
  block_number: u32,
  demand: UserDemand,
  signer: &sr25519::Pair,
  signer_nonce: crate::Nonce,
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
  signed_calls: &[RuntimeCall],
) -> AuthoredBlock {
  let signed_script = signed_calls
    .iter()
    .enumerate()
    .map(|(index, call)| {
      (
        signer,
        signer_nonce
          .checked_add(u32::try_from(index).unwrap())
          .unwrap(),
        call,
      )
    })
    .collect::<Vec<_>>();
  author_complete_block_after_with_context(
    pre_state,
    wasm,
    parent,
    block_number,
    demand,
    signer,
    signer_nonce,
    workload_profiles,
    &signed_script,
    inherent_data_for(parent, block_number),
  )
}

fn author_complete_block_after_with_context(
  pre_state: Storage,
  wasm: &[u8],
  parent: &Header,
  block_number: u32,
  demand: UserDemand,
  signer: &sr25519::Pair,
  signer_nonce: crate::Nonce,
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
  signed_calls: &[(&sr25519::Pair, crate::Nonce, &RuntimeCall)],
  inherent_data: InherentData,
) -> AuthoredBlock {
  author_complete_block_after_with_expected_dispatch(
    pre_state,
    wasm,
    parent,
    block_number,
    demand,
    signer,
    signer_nonce,
    workload_profiles,
    signed_calls,
    inherent_data,
    None,
  )
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn observe_authored_proof_prefix(
  recorder: &Recorder<polkadot_sdk::sp_core::Blake2Hasher>,
  proof_size: &RecordingProofSizeProvider,
  parent_hash: H256,
  block_number: u32,
  phase: &str,
) {
  let Ok(directory) = std::env::var("DEOS_PROOF_PREFIX_OUTPUT") else {
    return;
  };
  let before = recorder.estimate_encoded_size();
  let recorded_keys = recorder.recorded_keys();
  let weight = System::block_weight().total();
  let block_size = polkadot_sdk::frame_system::BlockSize::<Runtime>::get().unwrap_or(0);
  assert_eq!(recorder.estimate_encoded_size(), before);
  assert_eq!(recorder.recorded_keys(), recorded_keys);
  let storage_info = runtime_proof_storage_info();
  let mut keys = Vec::new();
  for (root, entries) in recorded_keys {
    for (key, _) in entries {
      let mut matching = storage_info
        .iter()
        .filter(|info| key.starts_with(&info.prefix));
      let owner = matching
        .next()
        .map(|info| {
          format!(
            "{}.{}",
            String::from_utf8_lossy(&info.pallet_name),
            String::from_utf8_lossy(&info.storage_name),
          )
        })
        .unwrap_or_else(|| {
          assert!(
            key.starts_with(b":"),
            "every ordinary key has a typed owner"
          );
          "WellKnownTop".to_owned()
        });
      assert!(matching.next().is_none(), "one owner per storage prefix");
      keys.push(serde_json::json!({
        "root": format!("{root:?}"),
        "key": key.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "owner": owner,
      }));
    }
  }
  keys.sort_by(|left, right| {
    left["key"]
      .as_str()
      .unwrap()
      .cmp(right["key"].as_str().unwrap())
  });
  let path =
    std::path::Path::new(&directory).join(format!("{parent_hash:?}-{block_number}-{phase}.json"));
  assert!(!path.exists(), "each prefix observation is retained once");
  std::fs::write(
    path,
    serde_json::to_vec_pretty(&serde_json::json!({
      "block": block_number,
      "parentHash": format!("{parent_hash:?}"),
      "phase": phase,
      "recordedProof": before,
      "recordedEstimations": proof_size.recorded_estimations(),
      "registeredWeight": [weight.ref_time(), weight.proof_size()],
      "blockSize": block_size,
      "keys": keys,
      "diagnosticReadsDoNotChangeRecorder": true,
    }))
    .unwrap(),
  )
  .unwrap();
}

fn author_complete_block_after_with_expected_dispatch(
  pre_state: Storage,
  wasm: &[u8],
  parent: &Header,
  block_number: u32,
  demand: UserDemand,
  signer: &sr25519::Pair,
  signer_nonce: crate::Nonce,
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
  signed_calls: &[(&sr25519::Pair, crate::Nonce, &RuntimeCall)],
  inherent_data: InherentData,
  expected_dispatch: Option<&[polkadot_sdk::sp_runtime::DispatchResult]>,
) -> AuthoredBlock {
  #[cfg(not(feature = "runtime-benchmarks"))]
  assert!(
    pre_state.children_default.is_empty(),
    "TOP-only fixtures require no initial child storage"
  );
  if let Some(expected) = expected_dispatch {
    assert_eq!(
      expected.len(),
      signed_calls.len(),
      "every scripted result is explicit"
    );
  }
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  let proof_size = RecordingProofSizeProvider::new(recorder.clone());
  ext.register_extension(ProofSizeExt::new(proof_size.clone()));
  let execution_recorder = recorder.clone();
  let (block, metrics) = ext.execute_with_recorder(recorder, || {
    let header = Header::new(
      block_number,
      Default::default(),
      Default::default(),
      parent.hash(),
      Digest {
        logs: vec![DigestItem::PreRuntime(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          u64::from(block_number).encode(),
        )],
      },
    );
    Executive::initialize_block(&header);
    let mut extrinsics = inherent_data.create_extrinsics();
    assert!(
      extrinsics.len() >= 3,
      "runtime inherent order must include parachain context, timestamp, and Actor Prepass"
    );
    for extrinsic in &extrinsics {
      Executive::apply_extrinsic(extrinsic.clone())
        .expect("ordered inherent is valid")
        .expect("ordered inherent dispatch succeeds");
    }
    let prepass_state =
      Actors::block_resource_state().expect("Actor Prepass inherent runs before signed demand");
    let prepass_usage = prepass_state.usage();
    let mut prepass_steps = 0u32;
    let mut prepass_trigger_occurrences = 0u32;
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(
          Event::TransferExecuted { actor_id, .. }
          | Event::SwapExecuted { actor_id, .. }
          | Event::CycleStopped { actor_id, .. }
          | Event::StepSkipped { actor_id, .. }
          | Event::StepFailed { actor_id, .. },
        ) if workload_profiles.contains_key(&actor_id) => {
          prepass_steps = prepass_steps.saturating_add(1);
        }
        RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, .. })
          if workload_profiles.contains_key(&actor_id) =>
        {
          prepass_trigger_occurrences = prepass_trigger_occurrences.saturating_add(1);
        }
        _ => {}
      }
    }
    let signed_call_count =
      u32::try_from(signed_calls.len()).expect("bounded scripted call count fits");
    #[cfg(not(feature = "runtime-benchmarks"))]
    observe_authored_proof_prefix(
      &execution_recorder,
      &proof_size,
      parent.hash(),
      block_number,
      "inherents",
    );
    let mut next_nonce = signer_nonce;
    for (call_index, (call_signer, call_nonce, call)) in signed_calls.iter().enumerate() {
      let extrinsic =
        actors_integration_tests::signed_extrinsic(call_signer, *call_nonce, (*call).clone());
      let result = Executive::apply_extrinsic(extrinsic.clone())
        .unwrap_or_else(|error| panic!("valid scripted call {call_index} rejected: {error:?}"));
      if let Some(expected) = expected_dispatch {
        assert_eq!(
          result, expected[call_index],
          "scripted dispatch result {call_index}"
        );
      } else {
        result.unwrap_or_else(|error| {
          panic!("valid scripted call {call_index} failed dispatch: {error:?}")
        });
      }
      #[cfg(not(feature = "runtime-benchmarks"))]
      observe_authored_proof_prefix(
        &execution_recorder,
        &proof_size,
        parent.hash(),
        block_number,
        &format!("signed-{call_index}"),
      );
      extrinsics.push(extrinsic);
      if call_signer.public() == signer.public() {
        next_nonce = call_nonce
          .checked_add(1)
          .expect("scripted account nonce stays finite");
      }
    }
    let (demand_calls, next_user_weight, user_admission_refusal) = match demand {
      UserDemand::ActorOnly => (0, None, None),
      UserDemand::ContinuousValid | UserDemand::ContinuousShared => {
        let (calls, next, refusal) = saturate_user_dispatch(
          signer,
          block_number,
          next_nonce,
          &mut extrinsics,
          demand == UserDemand::ContinuousShared,
        );
        (calls, Some(next), refusal)
      }
      UserDemand::RefTimeHeavy => {
        let (calls, next) = saturate_user_dispatch_with_router_swaps(
          signer,
          block_number,
          next_nonce,
          &mut extrinsics,
        );
        (calls, Some(next), None)
      }
    };
    let user_calls = signed_call_count.saturating_add(demand_calls);
    // FRAME's finalize_block only records the extrinsic count and runs the empty
    // PostTransactions hook before computing on_idle's remaining block Weight.
    let pre_idle_remaining = crate::configs::RuntimeBlockWeights::get()
      .max_block
      .saturating_sub(System::block_weight().total());
    #[cfg(not(feature = "runtime-benchmarks"))]
    observe_authored_proof_prefix(
      &execution_recorder,
      &proof_size,
      parent.hash(),
      block_number,
      "before-idle",
    );
    let block = Block {
      header: Executive::finalize_block(),
      extrinsics,
    };
    let metrics = authored_metrics(
      workload_profiles,
      user_calls,
      next_user_weight,
      user_admission_refusal,
      prepass_steps,
      prepass_trigger_occurrences,
      prepass_usage.actor_control_used(),
      prepass_usage.actor_effect_used(),
      pre_idle_remaining,
    );
    (block, metrics)
  });
  // The post-state diagnostic scan must not enter the block's trie-read recording.
  let execution_backend_keys = execution_recorder
    .recorded_keys()
    .into_iter()
    .flat_map(|(root, keys)| {
      keys
        .into_keys()
        .map(move |key| (root, key.as_ref().to_vec()))
    })
    .collect();
  // The overlay records surviving block writes, including same-value writes,
  // but not rolled-back transactions or the number of writes to each key.
  let execution_overlay_keys = ext
    .overlayed_changes()
    .changes()
    .map(|(key, _)| (None, key.clone()))
    .chain(ext.overlayed_changes().children().flat_map(|(keys, info)| {
      keys.map(|(key, _)| (Some(info.storage_key().to_vec()), key.clone()))
    }))
    .collect();
  #[cfg(not(feature = "runtime-benchmarks"))]
  assert_no_auxiliary_block_overlay(&ext);
  let post_state = ext.execute_with(current_top_storage);
  AuthoredBlock {
    pre_state,
    post_state,
    block,
    inherent_data,
    metrics,
    execution_backend_keys,
    execution_overlay_keys,
    #[cfg(not(feature = "runtime-benchmarks"))]
    execution_recorded_proof_bytes: Some(execution_recorder.estimate_encoded_size() as u64),
  }
}

fn author_complete_block_after(
  pre_state: Storage,
  wasm: &[u8],
  parent: &Header,
  block_number: u32,
  demand: UserDemand,
  signer: &sr25519::Pair,
  signer_nonce: crate::Nonce,
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
) -> AuthoredBlock {
  author_complete_block_after_with_calls(
    pre_state,
    wasm,
    parent,
    block_number,
    demand,
    signer,
    signer_nonce,
    workload_profiles,
    &[],
  )
}

fn author_complete_block(
  pre_state: Storage,
  wasm: &[u8],
  block_number: u32,
  demand: UserDemand,
  signer: &sr25519::Pair,
  workload_profiles: &BTreeMap<ActorId, WorkloadActorProfile>,
) -> AuthoredBlock {
  let parent = parent_header_for(pre_state.clone(), wasm, block_number.saturating_sub(1));
  author_complete_block_after(
    pre_state,
    wasm,
    &parent,
    block_number,
    demand,
    signer,
    0,
    workload_profiles,
  )
}

fn assert_runtime_version_equivalent(actual: &polkadot_sdk::sp_version::RuntimeVersion) {
  let expected = crate::VERSION;
  assert_eq!(actual.spec_name, expected.spec_name);
  assert_eq!(actual.impl_name, expected.impl_name);
  assert_eq!(actual.authoring_version, expected.authoring_version);
  assert_eq!(actual.spec_version, expected.spec_version);
  assert_eq!(actual.impl_version, expected.impl_version);
  assert_eq!(actual.transaction_version, expected.transaction_version);
  assert_eq!(actual.system_version, expected.system_version);
  let mut actual_apis = actual.apis.to_vec();
  let mut expected_apis = expected.apis.to_vec();
  actual_apis.sort_unstable();
  expected_apis.sort_unstable();
  assert_eq!(
    actual_apis, expected_apis,
    "runtime API declaration order is not semantic"
  );
}

fn assert_complete_block_replays_natively(authored: &AuthoredBlock, wasm: &[u8]) {
  let mut replay = TestExternalities::new_with_code_and_state(
    wasm,
    authored.pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  let proof_size = RecordingProofSizeProvider::new(recorder.clone());
  replay.register_extension(ProofSizeExt::new(proof_size));
  replay.execute_with_recorder(recorder, || {
    crate::apis::validate_context_inherent_geometry(&authored.inherent_data)
      .expect("full block context geometry is bounded");
    assert!(
      authored
        .inherent_data
        .check_extrinsics(&authored.block.clone().into())
        .ok()
    );
    Executive::execute_block(authored.block.clone().into());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn seal_complete_native_block(block: &Block) -> Block {
  use polkadot_sdk::sp_consensus_aura::{
    digests::CompatibleDigestItem,
    sr25519::{AuthorityPair, AuthoritySignature},
  };
  let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
  assert_eq!(authorities.len(), 2);
  let slot = <DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_pre_digest(
    &block.header.digest().logs[0],
  )
  .unwrap();
  let authority = &authorities[u64::from(slot) as usize % authorities.len()];
  let pair = ["//Alice", "//Bob"]
    .into_iter() // deos-bypass: bounded-iter -- two fixed native test authority seeds
    .map(|seed| AuthorityPair::from_string(seed, None).unwrap())
    .find(|pair| &pair.public() == authority)
    .unwrap();
  assert_eq!(block.header.digest().logs.len(), 2);
  let mut sealed = block.clone();
  sealed.header.digest_mut().push(DigestItem::aura_seal(
    pair.sign(block.header.hash().as_bytes()),
  ));
  assert_eq!(
    sealed.header.encoded_size(),
    block.header.encoded_size() + 71
  );
  sealed
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_complete_block_replays_with_aura(authored: &AuthoredBlock) -> Header {
  use polkadot_sdk::{cumulus_pallet_aura_ext::BlockExecutor, frame_support::traits::ExecuteBlock};
  assert!(
    authored.pre_state.children_default.is_empty(),
    "TOP-only fixtures require no initial child storage"
  );
  let mut replay = TestExternalities::new_with_code_and_state(
    &authored.pre_state.top[well_known_keys::CODE],
    authored.pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let sealed = replay.execute_with(|| seal_complete_native_block(&authored.block));
  let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  replay.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  replay.execute_with_recorder(recorder, || {
    crate::apis::validate_context_inherent_geometry(&authored.inherent_data).unwrap();
    assert!(
      authored
        .inherent_data
        .check_extrinsics(&sealed.clone().into())
        .ok()
    );
    <BlockExecutor<Runtime, Executive> as ExecuteBlock<Block>>::execute_block(
      sealed.clone().into(),
    );
  });
  replay.execute_with(|| assert_eq!(current_top_storage().top, authored.post_state.top));
  assert_no_auxiliary_block_overlay(&replay);
  replay.commit_all().unwrap();
  assert_eq!(replay.backend.root(), sealed.header.state_root());
  let lengths = sealed
    .extrinsics
    .iter() // deos-bypass: bounded-iter -- the selected native fragment has three inherents and at most forty signed calls
    .take(3)
    .map(Encode::encoded_size)
    .collect::<Vec<_>>();
  assert_eq!(lengths.len(), 3);
  let multiplier = replay.execute_with(|| {
    polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get().into_inner()
  });
  println!(
    "Sealed construction fragment: block={} parent={:?} hash={:?} state_root={:?} header_bytes={} inherent_lengths={lengths:?} signed_calls={} next_multiplier={multiplier} actual_aura_verified=true all_top_and_root_equal=true child_offchain_transaction_index_overlay_empty=true",
    sealed.header.number(),
    sealed.header.parent_hash(),
    sealed.header.hash(),
    sealed.header.state_root(),
    sealed.header.encoded_size(),
    sealed.extrinsics.len() - 3
  );
  sealed.header
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_no_auxiliary_block_overlay(ext: &TestExternalities) {
  let overlay = ext.overlayed_changes();
  assert!(
    overlay.children().next().is_none(),
    "TOP-only block fixture cannot discard child storage changes"
  );
  assert!(
    overlay.transaction_index_ops().is_empty(),
    "block fixture cannot discard transaction index changes"
  );
  // Inspect a copy so the diagnostic never consumes the import's offchain changes.
  assert!(
    overlay.clone().offchain_drain_committed().next().is_none(),
    "block fixture cannot discard offchain changes"
  );
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn complete_block_auxiliary_overlay_guard_rejects_unexported_writes() {
  use polkadot_sdk::sp_io::{default_child_storage, offchain_index, transaction_index};
  let writers: [(&str, fn()); 6] = [
    ("child set", || {
      default_child_storage::set(b"auxiliary-guard", b"key", b"value")
    }),
    ("child clear", || {
      default_child_storage::clear(b"auxiliary-guard", b"key")
    }),
    ("offchain set", || offchain_index::set(b"key", b"value")),
    ("offchain clear", || offchain_index::clear(b"key")),
    ("transaction index", || {
      transaction_index::index(0, 1, [1; 32])
    }),
    ("transaction renew", || transaction_index::renew(0, [1; 32])),
  ];
  for (name, write) in writers {
    let mut ext = TestExternalities::new_empty();
    assert_no_auxiliary_block_overlay(&ext);
    let before = ext.execute_with(current_top_storage);
    ext.execute_with(write);
    assert_eq!(
      ext.execute_with(current_top_storage).top,
      before.top,
      "{name} is invisible to the TOP-only observer before root calculation"
    );
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      assert_no_auxiliary_block_overlay(&ext)
    }));
    assert!(
      failure.is_err(),
      "{name} must be rejected before state export or commit"
    );
    assert!(
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_no_auxiliary_block_overlay(&ext)
      }))
      .is_err(),
      "{name} observation must not consume the auxiliary write"
    );
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn externalities_with_bound_code(storage: Storage) -> TestExternalities {
  let code = storage.top[well_known_keys::CODE].clone();
  TestExternalities::new_with_code_and_state(&code, storage, crate::VERSION.state_version())
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_construction_parent_matches_state(ext: &mut TestExternalities, parent: &Header) {
  let root = if ext.overlayed_changes().is_empty() {
    *ext.backend.root()
  } else {
    *ext.as_backend().root()
  };
  assert_eq!(
    root,
    *parent.state_root(),
    "construction parent root must bind the complete pre-state, including code"
  );
  let number = ext.execute_with(System::block_number);
  assert_eq!(
    number,
    *parent.number(),
    "construction parent number must match stored System number"
  );
  if number > 0 {
    if ext.overlayed_changes().is_empty() {
      use polkadot_sdk::{
        frame_support::storage::StoragePrefixedMap,
        sp_state_machine::{Backend, IterArgs},
      };
      let prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
      let mut args = IterArgs::default();
      args.prefix = Some(&prefix);
      let entries = ext.backend.pairs(args).unwrap().map(Result::unwrap);
      assert_system_hash_window_entries(number, entries);
    } else {
      ext.execute_with(assert_canonical_system_hash_window);
    }
  }
  println!(
    "Construction parent binding: parent={number} complete_prestate_root_matched=true stored_number_matched=true canonical_hash_keys_matched={} historical_values_opaque=true",
    number > 0
  );
}

// This witness belongs only to the native author/import harness, never consensus state.
#[cfg(not(feature = "runtime-benchmarks"))]
struct CanonicalHashWindow {
  root: H256,
  number: u32,
  actual: BTreeSet<Vec<u8>>,
  expected: BTreeSet<Vec<u8>>,
}

#[cfg(not(feature = "runtime-benchmarks"))]
impl CanonicalHashWindow {
  fn from_committed(ext: &mut TestExternalities, parent: &Header) -> Self {
    assert!(ext.overlayed_changes().is_empty());
    assert_construction_parent_matches_state(ext, parent);
    let number = *parent.number();
    assert!(number > 0);
    let count = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
    let expected = std::iter::once(0)
      .chain(number.saturating_sub(count).max(1)..number)
      .map(polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for)
      .collect::<BTreeSet<_>>();
    Self {
      root: *parent.state_root(),
      number,
      actual: expected.clone(),
      expected,
    }
  }

  fn assert_parent(&self, ext: &mut TestExternalities, parent: &Header) {
    assert!(ext.overlayed_changes().is_empty());
    assert_eq!(*ext.backend.root(), self.root);
    assert_eq!(*parent.state_root(), self.root);
    assert_eq!(*parent.number(), self.number);
    assert_eq!(ext.execute_with(System::block_number), self.number);
  }

  fn commit_successor(&mut self, ext: &mut TestExternalities, parent: &Header) {
    use codec::MaxEncodedLen;
    use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
    assert_eq!(*ext.backend.root(), self.root);
    let number = self.number.checked_add(1).unwrap();
    assert_eq!(*parent.number(), number);
    assert_eq!(ext.execute_with(System::block_number), number);
    let prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
    let changed = ext
      .overlayed_changes()
      .changes()
      .filter(|(key, _)| key.starts_with(&prefix))
      .map(|(key, _)| key.clone())
      .collect::<Vec<_>>();
    ext.execute_with(|| {
      for key in changed {
        if let Some(value) = polkadot_sdk::sp_io::storage::get(&key) {
          assert_eq!(value.len(), H256::max_encoded_len());
          self.actual.insert(key);
        } else {
          self.actual.remove(&key);
        }
      }
    });
    self
      .expected
      .insert(polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(number - 1));
    let count = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
    let expired = number.saturating_sub(count).saturating_sub(1);
    if expired > 0 {
      self
        .expected
        .remove(&polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(expired));
    }
    assert_eq!(self.actual, self.expected);
    assert_no_auxiliary_block_overlay(ext);
    ext.commit_all().unwrap();
    assert_eq!(*ext.backend.root(), *parent.state_root());
    self.root = *parent.state_root();
    self.number = number;
  }
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn construction_parent_guard_preserves_pending_overlay_and_committed_roots() {
  for preset in crate::genesis_config_presets::preset_names() {
    let code = b"nonempty-code-for-overlay-root-control";
    let storage = preset_genesis_storage(code, &preset);
    let parent = parent_header_for(storage.clone(), code, 0);
    let mut ext = externalities_with_bound_code(storage);
    let committed = *ext.backend.root();
    assert!(ext.overlayed_changes().is_empty());
    assert_eq!(*ext.as_backend().root(), committed);
    assert_construction_parent_matches_state(&mut ext, &parent);
    ext.execute_with(|| {
      polkadot_sdk::sp_io::storage::set(well_known_keys::HEAP_PAGES, &1u64.encode());
    });
    assert!(!ext.overlayed_changes().is_empty());
    let pending = *ext.as_backend().root();
    assert_ne!(pending, committed);
    let mut pending_parent = parent;
    pending_parent.set_state_root(pending);
    assert_construction_parent_matches_state(&mut ext, &pending_parent);
    assert!(!ext.overlayed_changes().is_empty());
    assert_eq!(*ext.backend.root(), committed);
    assert_eq!(*ext.as_backend().root(), pending);
    ext.commit_all().unwrap();
    assert!(ext.overlayed_changes().is_empty());
    assert_eq!(*ext.backend.root(), pending);
    assert_construction_parent_matches_state(&mut ext, &pending_parent);
  }
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn construction_parent_guard_rejects_code_root_and_number_mismatch() {
  for preset in crate::genesis_config_presets::preset_names() {
    let code = b"nonempty-code-for-state-root-control";
    let storage = preset_genesis_storage(code, &preset);
    let parent = parent_header_for(storage.clone(), code, 0);
    let mut clean = externalities_with_bound_code(storage.clone());
    assert_construction_parent_matches_state(&mut clean, &parent);
    assert_eq!(*clean.as_backend().root(), *parent.state_root());
    let empty_code_parent = parent_header_for(storage.clone(), &[], 0);
    let mut wrong_number = parent.clone();
    wrong_number.set_number(1);
    let mut changed_storage = storage.clone();
    changed_storage
      .top
      .insert(well_known_keys::HEAP_PAGES.to_vec(), 1u64.encode());
    let mut sparse_ext = externalities_with_bound_code(storage.clone());
    sparse_ext.execute_with(|| {
      System::set_block_number(2);
      polkadot_sdk::frame_system::BlockHash::<Runtime>::insert(0, H256::zero());
    });
    let sparse_history = sparse_ext.execute_with(current_top_storage);
    let sparse_parent = parent_header_for(sparse_history.clone(), code, 2);
    for (state, header) in [
      (storage.clone(), empty_code_parent),
      (storage, wrong_number),
      (changed_storage, parent),
      (sparse_history, sparse_parent),
    ] {
      let mut ext = externalities_with_bound_code(state);
      let before = *ext.as_backend().root();
      assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          assert_construction_parent_matches_state(&mut ext, &header)
        }))
        .is_err()
      );
      assert_eq!(
        *ext.as_backend().root(),
        before,
        "parent inspection must not change pre-state"
      );
    }
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn export_complete_native_pvf(
  authored: &AuthoredBlock,
  sealed_header: &Header,
  output: Option<(
    &polkadot_sdk::sp_genesis_builder::PresetId,
    &std::path::Path,
  )>,
) {
  let Some((preset, output)) = output else {
    return;
  };
  use codec::Decode;
  let data = authored
    .inherent_data
    .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
    .unwrap()
    .unwrap();
  let mut encoded_parent = data.validation_data.parent_head.0.as_slice();
  let parent = Header::decode(&mut encoded_parent).unwrap();
  assert!(encoded_parent.is_empty());
  assert_eq!(sealed_header.parent_hash(), &parent.hash());
  let directory = output.join(sealed_header.number().to_string());
  if !directory.exists() {
    std::fs::create_dir(&directory).unwrap();
  }
  let mut block = authored.block.clone();
  block.header = sealed_header.clone();
  pvf_fixture::export(
    &directory,
    preset,
    (
      authored.pre_state.clone(),
      authored.post_state.clone(),
      &parent,
      block,
      authored.inherent_data.clone(),
    ),
  )
  .unwrap();
}

#[test]
fn positive_parked_review_reclaims_interior_holes_through_complete_native_blocks() {
  use pallet_deos_actors::{
    ActorContract, AssetOps, DependencyPlans, DependencyRegistrationFreeIndices,
    DependencyRegistrationFreePositions, DependencyRegistrationHeaders,
    DependencyRegistrationPages, DependencyRegistrationPosition, DependencyRegistrationPositions,
    DependencySourceAllocatorState, DependencyTimedReviews, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule, PendingDependencyEvents,
    ProcessStatus, ServiceNodes, ServiceResidenceKind,
  };
  #[cfg(feature = "try-runtime")]
  use polkadot_sdk::frame_support::traits::Hooks;
  use primitives::AssetKind;

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let watches = <Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get();
  assert_eq!(watches, 1);
  let assets = (100..100 + watches).collect::<Vec<_>>();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    // Public Root preparation reuses the existing interior-page geometry. The linked
    // suffix alone is complete native execution, not an authenticated creation history.
    let (target, identity, contract, sovereign, plan, custody, counts, allocator, first_due) = ext
      .execute_with(|| {
        System::set_block_number(1);
        assert!(!Actors::user_creation_enabled());
        for asset in &assets {
          assert_ok!(super::common::create_test_asset(*asset, &owner));
        }
        let steps = (0..<Runtime as pallet_deos_actors::Config>::MaxContractSteps::get())
          .map(|_| StepOf::<Runtime> {
            precondition: None,
            task: Task::StopCycle,
            on_error: StepErrorPolicy::AbortCycle,
          })
          .collect::<Vec<_>>()
          .try_into()
          .unwrap();
        let mut contract: pallet_deos_actors::ActorContractOf<Runtime> = ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: 0,
          window: None,
          steps,
          on_unfunded: OnUnfunded::Close,
          close_when: None,
          parked_balance_activation: None,
          funding: FundingSourcePolicy::RuntimePolicy,
          auto_close_at_cycle_nonce: None,
        };
        contract.parked_balance_activation = Some(
          ParkedBalanceActivationOf::<Runtime>::try_from_rules(
            assets
              .iter()
              .map(|asset| ParkedBalanceRule {
                asset: AssetKind::Local(*asset),
                authored_min_delta: 100,
              })
              .collect(),
          )
          .unwrap(),
        );
        let mut actors = Vec::new();
        for _ in 0..128 {
          let actor = Actors::next_actor_id();
          assert_ok!(Actors::create_system_actor(
            RuntimeOrigin::root(),
            owner.clone(),
            Mutability::Mutable,
            Some(contract.clone()),
          ));
          actors.push(actor);
        }
        let target = actors[32];
        for index in (33..64).chain(64..95).chain(std::iter::once(96)) {
          assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actors[index]));
        }
        let identity = Actors::actor_identity(target).unwrap();
        let sovereign = identity.sovereign_account.clone();
        // Actual reference adapters publish source revisions. The target alone is
        // qualified; the preceding peer stays below its fixed anchor plus threshold.
        let peer = Actors::actor_identity(actors[0]).unwrap().sovereign_account;
        for asset in &assets {
          assert_ok!(crate::configs::actor_config::TmctolAssetOps::mint(
            &peer,
            AssetKind::Local(*asset),
            1,
          ));
        }
        assert_ok!(crate::configs::actor_config::TmctolAssetOps::mint(
          &sovereign,
          AssetKind::Local(assets[0]),
          117,
        ));
        assert_eq!(
          pallet_deos_actors::classify_parked_balance_plan(
            &ParkedBalanceEpisodes::<Runtime>::get(target)
              .unwrap()
              .watches,
            |asset| (
              crate::configs::actor_config::TmctolAssetOps::minimum_balance(asset),
              crate::configs::actor_config::TmctolAssetOps::total_balance(&sovereign, asset),
            ),
          ),
          Ok(pallet_deos_actors::ParkedBalanceQualification::Qualified)
        );
        let plan = DependencyPlans::<Runtime>::get(target);
        assert_eq!(plan.len(), assets.len());
        for entry in &plan {
          assert_eq!(
            DependencyRegistrationPositions::<Runtime>::get(entry.source, target),
            Some(DependencyRegistrationPosition { page: 1, slot: 0 })
          );
          let header = DependencyRegistrationHeaders::<Runtime>::get(entry.source);
          assert_eq!((header.count, header.free_count), (65, 63));
        }
        let first_due = match DependencyTimedReviews::<Runtime>::get(target)
          .unwrap()
          .deadline
        {
          WakeupKey::Block(block) => block,
          WakeupKey::Tick(_) => panic!("ParkedBalance review uses the Block clock"),
        };
        (
          target,
          identity,
          contract,
          sovereign.clone(),
          plan,
          assets
            .iter()
            .map(|asset| Assets::balance(*asset, &sovereign))
            .collect::<Vec<_>>(),
          (Actors::actor_identity_count(), Actors::active_actor_count()),
          DependencySourceAllocatorState::<Runtime>::get(),
          first_due,
        )
      });
    let mut storage = ext.execute_with(current_top_storage);
    let mut parent = parent_header_for(storage.clone(), &[], 1);
    let last_block = first_due.checked_add(65).unwrap();
    let mut woke = false;
    for number in 2..=last_block {
      let authored = author_complete_block_after_with_calls(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        0,
        &BTreeMap::new(),
        &[],
      );
      assert_complete_block_replays_natively(&authored, &[]);
      assert_eq!(authored.metrics.user_dispatch, Weight::zero());
      assert_eq!(authored.metrics.user_calls, 0);
      let result = TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert!(!Actors::user_creation_enabled());
        assert_eq!(Actors::actor_identity_count(), counts.0);
        assert_eq!(Actors::active_actor_count(), counts.1);
        assert_eq!(DependencySourceAllocatorState::<Runtime>::get(), allocator);
        for (asset, expected) in assets.iter().zip(&custody) {
          assert_eq!(Assets::balance(*asset, &sovereign), *expected);
        }
        #[cfg(feature = "try-runtime")]
        <Actors as Hooks<u32>>::try_state(number).unwrap();
        if ParkedBalanceEpisodes::<Runtime>::contains_key(target) {
          return false;
        }
        let process = Actors::actor_processes(target).unwrap();
        assert_eq!(process.status, ProcessStatus::Serving);
        assert_eq!(
          process.residence,
          Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
        );
        assert_eq!(
          ServiceNodes::<Runtime>::get(target).unwrap().eligible_from,
          number.checked_add(1).unwrap()
        );
        assert_eq!(Actors::actor_contract(target), Some(contract.clone()));
        assert!(Actors::actor_run_state(target).is_none());
        assert_eq!(Actors::actor_identity(target), Some(identity.clone()));
        assert!(Actors::pending_signal(target));
        assert!(DependencyPlans::<Runtime>::get(target).is_empty());
        assert!(!DependencyTimedReviews::<Runtime>::contains_key(target));
        for entry in &plan {
          let header = DependencyRegistrationHeaders::<Runtime>::get(entry.source);
          assert_eq!((header.count, header.free_count), (64, 32));
          assert!(!DependencyRegistrationPages::<Runtime>::contains_key(
            entry.source,
            1
          ));
          assert_eq!(
            DependencyRegistrationPages::<Runtime>::get(entry.source, 0)
              .unwrap()
              .next,
            Some(2)
          );
          assert_eq!(
            DependencyRegistrationPages::<Runtime>::get(entry.source, 2)
              .unwrap()
              .previous,
            Some(0)
          );
          for index in 0..32u64 {
            let position =
              DependencyRegistrationFreePositions::<Runtime>::get(entry.source, index).unwrap();
            assert_ne!(position.page, 1);
            assert_eq!(
              DependencyRegistrationFreeIndices::<Runtime>::get(entry.source, position),
              Some(index)
            );
          }
          for slot in 0..32u8 {
            assert!(!DependencyRegistrationFreeIndices::<Runtime>::contains_key(
              entry.source,
              DependencyRegistrationPosition { page: 1, slot },
            ));
          }
        }
        assert!(System::events().iter().any(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id,
            trigger_family: TriggerFamily::ParkedBalance,
            fee: 0,
          }) if actor_id == target
        )));
        assert!(!System::events().iter().any(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::ActorClosed { actor_id, .. }) if actor_id == target
        )));
        true
      });
      if result {
        // Read the path guard in a separate externalities instance so it does not prime
        // the recorder used below. The positive attempt consumes a due review, not an
        // already published source event or a later externally authored activation.
        TestExternalities::new(authored.pre_state.clone()).execute_with(|| {
          assert!(matches!(
            DependencyTimedReviews::<Runtime>::get(target).unwrap().deadline,
            WakeupKey::Block(due) if due <= number
          ));
          assert!(!PendingDependencyEvents::<Runtime>::contains_key(target));
        });
        assert!(authored.metrics.prepass_actor_control.proof_size() > 0);
        // Independently replay the authoring sequence around the mandatory Prepass.
        // Capture its raw increment before finalization or post-state inspection.
        let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
        let mut replay = TestExternalities::new_with_code_and_state(
          &[],
          authored.pre_state.clone(),
          crate::VERSION.state_version(),
        );
        replay.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
          recorder.clone(),
        )));
        let (before_keys, after_keys, prepass_raw_increment) =
          replay.execute_with_recorder(recorder.clone(), || {
            let header = Header::new(
              number,
              Default::default(),
              Default::default(),
              parent.hash(),
              Digest {
                logs: vec![DigestItem::PreRuntime(
                  polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
                  u64::from(number).encode(),
                )],
              },
            );
            Executive::initialize_block(&header);
            let mut before = None;
            let mut after = None;
            let keys = || {
              recorder
                .recorded_keys()
                .into_values()
                .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
                .collect::<BTreeSet<_>>()
            };
            for extrinsic in &authored.block.extrinsics {
              let is_prepass = matches!(
                extrinsic.function,
                RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
              );
              if is_prepass {
                assert!(before.is_none());
                before = Some((keys(), recorder.estimate_encoded_size()));
              }
              Executive::apply_extrinsic(extrinsic.clone())
                .unwrap()
                .unwrap();
              if is_prepass {
                after = Some((keys(), recorder.estimate_encoded_size()));
              }
            }
            assert_eq!(Executive::finalize_block(), authored.block.header);
            let (before_keys, before_bytes) = before.unwrap();
            let (after_keys, after_bytes) = after.unwrap();
            (
              before_keys,
              after_keys,
              after_bytes.checked_sub(before_bytes).unwrap(),
            )
          });
        assert!(prepass_raw_increment > 0);
        for entry in &plan {
          for index in (0..31u64).chain(32..64) {
            let key =
              DependencyRegistrationFreePositions::<Runtime>::hashed_key_for(entry.source, index);
            assert!(!before_keys.contains(&key));
            assert!(after_keys.contains(&key));
          }
          for position in (0..32u8)
            .map(|slot| DependencyRegistrationPosition { page: 1, slot })
            .chain((1..31u8).map(|slot| DependencyRegistrationPosition { page: 2, slot }))
            .chain(std::iter::once(DependencyRegistrationPosition {
              page: 3,
              slot: 0,
            }))
          {
            let key =
              DependencyRegistrationFreeIndices::<Runtime>::hashed_key_for(entry.source, position);
            assert!(!before_keys.contains(&key));
            assert!(after_keys.contains(&key));
          }
        }
        println!(
          "Internal Parked wake: preset={preset} block={number} watches={} prepass_raw_increment={prepass_raw_increment} charged_control={:?}",
          plan.len(),
          authored.metrics.prepass_actor_control
        );
        woke = true;
        break;
      }
      storage = authored.post_state;
      parent = authored.block.header;
    }
    assert!(woke, "the bounded due frontier reaches the prepared target");
  }
}

#[test]
fn cadenced_mint_keeps_parked_scans_active_through_complete_native_blocks() {
  use pallet_deos_actors::{
    ActorContract, AssetOps, BalanceDependencySources, DependencyRegistrationHeaders,
    DependencyRegistrationPosition, DependencyRegistrationPositions, DependencyRevisions,
    DependencyScanSourceListState, DependencyTimedReviews, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule,
  };
  #[cfg(feature = "try-runtime")]
  use polkadot_sdk::frame_support::traits::Hooks;
  use primitives::AssetKind;

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let watches = <Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get();
  assert_eq!(watches, 1);
  let assets = (100..100 + watches).collect::<Vec<_>>();
  let step = |task| StepOf::<Runtime> {
    precondition: None,
    task,
    on_error: StepErrorPolicy::AbortCycle,
  };
  let contract = |trigger, steps| ActorContract {
    trigger,
    cooldown_blocks: 0,
    window: None,
    steps,
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: None,
    funding: FundingSourcePolicy::RuntimePolicy,
    auto_close_at_cycle_nonce: None,
  };

  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    // Public Root calls prepare the prefix. Only the linked suffix below consists of
    // complete consensus blocks; preparation is not an authenticated creation history.
    let (parked, producers, sources, initial_episode) = ext.execute_with(|| {
      System::set_block_number(1);
      assert!(!Actors::user_creation_enabled());
      assert_eq!(
        Actors::actor_identity_count(),
        REFERENCE_SYSTEM_ACTOR_IDENTITIES
      );
      for asset in &assets {
        assert_ok!(super::common::create_test_asset(*asset, &owner));
      }
      let parked = Actors::next_actor_id();
      let mut parked_contract = contract(
        Trigger::manual(),
        (0..<Runtime as pallet_deos_actors::Config>::MaxContractSteps::get())
          .map(|_| step(Task::StopCycle))
          .collect::<Vec<_>>()
          .try_into()
          .unwrap(),
      );
      parked_contract.parked_balance_activation = Some(
        ParkedBalanceActivationOf::<Runtime>::try_from_rules(
          assets
            .iter()
            .map(|asset| ParkedBalanceRule {
              asset: AssetKind::Local(*asset),
              authored_min_delta: 100,
            })
            .collect(),
        )
        .unwrap(),
      );
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        owner.clone(),
        Mutability::Mutable,
        Some(parked_contract),
      ));
      let initial_episode = ParkedBalanceEpisodes::<Runtime>::get(parked).unwrap();
      let sources = assets
        .iter()
        .map(|asset| BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)).unwrap())
        .collect::<Vec<_>>();
      let producers = assets
        .iter()
        .map(|asset| {
          let actor = Actors::next_actor_id();
          assert_ok!(Actors::create_system_actor(
            RuntimeOrigin::root(),
            owner.clone(),
            Mutability::Mutable,
            Some(contract(
              Trigger::cadenced(1),
              vec![step(Task::Mint {
                asset: AssetKind::Local(*asset),
                amount: AmountResolution::Fixed(1),
              })]
              .try_into()
              .unwrap(),
            )),
          ));
          actor
        })
        .collect::<Vec<_>>();
      // Keep the fixed forest, all genesis Actors and the configured producers in scope.
      let construction_slots = 9_968 + producers.len() as u32 + 1;
      assert!(
        REFERENCE_SYSTEM_ACTOR_IDENTITIES + construction_slots
          <= <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get()
      );
      assert!(
        REFERENCE_SYSTEM_ACTOR_IDENTITIES + construction_slots
          <= <Runtime as pallet_deos_actors::Config>::MaxSystemSovereigns::get()
      );
      assert!(
        REFERENCE_ACTIVE_SYSTEM_ACTORS + construction_slots
          <= pallet_deos_actors::ActiveActorLimit::<Runtime>::get()
      );
      (parked, producers, sources, initial_episode)
    });
    let mut storage = ext.execute_with(current_top_storage);
    let mut parent = parent_header_for(storage.clone(), &[], 1);
    let mut previous = vec![pallet_deos_actors::DependencyRevisionState::default(); sources.len()];
    let mut handoffs = vec![0u32; sources.len()];
    let mut last_publication = vec![None; sources.len()];
    let mut maximum_gap = vec![0u32; sources.len()];
    let mut mint_counts = vec![0u64; sources.len()];

    for block in 2..=160 {
      let authored = author_complete_block_after(
        storage,
        &[],
        &parent,
        block,
        UserDemand::ActorOnly,
        &signer,
        0,
        &BTreeMap::new(),
      );
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert!(!Actors::user_creation_enabled());
        let episode = ParkedBalanceEpisodes::<Runtime>::get(parked).unwrap();
        assert_eq!(episode.owner, initial_episode.owner);
        assert_eq!(
          DependencyTimedReviews::<Runtime>::get(parked)
            .unwrap()
            .deadline,
          WakeupKey::Block(block + 1),
          "mandatory negative review re-arms every block"
        );
        assert!(matches!(
          Actors::actor_processes(parked).unwrap().residence,
          Some(ProcessResidence::Parked(_))
        ));
        assert!(
          !Actors::active_actor_state(parked)
            .unwrap()
            .hot
            .pending_signal
        );
        assert!(
          !Actors::finalized_block_resource_telemetry()
            .unwrap()
            .optional_actor_work_halted()
        );
        for event in System::events() {
          if let RuntimeEvent::Actors(Event::MintExecuted { actor_id, .. }) = event.event
            && let Some(index) = producers.iter().position(|actor| *actor == actor_id)
          {
            mint_counts[index] += 1;
            if let Some(last) = last_publication[index] {
              maximum_gap[index] = maximum_gap[index].max(block - last);
            }
            last_publication[index] = Some(block);
          }
        }
        for (index, source) in sources.iter().enumerate() {
          let watch = &episode.watches[index];
          let initial = &initial_episode.watches[index];
          assert_eq!(
            (
              watch.asset,
              watch.anchor,
              watch.authored_min_delta,
              watch.certified_minimum_balance
            ),
            (
              initial.asset,
              initial.anchor,
              initial.authored_min_delta,
              initial.certified_minimum_balance
            )
          );
          let custody = Actors::actor_identity(parked).unwrap().sovereign_account;
          assert_eq!(
            crate::configs::actor_config::TmctolAssetOps::total_balance(&custody, watch.asset),
            0
          );
          assert_eq!(
            DependencyRegistrationPositions::<Runtime>::get(source, parked),
            Some(DependencyRegistrationPosition { page: 0, slot: 0 })
          );
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          assert_eq!(
            (header.count, header.free_count, header.next_index),
            (1, 0, 1)
          );
          let state = DependencyRevisions::<Runtime>::get(source);
          assert_eq!(
            state.revision, mint_counts[index],
            "only committed Task Mint publishes this source"
          );
          if let (Some(before), Some(after)) = (previous[index].scan_target, state.scan_target)
            && after > before
          {
            handoffs[index] += 1;
          }
          if block >= 65 {
            assert!(
              state.scan_target.is_some(),
              "preset={preset}, block={block}, source={source}"
            );
          }
          previous[index] = state;
        }
        if block >= 65 {
          assert_eq!(
            DependencyScanSourceListState::<Runtime>::get().count,
            watches
          );
        }
        #[cfg(feature = "try-runtime")]
        <Actors as Hooks<u32>>::try_state(block).unwrap();
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
    assert!(
      handoffs.iter().all(|count| *count > 0),
      "preset={preset}, handoffs={handoffs:?}"
    );
    assert!(mint_counts.iter().all(|count| *count >= 2));
    assert!(
      maximum_gap.iter().all(|gap| *gap < 24),
      "preset={preset}, gaps={maximum_gap:?}"
    );

    let nonce =
      TestExternalities::new(storage.clone()).execute_with(|| System::account_nonce(&owner));
    let close = RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: parked });
    let authored = author_complete_block_after_with_calls(
      storage,
      &[],
      &parent,
      161,
      UserDemand::ActorOnly,
      &signer,
      nonce,
      &BTreeMap::new(),
      &[close],
    );
    assert_complete_block_replays_natively(&authored, &[]);
    TestExternalities::new(authored.post_state).execute_with(|| {
      assert!(Actors::actor_identity(parked).is_none());
      assert_eq!(DependencyScanSourceListState::<Runtime>::get().count, 0);
      for asset in &assets {
        assert!(!BalanceDependencySources::<Runtime>::contains_key(
          AssetKind::Local(*asset)
        ));
      }
      assert!(!Actors::user_creation_enabled());
      #[cfg(feature = "try-runtime")]
      <Actors as Hooks<u32>>::try_state(161).unwrap();
    });
  }
}

/// Complete native Governance chronology from the runtime-owned genesis, with fresh
/// proof accounting and independent import for every block. Persistent backends avoid
/// enumerating and reconstructing the whole state at every empty epoch.
#[cfg(not(feature = "runtime-benchmarks"))]
pub(super) fn signed_l2_admission_from_genesis(
  signer: &sr25519::Pair,
  preset: &polkadot_sdk::sp_genesis_builder::PresetId,
  domain: u32,
) -> (Storage, Header) {
  replay_signed_l2_admission_from_genesis(
    signer,
    preset,
    domain,
    SignedAdmissionReplayEnd::Enactment,
  )
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SignedAdmissionReplayEnd {
  Preparation,
  SealedPreparation,
  Enactment,
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn replay_signed_l2_admission_from_genesis(
  signer: &sr25519::Pair,
  preset: &polkadot_sdk::sp_genesis_builder::PresetId,
  domain: u32,
  end: SignedAdmissionReplayEnd,
) -> (Storage, Header) {
  replay_signed_l2_admission_with_pvf(signer, preset, domain, end, &[], None)
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn replay_signed_l2_admission_with_pvf(
  signer: &sr25519::Pair,
  preset: &polkadot_sdk::sp_genesis_builder::PresetId,
  domain: u32,
  end: SignedAdmissionReplayEnd,
  wasm: &[u8],
  pvf_output: Option<&std::path::Path>,
) -> (Storage, Header) {
  use pallet_governance::{
    ProposalCadenceMode, ProposalEnactmentOutcome, ProposalPayloadKind, ProposalVoteKind,
  };
  assert!(pvf_output.is_none() || end == SignedAdmissionReplayEnd::SealedPreparation);
  let storage = preset_genesis_storage(wasm, preset);
  assert!(
    storage.children_default.is_empty(),
    "TOP-only fixtures require no initial child storage"
  );
  let mut parent = parent_header_for(storage.clone(), wasm, 0);
  let mut author = TestExternalities::new_with_code_and_state(
    wasm,
    storage.clone(),
    crate::VERSION.state_version(),
  );
  let mut importer =
    TestExternalities::new_with_code_and_state(wasm, storage, crate::VERSION.state_version());
  let owner = crate::AccountId::from(signer.public());
  let payload =
    RuntimeCall::Actors(pallet_deos_actors::Call::set_user_creation_enabled { enabled: true })
      .encode();
  let payload_hash = BlakeTwo256::hash(&payload);
  let prepare = [
    RuntimeCall::Staking(pallet_staking::Call::stake {
      asset_id: 0,
      amount: 1_000,
    }),
    RuntimeCall::Preimage(polkadot_sdk::pallet_preimage::Call::note_preimage {
      bytes: payload.clone(),
    }),
    RuntimeCall::Governance(pallet_governance::Call::prepare_payload_admission_witness {
      domain,
      payload_kind: ProposalPayloadKind::L2ParameterChange,
      payload_hash,
      payload: payload.try_into().unwrap(),
    }),
    RuntimeCall::Governance(pallet_governance::Call::submit_signed_proposal {
      domain,
      item_id: 120,
      cadence_mode: ProposalCadenceMode::Ordinary,
      payload_kind: ProposalPayloadKind::L2ParameterChange,
      payload_hash,
    }),
  ];
  let vote = RuntimeCall::Governance(pallet_governance::Call::cast_vote {
    domain,
    item_id: 120,
    vote: ProposalVoteKind::Aye,
  });
  let mut timing = None;
  let mut due = None;
  let mut nonce = 0;
  let mut number = 0u32;
  let mut progress = std::env::var_os("DEOS_REPLAY_PROGRESS")
    .map(|_| (std::time::Instant::now(), [std::time::Duration::ZERO; 4]));
  let mut hash_window: Option<CanonicalHashWindow> = None;
  loop {
    let mut phase_started = progress.as_ref().map(|_| std::time::Instant::now());
    let mut measure_phase = |index: usize| {
      if let (Some((_, elapsed)), Some(previous)) = (&mut progress, &mut phase_started) {
        let now = std::time::Instant::now();
        elapsed[index] += now.duration_since(*previous);
        *previous = now;
      }
    };
    number = number.checked_add(1).unwrap();
    let call = if number <= prepare.len() as u32 {
      Some(&prepare[number as usize - 1])
    } else if timing
      .as_ref()
      .is_some_and(|timing: &pallet_governance::ProposalTiming<u32>| {
        number == timing.ordinary_primary_open_epoch
      })
    {
      Some(&vote)
    } else {
      None
    };
    if let Some(window) = &hash_window {
      window.assert_parent(&mut author, &parent);
    } else {
      assert_construction_parent_matches_state(&mut author, &parent);
    }
    measure_phase(0);
    let data = inherent_data_for(&parent, number);
    let pvf_pre_state = pvf_output.map(|_| author.execute_with(current_top_storage));
    let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    let proof_size = RecordingProofSizeProvider::new(recorder.clone());
    author.register_extension(ProofSizeExt::new(proof_size.clone()));
    let block = author.execute_with_recorder(recorder.clone(), || {
      let header = Header::new(
        number,
        Default::default(),
        Default::default(),
        parent.hash(),
        Digest {
          logs: vec![DigestItem::PreRuntime(
            polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
            u64::from(number).encode(),
          )],
        },
      );
      Executive::initialize_block(&header);
      let mut extrinsics = data.create_extrinsics();
      assert!(extrinsics.len() >= 3);
      for extrinsic in &extrinsics {
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
      }
      observe_authored_proof_prefix(&recorder, &proof_size, parent.hash(), number, "inherents");
      if let Some(call) = call {
        let extrinsic = actors_integration_tests::signed_extrinsic(signer, nonce, call.clone());
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
        extrinsics.push(extrinsic);
        nonce += 1;
        observe_authored_proof_prefix(&recorder, &proof_size, parent.hash(), number, "signed-0");
      }
      observe_authored_proof_prefix(&recorder, &proof_size, parent.hash(), number, "before-idle");
      Block {
        header: Executive::finalize_block(),
        extrinsics,
      }
    });
    assert!(recorder.estimate_encoded_size() > 0);
    assert_no_auxiliary_block_overlay(&author);
    if let Some(window) = &mut hash_window {
      window.commit_successor(&mut author, &block.header);
    } else {
      author.commit_all().unwrap();
      hash_window = Some(CanonicalHashWindow::from_committed(
        &mut author,
        &block.header,
      ));
    }
    measure_phase(1);
    let block = if end == SignedAdmissionReplayEnd::SealedPreparation {
      importer.execute_with(|| seal_complete_native_block(&block))
    } else {
      block
    };
    let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    importer.execute_with_recorder(recorder.clone(), || {
      crate::apis::validate_context_inherent_geometry(&data).unwrap();
      assert!(data.check_extrinsics(&block.clone().into()).ok());
      if end == SignedAdmissionReplayEnd::SealedPreparation {
        use polkadot_sdk::{
          cumulus_pallet_aura_ext::BlockExecutor, frame_support::traits::ExecuteBlock,
        };
        <BlockExecutor<Runtime, Executive> as ExecuteBlock<Block>>::execute_block(
          block.clone().into(),
        );
      } else {
        Executive::execute_block(block.clone().into());
      }
    });
    assert!(recorder.estimate_encoded_size() > 0);
    assert_no_auxiliary_block_overlay(&importer);
    importer.commit_all().unwrap();
    assert_eq!(author.backend.root(), importer.backend.root());
    assert_eq!(importer.backend.root(), block.header.state_root());
    measure_phase(2);
    if let Some(output) = pvf_output {
      let block_output = output.join(number.to_string());
      if !block_output.exists() {
        std::fs::create_dir(&block_output).unwrap();
      }
      pvf_fixture::export(
        &block_output,
        preset,
        (
          pvf_pre_state.unwrap(),
          importer.execute_with(current_top_storage),
          &parent,
          block.clone(),
          data.clone(),
        ),
      )
      .unwrap();
    }
    if end == SignedAdmissionReplayEnd::SealedPreparation {
      importer.execute_with(|| {
        assert_eq!(
          polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::get()
            .unwrap()
            .parent_head
            .0,
          parent.encode()
        );
        assert_eq!(System::parent_hash(), parent.hash());
        assert_eq!(System::block_hash(number - 1), parent.hash());
      });
      println!(
        "Sealed governance preparation: preset={preset} block={number} parent={:?} hash={:?} state_root={:?} actual_aura_verified=true author_import_root_equal=true child_offchain_transaction_index_overlay_empty=true",
        parent.hash(),
        block.header.hash(),
        block.header.state_root()
      );
    }
    // The test database retains dead trie nodes after commit. Bound that auxiliary
    // history without changing live storage, proof accounting or consensus execution.
    if number.is_multiple_of(256) {
      author.backend.backend_storage_mut().purge();
      importer.backend.backend_storage_mut().purge();
      assert_eq!(author.backend.root(), block.header.state_root());
      assert_eq!(importer.backend.root(), block.header.state_root());
    }
    importer.execute_with(|| {
      assert_eq!(System::block_number(), number);
      assert_eq!(System::account_nonce(&owner), nonce);
      assert!(!Actors::global_circuit_breaker());
      assert_eq!(Actors::user_creation_enabled(), due == Some(number));
      if number == prepare.len() as u32 {
        assert_eq!(
          crate::Governance::proposal_author(domain, 120),
          Some(owner.clone())
        );
        let proposal_timing = crate::Governance::proposal_timing(domain, 120).unwrap();
        let delay = crate::configs::governance_config::ProposalEnactmentDelay::get();
        assert_eq!(delay, 43_200);
        due = Some(
          proposal_timing
            .ordinary_primary_close_epoch
            .checked_add(delay)
            .unwrap(),
        );
        timing = Some(proposal_timing);
      }
      if let Some(timing) = &timing {
        if number == timing.ordinary_primary_open_epoch {
          assert!(
            crate::Governance::proposal_vote_tally(domain, 120)
              .unwrap()
              .aye_weight
              >= 200
          );
        }
        if number >= timing.ordinary_primary_close_epoch && Some(number) != due {
          assert_eq!(
            crate::Governance::finalized_proposal_outcome(domain, 120),
            Some(super::governance_integration_tests::approved_outcome(
              timing.ordinary_primary_close_epoch,
              1,
              ProposalEnactmentOutcome::NotAttempted
            ))
          );
          assert_eq!(
            pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(domain, 120),
            due
          );
          assert!(crate::Governance::finalized_proposal_outcome_expiry_bucket(number).is_empty());
        }
        if Some(number) == due {
          assert_eq!(
            crate::Governance::finalized_proposal_outcome(domain, 120),
            Some(super::governance_integration_tests::approved_outcome(
              timing.ordinary_primary_close_epoch,
              1,
              ProposalEnactmentOutcome::Enacted { epoch: number }
            ))
          );
          assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
          assert!(System::events().iter().any(|record| matches!(record.event,
            RuntimeEvent::Governance(pallet_governance::Event::ProposalParameterChangeExecuted {
              domain: event_domain, item_id: 120,
              surface: pallet_governance::ProposalParameterChangeSurface::ActorUserCreationEnabled,
              executed_epoch, ..
            }) if event_domain == domain && executed_epoch == number)));
        }
      }
      #[cfg(feature = "try-runtime")]
      <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
    });
    parent = block.header;
    if matches!(
      end,
      SignedAdmissionReplayEnd::Preparation | SignedAdmissionReplayEnd::SealedPreparation
    ) {
      importer.execute_with(|| {
        use polkadot_sdk::frame_support::dispatch::DispatchClass;
        let normal = *System::block_weight().get(DispatchClass::Normal);
        let limit = crate::configs::RuntimeBlockWeights::get()
          .get(DispatchClass::Normal)
          .max_total
          .unwrap();
        let target = Weight::from_parts(limit.ref_time() / 4, limit.proof_size() / 4);
        assert!(normal.all_lte(target));
        let multiplier =
          polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get();
        assert!(multiplier <= polkadot_sdk::pallet_transaction_payment::Multiplier::from_u32(1));
        let sink = Actors::sovereign_account_id_system(
          primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
        );
        let sink_total = Balances::free_balance(&sink);
        let pot_total = Balances::free_balance(crate::CollatorSelection::account_id());
        assert_eq!(pot_total, 0);
        println!(
          "Signed governance preparation meter: preset={preset} block={number} normal_ref={} normal_proof={} next_multiplier={} sink_total={sink_total} pot_total={pot_total}",
          normal.ref_time(),
          normal.proof_size(),
          multiplier.into_inner()
        );
      });
    }
    measure_phase(3);
    if let Some((started, elapsed)) = &progress
      && (number == prepare.len() as u32 || number.is_multiple_of(1_000) || due == Some(number))
    {
      use std::io::Write;
      // Direct stderr keeps sparse diagnostics visible under libtest capture.
      writeln!(
        std::io::stderr().lock(),
        "signed replay progress: preset={preset} block={number} enactment={due:?} wall_ms={} guard_ms={} author_ms={} import_ms={} checks_ms={}",
        started.elapsed().as_millis(),
        elapsed[0].as_millis(),
        elapsed[1].as_millis(),
        elapsed[2].as_millis(),
        elapsed[3].as_millis(),
      )
      .unwrap();
    }
    if matches!(
      end,
      SignedAdmissionReplayEnd::Preparation | SignedAdmissionReplayEnd::SealedPreparation
    ) && number == prepare.len() as u32
    {
      assert_eq!(nonce, 4);
      assert_eq!(due, Some(187_204));
      break;
    }
    if due == Some(number) {
      assert_eq!(nonce, 5);
      break;
    }
    if number.is_multiple_of(10_000) {
      eprintln!("signed genesis chronology: {preset} block {number}, enactment {due:?}");
    }
  }
  let storage = importer.execute_with(current_top_storage);
  (storage, parent)
}

/// Export the same four signed preparation blocks with code bound at named genesis.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires explicit diagnostic Wasm and a new PVF output directory"]
fn signed_l2_admission_preparation_exports_pvf_fixtures() {
  let wasm_path = std::env::var("DEOS_GOVERNANCE_PVF_WASM").expect("explicit Wasm path");
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_GOVERNANCE_PVF_OUTPUT").expect("new output directory"),
  );
  let wasm = std::fs::read(wasm_path).unwrap();
  assert!(!wasm.is_empty());
  std::fs::create_dir(&output).expect("output directory must be new");
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    replay_signed_l2_admission_with_pvf(
      &signer,
      &preset,
      0,
      SignedAdmissionReplayEnd::SealedPreparation,
      &wasm,
      Some(&output),
    );
  }
}

/// The public preparation prefix is finite and shares the exact full-chronology body.
/// Later Governance intervals and whole-runtime endpoint equivalence remain separate.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_admission_preparation_imports_four_blocks_on_declared_presets() {
  use polkadot_sdk::frame_support::traits::{
    fungible::Inspect,
    tokens::{Fortitude, Preservation},
  };
  use polkadot_sdk::pallet_asset_conversion::PoolLocator;
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let domain = 0;
  for preset in crate::genesis_config_presets::preset_names() {
    let (native_before, local_before, receipt_before, next_actor, active, sovereigns, donors) =
      TestExternalities::new(preset_genesis_storage(&[], &preset)).execute_with(|| {
        assert_eq!(
          Balances::free_balance(crate::CollatorSelection::account_id()),
          0
        );
        let donors = polkadot_sdk::sp_keyring::Sr25519Keyring::well_known()
          .map(|key| crate::AccountId::from(key.public()))
          .filter(|account| account != &owner)
          .map(|account| {
            let balance = Balances::free_balance(&account);
            (account, balance)
          })
          .collect::<BTreeMap<_, _>>();
        (
          Balances::free_balance(&owner),
          Assets::balance(0, &owner),
          crate::Staking::live_native_staked_receipt_balance(&owner).unwrap(),
          Actors::next_actor_id(),
          Actors::active_actor_count(),
          Actors::system_sovereign_count(),
          donors,
        )
      });
    let (storage, parent) = replay_signed_l2_admission_from_genesis(
      &signer,
      &preset,
      domain,
      SignedAdmissionReplayEnd::Preparation,
    );
    assert_eq!(*parent.number(), 4);
    TestExternalities::new(storage).execute_with(|| {
      assert_eq!(System::block_number(), 4);
      assert_eq!(System::account_nonce(&owner), 4);
      assert!(!Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      assert_eq!(Actors::next_actor_id(), next_actor);
      assert_eq!(Actors::active_actor_count(), active);
      assert_eq!(Actors::system_sovereign_count(), sovereigns);
      assert!(Actors::block_resource_state().is_none());
      assert!(pallet_deos_actors::BalanceDependencySources::<Runtime>::iter().next().is_none());
      assert!(polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter().next().is_none());
      assert!(polkadot_sdk::pallet_assets::NextAssetId::<Runtime>::get().is_none());
      // Authenticate the future pool inputs without modifying this public prefix.
      let pool_input_state = current_top_storage();
      let lp_start = primitives::assets::TYPE_LP | 1;
      assert_eq!(polkadot_sdk::pallet_asset_conversion::NextPoolAssetId::<Runtime>::get(), Some(lp_start));
      assert!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().is_empty());
      assert!(pallet_oracle::FeedIds::<Runtime>::get().is_empty());
      let creators = polkadot_sdk::sp_keyring::Sr25519Keyring::well_known()
        .map(|key| key.to_account_id())
        .chain((0..28).map(|index| {
          crate::AccountId::from(polkadot_sdk::sp_keyring::Sr25519Keyring::numeric(index).public())
        }))
        .collect::<BTreeSet<_>>();
      assert_eq!(creators.len(), 40);
      let mut protected = creators.clone();
      for creator in &creators {
        for slot in 0..<Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get() {
          protected.insert(Actors::sovereign_account_id(creator, u8::try_from(slot).unwrap()));
        }
      }
      protected.extend(crate::configs::actor_config::TmctolGenesisSystemActors::native_flow_anchor_accounts());
      protected.extend((0..=primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID)
        .map(Actors::sovereign_account_id_system));
      protected.insert(crate::CollatorSelection::account_id());
      protected.insert(crate::configs::governance_config::governance_vote_power_custody_account());
      let mut pool_accounts = BTreeSet::new();
      let mut pairs = Vec::new();
      for index in 0..4u32 {
        let paired = primitives::AssetKind::Local(if index == 0 { 0 } else { lp_start + index - 1 });
        let pair = (primitives::AssetKind::Native, paired);
        let pool_account = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pair).unwrap();
        assert!(!protected.contains(&pool_account));
        assert!(pool_accounts.insert(pool_account.clone()));
        assert!(!polkadot_sdk::frame_system::Account::<Runtime>::contains_key(&pool_account));
        assert_eq!(Balances::free_balance(&pool_account), 0);
        for id in std::iter::once(0).chain(lp_start..lp_start + 4) {
          assert!(!polkadot_sdk::pallet_assets::Account::<Runtime>::contains_key(id, &pool_account));
          assert_eq!(Assets::balance(id, &pool_account), 0);
        }
        assert!(!polkadot_sdk::pallet_assets::Asset::<Runtime>::contains_key(lp_start + index));
        assert_eq!(Assets::total_supply(lp_start + index), 0);
        assert_ok!(crate::configs::assets_config::preflight_register_pool_lp_pair(pair.0, pair.1, lp_start + index));
        let address = pool_account.encode().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        println!("Signed governance initial pool: preset={preset} ordinal={} lp_id={} address={address} native=0 paired=0 lp_supply=0 preflight=true", index + 1, lp_start + index);
        pairs.push((pair, pool_account));
      }
      let (owner_lp, native_debit, reserved_debit, feed_count) = TestExternalities::new(pool_input_state.clone()).execute_with(|| {
        let before_free = Balances::free_balance(&owner);
        let before_reserved = Balances::reserved_balance(&owner);
        for (index, (pair, pool_account)) in pairs.iter().enumerate() {
          assert_ok!(crate::DeosRouter::create_pool(RuntimeOrigin::signed(owner.clone()), pair.0, pair.1));
          let lp = lp_start + u32::try_from(index).unwrap();
          assert_eq!(polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair).unwrap().lp_token, lp);
          assert_ok!(crate::AssetConversion::add_liquidity(
            RuntimeOrigin::signed(owner.clone()), Box::new(pair.0), Box::new(pair.1),
            crate::UNIT, crate::UNIT / 4, 1, 1, owner.clone(),
          ));
          assert_eq!(Balances::free_balance(pool_account), crate::UNIT);
          let paired_id = if index == 0 { 0 } else { lp - 1 };
          assert_eq!(Assets::balance(paired_id, pool_account), crate::UNIT / 4);
          assert_eq!(Assets::total_supply(lp), crate::UNIT / 2);
          assert_eq!(Assets::balance(lp, pool_account), 100);
          assert_eq!(Assets::balance(lp, &owner), crate::UNIT / 2 - 100);
          assert_eq!(crate::DeosRouter::lp_pair_by_token_id(lp), Some(*pair));
        }
        let owner_lp = (lp_start..lp_start + 4).map(|lp| Assets::balance(lp, &owner)).collect::<Vec<_>>();
        assert_eq!(owner_lp, vec![249_999_999_900, 249_999_999_900, 249_999_999_900, 499_999_999_900]);
        let reserved_debit = Balances::reserved_balance(&owner) - before_reserved;
        let native_debit = before_free - Balances::free_balance(&owner);
        assert_eq!(reserved_debit, 7 * crate::EXISTENTIAL_DEPOSIT);
        assert_eq!(native_debit, 4 * crate::UNIT + reserved_debit);
        let feed_count = pallet_oracle::FeedIds::<Runtime>::get().len();
        assert_eq!(feed_count, 8);
        assert_eq!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().len(), 4);
        assert_eq!(polkadot_sdk::pallet_asset_conversion::NextPoolAssetId::<Runtime>::get(), Some(lp_start + 4));
        assert!(polkadot_sdk::pallet_assets::NextAssetId::<Runtime>::get().is_none());
        assert!(!Actors::user_creation_enabled());
        (owner_lp, native_debit, reserved_debit, feed_count)
      });
      assert_eq!(current_top_storage().top, pool_input_state.top);
      println!("Signed governance pool clone: preset={preset} pools=4 owner_lp={owner_lp:?} native_free_debit={native_debit} reserved_debit={reserved_debit} feeds={feed_count} outer_state_unchanged=true; component calls without signed fees or complete pool blocks");
      for (account, balance) in &donors {
        assert_eq!(Balances::free_balance(account), *balance);
      }
      let native_after = Balances::free_balance(&owner);
      let native_available =
        Balances::reducible_balance(&owner, Preservation::Preserve, Fortitude::Polite);
      let local_after = Assets::balance(0, &owner);
      let receipt_after = crate::Staking::live_native_staked_receipt_balance(&owner).unwrap();
      assert!(native_after < native_before);
      assert!(native_available >= 30_000 * crate::UNIT);
      assert_eq!(local_after, local_before - 1_000);
      assert!(local_after >= crate::UNIT / 4);
      assert_eq!(receipt_after, receipt_before + 1_000);
      let timing = crate::Governance::proposal_timing(domain, 120).unwrap();
      assert_eq!(timing.ordinary_primary_close_epoch, 144_004);
      let quote_storage = current_top_storage();
      let (vote_bytes, vote_ref, vote_proof, vote_fee_upper) =
        TestExternalities::new(quote_storage.clone()).execute_with(|| {
          let vote = RuntimeCall::Governance(pallet_governance::Call::cast_vote {
            domain,
            item_id: 120,
            vote: pallet_governance::ProposalVoteKind::Aye,
          });
          // The signer helper prepares only this disposable clone; no vote is dispatched.
          let extrinsic = actors_integration_tests::signed_extrinsic(&signer, 4, vote);
          let info = extrinsic.get_dispatch_info();
          let length = u32::try_from(extrinsic.encode().len()).unwrap();
          let base = crate::configs::RuntimeBlockWeights::get()
            .get(info.class)
            .base_extrinsic;
          let fee = crate::TransactionPayment::weight_to_fee(base)
            + crate::TransactionPayment::length_to_fee(length)
            + crate::TransactionPayment::weight_to_fee(info.total_weight());
          (length, info.total_weight().ref_time(), info.total_weight().proof_size(), fee)
        });
      assert_eq!(current_top_storage().top, quote_storage.top);
      let sink = Actors::sovereign_account_id_system(
        primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
      );
      let sink_total = Balances::free_balance(&sink);
      let sink_steps =
        crate::configs::actor_config::TmctolGenesisSystemActors::build_fee_sink_contract_steps();
      let condition = sink_steps[0].precondition.as_ref().unwrap();
      assert_eq!(condition.clauses.len(), 1);
      assert_eq!(condition.clauses[0].len(), 1);
      let minimum_visible = match &condition.clauses[0][0] {
        pallet_deos_actors::Predicate::BalanceAbove {
          asset: primitives::AssetKind::Native,
          threshold,
        } => threshold + 1,
        other => panic!("unexpected Fee Sink readiness guard: {other:?}"),
      };
      let prefix_collection_upper = crate::EXISTENTIAL_DEPOSIT
        + native_before
        - native_after
        + vote_fee_upper;
      assert!(sink_total <= crate::EXISTENTIAL_DEPOSIT + native_before - native_after);
      assert!(prefix_collection_upper < minimum_visible);
      println!(
        "Signed governance prefix fee guard: preset={preset} sink_total={sink_total} vote_bytes={vote_bytes} vote_ref={vote_ref} vote_proof={vote_proof} vote_fee_upper={vote_fee_upper} prefix_collection_upper={prefix_collection_upper} minimum_visible={minimum_visible} margin={}",
        minimum_visible - prefix_collection_upper
      );
      println!(
        "Signed governance preparation: preset={preset} complete_blocks=4 nonce=4 native_before={native_before} native_after={native_after} native_available={native_available} local_before={local_before} local_after={local_after} receipt_before={receipt_before} receipt_after={receipt_after} next_actor_id={next_actor} active_systems={active} unchanged_donors={} next_asset_id_absent=true proposal_close=144004 enactment=187204",
        donors.len()
      );
    });
  }
}

/// The public pool prefix runs before User birth without changing admission or clocks.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn public_signed_parked_pool_prefix_imports_thirty_blocks_after_preparation() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let (storage, parent) = replay_signed_l2_admission_from_genesis(
      &signer,
      &preset,
      0,
      SignedAdmissionReplayEnd::Preparation,
    );
    assert_eq!(*parent.number(), 4);
    TestExternalities::new(storage).execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, SignedParkedReplayEnd::Pools);
    });
    eprintln!("Public signed Parked pool prefix reconciled: {preset}");
  }
}

/// Demand-empty periods preserve the selected Fee Sink phase before public pool setup.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn public_quiet_fee_sink_phase_repeats_before_signed_pool_prefix() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let sink = Actors::sovereign_account_id_system(sink_id);
  for preset in crate::genesis_config_presets::preset_names() {
    let (mut storage, mut parent) = replay_signed_l2_admission_from_genesis(
      &signer,
      &preset,
      0,
      SignedAdmissionReplayEnd::Preparation,
    );
    let economic_initial = TestExternalities::new(storage.clone()).execute_with(|| {
      (
        Balances::free_balance(&owner),
        Balances::reserved_balance(&owner),
        Assets::balance(0, &owner),
        Balances::free_balance(&sink),
        Balances::total_issuance(),
        Assets::total_supply(0),
      )
    });
    let mut first_phase = None;
    for number in 5..=33 {
      let authored = author_complete_block_after_with_context(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        4,
        &BTreeMap::new(),
        &[],
        inherent_data_for(&parent, number),
      );
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        use polkadot_sdk::frame_support::dispatch::DispatchClass;
        assert!(!Actors::user_creation_enabled());
        assert!(!Actors::global_circuit_breaker());
        assert_eq!(System::account_nonce(&owner), 4);
        assert_eq!(*System::block_weight().get(DispatchClass::Normal), Weight::zero());
        assert!(Actors::block_resource_state().is_none());
        assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
        assert!(polkadot_sdk::sp_io::storage::get(polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY).is_none());
        assert_eq!(economic_initial, (
          Balances::free_balance(&owner), Balances::reserved_balance(&owner),
          Assets::balance(0, &owner), Balances::free_balance(&sink),
          Balances::total_issuance(), Assets::total_supply(0),
        ));
        assert_eq!(Actors::next_actor_id(), 15);
        assert_eq!(Actors::active_actor_count(), 3);
        assert!(pallet_deos_actors::BalanceDependencySources::<Runtime>::iter().next().is_none());
        assert!(polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter().next().is_none());
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
        if [13, 23, 33].contains(&number) {
          let delta = number - 13;
          let mut state = Actors::active_actor_state(sink_id).unwrap();
          assert_eq!(state.hot.cycle_state, CycleState::Idle);
          assert!(!state.hot.pending_signal);
          assert!(state.run_state.is_none());
          let nonce = state.identity.cycle_nonce;
          let due = state.hot.trigger_wakeup_pointer.unwrap().tick;
          state.identity.cycle_nonce = state.identity.cycle_nonce.checked_sub(u64::from(delta / 10)).unwrap();
          state.hot.last_cycle_block = state.hot.last_cycle_block.map(|at| at.checked_sub(delta).unwrap());
          state.hot.trigger_wakeup_pointer.as_mut().unwrap().tick = due.checked_sub(12 * u64::from(delta)).unwrap();
          let mut process = pallet_deos_actors::ActorProcesses::<Runtime>::get(sink_id).unwrap();
          process.last_attempted = process.last_attempted.map(|at| at.checked_sub(delta).unwrap());
          let mut service = pallet_deos_actors::ServiceHeader::<Runtime>::get();
          service.round_block = service.round_block.map(|at| at.checked_sub(delta).unwrap());
          let topology = pallet_deos_actors::ServiceNodes::<Runtime>::iter()
            .map(|(id, node)| (id, (node.generation, node.previous, node.next, node.kind)))
            .collect::<BTreeMap<_, _>>();
          assert!(topology.is_empty());
          assert_eq!(service.count, 0);
          assert!(process.residence.is_none());
          let projection = (state, process, service, topology).encode();
          if let Some(expected) = &first_phase { assert_eq!(&projection, expected); }
          else { first_phase = Some(projection.clone()); }
          println!("Public quiet Fee Sink phase: preset={preset} block={number} cycle_nonce={nonce} next_due_tick={due} phase_projection_hash={:?} custody_unchanged=true service_topology_empty=true passive_idle=true", BlakeTwo256::hash(&projection));
        }
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
    assert_eq!(*parent.number(), 33);
    TestExternalities::new(storage).execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, SignedParkedReplayEnd::Pools);
    });
    eprintln!("Public quiet-phase signed pool prefix reconciled: {preset}");
  }
}

/// Include every Service/Deadline field and the remaining raw Actors namespace in the
/// quiet cadence projection; observers run outside authored/imported execution.
#[cfg(not(feature = "runtime-benchmarks"))]
fn complete_quiet_actor_projection(number: u32, base: u32) -> BTreeMap<&'static str, Vec<u8>> {
  use pallet_deos_actors::*;
  let delta = number.checked_sub(base).unwrap();
  assert_eq!(delta % 10, 0);
  let tick_delta = 12 * u64::from(delta);
  let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let normalize_key = |key| match key {
    WakeupKey::Tick(tick) => WakeupKey::<u32>::Tick(tick.checked_sub(tick_delta).unwrap()),
    WakeupKey::Block(_) => panic!("quiet genesis has only the Fee Sink Tick deadline"),
  };
  let identities = ActorIdentities::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  assert_eq!(identities.len(), 12);
  assert!(!identities.contains_key(&sink));
  let semantics = ActorSemanticStates::<Runtime>::iter()
    .map(|(id, mut value)| {
      if let ActorSemanticState::Active(record) = &mut value {
        assert_eq!(record.hot.cycle_state, CycleState::Idle);
        assert!(!record.hot.pending_signal);
        assert!(record.hot.wakeup_pointer.is_none());
        if id == sink {
          record.identity.cycle_nonce = record
            .identity
            .cycle_nonce
            .checked_sub(u64::from(delta / 10))
            .unwrap();
          record.hot.last_cycle_block = record
            .hot
            .last_cycle_block
            .map(|at| at.checked_sub(delta).unwrap());
          record.hot.trigger_wakeup_pointer.as_mut().unwrap().tick = record
            .hot
            .trigger_wakeup_pointer
            .unwrap()
            .tick
            .checked_sub(tick_delta)
            .unwrap();
        } else {
          assert!(record.hot.trigger_wakeup_pointer.is_none());
        }
      }
      (id, value)
    })
    .collect::<BTreeMap<_, _>>();
  assert_eq!(semantics.len(), 15);
  assert_eq!(
    semantics
      .values()
      .filter(|value| matches!(value, ActorSemanticState::Active(_)))
      .count(),
    3
  );
  for (id, identity) in &identities {
    assert!(
      matches!(semantics.get(id), Some(ActorSemanticState::Dormant(record)) if &record.identity == identity)
    );
  }
  let processes = ActorProcesses::<Runtime>::iter()
    .map(|(id, mut value)| {
      value.last_attempted = value
        .last_attempted
        .map(|at| at.checked_sub(delta).unwrap());
      (id, value)
    })
    .collect::<BTreeMap<_, _>>();
  assert_eq!(processes.len(), 3);
  assert!(processes.values().all(|process| process.residence.is_none()
    && matches!(process.status, ProcessStatus::Disabled(disablement)
      if disablement.cause == ProcessDisableCause::Protocol
        && disablement.revival_authority == ProcessRevivalAuthority::Protocol
        && disablement.basis == SuspendedProcessBasis::Idle)));
  assert!(ActorRunStateStore::<Runtime>::iter().next().is_none());
  let mut service = ServiceHeader::<Runtime>::get();
  service.round_block = service.round_block.map(|at| at.checked_sub(delta).unwrap());
  let nodes = ServiceNodes::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  assert!(nodes.is_empty());
  assert_eq!(service.count, 0);
  assert!(service.cursor.is_none());
  let headers = DeadlineHeaders::<Runtime>::iter()
    .map(|(key, value)| (normalize_key(key), value))
    .collect::<BTreeMap<_, _>>();
  let pages = DeadlinePages::<Runtime>::iter()
    .map(|(key, page, value)| ((normalize_key(key), page), value))
    .collect::<BTreeMap<_, _>>();
  let handles = DeadlineHandles::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  assert!(handles.is_empty());
  let trigger_handles = TriggerDeadlineHandles::<Runtime>::iter()
    .map(|(id, mut value)| {
      value.key = normalize_key(value.key);
      (id, value)
    })
    .collect::<BTreeMap<_, _>>();
  let index_pages = DeadlineIndexPages::<Runtime>::iter()
    .map(|(clock, page, mut value)| {
      for key in value.iter_mut() {
        *key = normalize_key(*key);
      }
      ((clock, page), value)
    })
    .collect::<BTreeMap<_, _>>();
  let positions = DeadlineIndexPositions::<Runtime>::iter()
    .map(|(key, value)| (normalize_key(key), value))
    .collect::<BTreeMap<_, _>>();
  let lengths = DeadlineIndexLen::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  assert_eq!(
    (
      headers.len(),
      pages.len(),
      trigger_handles.len(),
      positions.len()
    ),
    (1, 1, 1, 1)
  );
  let cutoff = PrepassExecutionCutoff::<Runtime>::get()
    .map(|(at, ticket)| (at.checked_sub(delta).unwrap(), ticket));
  let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
  assert_eq!(telemetry.block_number(), number);
  assert!(!telemetry.optional_actor_work_halted());
  let resource = (
    telemetry.budget(),
    telemetry.usage(),
    telemetry.optional_actor_work_halted(),
  );
  // These exact families are typed above; every other Actors key/value stays raw.
  let items = [
    "ActorIdentities",
    "ActorSemanticStates",
    "ActorProcesses",
    "ServiceHeader",
    "ServiceNodes",
    "DeadlineHeaders",
    "DeadlinePages",
    "DeadlineHandles",
    "TriggerDeadlineHandles",
    "DeadlineIndexPages",
    "DeadlineIndexPositions",
    "DeadlineIndexLen",
    "PrepassExecutionCutoff",
    "FinalizedBlockResourceTelemetry",
  ]
  .map(|name| polkadot_sdk::sp_io::hashing::twox_128(name.as_bytes()));
  let prefix = polkadot_sdk::sp_io::hashing::twox_128(b"Actors");
  let rest = current_top_storage()
    .top
    .into_iter()
    .filter(|(key, _)| {
      key.starts_with(&prefix)
        && !items
          .iter()
          .any(|item| key.get(16..32) == Some(item.as_slice()))
    })
    .collect::<BTreeMap<_, _>>();
  println!(
    "Public complete quiet phase inputs: block={number} raw_remaining_keys={} dormant_identities=12 semantic_records=15 active_processes=3 service_nodes=0 deadline_headers=1 deadline_pages=1 trigger_handles=1 passive_idle=true",
    rest.len()
  );
  BTreeMap::from([
    ("identities", identities.encode()),
    ("semantics", semantics.encode()),
    ("processes", processes.encode()),
    ("service", service.encode()),
    ("nodes", nodes.encode()),
    ("headers", headers.encode()),
    ("pages", pages.encode()),
    ("handles", handles.encode()),
    ("trigger_handles", trigger_handles.encode()),
    ("index_pages", index_pages.encode()),
    ("positions", positions.encode()),
    ("lengths", lengths.encode()),
    ("next_clock", NextWakeupClock::<Runtime>::get().encode()),
    ("cutoff", cutoff.encode()),
    ("resource", resource.encode()),
    ("remaining_raw", rest.encode()),
  ])
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn public_quiet_fee_sink_complete_scheduler_phase_repeats() {
  use polkadot_sdk::frame_system;
  fn assert_declared_quiet_service_quanta() {
    use pallet_deos_actors::{StepControlWeightProvider, TaskEffectWeightProvider, WeightInfo};
    type W = <Runtime as pallet_deos_actors::Config>::WeightInfo;
    type Control = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
    type Effect = <Runtime as pallet_deos_actors::Config>::TaskEffectWeight;
    let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let state = Actors::active_actor_state(sink).unwrap();
    assert_eq!(state.contract.steps.len(), 2);
    assert!(state.contract.close_when.is_none());
    assert!(state.contract.parked_balance_activation.is_none());
    assert_eq!(
      state.contract.on_unfunded,
      pallet_deos_actors::OnUnfunded::Close
    );
    let head = pallet_deos_actors::ActorContractHeads::<Runtime>::get(sink).unwrap();
    assert_eq!(head.header.step_count, 2);
    let tail = pallet_deos_actors::ActorContractTailChunks::<Runtime>::get(sink, 0).unwrap();
    assert_eq!(tail.first_step_index, 1);
    assert_eq!(tail.steps.len(), 1);
    assert_eq!(tail.step_resources.len(), 1);
    assert_eq!(tail.authority.actor_id, sink);
    assert_eq!(
      tail.authority.semantic_contract_id,
      head.header.semantic_contract_id
    );
    assert_eq!(tail.authority.body_commitment, head.header.body_commitment);
    assert_eq!(
      tail.authority.admission_identity,
      head.header.admission_identity
    );
    let stored = [
      (
        head.first_step.as_ref().unwrap(),
        head.first_step_resources.unwrap(),
      ),
      (&tail.steps[0], tail.step_resources[0]),
    ];
    let mut resources = pallet_deos_actors::ActorStepResourceEnvelope {
      control: Weight::zero(),
      effect: Weight::zero(),
    };
    for (cursor, (step, declared)) in stored.into_iter().enumerate() {
      assert_eq!(step, &state.contract.steps[cursor]);
      let context = pallet_deos_actors::StepControlWeightContext {
        cursor: u32::try_from(cursor).unwrap(),
        steps_in_fragment: 1,
        opening_tail_chunks: u32::from(cursor == 0),
        predicate_evaluation_units: step
          .precondition
          .as_ref()
          .map_or(0, |value| value.evaluation_units()),
      };
      assert_eq!(
        Control::maximum_control_weight(context, step),
        Some(declared.control)
      );
      assert_eq!(
        Effect::maximum_effect_weight(&step.task),
        Some(declared.effect)
      );
      resources.control = resources.control.max(declared.control);
      resources.effect = resources.effect.max(declared.effect);
    }
    let service = pallet_deos_actors::ServiceHeader::<Runtime>::get();
    assert!(service.count <= 1);
    assert!(service.cursor.is_none_or(|actor| actor.actor_id == sink));
    let selector = W::service_round_begin_populated()
      .checked_add(&W::service_round_probe_eligible())
      .unwrap();
    let inspection = selector
      .checked_add(&W::scheduler_actor_state_probe())
      .unwrap();
    let suffix = W::service_round_admit_eligible()
      .max(W::service_member_retire_interior())
      .max(W::service_member_retire_pair_cursor())
      .max(W::service_member_retire_singleton());
    let idle = inspection
      .checked_add(&W::service_round_admit_eligible())
      .unwrap();
    // Cadenced Opening admits potential publication cleanup even when its predicate skips Task.
    let opening = inspection
      .checked_add(&resources.control)
      .and_then(|weight| weight.checked_add(&Actors::close_dispatch_weight_upper()))
      .and_then(|weight| weight.checked_add(&suffix))
      .unwrap();
    let quantum = idle.max(opening);
    let selectors = selector.checked_add(&selector).unwrap();
    let housekeeping = W::scheduler_on_idle_base()
      .checked_add(&W::block_resource_finalize())
      .unwrap();
    let dependency = W::dependency_scan_source_probe()
      .checked_add(
        &W::process_dependency_scan_unit().max(W::process_dependency_scan_completion_unit()),
      )
      .unwrap();
    let mandatory = W::scheduler_on_initialize_cutoff()
      .checked_add(&Actors::deadline_service_weight_upper())
      .and_then(|weight| weight.checked_add(&dependency))
      .and_then(|weight| weight.checked_add(&housekeeping))
      .unwrap();
    let limits = BlockResourceBudgetValue::get().limits();
    let control_spare = limits
      .actor_control()
      .checked_sub(&mandatory)
      .unwrap()
      .checked_sub(&quantum)
      .unwrap()
      .checked_sub(&selectors)
      .unwrap();
    let effect_spare = limits
      .actor_base_turn()
      .checked_sub(&resources.effect)
      .unwrap();
    let drain_required = housekeeping.checked_add(&selector).unwrap();
    let idle_reserve_spare = <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get()
      .checked_sub(&drain_required)
      .unwrap();
    let producer = pallet_deos_actors::StepOf::<Runtime> {
      precondition: None,
      task: pallet_deos_actors::Task::Transfer {
        to: state.identity.owner.clone(),
        asset: primitives::AssetKind::Local(1),
        amount: pallet_deos_actors::AmountResolution::Fixed(1),
      },
      on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
    };
    let producer_context = pallet_deos_actors::StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 0,
      predicate_evaluation_units: 0,
    };
    let producer_control = Control::maximum_control_weight(producer_context, &producer).unwrap();
    let producer_effect = Effect::maximum_effect_weight(&producer.task).unwrap();
    let producer_encounter = inspection
      .checked_add(&producer_control)
      .and_then(|weight| weight.checked_add(&Actors::close_dispatch_weight_upper()))
      .and_then(|weight| weight.checked_add(&suffix))
      .unwrap()
      .max(idle);
    // Keep two conservative spare-member inspections; passive genesis members are not polled.
    // Each producer and either Fee Sink Step still reserves its complete declared Action.
    let cohort_control = producer_encounter
      .checked_mul(4)
      .and_then(|weight| weight.checked_add(&quantum))
      .and_then(|weight| weight.checked_add(&idle.checked_mul(2)?))
      .unwrap();
    let cohort_effect = producer_effect
      .checked_mul(4)
      .and_then(|weight| weight.checked_add(&resources.effect))
      .unwrap();
    let cohort_control_spare = limits
      .actor_control()
      .checked_sub(&mandatory)
      .and_then(|weight| weight.checked_sub(&cohort_control))
      .and_then(|weight| weight.checked_sub(&selectors))
      .unwrap();
    let cohort_effect_spare = limits
      .actor_base_turn()
      .checked_sub(&cohort_effect)
      .unwrap();
    assert!(<Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() >= 7);
    assert!(<Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get() >= 8);
    println!(
      "Declared producer cohort: block={} producer_control={producer_control:?} producer_effect={producer_effect:?} producer_encounter={producer_encounter:?} cohort_control={cohort_control:?} cohort_effect={cohort_effect:?} control_spare={cohort_control_spare:?} effect_spare={cohort_effect_spare:?} members_upper=7 assumed_producer_publication=false declared_only=true",
      System::block_number()
    );
    assert!(<Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() >= 1);
    assert!(<Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get() >= 2);
    println!(
      "Declared quiet service quanta: block={} members={} steps=2 selector={selector:?} inspection={inspection:?} idle={idle:?} opening={opening:?} quantum={quantum:?} effect={:?} mandatory={mandatory:?} control_spare={control_spare:?} effect_spare={effect_spare:?} drain_required={drain_required:?} idle_reserve_spare={idle_reserve_spare:?} declared_only=true",
      System::block_number(),
      service.count,
      resources.effect
    );
  }
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let (mut storage, mut parent) = replay_signed_l2_admission_from_genesis(
      &signer,
      &preset,
      0,
      SignedAdmissionReplayEnd::Preparation,
    );
    let custody = TestExternalities::new(storage.clone()).execute_with(|| {
      let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
      let state = Actors::active_actor_state(sink).unwrap();
      assert_eq!(state.identity.cycle_nonce, 0);
      assert!(state.hot.last_cycle_block.is_none());
      assert_eq!(state.hot.trigger_wakeup_pointer.unwrap().tick, 132);
      assert_declared_quiet_service_quanta();
      println!("Public complete quiet initial clock: preset={preset} block=4 nonce=0 last_cycle=None next_due_tick=132 transient_first_cycle=true");
      (
        Balances::total_issuance(),
        Assets::total_supply(0),
        frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
        polkadot_sdk::pallet_assets::Account::<Runtime>::iter()
          .map(|(asset, account, value)| ((asset, account), value))
          .collect::<BTreeMap<_, _>>(),
      )
    });
    let mut first: Option<BTreeMap<&'static str, Vec<u8>>> = None;
    for number in 5..=34 {
      let authored = author_complete_block_after_with_context(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        4,
        &BTreeMap::new(),
        &[],
        inherent_data_for(&parent, number),
      );
      assert_complete_block_replays_natively(&authored, &[]);
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert!(!Actors::user_creation_enabled());
        assert!(!Actors::global_circuit_breaker());
        assert!(Actors::block_resource_state().is_none());
        assert_eq!(custody, (Balances::total_issuance(), Assets::total_supply(0), frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(), polkadot_sdk::pallet_assets::Account::<Runtime>::iter().map(|(asset, account, value)| ((asset, account), value)).collect::<BTreeMap<_, _>>()));
        assert_eq!(*System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal), Weight::zero());
        assert!(polkadot_sdk::sp_io::storage::get(polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY).is_none());
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
        if [14, 24, 34].contains(&number) {
          assert_declared_quiet_service_quanta();
          let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
          let state = Actors::active_actor_state(sink).unwrap();
          let process = pallet_deos_actors::ActorProcesses::<Runtime>::get(sink).unwrap();
          assert!(!pallet_deos_actors::ServiceNodes::<Runtime>::contains_key(sink));
          assert_eq!(process.residence, None);
          assert!(matches!(process.status, pallet_deos_actors::ProcessStatus::Disabled(disablement)
            if disablement.cause == pallet_deos_actors::ProcessDisableCause::Protocol
              && disablement.revival_authority == pallet_deos_actors::ProcessRevivalAuthority::Protocol
              && disablement.basis == pallet_deos_actors::SuspendedProcessBasis::Idle));
          assert_eq!(state.identity.cycle_nonce, u64::from((number - 4) / 10));
          assert_eq!(state.hot.last_cycle_block, Some(number - 1));
          assert_eq!(process.last_attempted, Some(number - 1));
          assert_eq!(state.hot.trigger_wakeup_pointer.unwrap().tick, 12 * u64::from(number + 7));
          println!("Public complete quiet steady clock: preset={preset} block={number} nonce={} last_cycle={} last_attempted={} next_due_tick={} passive_idle=true", state.identity.cycle_nonce, state.hot.last_cycle_block.unwrap(), process.last_attempted.unwrap(), state.hot.trigger_wakeup_pointer.unwrap().tick);
          let value = complete_quiet_actor_projection(number, 14);
          if let Some(expected) = &first {
            for (role, current) in &value {
              assert_eq!(BlakeTwo256::hash(current), BlakeTwo256::hash(&expected[role]), "quiet phase changed role {role} at {number}");
              assert!(current == &expected[role]);
            }
          } else { first = Some(value.clone()); }
          let encoded = value.encode();
          println!("Public complete quiet phase: preset={preset} block={number} projection_bytes={} hash={:?} all_actor_storage_included=true all_custody_unchanged=true", encoded.len(), BlakeTwo256::hash(&encoded));
        }
      });
      parent = authored.block.header;
      storage = authored.post_state;
    }
    eprintln!("Public complete quiet phase reconciled: {preset}");
  }
}

/// A finite linked native chain exercises Aura authentication before the complete Executive.
/// Relay inputs remain test proofs; this does not invoke the Wasm-only PVF wrapper.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn sealed_aura_blocks_preserve_native_parachain_context_and_custody() {
  use polkadot_sdk::{
    cumulus_pallet_aura_ext::BlockExecutor,
    cumulus_pallet_parachain_system as parachain,
    frame_support::traits::ExecuteBlock,
    sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthorityPair},
    sp_runtime::traits::Block as BlockT,
  };
  type AuraExecutor = BlockExecutor<Runtime, Executive>;
  let transaction_signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let receiver =
    crate::AccountId::from(sr25519::Pair::from_string("//Bob", None).unwrap().public());
  let authority_pairs = [
    AuthorityPair::from_string("//Alice", None).unwrap(),
    AuthorityPair::from_string("//Bob", None).unwrap(),
  ];
  for preset in crate::genesis_config_presets::preset_names() {
    let mut storage = preset_genesis_storage(&[], &preset);
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    let receiver_genesis_balance =
      TestExternalities::new(storage.clone()).execute_with(|| Balances::free_balance(&receiver));
    let mut custody = None;
    let mut first_actor_phase = None;
    for number in 1..=34u32 {
      let author_index = number as usize % authority_pairs.len();
      let (nonce, author_pair_index) = TestExternalities::new(storage.clone()).execute_with(|| {
        let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
        assert_eq!(authorities.len(), authority_pairs.len());
        let pair_index = authority_pairs
          .iter() // deos-bypass: bounded-iter -- two fixed local test authority pairs
          .position(|pair| pair.public() == authorities[author_index])
          .unwrap();
        (
          System::account_nonce(crate::AccountId::from(transaction_signer.public())),
          pair_index,
        )
      });
      let calls = if number <= 4 {
        vec![RuntimeCall::Balances(
          polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
            dest: receiver.clone().into(),
            value: crate::EXISTENTIAL_DEPOSIT,
          },
        )]
      } else {
        Vec::new()
      };
      let authored = author_complete_block_after_with_calls(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &transaction_signer,
        nonce,
        &BTreeMap::new(),
        &calls,
      );
      assert_eq!(authored.block.header.parent_hash(), &parent.hash());
      let pre_seal_hash = authored.block.header.hash();
      let seal = authority_pairs[author_pair_index].sign(pre_seal_hash.as_bytes());
      let mut sealed = authored.block.clone();
      sealed.header.digest_mut().push(DigestItem::aura_seal(seal));
      assert_eq!(sealed.header.digest().logs.len(), 3);
      assert_eq!(
        sealed.header.encoded_size(),
        authored.block.header.encoded_size() + 71
      );
      if number == 1 {
        for case in ["missing", "multiple", "invalid", "wrong_author"] {
          let mut rejected = sealed.clone();
          match case {
            "missing" => {
              rejected.header.digest_mut().pop().unwrap();
            }
            "multiple" => {
              let duplicate = rejected.header.digest().logs.last().unwrap().clone();
              rejected.header.digest_mut().push(duplicate);
            }
            "invalid" => {
              rejected.header.digest_mut().pop().unwrap();
              rejected.header.digest_mut().push(DigestItem::aura_seal(
                polkadot_sdk::sp_consensus_aura::sr25519::AuthoritySignature::from(
                  sr25519::Signature::from_raw([23; 64]),
                ),
              ));
            }
            "wrong_author" => {
              rejected.header.digest_mut().pop().unwrap();
              let wrong = authority_pairs[1 - author_pair_index].sign(pre_seal_hash.as_bytes());
              rejected
                .header
                .digest_mut()
                .push(DigestItem::aura_seal(wrong));
            }
            _ => unreachable!("fixed rejection cases"),
          }
          let mut ext = TestExternalities::new(authored.pre_state.clone());
          ext.execute_with(|| {
            let before = current_top_storage();
            let mut lazy: <Block as BlockT>::LazyBlock = rejected.into();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
              <AuraExecutor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut lazy);
            }));
            assert!(result.is_err(), "seal rejection must execute the SDK guard: {case}");
            assert_eq!(current_top_storage().top, before.top);
            println!("Native Aura rejection: preset={preset} case={case} rejected_before_execution=true all_top_state_unchanged=true");
          });
        }
      }
      let mut replay = TestExternalities::new_with_code_and_state(
        &[],
        authored.pre_state.clone(),
        crate::VERSION.state_version(),
      );
      let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
      replay.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
        recorder.clone(),
      )));
      replay.execute_with_recorder(recorder, || {
        crate::apis::validate_context_inherent_geometry(&authored.inherent_data).unwrap();
        assert!(
          authored
            .inherent_data
            .check_extrinsics(&sealed.clone().into())
            .ok()
        );
        <AuraExecutor as ExecuteBlock<Block>>::execute_block(sealed.clone().into());
      });
      // Diagnostics run after the complete recorded import, never inside its proof accounting.
      replay.execute_with(|| {
        assert_eq!(current_top_storage().top, authored.post_state.top);
        assert_eq!(System::block_number(), number);
        assert_eq!(System::parent_hash(), parent.hash());
        assert_eq!(System::block_hash(number - 1), parent.hash());
        assert_eq!(System::account_nonce(crate::AccountId::from(transaction_signer.public())), number.min(4));
        assert_eq!(Balances::free_balance(&receiver), receiver_genesis_balance + u128::from(number.min(4)) * crate::EXISTENTIAL_DEPOSIT);
        let validation = parachain::ValidationData::<Runtime>::get().unwrap();
        assert_eq!(validation.parent_head.0, parent.encode());
        assert_eq!(validation.relay_parent_number, number);
        assert_eq!(parachain::LastRelayChainBlockNumber::<Runtime>::get(), number);
        assert_eq!(parachain::HrmpWatermark::<Runtime>::get(), number);
        assert_eq!(parachain::UnincludedSegment::<Runtime>::get().len(), 1);
        assert!(Actors::block_resource_state().is_none());
        assert!(!Actors::user_creation_enabled());
        assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
        let current_custody = (
          Balances::total_issuance(),
          Assets::total_supply(0),
          polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
          polkadot_sdk::pallet_assets::Account::<Runtime>::iter()
            .map(|(asset, account, value)| ((asset, account), value))
            .collect::<BTreeMap<_, _>>(),
        );
        if number == 4 {
          custody = Some(current_custody);
        } else if number > 4 {
          assert_eq!(custody.as_ref().unwrap(), &current_custody);
        }
        if [14, 24, 34].contains(&number) {
          let phase = complete_quiet_actor_projection(number, 14);
          if let Some(expected) = &first_actor_phase {
            assert_eq!(&phase, expected);
          } else {
            first_actor_phase = Some(phase);
          }
        }
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
        println!("Native sealed complete block: preset={preset} block={number} author_index={author_index} authority_pair_index={author_pair_index} signed_calls={} sealed_header_bytes={} validation_data_bytes={} parent={:?} block_hash={:?} state_root={:?} actual_seal_verified=true author_import_top_equal=true signed_nonce_and_recipient_checked=true custody_preserved_after_four=true literal_history=false", calls.len(), sealed.header.encoded_size(), validation.encoded_size(), parent.hash(), sealed.header.hash(), sealed.header.state_root());
      });
      replay.commit_all().unwrap();
      assert_eq!(replay.backend.root(), sealed.header.state_root());
      storage = authored.post_state;
      parent = sealed.header;
    }
    assert!(first_actor_phase.is_some());
    eprintln!(
      "Native sealed caller reconciled: {preset} complete_blocks=34 signed_calls=4 rejected_seals=4"
    );
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn quiet_consensus_projection(number: u32, base: u32) -> BTreeMap<Vec<u8>, Vec<u8>> {
  use codec::Decode;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system as parachain, frame_support::storage::StoragePrefixedMap,
  };
  let delta = number.checked_sub(base).unwrap();
  let raw_key = |pallet, item| {
    [
      polkadot_sdk::sp_io::hashing::twox_128(pallet),
      polkadot_sdk::sp_io::hashing::twox_128(item),
    ]
    .concat()
  };
  let relay_key = raw_key(b"AuraExt", b"RelaySlotInfo");
  let mirror_key = raw_key(b"AuraExt", b"Authorities");
  let author_key = raw_key(b"Authorship", b"Author");
  let relay_bytes = polkadot_sdk::sp_io::storage::get(&relay_key).unwrap();
  let relay_info = <(u64, u32)>::decode(&mut &relay_bytes[..]).unwrap();
  assert_eq!(relay_info, (u64::from(number), 1));
  assert_eq!(
    u64::from(polkadot_sdk::pallet_aura::CurrentSlot::<Runtime>::get()),
    u64::from(number)
  );
  let mirror = polkadot_sdk::sp_io::storage::get(&mirror_key).unwrap();
  assert_eq!(
    &mirror[..],
    polkadot_sdk::pallet_aura::Authorities::<Runtime>::get()
      .encode()
      .as_slice()
  );
  assert!(polkadot_sdk::sp_io::storage::get(&author_key).is_none());
  let last_relay_key = parachain::LastRelayChainBlockNumber::<Runtime>::hashed_key();
  assert_eq!(
    parachain::LastRelayChainBlockNumber::<Runtime>::get(),
    number
  );
  let authors = polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::iter()
    .collect::<BTreeMap<_, _>>();
  let validators = polkadot_sdk::pallet_session::Validators::<Runtime>::get();
  assert_eq!(validators.len(), 2);
  assert_eq!(authors.len(), validators.len());
  for (index, author) in validators.iter().enumerate() {
    let lag = (number % 2 + 2 - u32::try_from(index).unwrap()) % 2;
    assert_eq!(authors[author], number - lag);
  }
  let prefixes =
    [b"Aura".as_slice(), b"AuraExt", b"Authorship"].map(polkadot_sdk::sp_io::hashing::twox_128);
  let authored_prefix =
    polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::final_prefix();
  let mut top = current_top_storage().top;
  top.retain(|key, _| {
    prefixes.iter().any(|prefix| key.starts_with(prefix))
      || key.starts_with(&authored_prefix)
      || key == &last_relay_key
  });
  top.insert(
    polkadot_sdk::pallet_aura::CurrentSlot::<Runtime>::hashed_key().to_vec(),
    u64::from(base).encode(),
  );
  top.insert(relay_key, (u64::from(base), 1u32).encode());
  top.insert(last_relay_key.to_vec(), base.encode());
  for (author, last) in authors {
    top.insert(
      polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::hashed_key_for(author),
      last.checked_sub(delta).unwrap().encode(),
    );
  }
  top
}
#[cfg(not(feature = "runtime-benchmarks"))]
fn transfer_quiet_fee_multiplier(from: u32, to: u32) {
  use polkadot_sdk::pallet_transaction_payment::{MultiplierUpdate, NextFeeMultiplier};
  use polkadot_sdk::sp_runtime::traits::Convert;
  type Update = <Runtime as polkadot_sdk::pallet_transaction_payment::Config>::FeeMultiplierUpdate;
  let count = to.checked_sub(from).unwrap();
  assert_eq!(
    *System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal),
    Weight::zero()
  );
  let before_top = current_top_storage().top;
  let before = NextFeeMultiplier::<Runtime>::get();
  let mut current = before;
  let minimum = Update::min();
  assert!(
    current >= minimum
      && current <= polkadot_sdk::pallet_transaction_payment::Multiplier::from_u32(1)
  );
  let mut scalar_updates = 0;
  for _ in 0..count {
    let next = Update::convert(current);
    scalar_updates += 1;
    assert!(next <= current && next >= minimum);
    if next == current {
      assert_eq!(current, minimum);
      break;
    }
    current = next;
  }
  // Only this scalar is materialized; skipped blocks, hooks and caller state are not executed.
  NextFeeMultiplier::<Runtime>::put(current);
  let mut expected_top = before_top;
  expected_top.insert(
    NextFeeMultiplier::<Runtime>::hashed_key().to_vec(),
    current.encode(),
  );
  assert_eq!(current_top_storage().top, expected_top);
  println!(
    "Prepared quiet fee transfer: from={from} to={to} count={count} before={} after={} scalar_updates={scalar_updates} fixed_minimum={} all_other_top_state_exact=true conditional=true literal_history=false",
    before.into_inner(),
    current.into_inner(),
    current == minimum
  );
}
#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_canonical_system_hash_window() {
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  let number = System::block_number();
  let prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
  assert!(polkadot_sdk::sp_io::storage::get(&prefix).is_none());
  let mut cursor = prefix.to_vec();
  let entries = std::iter::from_fn(|| {
    let key = polkadot_sdk::sp_io::storage::next_key(&cursor)?;
    if !key.starts_with(&prefix) {
      return None;
    }
    let value = polkadot_sdk::sp_io::storage::get(&key).unwrap().to_vec();
    cursor = key.clone();
    Some((key, value))
  });
  assert_system_hash_window_entries(number, entries);
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_system_hash_window_entries(
  number: u32,
  entries: impl Iterator<Item = (Vec<u8>, Vec<u8>)>,
) {
  use codec::MaxEncodedLen;
  assert!(number > 0);
  let count = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
  let actual = entries
    .map(|(key, value)| {
      assert_eq!(value.len(), H256::max_encoded_len());
      key
    })
    .collect::<BTreeSet<_>>();
  let expected = std::iter::once(0)
    .chain(number.saturating_sub(count).max(1)..number)
    .map(polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for)
    .collect::<BTreeSet<_>>();
  assert_eq!(
    actual, expected,
    "construction requires the canonical System hash-key window"
  );
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn incremental_system_hash_window_matches_full_scan_and_rejects_invalid_deltas() {
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  let count = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
  let preset = crate::genesis_config_presets::preset_names().remove(0);
  let prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
  for start in [1, count, count + 1] {
    let mut seed = TestExternalities::new(preset_genesis_storage(&[], &preset));
    seed.execute_with(|| {
      System::set_block_number(start);
      prepare_canonical_system_hash_window(start);
    });
    let storage = seed.execute_with(current_top_storage);
    let parent = parent_header_for(storage.clone(), &[], start);
    for fault in ["none", "missing", "prefix", "extra", "width", "expired"] {
      let mut ext = externalities_with_bound_code(storage.clone());
      let mut window = CanonicalHashWindow::from_committed(&mut ext, &parent);
      window.assert_parent(&mut ext, &parent);
      let number = start + 1;
      let expired = number.saturating_sub(count).saturating_sub(1);
      if fault == "expired" && expired == 0 {
        continue;
      }
      ext.execute_with(|| {
        System::set_block_number(number);
        polkadot_sdk::frame_system::BlockHash::<Runtime>::insert(number - 1, parent.hash());
        if expired > 0 {
          polkadot_sdk::frame_system::BlockHash::<Runtime>::remove(expired);
        }
        match fault {
          "none" => {}
          "missing" => polkadot_sdk::frame_system::BlockHash::<Runtime>::remove(number - 1),
          "prefix" => polkadot_sdk::sp_io::storage::set(&prefix, &[0; 32]),
          "extra" => {
            let mut key = prefix.to_vec();
            key.push(255);
            polkadot_sdk::sp_io::storage::set(&key, &[0; 32]);
          }
          "width" => polkadot_sdk::sp_io::storage::set(
            &polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(0),
            &[0; 31],
          ),
          "expired" => {
            polkadot_sdk::frame_system::BlockHash::<Runtime>::insert(expired, H256::zero())
          }
          _ => unreachable!(),
        }
      });
      let before = ext.execute_with(current_top_storage);
      let successor = parent_header_for(before.clone(), &[], number);
      let full_scan = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ext.execute_with(assert_canonical_system_hash_window);
      }));
      let incremental = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        window.commit_successor(&mut ext, &successor);
      }));
      assert_eq!(full_scan.is_ok(), fault == "none", "{start}: {fault}");
      assert_eq!(incremental.is_ok(), full_scan.is_ok(), "{start}: {fault}");
      assert_eq!(ext.execute_with(current_top_storage).top, before.top);
      if fault == "none" {
        window.assert_parent(&mut ext, &successor);
        ext.execute_with(|| polkadot_sdk::sp_io::storage::set(b"unobserved-write", &[1]));
        ext.commit_all().unwrap();
        assert!(
          std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            window.assert_parent(&mut ext, &successor);
          }))
          .is_err()
        );
      }
    }
  }
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn canonical_system_hash_window_guard_rejects_malformed_namespace_without_mutation() {
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      System::set_block_number(3);
      prepare_canonical_system_hash_window(3);
      polkadot_sdk::sp_io::storage::set(b"unrelated-window-guard", &[7; 128]);
      assert_canonical_system_hash_window();
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage.clone(), &[], 3);
    assert_construction_parent_matches_state(&mut ext, &parent);
    ext.commit_all().unwrap();
    assert_construction_parent_matches_state(&mut ext, &parent);
    let prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
    let mut extra = prefix.to_vec();
    extra.push(255);
    for (key, value) in [
      (
        polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(2),
        None,
      ),
      (prefix.to_vec(), Some(vec![0; 32])),
      (extra, Some(vec![0; 32])),
      (
        polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(0),
        Some(vec![0; 31]),
      ),
    ] {
      let mut invalid = TestExternalities::new(storage.clone());
      invalid.execute_with(|| match value {
        Some(value) => polkadot_sdk::sp_io::storage::set(&key, &value),
        None => polkadot_sdk::sp_io::storage::clear(&key),
      });
      let before = invalid.execute_with(current_top_storage);
      assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          invalid.execute_with(assert_canonical_system_hash_window);
        }))
        .is_err()
      );
      assert_eq!(invalid.execute_with(current_top_storage).top, before.top);
      let parent = parent_header_for(before.clone(), &[], 3);
      for committed in [false, true] {
        if committed {
          invalid.commit_all().unwrap();
          assert!(invalid.overlayed_changes().is_empty());
        }
        assert!(
          std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_construction_parent_matches_state(&mut invalid, &parent);
          }))
          .is_err()
        );
        assert_eq!(invalid.execute_with(current_top_storage).top, before.top);
      }
    }
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn prepare_canonical_system_hash_window(number: u32) {
  assert_eq!(System::block_number(), number);
  let count = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
  let old = polkadot_sdk::frame_system::BlockHash::<Runtime>::iter().collect::<BTreeMap<_, _>>();
  let genesis = System::block_hash(0);
  for key in old.keys() {
    polkadot_sdk::frame_system::BlockHash::<Runtime>::remove(key);
  }
  polkadot_sdk::frame_system::BlockHash::<Runtime>::insert(0, genesis);
  // Omitted hashes supply fixed-width opaque values, not reconstructed historical headers.
  for key in number.saturating_sub(count).max(1)..number {
    polkadot_sdk::frame_system::BlockHash::<Runtime>::insert(
      key,
      old
        .get(&key)
        .copied()
        .unwrap_or_else(|| BlakeTwo256::hash(&key.encode())),
    );
  }
  assert_eq!(System::block_hash(0), genesis);
  assert_canonical_system_hash_window();
  println!(
    "Prepared canonical System hash window: block={number} keys={} genesis_preserved=true retained_values_preserved=true omitted_values_opaque=true literal_history=false",
    number.min(count + 1)
  );
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn translate_empty_gap(from: u32, to: u32) {
  use codec::Decode;
  use pallet_deos_actors::*;
  use polkadot_sdk::cumulus_pallet_parachain_system as parachain;
  transfer_quiet_fee_multiplier(from, to);
  let delta = to.checked_sub(from).unwrap();
  let ticks = 12 * u64::from(delta);
  let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let translate = |key| match key {
    WakeupKey::Tick(tick) => WakeupKey::<u32>::Tick(tick.checked_add(ticks).unwrap()),
    WakeupKey::Block(_) => panic!("quiet preparation must contain only the Fee Sink Tick key"),
  };
  assert_eq!(ServiceHeader::<Runtime>::get().count, 0);
  assert!(ServiceHeader::<Runtime>::get().cursor.is_none());
  assert!(ServiceNodes::<Runtime>::iter().next().is_none());
  ActorSemanticStates::<Runtime>::mutate(sink, |value| {
    let ActorSemanticState::Active(record) = value.as_mut().unwrap() else {
      panic!("Fee Sink must be Active")
    };
    assert_eq!(record.hot.cycle_state, CycleState::Idle);
    assert!(!record.hot.pending_signal);
    assert!(record.hot.wakeup_pointer.is_none());
    record.identity.cycle_nonce += u64::from(delta / 10);
    record.hot.last_cycle_block = Some(record.hot.last_cycle_block.unwrap() + delta);
    record.hot.trigger_wakeup_pointer.as_mut().unwrap().tick += ticks;
  });
  ActorProcesses::<Runtime>::mutate(sink, |value| {
    let value = value.as_mut().unwrap();
    value.last_attempted = Some(value.last_attempted.unwrap() + delta);
  });
  ServiceHeader::<Runtime>::mutate(|value| {
    value.round_block = Some(value.round_block.unwrap() + delta)
  });
  let headers = DeadlineHeaders::<Runtime>::iter().collect::<Vec<_>>();
  assert_eq!(headers.len(), 1);
  for (key, value) in headers {
    DeadlineHeaders::<Runtime>::remove(key);
    DeadlineHeaders::<Runtime>::insert(translate(key), value);
  }
  for (key, page, value) in DeadlinePages::<Runtime>::iter().collect::<Vec<_>>() {
    DeadlinePages::<Runtime>::remove(key, page);
    DeadlinePages::<Runtime>::insert(translate(key), page, value);
  }
  assert!(DeadlineHandles::<Runtime>::iter().next().is_none());
  TriggerDeadlineHandles::<Runtime>::mutate(sink, |value| {
    let value = value.as_mut().unwrap();
    value.key = translate(value.key);
  });
  for (clock, page, mut value) in DeadlineIndexPages::<Runtime>::iter().collect::<Vec<_>>() {
    for key in value.iter_mut() {
      *key = translate(*key);
    }
    DeadlineIndexPages::<Runtime>::insert(clock, page, value);
  }
  for (key, value) in DeadlineIndexPositions::<Runtime>::iter().collect::<Vec<_>>() {
    DeadlineIndexPositions::<Runtime>::remove(key);
    DeadlineIndexPositions::<Runtime>::insert(translate(key), value);
  }
  PrepassExecutionCutoff::<Runtime>::mutate(|value| {
    if let Some((at, _)) = value {
      *at += delta;
    }
  });
  let snapshot = FinalizedBlockResourceTelemetry::<Runtime>::get().unwrap();
  assert_eq!(snapshot.block_number(), from);
  let mut bytes = snapshot.encode();
  bytes[..4].copy_from_slice(&to.encode());
  FinalizedBlockResourceTelemetry::<Runtime>::put(
    FinalizedBlockResourceSnapshot::<u32>::decode(&mut bytes.as_slice()).unwrap(),
  );
  // Fixed trusted keys and empty security obligations leave only these Session successors.
  let period = crate::configs::Period::get();
  assert_eq!(crate::Session::current_index(), from / period);
  if to / period > from / period {
    polkadot_sdk::pallet_session::CurrentIndex::<Runtime>::put(to / period);
    polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::put(true);
  }
  println!(
    "Prepared trusted Session transfer: from={from} to={to} old_index={} new_index={} queued_changed={} conditional=true",
    from / period,
    crate::Session::current_index(),
    polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::get()
  );
  // One advancing slot and the same two alternating authors define this closed clock recipe.
  let consensus_before = quiet_consensus_projection(from, from);
  polkadot_sdk::pallet_aura::CurrentSlot::<Runtime>::put(
    polkadot_sdk::sp_consensus_aura::Slot::from(u64::from(to)),
  );
  let relay_slot_key = [
    polkadot_sdk::sp_io::hashing::twox_128(b"AuraExt"),
    polkadot_sdk::sp_io::hashing::twox_128(b"RelaySlotInfo"),
  ]
  .concat();
  polkadot_sdk::sp_io::storage::set(&relay_slot_key, &(u64::from(to), 1u32).encode());
  parachain::LastRelayChainBlockNumber::<Runtime>::put(to);
  for (author, last) in
    polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::iter()
      .collect::<Vec<_>>()
  {
    polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::insert(
      author,
      last.checked_add(delta).unwrap(),
    );
  }
  assert_eq!(quiet_consensus_projection(to, from), consensus_before);
  println!(
    "Prepared consensus clock transfer: from={from} to={to} aura_slot={to} relay_slot={to} authored_in_slot=1 last_relay_block={to} alternating_authors=2 all_normalized_consensus_state_equal=true conditional=true"
  );
  let lag = from - pallet_governance::LastProcessedEpoch::<Runtime>::get();
  pallet_governance::LastProcessedEpoch::<Runtime>::put(to - lag);
  System::set_block_number(to);
  prepare_canonical_system_hash_window(to);
  polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(u64::from(to) * crate::SLOT_DURATION);
  println!(
    "Prepared empty Governance gap: from={from} to={to} blocks={delta} periods={} cursor_lag={lag} authorization_unchanged=true conditional=true",
    delta / 10
  );
}
/// Canonical hash windows use opaque H256 values; projected parents are not historical headers.
/// Real Aura import and complete Executive execution own every finite successor block.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn full_hash_windows_survive_sealed_native_pruning_and_horizon() {
  use codec::Compact;
  use polkadot_sdk::{
    cumulus_pallet_parachain_system as parachain, frame_support::storage::StoragePrefixedMap,
  };
  fn service_storage_entries(storage: &Storage) -> BTreeMap<String, (Vec<u8>, Vec<u8>)> {
    use polkadot_sdk::frame_support::traits::{
      PalletsInfoAccess, STORAGE_VERSION_STORAGE_KEY_POSTFIX,
    };
    let pallets = crate::AllPalletsWithSystem::infos();
    assert_eq!(pallets.len(), 27);
    let mut keys = pallets
      .into_iter()
      .map(|pallet| {
        (
          polkadot_sdk::frame_support::storage::storage_prefix(
            pallet.name.as_bytes(),
            STORAGE_VERSION_STORAGE_KEY_POSTFIX,
          )
          .to_vec(),
          format!("{}.FrameStorageVersion", pallet.name),
        )
      })
      .collect::<BTreeMap<_, _>>();
    assert_eq!(keys.len(), 27);
    keys.insert(b":code".to_vec(), "Runtime.Code".to_owned());
    keys.insert(b":c".to_vec(), "ParachainSystem.GenesisMarker".to_owned());
    let info = runtime_proof_storage_info();
    for key in storage.top.keys() {
      let transient = [
        polkadot_sdk::sp_core::storage::well_known_keys::EXTRINSIC_INDEX,
        polkadot_sdk::sp_core::storage::well_known_keys::INTRABLOCK_ENTROPY,
      ];
      assert_eq!(
        usize::from(keys.contains_key(key))
          + usize::from(transient.contains(&key.as_slice()))
          + info
            .iter()
            .filter(|owner| key.starts_with(&owner.prefix))
            .count(),
        1,
        "every TOP key has exactly one metadata, persistent-service or transient-kernel owner: {key:?}"
      );
    }
    keys
      .into_iter()
      .map(|(key, owner)| {
        let value = storage
          .top
          .get(&key)
          .expect("genesis installs each service key")
          .clone();
        if owner.ends_with(".FrameStorageVersion") {
          assert_eq!(value.len(), 2);
        } else {
          assert!(
            value.is_empty(),
            "this native fixture retains empty code and genesis marker"
          );
        }
        (owner, (key, value))
      })
      .collect()
  }
  fn assert_previous_system_buffers_replaced(authored: &AuthoredBlock, preset: &str) {
    use polkadot_sdk::frame_support::storage::storage_prefix;
    use polkadot_sdk::sp_core::{hexdisplay::HexDisplay, storage::well_known_keys};
    let mut perturbed = authored.pre_state.clone();
    let mut buffers = Vec::new();
    for name in [
      "Events",
      "EventCount",
      "Digest",
      "ParentHash",
      "BlockWeight",
      "BlockSize",
      "ExecutionPhase",
    ] {
      buffers.push((
        format!("System.{name}"),
        storage_prefix(b"System", name.as_bytes()).to_vec(),
      ));
    }
    buffers.push((
      "System.ExtrinsicIndex".to_owned(),
      well_known_keys::EXTRINSIC_INDEX.to_vec(),
    ));
    buffers.push((
      "System.IntrablockEntropy".to_owned(),
      well_known_keys::INTRABLOCK_ENTROPY.to_vec(),
    ));
    for (index, (_, key)) in buffers.iter().enumerate() {
      perturbed.top.insert(key.clone(), vec![255; index + 1]);
    }
    let topic_prefix = storage_prefix(b"System", b"EventTopics");
    for byte in 1..=3 {
      let topic = H256::repeat_byte(byte).encode();
      let key = [
        topic_prefix.as_slice(),
        &polkadot_sdk::sp_io::hashing::blake2_128(&topic),
        &topic,
      ]
      .concat();
      perturbed.top.insert(key, vec![255; byte as usize]);
    }
    assert_ne!(perturbed.top, authored.pre_state.top);
    let initialize = |storage| {
      TestExternalities::new(storage).execute_with(|| {
        let old_version = polkadot_sdk::frame_system::LastRuntimeUpgrade::<Runtime>::get().unwrap();
        assert!(!old_version.was_upgraded(&crate::VERSION));
        assert_eq!(System::block_number() + 1, *authored.block.header.number());
        Executive::initialize_block(&authored.block.header);
        assert_eq!(
          polkadot_sdk::frame_system::LastRuntimeUpgrade::<Runtime>::get(),
          Some(old_version)
        );
        current_top_storage()
      })
    };
    let expected = initialize(authored.pre_state.clone());
    let actual = initialize(perturbed);
    assert_eq!(
      actual.top, expected.top,
      "prior System buffers cannot change initialized runtime state"
    );
    assert_eq!(
      service_storage_entries(&actual),
      service_storage_entries(&authored.pre_state)
    );
    println!(
      "System buffer replacement: {}",
      serde_json::json!({
        "preset": preset, "block": authored.block.header.number(), "scalarBuffers": buffers.iter().map(|(owner, key)| serde_json::json!({"owner": owner, "key": format!("{}", HexDisplay::from(key))})).collect::<Vec<_>>(),
        "topicEntries": 3, "allInitializedTopEqual": true, "runtimeUpgradeExcluded": true,
        "serviceKeysExact": 29, "diagnosticClone": true, "admittedPredecessorClaim": false,
      })
    );
  }
  fn assert_previous_parachain_fields_replaced(authored: &AuthoredBlock, preset: &str) {
    use codec::Decode;
    let number = *authored.block.header.number();
    let input = authored
      .inherent_data
      .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
      .unwrap()
      .unwrap();
    let mut ext = TestExternalities::new(authored.pre_state.clone());
    let (before_aggregate, changed_keys) = ext.execute_with(|| {
      assert_eq!(System::block_number(), number - 1);
      assert_eq!(
        parachain::LastRelayChainBlockNumber::<Runtime>::get(),
        number - 1
      );
      let segment = parachain::UnincludedSegment::<Runtime>::get();
      assert_eq!(segment.len(), 1);
      assert!(segment[0].para_head_hash().is_none());
      assert_eq!(segment[0].encode(), vec![0; 11]);
      let mut aggregate = parachain::AggregatedUnincludedSegment::<Runtime>::get().unwrap();
      assert!(aggregate.consumed_go_ahead_signal().is_none());
      let before = aggregate.encode();
      assert_eq!(before.len(), 15);
      assert_eq!(&before[..9], &[0; 9]);
      assert_eq!(before[9], 1);
      assert_eq!(before[14], 0);
      let mut bytes = before.clone();
      bytes[10..14].copy_from_slice(&(number - 1).encode());
      let mut encoded = bytes.as_slice();
      aggregate = Decode::decode(&mut encoded).unwrap();
      assert!(encoded.is_empty());
      assert_eq!(aggregate.encode(), bytes);
      parachain::AggregatedUnincludedSegment::<Runtime>::put(aggregate);
      parachain::HrmpWatermark::<Runtime>::put(number - 1);
      let mut last = parachain::LastProcessedHrmpMessage::<Runtime>::get().unwrap();
      assert_eq!(last.reverse_idx, 0);
      last.sent_at = number - 1;
      parachain::LastProcessedHrmpMessage::<Runtime>::put(last);
      let mut tracker = parachain::PoVMessagesTracker::<Runtime>::get().unwrap();
      assert_eq!(tracker.core_selector, 0);
      assert_eq!(tracker.bundle_index, 0);
      assert_eq!(tracker.ump_msg_count, 0);
      assert_eq!(tracker.hrmp_outbound_count, 0);
      assert!(tracker.hrmp_outbound_recipients.is_empty());
      tracker.relay_storage_root_or_hash = input.validation_data.relay_parent_storage_root;
      parachain::PoVMessagesTracker::<Runtime>::put(tracker);
      // Replaced past payloads are diagnostic; they are not authenticated previous relay inputs.
      parachain::ValidationData::<Runtime>::put(input.validation_data.clone());
      parachain::RelayStateProof::<Runtime>::put(input.relay_chain_state.clone());
      let before_top = &authored.pre_state.top;
      let after_top = current_top_storage().top;
      let changed = before_top
        .keys()
        .chain(after_top.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|key| before_top.get(*key) != after_top.get(*key))
        .cloned()
        .collect::<BTreeSet<_>>();
      let allowed = BTreeSet::from([
        parachain::AggregatedUnincludedSegment::<Runtime>::hashed_key().to_vec(),
        parachain::HrmpWatermark::<Runtime>::hashed_key().to_vec(),
        parachain::LastProcessedHrmpMessage::<Runtime>::hashed_key().to_vec(),
        parachain::PoVMessagesTracker::<Runtime>::hashed_key().to_vec(),
        parachain::ValidationData::<Runtime>::hashed_key().to_vec(),
        parachain::RelayStateProof::<Runtime>::hashed_key().to_vec(),
      ]);
      assert!(changed.is_subset(&allowed));
      assert!(changed.len() >= 3);
      (before, changed.len())
    });
    let variant = AuthoredBlock {
      pre_state: ext.execute_with(current_top_storage),
      post_state: authored.post_state.clone(),
      block: authored.block.clone(),
      inherent_data: authored.inherent_data.clone(),
      metrics: authored.metrics.clone(),
      execution_backend_keys: authored.execution_backend_keys.clone(),
      execution_overlay_keys: authored.execution_overlay_keys.clone(),
      execution_recorded_proof_bytes: authored.execution_recorded_proof_bytes,
    };
    println!("Parachain replacement probe begin: preset={preset} block={number} diagnostic=true");
    let header = assert_complete_block_replays_with_aura(&variant);
    assert_eq!(header.state_root(), authored.block.header.state_root());
    println!(
      "Parachain replacement probe end: {}",
      serde_json::json!({
        "preset": preset, "block": number, "changedKeys": changed_keys,
        "oldAggregate": format!("{}", polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&before_aggregate)),
        "sameExtrinsics": true, "allFinalTopAndRootEqual": true, "proofSizeExt": true,
        "singletonEmptyAncestryRetained": true, "lastRelayClockRetained": true,
        "oldTrackerRootMatchesCurrent": true, "bundleIndexStrictIncreaseRejected": true,
        "diagnosticClone": true, "admittedPredecessorClaim": false,
      })
    );
  }
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let sender = crate::AccountId::from(signer.public());
  let recipient =
    crate::AccountId::from(sr25519::Pair::from_string("//Bob", None).unwrap().public());
  let transfer = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
    dest: recipient.clone().into(),
    value: crate::EXISTENTIAL_DEPOSIT,
  });
  for preset in crate::genesis_config_presets::preset_names() {
    println!("Full-window prefix: preset={preset}");
    let mut storage = preset_genesis_storage(&[], &preset);
    let service_entries = service_storage_entries(&storage);
    println!(
      "Runtime service key inventory: {}",
      serde_json::json!({
        "preset": preset,
        "owners": service_entries.iter().map(|(owner, (key, value))| serde_json::json!({
          "owner": owner, "key": format!("{}", polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key)),
          "value": format!("{}", polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(value)),
        })).collect::<Vec<_>>(),
      })
    );
    let mut parent = parent_header_for(storage.clone(), &[], 0);
    for number in 1..=13 {
      let nonce =
        TestExternalities::new(storage.clone()).execute_with(|| System::account_nonce(&sender));
      let calls = if number <= 4 {
        vec![transfer.clone()]
      } else {
        Vec::new()
      };
      let authored = author_complete_block_after_with_calls(
        storage,
        &[],
        &parent,
        number,
        UserDemand::ActorOnly,
        &signer,
        nonce,
        &BTreeMap::new(),
        &calls,
      );
      if number == 13 {
        assert_previous_system_buffers_replaced(&authored, &preset);
      }
      parent = assert_complete_block_replays_with_aura(&authored);
      if number == 13 {
        assert_previous_parachain_fields_replaced(&authored, &preset);
      }
      assert_eq!(
        service_storage_entries(&authored.post_state),
        service_entries
      );
      storage = authored.post_state;
    }
    let original = storage.clone();
    for start in [4_083u32, 2_552_520_723] {
      let mut projected = TestExternalities::new(original.clone());
      let genesis = projected.execute_with(|| {
        translate_empty_gap(13, start);
        System::block_hash(0)
      });
      let mut candidate = projected.execute_with(current_top_storage);
      assert!(candidate.children_default.is_empty());
      let mut parent = parent_header_for(candidate.clone(), &[], start);
      parent.digest_mut().push(DigestItem::PreRuntime(
        polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
        u64::from(start).encode(),
      ));
      parent.digest_mut().push(DigestItem::Consensus(
        *b"RPSR",
        (H256::repeat_byte(37), Compact(start)).encode(),
      ));
      parent = TestExternalities::new(candidate.clone()).execute_with(|| {
        seal_complete_native_block(&Block {
          header: parent,
          extrinsics: Vec::new(),
        })
        .header
      });
      let (recipient_before, nonce_before, issuance) = TestExternalities::new(candidate.clone())
        .execute_with(|| {
          (
            Balances::free_balance(&recipient),
            System::account_nonce(&sender),
            (Balances::total_issuance(), Assets::total_supply(0)),
          )
        });
      for number in start + 1..=start + 15 {
        let nonce =
          TestExternalities::new(candidate.clone()).execute_with(|| System::account_nonce(&sender));
        let calls = if number == start + 1 {
          vec![transfer.clone()]
        } else {
          Vec::new()
        };
        let authored = author_complete_block_after_with_calls(
          candidate,
          &[],
          &parent,
          number,
          UserDemand::ActorOnly,
          &signer,
          nonce,
          &BTreeMap::new(),
          &calls,
        );
        if number == start + 1 {
          assert_previous_system_buffers_replaced(&authored, &preset);
        }
        let input = authored
          .inherent_data
          .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
          .unwrap()
          .unwrap();
        assert_eq!(input.validation_data.parent_head, HeadData(parent.encode()));
        let hash_prefix = polkadot_sdk::frame_system::BlockHash::<Runtime>::final_prefix();
        let observed = authored
          .execution_backend_keys
          .iter()
          .filter_map(|(_, key)| key.starts_with(&hash_prefix).then_some(key.clone()))
          .collect::<BTreeSet<_>>();
        let mut allowed = BTreeSet::from([
          polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(0),
          polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(number - 1),
        ]);
        if number > 4_097 {
          allowed.insert(
            polkadot_sdk::frame_system::BlockHash::<Runtime>::hashed_key_for(number - 4_097),
          );
        }
        assert!(
          observed.is_subset(&allowed),
          "System hash access must not scan its full window"
        );
        let sealed = assert_complete_block_replays_with_aura(&authored);
        if number == start + 1 {
          assert_previous_parachain_fields_replaced(&authored, &preset);
        }
        assert_eq!(
          service_storage_entries(&authored.post_state),
          service_entries
        );
        TestExternalities::new(authored.post_state.clone()).execute_with(|| {
          let first = number.saturating_sub(4_096).max(1);
          let keys = polkadot_sdk::frame_system::BlockHash::<Runtime>::iter_keys().collect::<BTreeSet<_>>();
          let expected = std::iter::once(0).chain(first..number).collect::<BTreeSet<_>>();
          assert_eq!(keys, expected);
          assert_eq!(System::block_hash(0), genesis);
          assert_eq!(System::block_hash(number - 1), parent.hash());
          if number > 4_097 { assert!(!polkadot_sdk::frame_system::BlockHash::<Runtime>::contains_key(number - 4_097)); }
          assert_eq!(parachain::ValidationData::<Runtime>::get().unwrap().encode(), input.validation_data.encode());
          assert_eq!(parachain::RelayStateProof::<Runtime>::get().unwrap().encode(), input.relay_chain_state.encode());
          assert_eq!(Balances::free_balance(&recipient), recipient_before + crate::EXISTENTIAL_DEPOSIT);
          assert_eq!(System::account_nonce(&sender), nonce_before + 1);
          assert_eq!((Balances::total_issuance(), Assets::total_supply(0)), issuance);
          assert!(!Actors::user_creation_enabled());
          assert!(!Actors::global_circuit_breaker());
          assert!(Actors::block_resource_state().is_none());
          let finalized = Actors::finalized_block_resource_telemetry().unwrap();
          assert_eq!(finalized.block_number(), number);
          assert!(!finalized.optional_actor_work_halted());
          assert_eq!(finalized.usage().actor_effect_used(), Weight::zero());
          assert_eq!(crate::Session::current_index(), number / crate::configs::Period::get());
          assert_eq!(polkadot_sdk::pallet_timestamp::Now::<Runtime>::get(), u64::from(number) * crate::SLOT_DURATION);
          quiet_consensus_projection(number, start);
          #[cfg(feature = "try-runtime")]
          <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
          println!("Full-window successor: preset={preset} start={start} block={number} hashes={} read_keys={} genesis_preserved=true parent_link=true pruning_exact=true custody_and_issuance=true fresh_inherent_values=true finalized_no_halt=true header_bytes={}", keys.len(), observed.len(), sealed.encoded_size());
        });
        parent = sealed;
        candidate = authored.post_state;
      }
      println!(
        "Full-window canary completed: preset={preset} start={start} end={} blocks=15 signed_calls=1 opaque_history=true conditional_parent=true",
        start + 15
      );
    }
  }
}

/// Empty-gap preparation is conditional; every selected transition and cleanup block is real
/// Executive execution. Pending authorization and terminal retention are never fabricated.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn clock_prepared_signed_parked_setup_overlaps_governance_history_cleanup() {
  let preset = polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.to_owned();
  clock_prepared_signed_parked_setup_with_pvf(
    &[],
    None,
    Some(&preset),
    SignedParkedReplayEnd::CleanupOverlap,
  );
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn clock_prepared_signed_parked_setup_overlaps_governance_history_cleanup_local_testnet() {
  let preset = polkadot_sdk::sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET.to_owned();
  clock_prepared_signed_parked_setup_with_pvf(
    &[],
    None,
    Some(&preset),
    SignedParkedReplayEnd::CleanupOverlap,
  );
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn clock_prepared_signed_parked_setup_with_pvf(
  wasm: &[u8],
  pvf_output: Option<&std::path::Path>,
  selected_preset: Option<&polkadot_sdk::sp_genesis_builder::PresetId>,
  end: SignedParkedReplayEnd,
) {
  use pallet_deos_actors::*;
  use pallet_governance::{EpochServicePhase, ProposalEnactmentOutcome, ProposalVoteKind};
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  fn advance(
    storage: &mut Storage,
    parent: &mut Header,
    signer: &sr25519::Pair,
    calls: &[RuntimeCall],
    pvf_output: Option<(
      &polkadot_sdk::sp_genesis_builder::PresetId,
      &std::path::Path,
    )>,
  ) -> FullExecutiveBlockMetrics {
    use polkadot_sdk::sp_runtime::traits::Convert;
    type Update =
      <Runtime as polkadot_sdk::pallet_transaction_payment::Config>::FeeMultiplierUpdate;
    let (before_multiplier, nonce) = {
      let mut input = externalities_with_bound_code(storage.clone());
      assert_construction_parent_matches_state(&mut input, parent);
      input.execute_with(|| {
        (
          polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get(),
          System::account_nonce(crate::AccountId::from(signer.public())),
        )
      })
    };
    let number = parent.number().checked_add(1).unwrap();
    let script = calls
      .iter()
      .enumerate()
      .map(|(index, call)| (signer, nonce + index as u32, call))
      .collect::<Vec<_>>();
    let code = storage.top[well_known_keys::CODE].clone();
    let authored = author_complete_block_after_with_context(
      core::mem::take(storage),
      &code,
      parent,
      number,
      UserDemand::ActorOnly,
      signer,
      nonce,
      &BTreeMap::new(),
      &script,
      inherent_data_for(parent, number),
    );
    let sealed_header = assert_complete_block_replays_with_aura(&authored);
    externalities_with_bound_code(authored.post_state.clone()).execute_with(|| {
      let after_multiplier = polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get();
      assert_eq!(after_multiplier, Update::convert(before_multiplier));
      if calls.is_empty() {
        assert_eq!(*System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal), Weight::zero());
      }
      println!("Complete fee recurrence: block={number} calls={} before={} after={} normal_ref={} normal_proof={} author_import_equal=true", calls.len(), before_multiplier.into_inner(), after_multiplier.into_inner(), System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal).ref_time(), System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal).proof_size());
      assert!(Actors::block_resource_state().is_none());
      assert!(
        !Actors::finalized_block_resource_telemetry()
          .unwrap()
          .optional_actor_work_halted()
      );
      #[cfg(feature = "try-runtime")]
      <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
    });
    export_complete_native_pvf(&authored, &sealed_header, pvf_output);
    *storage = authored.post_state;
    *parent = sealed_header;
    authored.metrics
  }
  fn finish_prepared_quiet_gap(
    storage: &mut Storage,
    parent: &mut Header,
    to: u32,
    signer: &sr25519::Pair,
    pvf_output: Option<(
      &polkadot_sdk::sp_genesis_builder::PresetId,
      &std::path::Path,
    )>,
  ) {
    use polkadot_sdk::cumulus_pallet_parachain_system as parachain;
    let from = *parent.number();
    let start = to.checked_sub(10).unwrap();
    assert!(start > from);
    assert_eq!((start - from) % 10, 0);
    let mut ext = externalities_with_bound_code(core::mem::take(storage));
    ext.execute_with(|| translate_empty_gap(from, start));
    *storage = ext.execute_with(current_top_storage);
    let code = storage.top[well_known_keys::CODE].clone();
    *parent = parent_header_for(storage.clone(), &code, start);
    // Only the first predecessor is projected; the ten successors execute and import normally.
    for number in start + 1..=to {
      let previous = parent.clone();
      let expected = inherent_data_for(&previous, number)
        .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
        .unwrap()
        .unwrap();
      let metrics = advance(storage, parent, signer, &[], pvf_output);
      assert_eq!(metrics.user_calls, 0);
      assert_eq!(metrics.actor_effect, Weight::zero());
      assert_eq!(*parent.parent_hash(), previous.hash());
      let mut post = externalities_with_bound_code(storage.clone());
      assert_construction_parent_matches_state(&mut post, parent);
      post.execute_with(|| {
        assert_eq!(System::parent_hash(), previous.hash());
        assert_eq!(System::block_hash(number - 1), previous.hash());
        assert_eq!(
          parachain::ValidationData::<Runtime>::get(),
          Some(expected.validation_data.clone())
        );
        assert_eq!(
          parachain::RelayStateProof::<Runtime>::get(),
          Some(expected.relay_chain_state.clone())
        );
        assert_eq!(
          polkadot_sdk::cumulus_primitives_core::rpsr_digest::extract_relay_parent_storage_root(
            parent.digest()
          ),
          Some((expected.validation_data.relay_parent_storage_root, number))
        );
        assert_eq!(
          u64::from(polkadot_sdk::pallet_aura::CurrentSlot::<Runtime>::get()),
          u64::from(number)
        );
        assert_eq!(
          polkadot_sdk::pallet_timestamp::Now::<Runtime>::get(),
          u64::from(number) * crate::SLOT_DURATION
        );
      });
      println!(
        "Executed quiet gap parent: block={number} predecessor={} parent_hash_and_system_hash_equal=true canonical_window=true code_root_number=true validation_and_relay_proof_equal=true rpsr_and_clock_equal=true aura_sealed_import=true historical_gap_conditional=true",
        previous.number()
      );
    }
    assert_eq!(*parent.number(), to);
    println!(
      "Prepared gap finished by execution: from={from} projected_to={start} end={to} actual_blocks=10 zero_normal=true zero_actor_effect=true current_parent_executed=true literal_history=false"
    );
  }
  fn report_quiet_runtime_delta(
    from: u32,
    to: u32,
    phase: &str,
    before: &Storage,
    after: &Storage,
  ) {
    use polkadot_sdk::sp_core::hexdisplay::HexDisplay;
    let storage_info = runtime_proof_storage_info();
    assert!(matches!(phase, "cadence" | "session"));
    assert_eq!(before.children_default, after.children_default);
    assert_eq!(from % 10, 3);
    assert_eq!(to % 10, 3);
    let normalized_events = |storage: &Storage, number: u32| {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        let mut events = System::events();
        assert_eq!(System::event_count(), u32::try_from(events.len()).unwrap());
        let raw = polkadot_sdk::sp_io::storage::get(
          &polkadot_sdk::frame_support::storage::storage_prefix(b"System", b"Events"),
        )
        .unwrap();
        assert_eq!(raw.as_ref(), events.encode().as_slice());
        let mut completions = 0;
        let mut skips = 0;
        for record in &mut events {
          let (actor, nonce) = match &mut record.event {
            RuntimeEvent::Actors(Event::StepSkipped {
              actor_id,
              cycle_nonce,
              step_index,
              reason,
            }) => {
              assert_eq!(*step_index, 1);
              assert_eq!(*reason, StepSkippedReason::ResolutionSkipped);
              skips += 1;
              (actor_id, cycle_nonce)
            }
            RuntimeEvent::Actors(Event::CycleSummary {
              actor_id,
              cycle_nonce,
              result,
              outcomes,
            }) => {
              assert_eq!(*result, CycleResult::Completed);
              assert_eq!(
                *outcomes,
                OutcomeTotals {
                  precondition_skips: 1,
                  skipped_resolution: 1,
                  ..Default::default()
                }
              );
              completions += 1;
              (actor_id, cycle_nonce)
            }
            RuntimeEvent::Actors(event) => panic!("unexpected quiet completion event: {event:?}"),
            _ => continue,
          };
          assert_eq!(*actor, primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
          assert_eq!(*nonce, u64::from((number - 3) / 10));
          *nonce = nonce.checked_sub(u64::from((number - from) / 10)).unwrap();
        }
        assert_eq!((skips, completions), (1, 1));
        events
      })
    };
    assert_eq!(
      normalized_events(before, from),
      normalized_events(after, to)
    );
    let mutable_owners = BTreeSet::from([
      "Aura.CurrentSlot",
      "AuraExt.RelaySlotInfo",
      "CollatorSelection.LastAuthoredBlock",
      "Governance.LastProcessedEpoch",
      "ParachainSystem.AggregatedUnincludedSegment",
      "ParachainSystem.HrmpWatermark",
      "ParachainSystem.LastProcessedHrmpMessage",
      "ParachainSystem.LastRelayChainBlockNumber",
      "ParachainSystem.PoVMessagesTracker",
      "ParachainSystem.RelayStateProof",
      "ParachainSystem.ValidationData",
      "System.BlockHash",
      "System.BlockWeight",
      "System.Digest",
      "System.Events",
      "System.Number",
      "System.ParentHash",
      "Timestamp.Now",
      "TransactionPayment.NextFeeMultiplier",
    ]);
    let keys = before
      .top
      .keys()
      .chain(after.top.keys())
      .collect::<BTreeSet<_>>();
    let mut owners = BTreeMap::<String, [usize; 8]>::new();
    let mut changes = Vec::new();
    for key in keys {
      let mut matching = storage_info
        .iter()
        .filter(|info| key.starts_with(&info.prefix));
      let owner = matching
        .next()
        .map(|info| {
          format!(
            "{}.{}",
            String::from_utf8_lossy(&info.pallet_name),
            String::from_utf8_lossy(&info.storage_name)
          )
        })
        .unwrap_or_else(|| format!("UnmappedTop.{}", HexDisplay::from(key)));
      assert!(matching.next().is_none(), "one owner per storage prefix");
      let old = before.top.get(key);
      let new = after.top.get(key);
      let row = owners.entry(owner.clone()).or_default();
      row[0] += usize::from(old.is_some());
      row[1] += usize::from(new.is_some());
      row[6] += old.map_or(0, Vec::len);
      row[7] += new.map_or(0, Vec::len);
      if old == new {
        row[2] += 1;
      } else {
        // Complete Actor/consensus projections separately validate their typed successors.
        // Every other changed owner must belong to this closed quiet caller.
        assert!(
          owner.starts_with("Actors.")
            || mutable_owners.contains(owner.as_str())
            || (phase == "session"
              && matches!(
                owner.as_str(),
                "Session.CurrentIndex" | "Session.QueuedChanged"
              )),
          "quiet {phase} changed undeclared owner {owner} at {key:?} from {from} to {to}"
        );
        match (old, new) {
          (None, Some(_)) => row[3] += 1,
          (Some(_), None) => row[4] += 1,
          (Some(_), Some(_)) => row[5] += 1,
          (None, None) => unreachable!("key came from the union"),
        }
        changes.push(serde_json::json!({
          "owner": owner,
          "key": format!("{}", HexDisplay::from(key)),
          "before": old.map(|value| format!("{}", HexDisplay::from(value))),
          "after": new.map(|value| format!("{}", HexDisplay::from(value))),
        }));
      }
    }
    println!(
      "Quiet runtime delta: {}",
      serde_json::json!({
        "from": from, "to": to, "phase": phase,
        "blockHashCount": <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get(),
        "beforeKeys": before.top.len(), "afterKeys": after.top.len(),
        "beforeChildren": before.children_default.len(), "afterChildren": after.children_default.len(),
        "allStaticOwnerBytesEqual": true, "childrenExactEqual": true,
        "actorAndDynamicOwnerSuccessors": "independently checked by the surrounding complete-block projections",
        "ownerFields": ["beforeKeys", "afterKeys", "same", "added", "removed", "changed", "beforeValueBytes", "afterValueBytes"],
        "owners": owners, "changes": changes,
      })
    );
  }
  fn prepare_empty_gap(
    storage: &mut Storage,
    parent: &mut Header,
    to: u32,
    signer: &sr25519::Pair,
    pvf_output: Option<(
      &polkadot_sdk::sp_genesis_builder::PresetId,
      &std::path::Path,
    )>,
  ) {
    let from = *parent.number();
    let delta = to.checked_sub(from).unwrap();
    assert_eq!(from % 10, 3);
    assert_eq!(delta % 10, 0);
    let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let mut ext = externalities_with_bound_code(core::mem::take(storage));
    ext.execute_with(|| {
      assert_eq!(System::block_number(), from);
      assert!(!Actors::user_creation_enabled());
      assert!(Actors::block_resource_state().is_none());
      assert_eq!(crate::Session::current_index(), from / crate::configs::Period::get());
      assert_eq!(polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::get(), crate::Session::current_index() > 0);
      assert!(polkadot_sdk::pallet_session::DisabledValidators::<Runtime>::get().is_empty());
      assert_eq!(crate::Staking::native_security_mode(), pallet_staking::NativeSecurityMode::TrustedSet);
      assert!(pallet_staking::NativeSecurityRewardPots::<Runtime>::iter_keys().next().is_none());
      assert!(pallet_staking::NativeSecurityEpochSnapshots::<Runtime>::iter_keys().next().is_none());
      assert!(crate::Staking::active_native_security_epoch_snapshot().is_none());
      assert_eq!(crate::Staking::native_security_reward_liability(), 0);
      assert_eq!(Balances::free_balance(crate::CollatorSelection::account_id()), 0);
      assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), EpochServicePhase::Maturing);
      for (epoch, bucket) in pallet_governance::ProposalMaturityBuckets::<Runtime>::iter() { assert!(bucket.is_empty() || epoch > to); }
      for (epoch, bucket) in pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter() { assert!(bucket.is_empty() || epoch > to); }
      for (epoch, bucket) in pallet_governance::ExpiryBuckets::<Runtime>::iter() { assert!(bucket.is_empty() || epoch > to); }
      for touch in pallet_governance::PendingEnactments::<Runtime>::get() {
        assert!(pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(touch.domain, touch.item_id).unwrap() > to);
      }
      // This closed recipe has at most one undue pending member; its rotation is stable.
      let pending_members = pallet_governance::PendingEnactments::<Runtime>::get().len();
      assert!(pending_members <= 1);
      assert_eq!(pallet_governance::PendingEnactmentCursor::<Runtime>::get(), 0);
      if pending_members == 1 { assert!(pallet_governance::PendingEnactmentCursor::<Runtime>::exists()); }
      assert_eq!(crate::configs::governance_config::MaxGovernanceEpochCatchUpPerBlock::get(), 1);
      let previous_cursor = pallet_governance::LastProcessedEpoch::<Runtime>::get();
      assert!(previous_cursor <= from);
      let system_prefix = polkadot_sdk::sp_io::hashing::twox_128(b"System");
      let cursor_key = pallet_governance::LastProcessedEpoch::<Runtime>::hashed_key();
      let preserved = || {
        let mut top = current_top_storage().top;
        top.retain(|key, _| !key.starts_with(&system_prefix) && key != &cursor_key);
        (top, polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>())
      };
      let before = preserved();
      let original = current_top_storage();
      for probe_number in [from.checked_add(1).unwrap(), to] {
        externalities_with_bound_code(original.clone()).execute_with(|| {
          System::set_block_number(probe_number);
          <crate::Governance as polkadot_sdk::frame_support::traits::Hooks<u32>>::on_initialize(probe_number);
          assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), previous_cursor.checked_add(1).unwrap());
          assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), EpochServicePhase::Maturing);
          assert_eq!(preserved(), before);
        });
      }
      assert_eq!(System::block_number(), from);
      assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), previous_cursor);
      println!("Governance empty-gap preservation: from={from} to={to} pending_members={pending_members} one_step_cursor={} one_jump_cursor={} projected_cursor={} all_non_system_state_unchanged=true all_account_records_unchanged=true", previous_cursor + 1, previous_cursor + 1, previous_cursor + delta);
      // These steady-state premises select one recurring cadence branch without allocator drift.
      assert!(from >= 13 && to <= 187_203);
      let quiet = Actors::active_actor_state(sink).unwrap();
      assert_eq!(quiet.identity.actor_class.actor_type(), ActorType::System);
      assert_eq!(quiet.identity.cycle_nonce, u64::from((from - 3) / 10));
      assert_eq!(quiet.hot.last_cycle_block, Some(from));
      assert_eq!(quiet.hot.unsuccessful_attempt_streak, 0);
      assert!(!quiet.hot.lifecycle.is_paused());
      assert!(quiet.run_state.is_none());
      assert_eq!(quiet.contract.trigger, Trigger::cadenced(120));
      assert_eq!(quiet.contract.cooldown_blocks, 0);
      assert!(quiet.contract.window.is_none());
      assert!(quiet.contract.close_when.is_none());
      assert!(quiet.contract.parked_balance_activation.is_none());
      assert!(quiet.contract.auto_close_at_cycle_nonce.is_none());
      assert_eq!(quiet.contract.on_unfunded, OnUnfunded::Close);
      assert_eq!(quiet.contract.steps, crate::configs::actor_config::TmctolGenesisSystemActors::build_fee_sink_contract_steps());
      assert_eq!(quiet.hot.trigger_runtime_state, TriggerRuntimeState::Cadenced { anchor_tick: Some(12) });
      let predicate = &quiet.contract.steps[0].precondition.as_ref().unwrap().clauses[0][0];
      assert_eq!(predicate, &Predicate::BalanceAbove { asset: primitives::AssetKind::Native, threshold: 20_999_999_999 });
      let visible = <crate::configs::actor_config::TmctolAssetOps as AssetOps<crate::AccountId, primitives::AssetKind, crate::Balance>>::balance(&quiet.identity.sovereign_account, primitives::AssetKind::Native);
      assert!(visible <= 18_898_261_536);
      assert!(!ActorStateHolds::<Runtime>::contains_key(sink));
      assert!(!ActorFinancialWaits::<Runtime>::contains_key(sink));
      let process = ActorProcesses::<Runtime>::get(sink).unwrap();
      assert_eq!(process.last_attempted, Some(from));
      assert_eq!(process.residence, None);
      assert!(matches!(process.status, ProcessStatus::Disabled(disablement)
        if disablement.cause == ProcessDisableCause::Protocol
          && disablement.revival_authority == ProcessRevivalAuthority::Protocol
          && disablement.basis == SuspendedProcessBasis::Idle));
      assert!(ServiceNodes::<Runtime>::iter().next().is_none());
      assert_eq!(ServiceHeader::<Runtime>::get().count, 0);
      assert_eq!(ServiceHeader::<Runtime>::get().round_block, Some(from));
      let key = WakeupKey::Tick(12 * u64::from(from + 8));
      let handle = TriggerDeadlineHandles::<Runtime>::get(sink).unwrap();
      assert_eq!(handle.key, key);
      assert_eq!((handle.page, handle.slot), (0, 0));
      let header = DeadlineHeaders::<Runtime>::get(key).unwrap();
      assert_eq!((header.first_page, header.last_page, header.next_page, header.page_count, header.count), (0, 0, 1, 1, 1));
      let page = DeadlinePages::<Runtime>::get(key, 0).unwrap();
      assert_eq!(page.live_entries, 1);
      assert_eq!(page.entries[0], Some(handle.actor));
      assert!(page.entries[1..].iter().all(Option::is_none));
      assert!(page.previous_page.is_none() && page.next_page.is_none());
      assert_eq!(DeadlineIndexLen::<Runtime>::get(WakeupClock::Tick), 1);
      assert_eq!(DeadlineIndexLen::<Runtime>::get(WakeupClock::Block), 0);
      assert_eq!(DeadlineIndexPositions::<Runtime>::get(key), Some(0));
      assert_eq!(DeadlineIndexPages::<Runtime>::get(WakeupClock::Tick, 0).unwrap().as_slice(), &[key]);
      assert!(PrepassExecutionCutoff::<Runtime>::get().is_none());
      assert_eq!(IdleStarvationState::<Runtime>::get(), IdleStarvationPhase::Healthy);
      assert_eq!(crate::SLOT_DURATION, 6_000);
      assert_eq!(crate::configs::actor_config::ActorCadenceTickMillis::get(), 500);
      assert_eq!(polkadot_sdk::pallet_timestamp::Now::<Runtime>::get(), 6_000 * u64::from(from));
      println!("Actors quiet-clock premises: from={from} nonce={} last_cycle={} next_due_tick={} visible_native={visible} passive_idle=true service_empty=true singleton_page=0 next_page=1 cutoff_absent=true", quiet.identity.cycle_nonce, from, 12 * u64::from(from + 8));
      // Exercise a complete cadence period on a disposable prefix before projecting its clock.
      let custody = || (
        Balances::total_issuance(),
        Assets::total_supply(0),
        polkadot_sdk::frame_system::Account::<Runtime>::iter().collect::<BTreeMap<_, _>>(),
        polkadot_sdk::pallet_assets::Account::<Runtime>::iter()
          .map(|(asset, account, value)| ((asset, account), value))
          .collect::<BTreeMap<_, _>>(),
      );
      let before_actors = complete_quiet_actor_projection(from, from);
      let before_custody = custody();
      let before_consensus = quiet_consensus_projection(from, from);
      println!("Quiet consensus premises: from={from} aura_slot={from} relay_slot={from} authored_in_slot=1 last_relay_block={from} authors=2 raw_entries={} normalized_bytes={} author_cache_absent=true authority_mirror_equal=true", before_consensus.len(), before_consensus.encode().len());
      let steady_session = || {
        let prefixes = [b"Session".as_slice(), b"Staking", b"CollatorSelection"]
          .map(polkadot_sdk::sp_io::hashing::twox_128);
        let index_key = polkadot_sdk::pallet_session::CurrentIndex::<Runtime>::hashed_key();
        let changed_key = polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::hashed_key();
        let authored_prefix = polkadot_sdk::pallet_collator_selection::LastAuthoredBlock::<Runtime>::final_prefix();
        let mut top = current_top_storage().top;
        top.retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix))
          && key != &index_key && key != &changed_key && !key.starts_with(&authored_prefix));
        (top, polkadot_sdk::pallet_aura::Authorities::<Runtime>::get())
      };
      let before_session = steady_session();
      let session_keys = polkadot_sdk::pallet_session::QueuedKeys::<Runtime>::get();
      assert_eq!(session_keys.iter().map(|(account, _)| account.clone()).collect::<Vec<_>>(),
        polkadot_sdk::pallet_session::Validators::<Runtime>::get());
      assert_eq!(session_keys.iter().map(|(_, keys)| keys.aura.clone()).collect::<Vec<_>>(),
        polkadot_sdk::pallet_aura::Authorities::<Runtime>::get().into_inner());
      for (account, keys) in &session_keys {
        assert_eq!(polkadot_sdk::pallet_session::NextKeys::<Runtime>::get(account), Some(keys.clone()));
      }
      println!("Trusted Session gap premises: from={from} index={} queued_changed={} validators={} security_obligations_empty=true collator_pot=0 steady_bytes={}", crate::Session::current_index(), polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::get(), session_keys.len(), before_session.encode().len());
      let mut probe_storage = original.clone();
      let mut probe_parent = parent.clone();
      assert!(from.checked_add(10).unwrap() < to);
      for number in from + 1..=from + 10 {
        advance(&mut probe_storage, &mut probe_parent, signer, &[], None);
        externalities_with_bound_code(probe_storage.clone()).execute_with(|| {
          assert_eq!(custody(), before_custody);
          assert_eq!(Actors::finalized_block_resource_telemetry().unwrap().usage().actor_effect_used(), Weight::zero());
          assert_eq!(polkadot_sdk::pallet_timestamp::Now::<Runtime>::get(), u64::from(number) * crate::SLOT_DURATION);
          quiet_consensus_projection(number, from);
        });
      }
      report_quiet_runtime_delta(from, from + 10, "cadence", &original, &probe_storage);
      externalities_with_bound_code(probe_storage).execute_with(|| {
        let after_actors = complete_quiet_actor_projection(from + 10, from);
        for (role, value) in &after_actors {
          assert_eq!(value, &before_actors[role], "quiet gap period changed {role} from {from}");
        }
        assert_eq!(quiet_consensus_projection(from + 10, from), before_consensus);
      });
      assert_eq!(current_top_storage().top, original.top);
      println!("Actors empty-gap period: from={from} to={} complete_blocks=10 all_actor_storage_normalized_equal=true all_custody_unchanged=true zero_effects=true original_prefix_unchanged=true", from + 10);
      println!("Quiet consensus period: from={from} to={} complete_blocks=10 all_normalized_consensus_state_equal=true", from + 10);
      // Session is the only scheduled quiet-prefix spike; its boundary precedes cadence readiness.
      let period = crate::configs::Period::get();
      assert_eq!(period, 3_600);
      assert_eq!(period % 10, 0);
      let boundary = (from / period + 1) * period;
      assert!(boundary + 3 < to);
      let session_start = boundary - 7;
      let mut session_storage = original.clone();
      let mut session_parent = parent.clone();
      finish_prepared_quiet_gap(&mut session_storage, &mut session_parent, session_start, signer, None);
      let session_before = session_storage.clone();
      let session_consensus = externalities_with_bound_code(session_storage.clone()).execute_with(|| quiet_consensus_projection(session_start, session_start));
      let (session_actors, session_index) = externalities_with_bound_code(session_storage.clone()).execute_with(|| (
        complete_quiet_actor_projection(session_start, session_start),
        crate::Session::current_index(),
      ));
      type W = <Runtime as pallet_deos_actors::Config>::WeightInfo;
      let fixed = W::scheduler_on_idle_base().saturating_add(W::block_resource_finalize());
      let reserve = <Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get();
      for number in session_start + 1..=boundary + 3 {
        let metrics = advance(&mut session_storage, &mut session_parent, signer, &[], None);
        let frame_after_xcmp = metrics.pre_idle_remaining.checked_sub(&crate::XcmpQueue::on_idle_weight()).unwrap();
        let residual_control = BlockResourceBudgetValue::get().limits().actor_control().checked_sub(&metrics.prepass_actor_control).unwrap();
        let available = Weight::from_parts(
          frame_after_xcmp.ref_time().min(reserve.ref_time()).min(residual_control.ref_time()),
          frame_after_xcmp.proof_size().min(reserve.proof_size()).min(residual_control.proof_size()),
        );
        assert!(fixed.all_lte(available));
        assert_eq!(metrics.actor_effect, Weight::zero());
        assert_eq!(metrics.user_calls, 0);
        externalities_with_bound_code(session_storage.clone()).execute_with(|| {
          assert_eq!(custody(), before_custody);
          assert_eq!(crate::Session::current_index(), session_index + u32::from(number >= boundary));
          assert_eq!(crate::Session::current_index(), number / period);
          assert_eq!(polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::get(), number >= period);
          assert_eq!(steady_session(), before_session);
          quiet_consensus_projection(number, session_start);
          assert!(!Actors::user_creation_enabled());
          assert_eq!(*System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal), Weight::zero());
        });
        let phase = match number - session_start {
          7 => "session",
          8 => "readiness",
          9 => "opening",
          10 => "completion",
          _ => "quiet",
        };
        println!("Quiet Session resource: from={from} block={number} boundary={boundary} phase={phase} frame_remaining={:?} prepass_control={:?} housekeeping_available={available:?} housekeeping_required={fixed:?} custody_unchanged=true zero_effects=true", metrics.pre_idle_remaining, metrics.prepass_actor_control);
      }
      report_quiet_runtime_delta(session_start, boundary + 3, "session", &session_before, &session_storage);
      externalities_with_bound_code(session_storage).execute_with(|| {
        assert_eq!(complete_quiet_actor_projection(boundary + 3, session_start), session_actors);
        assert_eq!(quiet_consensus_projection(boundary + 3, session_start), session_consensus);
      });
      assert_eq!(current_top_storage().top, original.top);
      println!("Quiet Session period: from={from} start={session_start} end={} boundary={boundary} complete_blocks=10 all_actor_storage_normalized_equal=true all_custody_unchanged=true original_prefix_unchanged=true", boundary + 3);
      println!("Trusted Session period: from={from} start_index={session_index} final_index={} queued_changed=true all_steady_session_staking_collator_state_equal=true authority_keys_equal=true", (boundary + 3) / period);
      println!("Quiet consensus Session period: from={from} start={session_start} end={} complete_blocks=10 all_normalized_consensus_state_equal=true", boundary + 3);
    });
    *storage = ext.execute_with(current_top_storage);
    finish_prepared_quiet_gap(storage, parent, to, signer, pvf_output);
  }
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    if selected_preset.is_some_and(|selected| selected != &preset) {
      continue;
    }
    let pvf = pvf_output.map(|output| (&preset, output));
    let (mut storage, mut parent) = replay_signed_l2_admission_with_pvf(
      &signer,
      &preset,
      0,
      SignedAdmissionReplayEnd::SealedPreparation,
      wasm,
      pvf_output,
    );
    for _ in 5..=13 {
      advance(&mut storage, &mut parent, &signer, &[], pvf);
    }
    prepare_empty_gap(&mut storage, &mut parent, 43_203, &signer, pvf);
    advance(
      &mut storage,
      &mut parent,
      &signer,
      &[RuntimeCall::Governance(
        pallet_governance::Call::cast_vote {
          domain: 0,
          item_id: 120,
          vote: ProposalVoteKind::Aye,
        },
      )],
      pvf,
    );
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      assert!(
        crate::Governance::proposal_vote_tally(0, 120)
          .unwrap()
          .aye_weight
          >= 200
      );
    });
    for _ in 43_205..=43_213 {
      advance(&mut storage, &mut parent, &signer, &[], pvf);
    }
    prepare_empty_gap(&mut storage, &mut parent, 144_003, &signer, pvf);
    advance(&mut storage, &mut parent, &signer, &[], pvf);
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      assert_eq!(
        crate::Governance::finalized_proposal_outcome(0, 120),
        Some(super::governance_integration_tests::approved_outcome(
          144_004,
          1,
          ProposalEnactmentOutcome::NotAttempted
        ))
      );
      assert_eq!(
        pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(0, 120),
        Some(187_204)
      );
    });
    for _ in 144_005..=144_013 {
      advance(&mut storage, &mut parent, &signer, &[], pvf);
    }
    prepare_empty_gap(&mut storage, &mut parent, 187_203, &signer, pvf);
    advance(&mut storage, &mut parent, &signer, &[], pvf);
    assert_eq!(*parent.number(), 187_204);
    externalities_with_bound_code(storage).execute_with(|| {
      assert_eq!(
        crate::Governance::finalized_proposal_outcome(0, 120),
        Some(super::governance_integration_tests::approved_outcome(
          144_004,
          1,
          ProposalEnactmentOutcome::Enacted { epoch: 187_204 }
        ))
      );
      assert_eq!(
        pallet_governance::LastProcessedEpoch::<Runtime>::get(),
        187_203
      );
      assert_eq!(
        crate::Governance::finalized_proposal_outcome_expiry_bucket(187_220).len(),
        1
      );
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      assert!(Actors::user_creation_enabled());
      assert_eq!(
        System::account_nonce(crate::AccountId::from(signer.public())),
        5
      );
      assert_eq!(crate::Session::current_index(), 52);
      assert!(polkadot_sdk::pallet_session::QueuedChanged::<Runtime>::get());
      assert_eq!(polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get().into_inner(), 100_000_000_000_000_000);
      println!("Prepared public admission fee endpoint: block=187204 next_multiplier=100000000000000000 minimum=true canonical_history_or_host_equivalence=false");
      let sink =
        Actors::active_actor_state(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID).unwrap();
      assert_eq!(sink.identity.cycle_nonce, 18_720);
      assert_eq!(sink.hot.last_cycle_block, Some(187_203));
      assert_eq!(sink.hot.trigger_wakeup_pointer.unwrap().tick, 2_246_532);
      replay_signed_parked_user_epoch_churn_with_pvf(
        &signer,
        parent,
        end,
        pvf,
      );
    });
    eprintln!("Clock-prepared signed construction reconciled: {preset}; literal_history=false");
  }
}

/// Export the complete finite construction; prepared clock gaps remain conditional.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires explicit diagnostic Wasm and a new PVF output directory"]
fn clock_prepared_signed_parked_setup_exports_pvf_fixtures() {
  let wasm =
    std::fs::read(std::env::var("DEOS_GOVERNANCE_PVF_WASM").expect("explicit Wasm path")).unwrap();
  assert!(!wasm.is_empty());
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_GOVERNANCE_PVF_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).expect("output directory must be new");
  clock_prepared_signed_parked_setup_with_pvf(
    &wasm,
    Some(&output),
    None,
    SignedParkedReplayEnd::Complete,
  );
}

/// Check the identical complete native route with bound code, without exporting another corpus.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires explicit diagnostic Wasm"]
fn clock_prepared_signed_parked_setup_uses_code_bound_parents() {
  let path = std::path::PathBuf::from(
    std::env::var("DEOS_GOVERNANCE_PVF_WASM").expect("explicit Wasm path"),
  );
  assert!(path.is_absolute());
  let wasm = std::fs::read(path).unwrap();
  assert!(!wasm.is_empty());
  clock_prepared_signed_parked_setup_with_pvf(&wasm, None, None, SignedParkedReplayEnd::Complete);
}

/// Retain actual signed Governance and Aura construction prefixes for one explicit preset.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires a preset, diagnostic Wasm and a fresh recorder output directory"]
fn retained_signed_governance_parked_recorder_selected_preset() {
  let preset = std::env::var("DEOS_FUNDING_PRESET").expect("explicit preset");
  assert!(crate::genesis_config_presets::preset_names().contains(&preset));
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_PROOF_PREFIX_OUTPUT").expect("fresh recorder output directory"),
  );
  assert!(output.is_dir());
  assert!(std::fs::read_dir(output).unwrap().next().is_none());
  let path = std::path::PathBuf::from(
    std::env::var("DEOS_GOVERNANCE_PVF_WASM").expect("explicit diagnostic Wasm"),
  );
  assert!(path.is_absolute());
  let wasm = std::fs::read(path).unwrap();
  assert!(!wasm.is_empty());
  clock_prepared_signed_parked_setup_with_pvf(
    &wasm,
    None,
    Some(&preset),
    SignedParkedReplayEnd::Complete,
  );
}

/// Export current initial signed call declarations from disposable quote clones.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires a fresh declaration output directory"]
fn retained_initial_signed_governance_dispatch_declarations() {
  use pallet_governance::{ProposalCadenceMode, ProposalPayloadKind, ProposalVoteKind};
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_ADMISSION_DECLARATION_OUTPUT").expect("fresh declaration output"),
  );
  assert!(output.is_absolute() && output.is_dir());
  assert!(std::fs::read_dir(&output).unwrap().next().is_none());
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let payload =
    RuntimeCall::Actors(pallet_deos_actors::Call::set_user_creation_enabled { enabled: true })
      .encode();
  let payload_hash = BlakeTwo256::hash(&payload);
  let calls = [
    (
      "stake",
      RuntimeCall::Staking(pallet_staking::Call::stake {
        asset_id: 0,
        amount: 1_000,
      }),
    ),
    (
      "note_preimage",
      RuntimeCall::Preimage(polkadot_sdk::pallet_preimage::Call::note_preimage {
        bytes: payload.clone(),
      }),
    ),
    (
      "prepare_witness",
      RuntimeCall::Governance(pallet_governance::Call::prepare_payload_admission_witness {
        domain: 0,
        payload_kind: ProposalPayloadKind::L2ParameterChange,
        payload_hash,
        payload: payload.clone().try_into().unwrap(),
      }),
    ),
    (
      "submit_proposal",
      RuntimeCall::Governance(pallet_governance::Call::submit_signed_proposal {
        domain: 0,
        item_id: 120,
        cadence_mode: ProposalCadenceMode::Ordinary,
        payload_kind: ProposalPayloadKind::L2ParameterChange,
        payload_hash,
      }),
    ),
    (
      "vote",
      RuntimeCall::Governance(pallet_governance::Call::cast_vote {
        domain: 0,
        item_id: 120,
        vote: ProposalVoteKind::Aye,
      }),
    ),
  ];
  let weight = |value: Weight| [value.ref_time(), value.proof_size()];
  for preset in crate::genesis_config_presets::preset_names() {
    let source = preset_genesis_storage(&[], &preset);
    let rows = calls
      .iter()
      .enumerate()
      .map(|(nonce, (name, call))| {
        // The signer helper prepares resource state only in this discarded clone.
        TestExternalities::new(source.clone()).execute_with(|| {
          let extrinsic =
            actors_integration_tests::signed_extrinsic(&signer, nonce as u32, call.clone());
          let info = extrinsic.get_dispatch_info();
          let length = extrinsic.encode().len() as u64;
          let base = crate::configs::RuntimeBlockWeights::get()
            .get(info.class)
            .base_extrinsic;
          serde_json::json!({
            "name": name, "nonce": nonce, "class": format!("{:?}", info.class),
            "call": weight(info.call_weight), "extension": weight(info.extension_weight),
            "total": weight(info.total_weight()), "base": weight(base),
            "length": length, "callLength": call.encode().len(),
            "frameCharge": weight(info.total_weight().saturating_add(base).saturating_add(Weight::from_parts(0, length))),
          })
        })
      })
      .collect::<Vec<_>>();
    let accounts = TestExternalities::new(source).execute_with(|| {
      serde_json::json!({
        "stakingPoolAccount": format!("{}", polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&crate::Staking::pool_account_for(0).encode())),
        "receiptAssetId": crate::Staking::staked_asset_id(0).unwrap(),
      })
    });
    let path = output.join(format!("{preset}.json"));
    assert!(!path.exists());
    std::fs::write(
      path,
      serde_json::to_vec_pretty(&serde_json::json!({
        "preset": preset, "payload": format!("{}", polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&payload)),
        "payloadLength": payload.len(), "calls": rows, "accounts": accounts,
        "scope": "Native declarations and encoded lengths; signer preparation occurs only in discarded clones; no call is dispatched.",
      }))
      .unwrap(),
    )
    .unwrap();
  }
}

/// Bind the financial seeds to current System, Session, Governance and message authority.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires exact retained TOP seed inputs and diagnostic Wasm"]
fn retained_parked_seeds_match_current_control_and_hook_bases() {
  use pallet_deos_actors::GenesisSystemActors;
  fn bytes(value: &serde_json::Value) -> Vec<u8> {
    let encoded = value.as_str().unwrap();
    assert!(encoded.len().is_multiple_of(2));
    (0..encoded.len())
      .step_by(2)
      .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
      .collect()
  }
  let inputs: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::env::var("DEOS_PARKED_CONTROL_SEED_INPUTS").unwrap()).unwrap(),
  )
  .unwrap();
  assert_eq!(inputs.as_array().unwrap().len(), 4);
  for seed in inputs.as_array().unwrap() {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    let digest = polkadot_sdk::sp_io::hashing::sha2_256(&raw)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    assert_eq!(digest, seed["sha256"].as_str().unwrap());
    let snapshot: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(snapshot["phase"], seed["phase"]);
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let before = storage.clone();
    let mut ext = externalities_with_bound_code(storage);
    ext.execute_with(|| {
      let block = snapshot["block"].as_u64().unwrap() as u32;
      assert!(matches!(block, 187_309 | 187_469));
      assert_eq!(System::block_number(), block);
      assert!(Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      assert!(Actors::block_resource_state().is_none());
      assert!(!polkadot_sdk::frame_support::storage::transactional::is_transactional());
      assert_eq!(crate::Staking::native_security_mode(), pallet_staking::NativeSecurityMode::TrustedSet);
      let expected = <Runtime as pallet_deos_actors::Config>::GenesisSystemActors::system_actors();
      let dormant = <Runtime as pallet_deos_actors::Config>::GenesisSystemActors::dormant_system_actors();
      assert_eq!(expected.iter().map(|row| row.0).collect::<Vec<_>>(), [0, 1, 10]);
      assert_eq!(expected.len() + dormant.len(), 15);
      for (actor, owner, mutability, contract) in expected {
        let view = Actors::active_actor_state(actor).unwrap();
        assert_eq!(view.identity.owner, owner);
        assert_eq!(view.identity.mutability, mutability);
        assert_eq!(view.identity.actor_class, pallet_deos_actors::ActorClass::System { sovereign_id: actor });
        assert_eq!(view.identity.sovereign_account, Actors::sovereign_account_id_system(actor));
        assert_eq!(view.contract, contract);
        assert_eq!(view.hot.cycle_state, CycleState::Idle);
        assert_eq!(view.hot.unsuccessful_attempt_streak, 0);
        assert!(view.hot.terminal_at.is_none());
        assert!(view.run_state.is_none());
        assert!(view.identity.cycle_nonce <= u64::from(block));
      }
      for (actor, owner, mutability) in dormant {
        let identity = Actors::actor_identity(actor).unwrap();
        assert_eq!(identity.owner, owner);
        assert_eq!(identity.mutability, mutability);
        assert_eq!(identity.actor_class, pallet_deos_actors::ActorClass::System { sovereign_id: actor });
        assert_eq!(identity.sovereign_account, Actors::sovereign_account_id_system(actor));
        assert_eq!(identity.cycle_nonce, 0);
        assert!(Actors::active_actor_state(actor).is_none());
        assert!(Actors::actor_contract(actor).is_none());
        assert!(pallet_deos_actors::ActorProcesses::<Runtime>::get(actor).is_none());
        assert!(pallet_deos_actors::ServiceNodes::<Runtime>::get(actor).is_none());
      }
      assert_eq!(Actors::system_sovereign_count(), 15);
      for account in crate::configs::actor_config::TmctolGenesisSystemActors::native_flow_anchor_accounts() {
        assert!(Balances::free_balance(account) >= crate::EXISTENTIAL_DEPOSIT);
      }
      let local = polkadot_sdk::pallet_assets::Asset::<Runtime>::get(0).unwrap();
      assert_eq!(local.min_balance, 1);
      assert!(local.is_sufficient);
      assert_eq!(local.status, polkadot_sdk::pallet_assets::AssetStatus::Live);
      assert!(Balances::total_issuance().checked_add(local.supply).unwrap() <= 14_987_979_576_889_010_688);
      let validators = crate::Session::validators();
      let invulnerables = polkadot_sdk::pallet_collator_selection::Invulnerables::<Runtime>::get().into_inner();
      assert_eq!(validators.iter().cloned().collect::<BTreeSet<_>>(), invulnerables.iter().cloned().collect());
      assert_eq!(validators.len(), 2);
      assert_eq!(polkadot_sdk::pallet_session::QueuedKeys::<Runtime>::get().iter().map(|row| row.0.clone()).collect::<BTreeSet<_>>(), validators.iter().cloned().collect());
      assert!(polkadot_sdk::pallet_session::DisabledValidators::<Runtime>::get().is_empty());
      assert_eq!(crate::Session::current_index(), block / crate::configs::Period::get());
      assert!(pallet_staking::NativeSecurityRewardPots::<Runtime>::iter_keys().next().is_none());
      assert!(pallet_staking::NativeSecurityEpochSnapshots::<Runtime>::iter_keys().next().is_none());
      assert!(crate::Staking::active_native_security_epoch_snapshot().is_none());
      assert_eq!(crate::Staking::native_security_reward_liability(), 0);
      assert_eq!(Balances::free_balance(crate::CollatorSelection::account_id()), 0);
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      assert!(pallet_governance::ActiveProposalIdsByDomain::<Runtime>::iter_values().all(|ids| ids.is_empty()));
      assert!(pallet_governance::ProposalMaturityBuckets::<Runtime>::iter_values().all(|bucket| bucket.is_empty()));
      assert!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter_values().all(|bucket| bucket.is_empty()));
      assert!(pallet_governance::ExpiryBuckets::<Runtime>::iter_values().all(|bucket| bucket.is_empty()));
      assert!(crate::Governance::proposal_metadata(0, 120).is_none());
      assert!(crate::Governance::finalized_proposal_outcome(0, 120).is_none());
      assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), block - 2);
      assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), pallet_governance::EpochServicePhase::Maturing);
      let information = <crate::AllPalletsWithSystem as StorageInfoTrait>::storage_info();
      for (pallet, item) in [
        (b"MessageQueue".as_slice(), b"BookStateFor".as_slice()),
        (b"MessageQueue".as_slice(), b"Pages".as_slice()),
        (b"MessageQueue".as_slice(), b"ServiceHead".as_slice()),
        (b"PolkadotXcm".as_slice(), b"CurrentMigration".as_slice()),
      ] {
        let info = information.iter().find(|info| info.pallet_name == pallet && info.storage_name == item).unwrap();
        assert!(polkadot_sdk::sp_io::storage::get(&info.prefix).is_none());
        assert!(polkadot_sdk::sp_io::storage::next_key(&info.prefix).is_none_or(|key| !key.starts_with(&info.prefix)));
      }
      assert_eq!(current_top_storage().top, before.top);
      println!("Retained control seed: preset={} phase={} block={block} system_identities=15 active_systems=3 dormant_systems=12 current_contracts_exact=true session_index={} validators=2 governance_lag=2 empty_message_queue=true hold_financial_seed_same=true all_top_state_read_only=true", seed["preset"].as_str().unwrap(), seed["phase"].as_str().unwrap(), crate::Session::current_index());
    });
  }
}

/// Replay the retained first cohort through the common four-source gap without preparing state.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires exact retained TOP seed inputs and diagnostic Wasm"]
fn retained_parked_seed_replays_common_gap_with_full_executive() {
  use codec::Decode;
  use pallet_deos_actors::{
    ActorContract, AssetOps, BalanceDependencySources, DependencyRegistrationFreeIndices,
    DependencyRegistrationFreePositions, DependencyRegistrationHeaders,
    DependencyRegistrationPages, DependencyRegistrationPosition, DependencyRevisions,
    DependencyScanSourceListState, DependencyScanSourceNodes, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule,
  };
  use polkadot_sdk::frame_support::traits::Get;
  use primitives::AssetKind;
  fn bytes(value: &serde_json::Value) -> Vec<u8> {
    let encoded = value.as_str().unwrap();
    assert!(encoded.len().is_multiple_of(2));
    (0..encoded.len())
      .step_by(2)
      .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
      .collect()
  }
  let inputs: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::env::var("DEOS_PARKED_SEED_INPUTS").expect("exact seed manifest")).unwrap(),
  )
  .unwrap();
  let seeds = inputs.as_array().unwrap();
  assert_eq!(seeds.len(), 2);
  let alice = sr25519::Pair::from_string("//Alice", None).unwrap();
  let bob = sr25519::Pair::from_string("//Bob", None).unwrap();
  let signers = [&alice, &bob];
  let assets = [0x7000_0001, 0x7000_0002, 0x7000_0003, 0x7000_0004];
  let contract = ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        assets
          .iter()
          .map(|asset| ParkedBalanceRule {
            asset: AssetKind::Local(*asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  for seed in seeds {
    let preset = seed["preset"].as_str().unwrap();
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    let digest = polkadot_sdk::sp_io::hashing::sha2_256(&raw)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    assert_eq!(digest, seed["sha256"].as_str().unwrap());
    let snapshot: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(snapshot["phase"], "first-cohort-holes");
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let encoded_header = bytes(&snapshot["header"]);
    let mut encoded = encoded_header.as_slice();
    let mut parent = Header::decode(&mut encoded).unwrap();
    assert!(encoded.is_empty());
    assert_eq!(*parent.number(), 187_469);
    let code = storage.top[well_known_keys::CODE].clone();
    assert_eq!(
      code,
      std::fs::read(std::env::var("DEOS_GOVERNANCE_PVF_WASM").unwrap()).unwrap()
    );
    let permanent = externalities_with_bound_code(storage.clone()).execute_with(|| {
      assert_eq!(Actors::next_actor_id(), 22);
      (
        Actors::actor_identity(15).unwrap(),
        Actors::actor_contract(15).unwrap(),
        Actors::actor_state_hold(15).unwrap(),
        ParkedBalanceEpisodes::<Runtime>::get(15).unwrap().encode(),
      )
    });
    let mut active = None;
    let mut first_handoffs = BTreeMap::new();
    let mut last_publications = [187_469u32; 4];
    let mut publications = 0u32;
    let mut maximum_gap = 0u32;
    for offset in 1..=165u32 {
      let now = 187_469 + offset;
      let mut calls = Vec::new();
      if offset <= 160 && (offset - 1).is_multiple_of(5) {
        if let Some(actor) = active {
          calls.push(RuntimeCall::Actors(
            pallet_deos_actors::Call::deactivate_actor { actor_id: actor },
          ));
        }
        let actor = 20 + u64::from(((offset - 1) / 5 + 31) % 2);
        calls.push(RuntimeCall::Actors(
          pallet_deos_actors::Call::activate_actor {
            actor_id: actor,
            contract: contract.clone(),
          },
        ));
        active = Some(actor);
      } else if offset == 161 {
        calls.push(RuntimeCall::Actors(
          pallet_deos_actors::Call::deactivate_actor {
            actor_id: active.take().unwrap(),
          },
        ));
      }
      let mut before = externalities_with_bound_code(storage.clone());
      assert_construction_parent_matches_state(&mut before, &parent);
      let (nonce, script, scans, selected, next_source, frontier, custody) =
        before.execute_with(|| {
          let script = calls
            .iter()
            .map(|call| {
              let actor = match call {
                RuntimeCall::Actors(
                  pallet_deos_actors::Call::activate_actor { actor_id, .. }
                  | pallet_deos_actors::Call::deactivate_actor { actor_id },
                ) => *actor_id,
                _ => unreachable!(),
              };
              let owner = Actors::actor_identity(actor).unwrap().owner;
              let signer = *signers
                .iter()
                .find(|pair| crate::AccountId::from(pair.public()) == owner)
                .unwrap();
              (signer, System::account_nonce(&owner), call)
            })
            .collect::<Vec<_>>();
          let scans = (0..4)
            .map(DependencyRevisions::<Runtime>::get)
            .collect::<Vec<_>>();
          let selected = DependencyScanSourceListState::<Runtime>::get()
            .cursor
            .unwrap();
          let next_source = DependencyScanSourceNodes::<Runtime>::get(selected)
            .unwrap()
            .next;
          let frontier = DependencyRegistrationHeaders::<Runtime>::get(selected).next_index;
          let custody = (0..4)
            .map(|source| {
              let account = Actors::actor_identity(16 + source)
                .unwrap()
                .sovereign_account;
              let asset = AssetKind::Local(assets[source as usize]);
              let native = crate::configs::actor_config::TmctolAssetOps::total_balance(
                &account,
                AssetKind::Native,
              );
              let available =
                crate::configs::actor_config::TmctolAssetOps::balance(&account, AssetKind::Native);
              assert!(native - available <= crate::EXISTENTIAL_DEPOSIT);
              assert!(
                available
                  >= 90_738_792 + <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
              );
              assert_ok!(
                crate::configs::actor_config::TmctolAssetOps::preflight_transfer(
                  &account,
                  &crate::AccountId::from(alice.public()),
                  asset,
                  1
                )
              );
              (
                native,
                crate::configs::actor_config::TmctolAssetOps::total_balance(&account, asset),
              )
            })
            .collect::<Vec<_>>();
          (
            System::account_nonce(&crate::AccountId::from(alice.public())),
            script,
            scans,
            selected,
            next_source,
            frontier,
            custody,
          )
        });
      drop(before);
      let authored = author_complete_block_after_with_context(
        storage,
        &code,
        &parent,
        now,
        UserDemand::ActorOnly,
        &alice,
        nonce,
        &BTreeMap::new(),
        &script,
        inherent_data_for(&parent, now),
      );
      let checked = assert_complete_block_replays_with_aura(&authored);
      assert_eq!(*checked.parent_hash(), parent.hash());
      let mut after = externalities_with_bound_code(authored.post_state.clone());
      assert_construction_parent_matches_state(&mut after, &checked);
      after.execute_with(|| {
        assert!(Actors::user_creation_enabled() && !Actors::global_circuit_breaker());
        assert!(Actors::block_resource_state().is_none());
        assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
        assert_eq!(DependencyScanSourceListState::<Runtime>::get().count, 4);
        assert_eq!(DependencyScanSourceListState::<Runtime>::get().cursor, Some(next_source));
        assert_eq!(Actors::next_actor_id(), 22);
        assert_eq!((Actors::actor_identity(15).unwrap(), Actors::actor_contract(15).unwrap(),
                    Actors::actor_state_hold(15).unwrap(), ParkedBalanceEpisodes::<Runtime>::get(15).unwrap().encode()), permanent);
        if calls.is_empty() {
          assert_eq!(*System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal), Weight::zero());
        }
        for source in 0..4u64 {
          let state = DependencyRevisions::<Runtime>::get(source);
          assert!(state.scan_target.is_some() && !state.exhausted);
          if offset <= 100 {
            let previous = scans[source as usize];
            let expected = if source != selected { previous.scan_cursor }
              else if previous.scan_cursor < previous.scan_end { previous.scan_cursor + 1 }
              else { 0 };
            assert_eq!(state.scan_cursor, expected);
            if source == selected && previous.scan_cursor == previous.scan_end {
              assert_eq!(state.scan_target, Some(previous.revision));
              assert_eq!(state.scan_end, frontier);
              first_handoffs.entry(source).or_insert(offset);
            } else {
              assert_eq!(state.scan_target, previous.scan_target);
              assert_eq!(state.scan_end, previous.scan_end);
            }
          }
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          assert_eq!(header.free_count, 31);
          assert_eq!(header.first_page, Some(0));
          for index in 0..31 {
            let position = DependencyRegistrationPosition { page: 0, slot: index as u8 + 1 };
            assert_eq!(DependencyRegistrationFreePositions::<Runtime>::get(source, index), Some(position));
            assert_eq!(DependencyRegistrationFreeIndices::<Runtime>::get(source, position), Some(index));
          }
          assert_eq!(BalanceDependencySources::<Runtime>::get(AssetKind::Local(assets[source as usize])), Some(source));
          let account = Actors::actor_identity(16 + source).unwrap().sovereign_account;
          let native = crate::configs::actor_config::TmctolAssetOps::total_balance(&account, AssetKind::Native);
          let lp = crate::configs::actor_config::TmctolAssetOps::total_balance(&account, AssetKind::Local(assets[source as usize]));
          assert!(custody[source as usize].0 - native <= 90_738_792);
          match custody[source as usize].1 - lp {
            0 => {}
            1 => {
              let gap = now - last_publications[source as usize];
              assert!(gap <= 32);
              maximum_gap = maximum_gap.max(gap);
              last_publications[source as usize] = now;
              publications += 1;
            }
            _ => panic!("producer must publish at most one unit per block"),
          }
          assert_eq!(crate::configs::actor_config::TmctolAssetOps::total_balance(&permanent.0.sovereign_account, AssetKind::Local(assets[source as usize])), 0);
        }
        assert_eq!(crate::configs::actor_config::TmctolAssetOps::total_balance(&permanent.0.sovereign_account, AssetKind::Native), 0);
        if offset == 100 {
          assert_eq!(first_handoffs, BTreeMap::from([(0, 37), (1, 91), (2, 100), (3, 2)]));
          println!("Retained seed Executive bootstrap: preset={preset} blocks=100 first_handoffs={first_handoffs:?} producer_transfers={publications} maximum_gap={maximum_gap} source_and_deadline_service_committed=true");
        }
        if offset == 165 {
          for source in 0..4 {
            let header = DependencyRegistrationHeaders::<Runtime>::get(source);
            assert_eq!((header.next_index, header.count, header.free_count), (64, 1, 31));
            assert!(DependencyRegistrationPages::<Runtime>::get(source, 1).is_none());
          }
          println!("Retained seed Executive common gap: preset={preset} blocks=165 epochs=32 signed_calls=64 producer_transfers={publications} maximum_gap={maximum_gap} frontier=64 original_holes_preserved=true");
        }
      });
      if offset.is_multiple_of(32) {
        println!("Retained seed Executive progress: preset={preset} offset={offset} block={now}");
      }
      parent = checked;
      storage = authored.post_state;
    }
  }
}

/// Active User births and reactivation preserve the closed native-account key domain.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires exact retained TOP seed inputs"]
fn retained_parked_birth_preserves_system_account_domain() {
  use pallet_deos_actors::{
    ActorContract, FundingSourcePolicy, Mutability, OnUnfunded, ParkedBalanceActivationOf,
    ParkedBalanceRule,
  };
  use primitives::AssetKind;
  fn bytes(value: &serde_json::Value) -> Vec<u8> {
    let encoded = value.as_str().unwrap();
    assert!(encoded.len().is_multiple_of(2));
    (0..encoded.len())
      .step_by(2)
      .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
      .collect()
  }
  let seeds: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::env::var("DEOS_PARKED_SEED_INPUTS").unwrap()).unwrap(),
  )
  .unwrap();
  assert_eq!(seeds.as_array().unwrap().len(), 4);
  let owner = crate::AccountId::from(
    sr25519::Pair::from_string("//Alice", None)
      .unwrap()
      .public(),
  );
  let contract = ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        (1..=4)
          .map(|asset| ParkedBalanceRule {
            asset: AssetKind::Local(0x7000_0000 + asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  let mut observations = Vec::new();
  for seed in seeds.as_array().unwrap() {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    let digest = polkadot_sdk::sp_io::hashing::sha2_256(&raw)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    assert_eq!(digest, seed["sha256"].as_str().unwrap());
    let snapshot: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let mut ext = externalities_with_bound_code(storage);
    let observation = ext.execute_with(|| {
      let original = current_top_storage();
      assert!(original.children_default.is_empty());
      let accounts = || {
        polkadot_sdk::frame_system::Account::<Runtime>::iter_keys().collect::<BTreeSet<_>>()
      };
      let before = accounts();
      assert_eq!(before.len(), 70, "exact retained account-domain seed");
      let now = System::block_number().checked_add(1).unwrap();
      let actor = Actors::next_actor_id();
      let sovereign = Actors::sovereign_account_id(&owner, 5);
      assert!(!before.contains(&sovereign));
      let active = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        System::set_block_number(now);
        assert_ok!(Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(owner.clone()),
          5,
          Mutability::Mutable,
          Some(contract.clone())
        ));
        assert_eq!(Actors::actor_identity(actor).unwrap().sovereign_account, sovereign);
        assert_eq!(accounts(), before);
        assert!(!polkadot_sdk::frame_system::Account::<Runtime>::contains_key(&sovereign));
        assert_eq!(System::providers(&sovereign), 0);
        assert_eq!(Balances::free_balance(&sovereign), 0);
        System::set_block_number(now.checked_add(1).unwrap());
        assert_ok!(Actors::deactivate_actor(RuntimeOrigin::signed(owner.clone()), actor));
        assert_eq!(accounts(), before);
        System::set_block_number(now.checked_add(2).unwrap());
        assert_ok!(Actors::activate_actor(
          RuntimeOrigin::signed(owner.clone()),
          actor,
          contract.clone()
        ));
        assert_eq!(accounts(), before);
        assert_eq!(System::providers(&sovereign), 0);
        assert_eq!(Balances::free_balance(&sovereign), 0);
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
          serde_json::json!({"nativeCalls": 3, "systemAccountKeys": accounts().len(), "sovereignAccountAbsent": true, "providerCount": 0}),
        )
      });
      let after = current_top_storage();
      assert!(after.children_default.is_empty());
      assert_eq!(after.top, original.top, "Active-case full TOP rollback");
      let dormant = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        System::set_block_number(now);
        assert_ok!(Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(owner.clone()),
          5,
          Mutability::Mutable,
          None
        ));
        let mut expected = before.clone();
        assert!(expected.insert(sovereign.clone()));
        assert_eq!(accounts(), expected);
        assert_eq!(System::providers(&sovereign), 1);
        assert_eq!(Balances::free_balance(&sovereign), 0);
        System::set_block_number(now.checked_add(1).unwrap());
        assert_ok!(Actors::activate_actor(
          RuntimeOrigin::signed(owner.clone()),
          actor,
          contract.clone()
        ));
        assert_eq!(accounts(), expected);
        assert_eq!(System::providers(&sovereign), 1);
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
          serde_json::json!({"nativeCalls": 2, "systemAccountKeys": accounts().len(), "onlyNewAccountIsSovereign": true, "providerCount": 1}),
        )
      });
      let after = current_top_storage();
      assert!(after.children_default.is_empty());
      assert_eq!(after.top, original.top, "Dormant-case full TOP rollback");
      serde_json::json!({"seed": seed, "initialSystemAccountKeys": before.len(), "activeBirth": active, "dormantBirth": dormant, "fullTopRollbackChecks": 2})
    });
    observations.push(observation);
  }
  std::fs::write(
    std::env::var("DEOS_BIRTH_ACCOUNT_OUTPUT").unwrap(),
    serde_json::to_vec_pretty(&observations).unwrap(),
  )
  .unwrap();
}

/// Retained TOP cuts exercise signed proof reclaim and subsequent Drain together.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires exact retained TOP seed inputs and diagnostic Wasm"]
fn retained_parked_seed_recorder_frame_and_drain() {
  use codec::Decode;
  use pallet_deos_actors::{
    ActorContract, DependencyRegistrationHeaders, DependencyRevisions,
    DependencyScanSourceListState, DependencyScanSourceNodes, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceRule,
  };
  use primitives::AssetKind;
  fn bytes(value: &serde_json::Value) -> Vec<u8> {
    let encoded = value.as_str().unwrap();
    assert!(encoded.len().is_multiple_of(2));
    (0..encoded.len())
      .step_by(2)
      .map(|at| u8::from_str_radix(&encoded[at..at + 2], 16).unwrap())
      .collect()
  }
  let seeds: serde_json::Value = serde_json::from_slice(
    &std::fs::read(std::env::var("DEOS_PARKED_SEED_INPUTS").unwrap()).unwrap(),
  )
  .unwrap();
  assert_eq!(seeds.as_array().unwrap().len(), 4);
  let alice = sr25519::Pair::from_string("//Alice", None).unwrap();
  let bob = sr25519::Pair::from_string("//Bob", None).unwrap();
  let contract = ActorContract {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        (1..=4)
          .map(|asset| ParkedBalanceRule {
            asset: AssetKind::Local(0x7000_0000 + asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    ),
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  };
  let housekeeping = <Runtime as pallet_deos_actors::Config>::WeightInfo::scheduler_on_idle_base()
    .saturating_add(<Runtime as pallet_deos_actors::Config>::WeightInfo::block_resource_finalize());
  let selector =
    <Runtime as pallet_deos_actors::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(
        <Runtime as pallet_deos_actors::Config>::WeightInfo::service_round_probe_eligible(),
      );
  let required = housekeeping.saturating_add(selector);
  for seed in seeds.as_array().unwrap() {
    let raw = std::fs::read(seed["path"].as_str().unwrap()).unwrap();
    let digest = polkadot_sdk::sp_io::hashing::sha2_256(&raw)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    assert_eq!(digest, seed["sha256"].as_str().unwrap());
    let snapshot: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let mut storage = Storage::default();
    for entry in snapshot["entries"].as_array().unwrap() {
      assert!(
        storage
          .top
          .insert(bytes(&entry["key"]), bytes(&entry["value"]))
          .is_none()
      );
    }
    let encoded_header = bytes(&snapshot["header"]);
    let mut encoded = encoded_header.as_slice();
    let mut parent = Header::decode(&mut encoded).unwrap();
    assert!(encoded.is_empty());
    let code = storage.top[well_known_keys::CODE].clone();
    assert_eq!(
      code,
      std::fs::read(std::env::var("DEOS_GOVERNANCE_PVF_WASM").unwrap()).unwrap()
    );
    let start = *parent.number();
    for offset in 1..=6 {
      let number = start + offset;
      let calls = match offset {
        1 => vec![RuntimeCall::Actors(
          pallet_deos_actors::Call::activate_actor {
            actor_id: 20,
            contract: contract.clone(),
          },
        )],
        6 => vec![
          RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor { actor_id: 20 }),
          RuntimeCall::Actors(pallet_deos_actors::Call::activate_actor {
            actor_id: 21,
            contract: contract.clone(),
          }),
        ],
        _ => Vec::new(),
      };
      let mut before = externalities_with_bound_code(storage.clone());
      assert_construction_parent_matches_state(&mut before, &parent);
      let (selected, next_source, scan, frontier, nonces) = before.execute_with(|| {
        let selected = DependencyScanSourceListState::<Runtime>::get()
          .cursor
          .unwrap();
        (
          selected,
          DependencyScanSourceNodes::<Runtime>::get(selected)
            .unwrap()
            .next,
          DependencyRevisions::<Runtime>::get(selected),
          DependencyRegistrationHeaders::<Runtime>::get(selected).next_index,
          [
            System::account_nonce(&crate::AccountId::from(alice.public())),
            System::account_nonce(&crate::AccountId::from(bob.public())),
          ],
        )
      });
      let script = match offset {
        1 => vec![(&alice, nonces[0], &calls[0])],
        6 => vec![(&alice, nonces[0], &calls[0]), (&bob, nonces[1], &calls[1])],
        _ => Vec::new(),
      };
      let authored = author_complete_block_after_with_context(
        storage,
        &code,
        &parent,
        number,
        UserDemand::ActorOnly,
        &alice,
        nonces[0],
        &BTreeMap::new(),
        &script,
        inherent_data_for(&parent, number),
      );
      let recorded_proof_bytes = authored.execution_recorded_proof_bytes.unwrap();
      assert!(recorded_proof_bytes > 0);
      assert!(!authored.execution_backend_keys.is_empty());
      let frame_remaining = authored
        .metrics
        .pre_idle_remaining
        .checked_sub(&crate::XcmpQueue::on_idle_weight())
        .unwrap();
      assert!(
        required.all_lte(frame_remaining),
        "recorder-adjusted FRAME must admit housekeeping and source selection"
      );
      let checked = assert_complete_block_replays_with_aura(&authored);
      let mut after = externalities_with_bound_code(authored.post_state.clone());
      assert_construction_parent_matches_state(&mut after, &checked);
      let (normal, registered) = after.execute_with(|| {
        assert!(Actors::block_resource_state().is_none());
        let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
        assert!(!telemetry.optional_actor_work_halted());
        assert_eq!(
          DependencyScanSourceListState::<Runtime>::get().cursor,
          Some(next_source)
        );
        let current = DependencyRevisions::<Runtime>::get(selected);
        assert_eq!(
          current.scan_cursor,
          if scan.scan_cursor < scan.scan_end {
            scan.scan_cursor + 1
          } else {
            0
          }
        );
        if scan.scan_cursor == scan.scan_end {
          assert_eq!(current.scan_target, Some(current.revision));
          assert!(current.scan_end >= frontier);
        }
        let normal =
          *System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal);
        if calls.is_empty() {
          assert_eq!(normal, Weight::zero());
        }
        (normal, System::block_weight().total())
      });
      assert!(registered.all_lte(crate::MAXIMUM_BLOCK_WEIGHT));
      println!(
        "Parked recorder Drain: preset={} phase={} block={number} offset={offset} signed_calls={} recorded_proof={} backend_keys={} pre_idle_ref={} pre_idle_proof={} normal_ref={} normal_proof={} final_ref={} final_proof={} required_ref={} required_proof={} source_advanced=true author_import_equal=true",
        seed["preset"].as_str().unwrap(),
        seed["phase"].as_str().unwrap(),
        calls.len(),
        recorded_proof_bytes,
        authored.execution_backend_keys.len(),
        authored.metrics.pre_idle_remaining.ref_time(),
        authored.metrics.pre_idle_remaining.proof_size(),
        normal.ref_time(),
        normal.proof_size(),
        registered.ref_time(),
        registered.proof_size(),
        required.ref_time(),
        required.proof_size()
      );
      parent = checked;
      storage = authored.post_state;
    }
  }
}

/// Governance has enabled User admission through its real signed/maturity/delay path.
/// This suffix uses only signed calls and normal consensus blocks; no Root setup or hook
/// invocation substitutes for its registration, producer or lifecycle transitions.
#[cfg(not(feature = "runtime-benchmarks"))]
pub(super) fn replay_signed_parked_user_epoch_churn(signer: &sr25519::Pair, parent: Header) {
  replay_signed_parked_user_epoch_churn_to_phase(signer, parent, SignedParkedReplayEnd::Complete);
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SignedParkedReplayEnd {
  CleanupOverlap,
  Pools,
  Setup,
  Rotation,
  Bootstrap,
  WatchMasks,
  Birth,
  BirthPair,
  WideBirth,
  Complete,
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn replay_signed_parked_user_epoch_churn_to_phase(
  signer: &sr25519::Pair,
  parent: Header,
  end: SignedParkedReplayEnd,
) {
  replay_signed_parked_user_epoch_churn_with_pvf(signer, parent, end, None);
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn replay_signed_parked_user_epoch_churn_with_pvf(
  signer: &sr25519::Pair,
  mut parent: Header,
  end: SignedParkedReplayEnd,
  pvf_output: Option<(
    &polkadot_sdk::sp_genesis_builder::PresetId,
    &std::path::Path,
  )>,
) {
  use pallet_deos_actors::{
    ActorClass, ActorContract, AssetOps, BalanceDependencySources,
    DependencyRegistrationFreeIndices, DependencyRegistrationFreePositions,
    DependencyRegistrationHeaders, DependencyRegistrationPages, DependencyRegistrationPosition,
    DependencyRegistrationPositions, DependencyRevisions, FundingSourcePolicy, OnUnfunded,
    ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule, TaskEffectWeightProvider,
  };
  use polkadot_sdk::frame_support::traits::Get;
  #[cfg(feature = "try-runtime")]
  use polkadot_sdk::frame_support::traits::Hooks;
  use primitives::AssetKind;

  let watch_count = <Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get();
  assert_eq!(watch_count, 1);

  fn retain_current_state(storage: &Storage, parent: &Header, phase: &str) {
    use polkadot_sdk::sp_core::hexdisplay::HexDisplay;
    use std::io::Write;
    let Ok(output) = std::env::var("DEOS_PARKED_STATE_OUTPUT") else {
      return;
    };
    let output = std::path::PathBuf::from(output);
    assert!(output.is_absolute() && output.is_dir());
    assert!(storage.children_default.is_empty());
    let mut ext = externalities_with_bound_code(storage.clone());
    assert_construction_parent_matches_state(&mut ext, parent);
    ext.execute_with(|| {
      assert!(Actors::block_resource_state().is_none());
      assert!(
        polkadot_sdk::sp_io::storage::get(
          polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
        )
        .is_none()
      );
    });
    let info = runtime_proof_storage_info();
    let declarations = info.iter().map(|item| serde_json::json!({
      "owner": format!("{}.{}", String::from_utf8_lossy(&item.pallet_name), String::from_utf8_lossy(&item.storage_name)),
      "prefix": format!("{}", HexDisplay::from(&item.prefix)),
      "maxValues": item.max_values,
      "maxSize": item.max_size,
    })).collect::<Vec<_>>();
    let entries = storage
      .top
      .iter()
      .map(|(key, value)| {
        let mut matching = info.iter().filter(|item| key.starts_with(&item.prefix));
        let owner = matching.next().map(|item| {
          format!(
            "{}.{}",
            String::from_utf8_lossy(&item.pallet_name),
            String::from_utf8_lossy(&item.storage_name)
          )
        });
        assert!(matching.next().is_none(), "one owner per storage prefix");
        serde_json::json!({
          "key": format!("{}", HexDisplay::from(key)),
          // Debug preserves every byte; Display abbreviates large values.
          "value": format!("{:?}", HexDisplay::from(value)),
          "owner": owner,
        })
      })
      .collect::<Vec<_>>();
    let block = *parent.number();
    let parent_hash = parent.hash();
    let file_name = format!(
      "{phase}-{block}-{}.json",
      HexDisplay::from(&parent_hash.as_bytes())
    );
    let checkpoint = serde_json::json!({
      "scope": "Complete finite native TOP state; earlier prepared starts/history remain conditional",
      "phase": phase, "block": block,
      "header": format!("{}", HexDisplay::from(&parent.encode())),
      "headerHash": format!("{}", HexDisplay::from(&parent_hash.as_bytes())),
      "stateRoot": format!("{}", HexDisplay::from(&parent.state_root().as_bytes())),
      "codeRootNumberAndCanonicalWindowMatched": true,
      "frameTransactionLayerAbsent": true, "actorTransientAuthorityAbsent": true,
      "childEntries": 0, "topKeys": storage.top.len(),
      "declarations": declarations, "entries": entries,
      "literalHistory": false, "uniformHorizonProved": false,
    });
    let mut file = std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(output.join(&file_name))
      .unwrap();
    file
      .write_all(&serde_json::to_vec_pretty(&checkpoint).unwrap())
      .unwrap();
    file.write_all(b"\n").unwrap();
    println!(
      "Parked complete state retained: phase={phase} block={block} file={file_name} top_keys={} code_root_number_window_equal=true historical_gap_conditional=true",
      storage.top.len()
    );
  }

  let setup_only = matches!(
    end,
    SignedParkedReplayEnd::CleanupOverlap
      | SignedParkedReplayEnd::Pools
      | SignedParkedReplayEnd::Setup
      | SignedParkedReplayEnd::Rotation
      | SignedParkedReplayEnd::WideBirth
  );
  let bootstrap_only = matches!(
    end,
    SignedParkedReplayEnd::Bootstrap
      | SignedParkedReplayEnd::WatchMasks
      | SignedParkedReplayEnd::Birth
      | SignedParkedReplayEnd::BirthPair
  );
  fn advance_signed_unobserved(
    storage: &mut Storage,
    parent: &mut Header,
    block: &mut u32,
    signer: &sr25519::Pair,
    calls: &[(&sr25519::Pair, &RuntimeCall)],
    expected_user_creation_enabled: bool,
    pvf_output: Option<(
      &polkadot_sdk::sp_genesis_builder::PresetId,
      &std::path::Path,
    )>,
  ) {
    *block += 1;
    let owner = crate::AccountId::from(signer.public());
    let secondary = sr25519::Pair::from_string("//Bob", None).unwrap();
    let secondary_owner = crate::AccountId::from(secondary.public());
    let mut input = externalities_with_bound_code(storage.clone());
    assert_construction_parent_matches_state(&mut input, parent);
    let (nonce, script, next_nonces) = input.execute_with(|| {
      assert!(Actors::block_resource_state().is_none());
      assert!(
        polkadot_sdk::sp_io::storage::get(
          polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
        )
        .is_none(),
        "the preceding complete block leaves no FRAME transaction layer"
      );
      let nonce = System::account_nonce(&owner);
      let secondary_nonce = System::account_nonce(&secondary_owner);
      let mut next_nonces =
        BTreeMap::from([(owner.clone(), nonce), (secondary_owner, secondary_nonce)]);
      let script = calls
        .iter()
        .map(|(pair, call)| {
          let account = crate::AccountId::from(pair.public());
          let next = next_nonces
            .entry(account.clone())
            .or_insert_with(|| System::account_nonce(&account));
          let current = *next;
          *next = current.checked_add(1).unwrap();
          (*pair, current, *call)
        })
        .collect::<Vec<_>>();
      (nonce, script, next_nonces)
    });
    drop(input);
    let code = storage.top[well_known_keys::CODE].clone();
    let authored = author_complete_block_after_with_context(
      core::mem::take(storage),
      &code,
      parent,
      *block,
      UserDemand::ActorOnly,
      signer,
      nonce,
      &BTreeMap::new(),
      &script,
      inherent_data_for(parent, *block),
    );
    let checked_header = if matches!(parent.digest().logs.last(), Some(DigestItem::Seal(engine, _)) if engine == &polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID)
    {
      assert_complete_block_replays_with_aura(&authored)
    } else {
      assert_complete_block_replays_natively(&authored, &code);
      authored.block.header.clone()
    };
    externalities_with_bound_code(authored.post_state.clone()).execute_with(|| {
      assert_eq!(Actors::user_creation_enabled(), expected_user_creation_enabled);
      assert!(!Actors::global_circuit_breaker());
      assert!(Actors::block_resource_state().is_none());
      assert!(
        polkadot_sdk::sp_io::storage::get(
          polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
        )
        .is_none(),
        "signed calls, source service and finalization close their FRAME transaction layers"
      );
      let finalized = Actors::finalized_block_resource_telemetry().unwrap();
      assert_eq!(finalized.block_number(), *block);
      assert_eq!(finalized.budget(), BlockResourceBudgetValue::get());
      assert!(!finalized.optional_actor_work_halted());
      for (account, expected) in next_nonces {
        assert_eq!(System::account_nonce(&account), expected);
      }
      if calls.is_empty() {
        assert_eq!(
          *System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal),
          Weight::zero(),
          "Actor service and consensus hooks leave an empty signed-demand block Normal-empty"
        );
      }
      if !expected_user_creation_enabled {
        let normal = *System::block_weight().get(polkadot_sdk::frame_support::dispatch::DispatchClass::Normal);
        let sink = Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
        println!("Signed Parked public pool meter: block={block} calls={} normal_ref={} normal_proof={} next_multiplier={} sink_native={}", calls.len(), normal.ref_time(), normal.proof_size(), polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get().into_inner(), Balances::free_balance(&sink));
      }
      #[cfg(feature = "try-runtime")]
      <Actors as Hooks<u32>>::try_state(*block).unwrap();
    });
    if block.is_multiple_of(64) {
      println!("Signed Parked churn: authored/imported block {block}; post-state reconciled");
    }
    export_complete_native_pvf(&authored, &checked_header, pvf_output);
    *parent = checked_header;
    *storage = authored.post_state;
  }

  fn advance_unobserved(
    storage: &mut Storage,
    parent: &mut Header,
    block: &mut u32,
    signer: &sr25519::Pair,
    calls: &[RuntimeCall],
    expected_user_creation_enabled: bool,
    pvf_output: Option<(
      &polkadot_sdk::sp_genesis_builder::PresetId,
      &std::path::Path,
    )>,
  ) {
    let secondary = sr25519::Pair::from_string("//Bob", None).unwrap();
    let secondary_owner = crate::AccountId::from(secondary.public());
    let secondary_calls = externalities_with_bound_code(storage.clone()).execute_with(|| {
      calls
        .iter()
        .map(|call| match call {
          RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor_at_slot {
            owner_slot: 254,
            ..
          }) => true,
          RuntimeCall::Actors(
            pallet_deos_actors::Call::activate_actor { actor_id, .. }
            | pallet_deos_actors::Call::deactivate_actor { actor_id }
            | pallet_deos_actors::Call::close_actor { actor_id },
          ) => Actors::actor_identity(*actor_id).unwrap().owner == secondary_owner,
          _ => false,
        })
        .collect::<Vec<_>>()
    });
    let signed_calls = calls
      .iter()
      .zip(secondary_calls)
      .map(|(call, secondary_call)| (if secondary_call { &secondary } else { signer }, call))
      .collect::<Vec<_>>();
    advance_signed_unobserved(
      storage,
      parent,
      block,
      signer,
      &signed_calls,
      expected_user_creation_enabled,
      pvf_output,
    );
  }

  // --- Producer Publication Trace ---

  #[derive(Clone, Copy)]
  struct ProducerCustodySnapshot {
    native_total: u128,
    native_available: u128,
    lp_total: u128,
    revision: Option<(u64, u64)>,
    fee_bound: u128,
  }

  #[derive(Default)]
  struct ProducerPublicationTrace {
    producers: BTreeMap<ActorId, (u32, ProducerCustodySnapshot)>,
    publications: u32,
    maximum_gap: u32,
    maximum_block_bucket: u32,
    maximum_tick_bucket: u32,
    maximum_native_unavailable: u128,
    maximum_native_debit: u128,
  }

  impl ProducerPublicationTrace {
    fn observe(&mut self, storage: &Storage, block: u32) {
      let (current, block_bucket, tick_bucket) = externalities_with_bound_code(storage.clone())
        .execute_with(|| {
          let producers = pallet_deos_actors::ActorContractHeads::<Runtime>::iter()
            .filter_map(|(actor, head)| {
              if head.header.trigger != Trigger::cadenced(1) {
                return None;
              }
              let state = Actors::active_actor_state(actor).unwrap();
              if state.identity.actor_class.actor_type() != ActorType::User {
                return None;
              }
              assert_eq!(head.header.step_count, 1);
              assert_eq!(state.contract.funding, FundingSourcePolicy::OwnerOnly);
              assert_eq!(state.contract.on_unfunded, OnUnfunded::Close);
              assert!(state.contract.close_when.is_none());
              assert!(state.contract.window.is_none());
              assert!(state.contract.parked_balance_activation.is_none());
              assert!(state.contract.auto_close_at_cycle_nonce.is_none());
              let step = head.first_step.unwrap();
              assert!(step.precondition.is_none());
              assert_eq!(step.on_error, StepErrorPolicy::AbortCycle);
              let Task::Transfer {
                ref to,
                asset,
                amount: AmountResolution::Fixed(1),
              } = step.task
              else {
                panic!("construction producers must transfer one unit");
              };
              assert_eq!(to, &state.identity.owner);
              let custody = &state.identity.sovereign_account;
              let balance =
                crate::configs::actor_config::TmctolAssetOps::total_balance(custody, asset);
              assert_eq!(
                crate::configs::actor_config::TmctolAssetOps::balance(custody, asset),
                balance,
                "producer LP remains fully transferable"
              );
              assert_eq!(
                crate::configs::actor_config::TmctolAssetOps::minimum_balance(asset),
                1
              );
              assert_ok!(
                crate::configs::actor_config::TmctolAssetOps::preflight_transfer(
                  custody, to, asset, 1
                )
              );
              let native_total = crate::configs::actor_config::TmctolAssetOps::total_balance(
                custody,
                AssetKind::Native,
              );
              let native_available =
                crate::configs::actor_config::TmctolAssetOps::balance(custody, AssetKind::Native);
              assert!(
                native_total.checked_sub(native_available).unwrap() <= crate::EXISTENTIAL_DEPOSIT,
                "only the native account floor may limit producer availability"
              );
              let steps: ContractSteps<Runtime> = vec![step.clone()].try_into().unwrap();
              let machine_fee = Actors::user_pipeline_machine_capacity_requirement(&steps).unwrap()
                - <Runtime as pallet_deos_actors::Config>::MinUserBalance::get();
              let trigger_fee = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
                &<Runtime as pallet_deos_actors::Config>::WeightInfo::cadenced_trigger_occurrence(),
              );
              let action_fee = <Runtime as pallet_deos_actors::Config>::WeightToFee::weight_to_fee(
                &<Runtime as pallet_deos_actors::Config>::TaskEffectWeight::maximum_effect_weight(
                  &step.task,
                )
                .unwrap(),
              );
              let fee_bound = trigger_fee + machine_fee + action_fee;
              assert!(
                native_available
                  >= <Runtime as pallet_deos_actors::Config>::MinUserBalance::get() + fee_bound
              );
              let revision = BalanceDependencySources::<Runtime>::get(asset)
                .map(|source| (source, DependencyRevisions::<Runtime>::get(source).revision));
              Some((
                actor,
                ProducerCustodySnapshot {
                  native_total,
                  native_available,
                  lp_total: balance,
                  revision,
                  fee_bound,
                },
              ))
            })
            .collect::<BTreeMap<_, _>>();
          let service = pallet_deos_actors::ServiceHeader::<Runtime>::get();
          assert!(service.count <= 7);
          for actor in pallet_deos_actors::ServiceNodes::<Runtime>::iter_keys() {
            assert!(
              Actors::actor_identity(actor)
                .unwrap()
                .actor_class
                .actor_type()
                == ActorType::System
                || producers.contains_key(&actor),
              "an unqualified Parked Actor must not enter construction Service"
            );
          }
          let temporal = pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::iter().count();
          assert!(temporal <= 5);
          let tick_members = pallet_deos_actors::DeadlineHeaders::<Runtime>::iter()
            .filter(|(key, _)| matches!(key, WakeupKey::Tick(_)))
            .map(|(_, header)| header.count as usize)
            .sum::<usize>();
          assert_eq!(tick_members, temporal);
          let mut block_bucket = 0;
          let mut tick_bucket = 0;
          for (key, header) in pallet_deos_actors::DeadlineHeaders::<Runtime>::iter() {
            assert_eq!(header.page_count, 1);
            assert_eq!(
              (header.first_page, header.last_page, header.next_page),
              (0, 0, 1)
            );
            match key {
              WakeupKey::Block(due) => {
                assert!(due <= block.checked_add(1).unwrap());
                assert!(header.count <= 3);
                block_bucket = block_bucket.max(header.count);
              }
              WakeupKey::Tick(_) => {
                assert!(header.count <= 5);
                tick_bucket = tick_bucket.max(header.count);
              }
            }
            let page = pallet_deos_actors::DeadlinePages::<Runtime>::get(key, 0).unwrap();
            assert_eq!(u32::from(page.live_entries), header.count);
          }
          (producers, block_bucket, tick_bucket)
        });
      self.maximum_block_bucket = self.maximum_block_bucket.max(block_bucket);
      self.maximum_tick_bucket = self.maximum_tick_bucket.max(tick_bucket);
      if current.is_empty() {
        assert!(self.producers.is_empty());
        return;
      }
      assert_eq!(
        current.len(),
        <Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get() as usize
      );
      for (actor, snapshot) in current {
        self.maximum_native_unavailable = self.maximum_native_unavailable.max(
          snapshot
            .native_total
            .checked_sub(snapshot.native_available)
            .unwrap(),
        );
        let previous = self
          .producers
          .entry(actor)
          .or_insert_with(|| {
            assert_eq!(snapshot.native_total, 320_000 * crate::UNIT);
            assert_eq!(snapshot.lp_total, 10_000_000_000);
            println!(
              "Signed Parked producer {actor}: native_total={} native_available={} lp_total={} fee_bound={}",
              snapshot.native_total, snapshot.native_available, snapshot.lp_total, snapshot.fee_bound
            );
            (block, snapshot)
          });
        let native_debit = previous
          .1
          .native_total
          .checked_sub(snapshot.native_total)
          .unwrap();
        assert!(
          native_debit <= snapshot.fee_bound,
          "producer {actor} exceeds its block fee bound"
        );
        self.maximum_native_debit = self.maximum_native_debit.max(native_debit);
        match previous.1.lp_total.checked_sub(snapshot.lp_total).unwrap() {
          0 => {}
          1 => {
            let gap = block.checked_sub(previous.0).unwrap();
            assert!(gap <= 32, "producer {actor} publication gap {gap}");
            self.maximum_gap = self.maximum_gap.max(gap);
            self.publications += 1;
            if let (Some(before), Some(after)) = (previous.1.revision, snapshot.revision)
              && before.0 == after.0
            {
              assert!(
                after.1 > before.1,
                "committed Transfer must publish its source"
              );
            }
            previous.0 = block;
          }
          _ => panic!("a one-Step Q1 producer transferred more than one unit"),
        }
        assert!(
          block.checked_sub(previous.0).unwrap() <= 32,
          "producer {actor} stopped publishing"
        );
        previous.1 = snapshot;
      }
    }
  }

  let publication_trace = std::cell::RefCell::new(ProducerPublicationTrace::default());
  let expected_user_creation_enabled = end != SignedParkedReplayEnd::Pools;
  assert_eq!(
    Actors::user_creation_enabled(),
    expected_user_creation_enabled
  );
  let pool_economic_start = (end == SignedParkedReplayEnd::Pools).then(|| {
    (
      Balances::total_issuance(),
      Balances::free_balance(crate::Staking::pool_account_for(0)),
      Assets::balance(
        0,
        Actors::sovereign_account_id_system(
          primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
        ),
      ),
      Assets::total_supply(0),
      Balances::reserved_balance(crate::AccountId::from(signer.public())),
    )
  });
  let setup_normal_maximum = std::cell::Cell::new(Weight::zero());
  let setup_signed_calls = std::cell::Cell::new(0usize);
  let signed_parent_multiplier_maximum =
    std::cell::Cell::new(polkadot_sdk::pallet_transaction_payment::Multiplier::from_u32(0));
  let inspect_cleanup_overlap = |storage: &Storage, block: u32| {
    if !matches!(
      end,
      SignedParkedReplayEnd::CleanupOverlap | SignedParkedReplayEnd::Complete
    ) || !(187_205..=187_223).contains(&block)
    {
      return;
    }
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      use pallet_governance::EpochServicePhase;
      assert!(Actors::user_creation_enabled());
      let first_birth = end == SignedParkedReplayEnd::Complete && block >= 187_220;
      assert_eq!(Actors::next_actor_id(), 15 + u64::from(first_birth));
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      if block < 187_221 {
        assert!(crate::Governance::finalized_proposal_outcome(0, 120).is_some());
        assert!(crate::Governance::proposal_metadata(0, 120).is_some());
        assert_eq!(crate::Governance::finalized_proposal_outcome_expiry_bucket(187_220).len(), 1);
        assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), block - 1);
      } else {
        assert!(crate::Governance::finalized_proposal_outcome(0, 120).is_none());
        assert!(crate::Governance::proposal_metadata(0, 120).is_none());
        assert!(crate::Governance::recent_finalized_proposals(0).is_empty());
        assert!(crate::Governance::finalized_proposal_outcome_expiry_bucket(187_220).is_empty());
        assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), block - 2);
        let phase = if block == 187_221 { EpochServicePhase::RewardExpiry } else { EpochServicePhase::Maturing };
        assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), phase);
        let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
        let state = Actors::active_actor_state(sink).unwrap();
        println!("Signed cleanup Fee Sink phase: block={block} nonce={} cycle={:?} pending={} last_cycle={:?} next_due={:?}", state.identity.cycle_nonce, state.hot.cycle_state, state.hot.pending_signal, state.hot.last_cycle_block, state.hot.trigger_wakeup_pointer);
        if block == 187_221 {
          assert_eq!(state.hot.cycle_state, CycleState::Idle);
          assert!(state.run_state.is_none());
          assert!(state.hot.pending_signal);
          assert_eq!(state.identity.cycle_nonce, 18_721);
          assert!(!System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) if actor_id == sink)));
        } else if block == 187_222 {
          assert_eq!(state.hot.cycle_state, CycleState::Running);
          let run = state.run_state.unwrap();
          assert_eq!(run.cursor, 1);
          assert!(run.balance_snapshot.is_some());
          assert!(!state.hot.pending_signal);
          assert_eq!(state.identity.cycle_nonce, 18_721);
          assert!(System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) if actor_id == sink)));
          assert!(!System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) if actor_id == sink)));
        } else {
          assert_eq!(state.hot.cycle_state, CycleState::Idle);
          assert!(state.run_state.is_none());
          assert!(!state.hot.pending_signal);
          assert_eq!(state.identity.cycle_nonce, 18_722);
          assert_eq!(state.hot.last_cycle_block, Some(block));
          assert!(!System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) if actor_id == sink)));
          assert!(System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes, .. }) if actor_id == sink && outcomes.executed_steps == 2)));
        }
        let finalized = Actors::finalized_block_resource_telemetry().unwrap();
        let usage = finalized.usage();
        if block >= 187_222 { assert!(usage.actor_effect_used().ref_time() > 0); }
        let sink_phase = match block { 187_221 => "readiness", 187_222 => "opening", _ => "completion" };
        println!("Signed cleanup overlap: block={block} relative_block={} history_removed=true fee_sink_{sink_phase}=true control={:?} effect={:?} optional_halted={} phase={phase:?}", block - 187_204, usage.actor_control_used(), usage.actor_effect_used(), finalized.optional_actor_work_halted());
      }
    });
  };
  let inspect_setup_fee_start = |storage: &Storage, calls: usize| {
    let fee = externalities_with_bound_code(storage.clone()).execute_with(|| {
      polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get()
    });
    if setup_only || calls > 0 {
      assert!(fee <= polkadot_sdk::pallet_transaction_payment::Multiplier::from_u32(1));
    }
    if calls > 0 {
      signed_parent_multiplier_maximum.set(signed_parent_multiplier_maximum.get().max(fee));
    }
    setup_only.then(|| {
      setup_signed_calls.set(setup_signed_calls.get() + calls);
      fee
    })
  };
  let inspect_setup_fee_end = |storage: &Storage, before| {
    if let Some(before) = before {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        use polkadot_sdk::frame_support::dispatch::DispatchClass;
        let normal = *System::block_weight().get(DispatchClass::Normal);
        let normal_limit = crate::configs::RuntimeBlockWeights::get()
          .get(DispatchClass::Normal)
          .max_total
          .unwrap();
        let target = Weight::from_parts(normal_limit.ref_time() / 4, normal_limit.proof_size() / 4);
        assert!(
          normal.all_lte(target),
          "setup must not raise the fee multiplier"
        );
        assert!(
          polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get() <= before
        );
        setup_normal_maximum.set(setup_normal_maximum.get().max(normal));
      });
    }
  };
  let inspect_pool_event_carriers = |storage: &Storage, block: u32, calls: usize| {
    if end != SignedParkedReplayEnd::Pools {
      return;
    }
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      use polkadot_sdk::frame_support::storage::storage_prefix;
      let events = System::events();
      let encoded = events.encode();
      let stored = polkadot_sdk::sp_io::storage::get(
        &storage_prefix(b"System", b"Events"),
      )
      .unwrap();
      assert_eq!(stored.as_ref(), encoded.as_slice());
      assert_eq!(System::event_count(), u32::try_from(events.len()).unwrap());
      assert!(events.iter().all(|record| record.topics.is_empty()));
      let applied = events
        .iter()
        .filter(|record| {
          matches!(
            record.event,
            RuntimeEvent::System(polkadot_sdk::frame_system::Event::ExtrinsicSuccess { .. })
          )
        })
        .count();
      assert_eq!(applied, calls + 3);
      assert!(applied <= 43);
      let extrinsic_prefix = storage_prefix(b"System", b"ExtrinsicData");
      assert!(polkadot_sdk::sp_io::storage::next_key(&extrinsic_prefix)
        .is_none_or(|key| !key.starts_with(&extrinsic_prefix)));
      assert!(!polkadot_sdk::sp_io::storage::exists(&storage_prefix(
        b"System",
        b"ExtrinsicCount"
      )));
      assert!(polkadot_sdk::frame_system::BlockSize::<Runtime>::get().is_none());
      assert!(System::extrinsic_index().is_none());
      let mut families = BTreeMap::new();
      for record in &events {
        let event = record.event.encode();
        let entry = families.entry((event[0], event[1])).or_insert((0usize, 0usize));
        entry.0 += 1;
        entry.1 = entry.1.max(record.encoded_size());
      }
      println!(
        "Signed Parked event carriers: block={block} signed_calls={calls} applied={applied} events={} encoded_bytes={} families={families:?}; transient_extrinsic_carriers_empty=true topics_empty=true",
        events.len(),
        encoded.len()
      );
    });
  };
  let advance_signed = |storage: &mut Storage,
                        parent: &mut Header,
                        block: &mut u32,
                        signer: &sr25519::Pair,
                        calls: &[(&sr25519::Pair, &RuntimeCall)]| {
    let fee_before = inspect_setup_fee_start(storage, calls.len());
    advance_signed_unobserved(
      storage,
      parent,
      block,
      signer,
      calls,
      expected_user_creation_enabled,
      pvf_output,
    );
    inspect_setup_fee_end(storage, fee_before);
    inspect_pool_event_carriers(storage, *block, calls.len());
    publication_trace.borrow_mut().observe(storage, *block);
    inspect_cleanup_overlap(storage, *block);
  };
  let advance = |storage: &mut Storage,
                 parent: &mut Header,
                 block: &mut u32,
                 signer: &sr25519::Pair,
                 calls: &[RuntimeCall]| {
    let rotation_source_next = (end == SignedParkedReplayEnd::Rotation
      && block.checked_add(1).unwrap() == crate::configs::Period::get())
    .then(|| {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        let list = pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get();
        assert_eq!(list.count, watch_count);
        for source in 0..u64::from(watch_count) {
          assert!(
            DependencyRevisions::<Runtime>::get(source)
              .scan_target
              .is_some()
          );
        }
        pallet_deos_actors::DependencyScanSourceNodes::<Runtime>::get(list.cursor.unwrap())
          .unwrap()
          .next
      })
    });
    let fee_before = inspect_setup_fee_start(storage, calls.len());
    advance_unobserved(
      storage,
      parent,
      block,
      signer,
      calls,
      expected_user_creation_enabled,
      pvf_output,
    );
    inspect_setup_fee_end(storage, fee_before);
    inspect_pool_event_carriers(storage, *block, calls.len());
    publication_trace.borrow_mut().observe(storage, *block);
    inspect_cleanup_overlap(storage, *block);
    if let Some(next) = rotation_source_next {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert_eq!(
          pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().cursor,
          Some(next)
        );
        for source in 0..u64::from(watch_count) {
          assert!(
            DependencyRevisions::<Runtime>::get(source)
              .scan_target
              .is_some()
          );
        }
        assert_eq!(
          pallet_deos_actors::DependencyTimedReviews::<Runtime>::get(15)
            .unwrap()
            .deadline,
          WakeupKey::Block(block.checked_add(1).unwrap())
        );
      });
      println!(
        "Signed Parked rotation quantum: block={block} source_cursor_advanced_to={next} active_scans={watch_count} negative_review_rearmed=true"
      );
    }
  };

  // --- Signed Construction ---

  let owner = crate::AccountId::from(signer.public());
  let parked_contract = |assets: &[u32]| -> pallet_deos_actors::ActorContractOf<Runtime> {
    ActorContract {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
      window: None,
      steps: vec![
        StepOf::<Runtime> {
          precondition: None,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        };
        <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get() as usize
      ]
      .try_into()
      .unwrap(),
      on_unfunded: OnUnfunded::Close,
      close_when: None,
      parked_balance_activation: Some(
        ParkedBalanceActivationOf::<Runtime>::try_from_rules(
          assets
            .iter()
            .map(|asset| ParkedBalanceRule {
              asset: AssetKind::Local(*asset),
              authored_min_delta: 100,
            })
            .collect(),
        )
        .unwrap(),
      ),
      funding: FundingSourcePolicy::OwnerOnly,
      auto_close_at_cycle_nonce: None,
    }
  };
  let temporary_contract = |assets: &[u32]| {
    let mut contract = parked_contract(assets);
    // The fixed capital recipe prices one Step independently of the watch count.
    contract.steps.truncate(1);
    contract
  };
  let create = |slot, contract| {
    RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor_at_slot {
      owner_slot: slot,
      mutability: Mutability::Mutable,
      contract,
    })
  };
  let mut block = System::block_number();
  let initial_block = block;
  let initial_next = Actors::next_actor_id();
  let original_system_sovereigns = Actors::system_sovereign_count();
  let mut storage = current_top_storage();
  retain_current_state(&storage, &parent, "public-admission");
  assert_eq!(*parent.number(), block);
  let owner_pairs = polkadot_sdk::sp_keyring::Sr25519Keyring::well_known()
    .map(|key| key.pair())
    .chain((0..28).map(polkadot_sdk::sp_keyring::Sr25519Keyring::numeric))
    .collect::<Vec<_>>();
  assert_eq!(owner_pairs.len(), 40);
  assert_eq!(owner_pairs[0].public(), signer.public());
  let owners = owner_pairs
    .iter()
    .map(|pair| crate::AccountId::from(pair.public()))
    .collect::<BTreeSet<_>>();
  assert_eq!(owners.len(), 40);
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    for pair in &owner_pairs[12..] {
      assert_eq!(
        Balances::free_balance(crate::AccountId::from(pair.public())),
        0
      );
    }
  });
  let top_ups = (0..7)
    .map(|index| {
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(crate::AccountId::from(
          owner_pairs[usize::from(index == 6)].public(),
        )),
        value: if index == 6 { 200_000 } else { 900_000 } * crate::UNIT,
      })
    })
    .collect::<Vec<_>>();
  let top_up_script = owner_pairs[2..9].iter().zip(&top_ups).collect::<Vec<_>>();
  advance_signed(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &top_up_script,
  );
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  let owner_funding = owner_pairs
    .iter()
    .map(|pair| {
      RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(crate::AccountId::from(pair.public())),
        value: 30_000 * crate::UNIT,
      })
    })
    .collect::<Vec<_>>();
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &owner_funding,
  );
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  // The rounded epoch cash exceeds the fixed-corpus coded reservation, independently
  // of producer custody. Every other creator keeps its own 30,000-unit allowance.
  let operator_epoch_cash = 1_322_000 * crate::UNIT;
  let initial_owner_free = externalities_with_bound_code(storage.clone()).execute_with(|| {
    for pair in &owner_pairs {
      assert!(
        Balances::free_balance(crate::AccountId::from(pair.public())) >= 30_000 * crate::UNIT
      );
    }
    for pair in &owner_pairs[12..] {
      assert_eq!(
        Balances::free_balance(crate::AccountId::from(pair.public())),
        30_000 * crate::UNIT
      );
    }
    assert_eq!(Actors::next_actor_id(), initial_next);
    assert_eq!(Actors::system_sovereign_count(), original_system_sovereigns);
    assert!(
      Balances::free_balance(&owner)
        >= (u128::from(<Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get())
          * 320_000
          + 30_000)
          * crate::UNIT
          + operator_epoch_cash
    );
    assert!(
      Balances::free_balance(crate::AccountId::from(owner_pairs[1].public()))
        >= 30_000 * crate::UNIT + operator_epoch_cash
    );
    Balances::free_balance(&owner)
  });
  // Asset creation is Root-only. Public pool creation/liquidity can nevertheless supply
  // Pool conformance is independent of the per-Actor watch bound.
  let pool_count = match end {
    SignedParkedReplayEnd::Pools => 4,
    SignedParkedReplayEnd::CleanupOverlap => 2,
    _ => watch_count,
  };
  let mut assets = Vec::new();
  let mut paired = AssetKind::Local(0);
  for _ in 0..pool_count {
    let calls = [
      RuntimeCall::DeosRouter(pallet_deos_router::Call::create_pool {
        asset_a: AssetKind::Native,
        asset_b: paired,
      }),
      RuntimeCall::AssetConversion(polkadot_sdk::pallet_asset_conversion::Call::add_liquidity {
        asset1: Box::new(AssetKind::Native),
        asset2: Box::new(paired),
        amount1_desired: crate::UNIT,
        amount2_desired: crate::UNIT / 4,
        amount1_min: 1,
        amount2_min: 1,
        mint_to: owner.clone(),
      }),
    ];
    advance(&mut storage, &mut parent, &mut block, signer, &calls);
    let lp = externalities_with_bound_code(storage.clone()).execute_with(|| {
      polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((AssetKind::Native, paired))
        .unwrap()
        .lp_token
    });
    assets.push(lp);
    paired = AssetKind::Local(lp);
    // Bound the next mutation's fee multiplier after each two-call pool block,
    // independently of observed Normal load. The last gap also primes Governance.
    for _ in 0..4 {
      advance(&mut storage, &mut parent, &mut block, signer, &[]);
      if end == SignedParkedReplayEnd::CleanupOverlap && block - initial_block == 19 {
        assert_eq!(setup_signed_calls.get(), 51);
        assert_eq!(assets.len(), 2);
        externalities_with_bound_code(storage).execute_with(|| {
          assert_eq!(pallet_oracle::FeedIds::<Runtime>::get().len(), 4);
          assert_eq!(crate::Actors::active_actor_count(), 3);
          assert!(BalanceDependencySources::<Runtime>::iter().next().is_none());
          assert!(
            Balances::free_balance(&owner)
              >= (30_000 + operator_epoch_cash / crate::UNIT) * crate::UNIT
          );
        });
        println!(
          "Signed cleanup overlap prefix: initial_block={initial_block} final_block={block} complete_blocks=19 signed_calls=51 pools=2 next_actor=15 user_admission=true"
        );
        return;
      }
    }
  }
  if end == SignedParkedReplayEnd::Pools {
    assert_eq!(
      block - initial_block,
      10 + 5 * u32::try_from(assets.len()).unwrap()
    );
    assert_eq!(setup_signed_calls.get(), 7 + 40 + 2 * assets.len());
    externalities_with_bound_code(storage).execute_with(|| {
      use polkadot_sdk::frame_support::traits::{fungible::Inspect as _, fungibles::Inspect as _};
      assert!(!Actors::user_creation_enabled());
      assert_eq!(Actors::next_actor_id(), initial_next);
      assert_eq!(Actors::active_actor_count(), 3);
      assert_eq!(Actors::system_sovereign_count(), original_system_sovereigns);
      assert!(BalanceDependencySources::<Runtime>::iter().next().is_none());
      assert_eq!(assets, ((primitives::assets::TYPE_LP | 1)..=(primitives::assets::TYPE_LP | 4)).collect::<Vec<_>>());
      let lp_balances = assets.iter().map(|asset| Assets::balance(*asset, &owner)).collect::<Vec<_>>();
      assert_eq!(lp_balances, vec![249_999_999_900, 249_999_999_900, 249_999_999_900, 499_999_999_900]);
      for (index, asset) in assets.iter().enumerate() {
        use polkadot_sdk::pallet_asset_conversion::PoolLocator;
        let paired = AssetKind::Local(if index == 0 { 0 } else { assets[index - 1] });
        let pair = (AssetKind::Native, paired);
        let account = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pair).unwrap();
        assert_eq!(polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair).unwrap().lp_token, *asset);
        assert_eq!(Balances::free_balance(&account), crate::UNIT);
        assert_eq!(Assets::balance(if index == 0 { 0 } else { assets[index - 1] }, &account), crate::UNIT / 4);
        assert_eq!(Assets::total_supply(*asset), crate::UNIT / 2);
        assert_eq!(Assets::balance(*asset, &account), 100);
        assert_eq!(crate::DeosRouter::lp_pair_by_token_id(*asset), Some(pair));
        assert!(Assets::reducible_balance(*asset, &owner, polkadot_sdk::frame_support::traits::tokens::Preservation::Preserve, polkadot_sdk::frame_support::traits::tokens::Fortitude::Polite) >= 10_000_000_000);
      }
      assert_eq!(pallet_oracle::FeedIds::<Runtime>::get().len(), 8);
      assert_eq!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().len(), 4);
      let (initial_issuance, initial_staking_native, initial_liquidity_local, initial_local_supply, initial_reserved) = pool_economic_start.unwrap();
      let issuance_debit = initial_issuance - Balances::total_issuance();
      let staking_native_delta = Balances::free_balance(crate::Staking::pool_account_for(0)) - initial_staking_native;
      let liquidity_local_delta = Assets::balance(0, Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID)) - initial_liquidity_local;
      let local_supply_credit = Assets::total_supply(0) - initial_local_supply;
      assert!(staking_native_delta > 0);
      assert_eq!(issuance_debit, staking_native_delta);
      assert_eq!(liquidity_local_delta, staking_native_delta);
      assert_eq!(local_supply_credit, staking_native_delta);
      assert_eq!(Balances::reserved_balance(&owner) - initial_reserved, 7 * crate::EXISTENTIAL_DEPOSIT);
      println!("Signed Parked public pool effects: staking_native_delta={staking_native_delta} liquidity_local_delta={liquidity_local_delta} native_issuance_debit={issuance_debit} local_supply_credit={local_supply_credit} pool_touch_reserved_delta={}", Balances::reserved_balance(&owner) - initial_reserved);
      let available = Balances::reducible_balance(&owner, polkadot_sdk::frame_support::traits::tokens::Preservation::Preserve, polkadot_sdk::frame_support::traits::tokens::Fortitude::Polite);
      assert!(available >= 2_632_000 * crate::UNIT);
      println!("Signed Parked public pool prefix: initial_block={initial_block} final_block={block} complete_blocks={} signed_calls={} assets={assets:?} owner_lp={lp_balances:?} owner_native_available={available} owner_native_reserved={} feeds=8 user_admission=false next_actor={initial_next} source_count=0 maximum_normal={:?}", block - initial_block, setup_signed_calls.get(), Balances::reserved_balance(&owner), setup_normal_maximum.get());
    });
    return;
  }
  let first = initial_next;
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &[create(0, Some(parked_contract(&assets)))],
  );
  let sources = externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert_eq!(block - initial_block, 11 + 5 * assets.len() as u32);
    assert_eq!(first, 15);
    assert_eq!(
      Actors::dependency_source_allocator().next,
      u64::from(watch_count)
    );
    assert!(!Actors::dependency_source_allocator().exhausted);
    assets
      .iter()
      .map(|asset| BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)).unwrap())
      .collect::<Vec<_>>()
  });
  assert_eq!(sources, (0..u64::from(watch_count)).collect::<Vec<_>>());
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    for source in &sources {
      let header = DependencyRegistrationHeaders::<Runtime>::get(source);
      assert_eq!(
        (header.count, header.free_count, header.next_index),
        (1, 0, 1)
      );
      let revision = DependencyRevisions::<Runtime>::get(source);
      assert_eq!(revision.revision, 0);
      assert!(revision.scan_target.is_none());
    }
    assert_eq!(
      pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count,
      0
    );
  });
  println!(
    "Signed Parked initial-source birth: relative_block={} actor={first} slot=0 allocator_next={watch_count} source_ids={sources:?}; one slot-zero registration per source, revision zero and no active scan",
    block - initial_block
  );
  retain_current_state(&storage, &parent, "first-source-birth");
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }

  let mut producer_calls = Vec::new();
  let mut producer_ids = Vec::new();
  for (index, asset) in assets.iter().enumerate() {
    let slot = index as u8 + 1;
    let custody = Actors::sovereign_account_id(&owner, slot);
    producer_calls.push(RuntimeCall::Balances(
      polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(custody.clone()),
        value: 320_000 * crate::UNIT,
      },
    ));
    producer_calls.push(RuntimeCall::Assets(
      polkadot_sdk::pallet_assets::Call::transfer {
        id: *asset,
        target: MultiAddress::Id(custody.clone()),
        amount: 10_000_000_000,
      },
    ));
    producer_calls.push(create(
      slot,
      Some(ActorContract {
        trigger: Trigger::cadenced(1),
        cooldown_blocks: 0,
        window: None,
        steps: vec![StepOf::<Runtime> {
          precondition: None,
          task: Task::Transfer {
            to: owner.clone(),
            asset: AssetKind::Local(*asset),
            amount: AmountResolution::Fixed(1),
          },
          on_error: StepErrorPolicy::AbortCycle,
        }]
        .try_into()
        .unwrap(),
        on_unfunded: OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        funding: FundingSourcePolicy::OwnerOnly,
        auto_close_at_cycle_nonce: None,
      }),
    ));
    producer_ids.push(initial_next + 1 + index as u64);
  }
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &producer_calls,
  );
  let after_producers = initial_next + 1 + assets.len() as u64;
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  let temporary = [after_producers, after_producers + 1];
  let second = after_producers + 2;
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &[create(253, None), create(254, None)],
  );
  for _ in 0..64 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  let dormant_holds = externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert!(sources.iter().all(|source| {
      DependencyRevisions::<Runtime>::get(source)
        .scan_target
        .is_some()
    }));
    temporary.map(|actor| Actors::actor_state_hold(actor).unwrap())
  });
  retain_current_state(&storage, &parent, "setup-complete");
  if setup_only {
    {
      let trace = publication_trace.borrow();
      println!(
        "Signed Parked producer custody: maximum_native_unavailable={} maximum_native_block_debit={}",
        trace.maximum_native_unavailable, trace.maximum_native_debit
      );
    }
    assert_eq!(setup_signed_calls.get(), 7 + 40 + 5 * assets.len() + 3);
    assert_eq!(
      block - initial_block,
      85 + 5 * u32::try_from(assets.len()).unwrap()
    );
    println!(
      "Signed Parked setup: blocks={} signed_calls={} maximum_normal={:?}; multiplier never increased",
      block - initial_block,
      setup_signed_calls.get(),
      setup_normal_maximum.get()
    );
    if end == SignedParkedReplayEnd::WideBirth {
      // The setup already supplies a real watched asset, funded creators, a paid producer
      // and active scans. One temporary publication makes this a two-call birth pair.
      advance(
        &mut storage,
        &mut parent,
        &mut block,
        signer,
        &[RuntimeCall::Actors(
          pallet_deos_actors::Call::activate_actor {
            actor_id: temporary[0],
            contract: temporary_contract(&assets),
          },
        )],
      );
      let creator = &owner_pairs[12];
      let creator_account = crate::AccountId::from(creator.public());
      let roster = owner_pairs
        .iter()
        .map(|pair| crate::AccountId::from(pair.public()))
        .collect::<Vec<_>>();
      let mut birth_contract = parked_contract(&assets);
      retention_wide_contract::wide_close_contract(&assets, &roster, &mut birth_contract);
      let snapshot = externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert_eq!(Actors::next_actor_id(), second);
        assert_eq!(System::account_nonce(&creator_account), 0);
        for source in &sources {
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          assert_eq!(
            (header.count, header.free_count, header.next_index),
            (2, 0, 2)
          );
          assert!(
            DependencyRevisions::<Runtime>::get(source)
              .scan_target
              .is_some()
          );
        }
        (
          Actors::actor_identity_count(),
          Actors::active_actor_count(),
          Actors::actor_identity(first).unwrap(),
          Actors::actor_contract(first).unwrap(),
          Actors::actor_state_hold(first).unwrap(),
          ParkedBalanceEpisodes::<Runtime>::get(first)
            .unwrap()
            .encode(),
        )
      });
      let retire_call = RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor {
        actor_id: temporary[0],
      });
      let create_call = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
        mutability: Mutability::Mutable,
        contract: Some(birth_contract.clone()),
      });
      advance_signed(
        &mut storage,
        &mut parent,
        &mut block,
        signer,
        &[(signer, &retire_call), (creator, &create_call)],
      );
      let birth_block = block;
      let inspect = |storage: &Storage, now: u32| {
        externalities_with_bound_code(storage.clone()).execute_with(|| {
          assert_eq!(Actors::next_actor_id(), second + 1);
          assert_eq!(System::account_nonce(&creator_account), 1);
          assert_eq!(
            (Actors::actor_identity_count(), Actors::active_actor_count()),
            (snapshot.0 + 1, snapshot.1)
          );
          let identity = Actors::actor_identity(second).unwrap();
          assert_eq!(identity.owner, creator_account);
          assert_eq!(identity.actor_class, ActorClass::User { owner_slot: 0 });
          assert_eq!(identity.cycle_nonce, 0);
          assert_eq!(Actors::actor_contract(second).unwrap(), birth_contract);
          assert!(Actors::actor_state_hold(second).is_some());
          assert!(Actors::actor_run_state(second).is_none());
          assert!(!pallet_deos_actors::ServiceNodes::<Runtime>::contains_key(
            second
          ));
          assert!(!pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::contains_key(second));
          let hot = Actors::actor_hot(second).unwrap();
          assert!(!hot.pending_signal);
          assert_eq!(hot.terminal_at, Some(u32::MAX));
          assert!(matches!(
            Actors::actor_processes(second).unwrap().residence,
            Some(ProcessResidence::Parked(_))
          ));
          assert_eq!(
            crate::configs::actor_config::TmctolAssetOps::total_balance(
              &identity.sovereign_account,
              AssetKind::Native
            ),
            0
          );
          for (asset, source) in assets.iter().zip(&sources) {
            assert_eq!(
              crate::configs::actor_config::TmctolAssetOps::total_balance(
                &identity.sovereign_account,
                AssetKind::Local(*asset)
              ),
              0
            );
            assert_eq!(
              BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)),
              Some(*source)
            );
            let header = DependencyRegistrationHeaders::<Runtime>::get(source);
            assert_eq!(
              (header.count, header.free_count, header.next_index),
              (2, 1, 3)
            );
            assert_eq!(
              DependencyRegistrationPositions::<Runtime>::get(source, second),
              Some(DependencyRegistrationPosition { page: 0, slot: 2 })
            );
            assert_eq!(
              DependencyRegistrationFreePositions::<Runtime>::get(source, 0),
              Some(DependencyRegistrationPosition { page: 0, slot: 1 })
            );
            assert_eq!(
              DependencyRegistrationFreeIndices::<Runtime>::get(
                source,
                DependencyRegistrationPosition { page: 0, slot: 1 }
              ),
              Some(0)
            );
            assert!(
              DependencyRevisions::<Runtime>::get(source)
                .scan_target
                .is_some()
            );
          }
          for tail in 0..3 {
            assert!(
              pallet_deos_actors::ActorContractTailChunks::<Runtime>::contains_key(second, tail)
            );
          }
          assert_eq!(
            pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count,
            watch_count
          );
          assert_eq!(
            (
              Actors::actor_identity(first).unwrap(),
              Actors::actor_contract(first).unwrap(),
              Actors::actor_state_hold(first).unwrap(),
              ParkedBalanceEpisodes::<Runtime>::get(first)
                .unwrap()
                .encode()
            ),
            (
              snapshot.2.clone(),
              snapshot.3.clone(),
              snapshot.4.clone(),
              snapshot.5.clone()
            )
          );
          let review = pallet_deos_actors::DependencyTimedReviews::<Runtime>::get(second)
            .or_else(|| pallet_deos_actors::PendingDependencyReviews::<Runtime>::get(second))
            .unwrap();
          assert!(
            matches!(review.deadline, WakeupKey::Block(due) if due > birth_block && due <= now + 1)
          );
          review.deadline
        })
      };
      let initial_review = inspect(&storage, block);
      let mut advanced = false;
      for _ in 0..4 {
        advance(&mut storage, &mut parent, &mut block, signer, &[]);
        advanced |= inspect(&storage, block) != initial_review;
      }
      assert!(advanced);
      assert_eq!(block - initial_block, 91 + 5 * assets.len() as u32);
      println!(
        "Signed Parked wide birth: birth_block={birth_block} blocks={} actor={second} source_frontiers=3 retained_holes=1 steps=12 predicates_per_step=4 task=RemoveLiquidity source_allowlist=16 asset_allowlist=16 funding_allowlist=16 creator_slot=0 terminal_at={} review_advanced={advanced}; no Trigger deadline, Run, Service or custody; prior authority preserved",
        block - initial_block,
        u32::MAX
      );
      return;
    }
    if end == SignedParkedReplayEnd::Rotation {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        let period = crate::configs::Period::get();
        assert_eq!(initial_block, period - 32);
        assert_eq!(block, period - 32 + 85 + 5 * watch_count);
        assert_eq!(crate::Session::current_index(), 1);
        assert_eq!(
          crate::Staking::native_security_mode(),
          pallet_staking::NativeSecurityMode::TrustedSet
        );
        assert!(
          pallet_staking::NativeSecurityRewardPots::<Runtime>::iter_keys()
            .next()
            .is_none()
        );
        assert!(
          pallet_staking::NativeSecurityEpochSnapshots::<Runtime>::iter_keys()
            .next()
            .is_none()
        );
        assert!(crate::Staking::active_native_security_epoch_snapshot().is_none());
        assert_eq!(crate::Staking::native_security_reward_liability(), 0);
        assert_eq!(
          pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count,
          watch_count
        );
      });
      let trace = publication_trace.borrow();
      assert_eq!(trace.producers.len(), watch_count as usize);
      assert!(trace.publications > 0);
      assert!(trace.maximum_gap <= 9);
      println!(
        "Signed Parked rotation: blocks={}..={} session_index=1 producer_transfers={} maximum_gap={} maximum_block_bucket={} maximum_tick_bucket={}; no optional-work halt and {watch_count} active scans",
        initial_block + 1,
        block,
        trace.publications,
        trace.maximum_gap,
        trace.maximum_block_bucket,
        trace.maximum_tick_bucket
      );
    }
    return;
  }
  let mut active_temporary = None;
  let mut turn = 0usize;
  // Alternating two identities admits one new registration per block without violating
  // the per-identity mutation barrier or paying Creation again for every frontier slot.
  let bootstrap_gap_epochs = if matches!(
    end,
    SignedParkedReplayEnd::Birth | SignedParkedReplayEnd::BirthPair
  ) {
    32
  } else {
    (assets.len() * 32).div_ceil(5)
  };
  for (phase_index, watches) in [assets.clone(), assets.clone(), assets.clone()]
    .into_iter()
    .enumerate()
  {
    let rounds = if bootstrap_only && phase_index == 1 {
      bootstrap_gap_epochs
    } else if phase_index == 1 {
      32
    } else {
      31
    };
    for _ in 0..rounds {
      let before = externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert!(sources.iter().all(|source| {
          DependencyRevisions::<Runtime>::get(source)
            .scan_target
            .is_some()
        }));
        sources
          .iter()
          .map(|source| DependencyRegistrationHeaders::<Runtime>::get(source).next_index)
          .collect::<Vec<_>>()
      });
      let actor = temporary[turn % 2];
      let mut calls = Vec::new();
      if let Some(previous) = active_temporary {
        calls.push(RuntimeCall::Actors(
          pallet_deos_actors::Call::deactivate_actor { actor_id: previous },
        ));
      }
      calls.push(RuntimeCall::Actors(
        pallet_deos_actors::Call::activate_actor {
          actor_id: actor,
          contract: temporary_contract(&watches),
        },
      ));
      let fee_before = externalities_with_bound_code(storage.clone()).execute_with(|| {
        polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get()
      });
      advance(&mut storage, &mut parent, &mut block, signer, &calls);
      active_temporary = Some(actor);
      turn += 1;
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        let contract = Actors::actor_contract(actor).unwrap();
        assert_eq!(contract.steps.len(), 1);
        assert_eq!(
          contract.parked_balance_activation.unwrap().watches.len(),
          watches.len()
        );
        assert_eq!(Actors::next_actor_id(), second + u64::from(turn > 63));
        for (index, source) in sources.iter().enumerate() {
          let watched = watches.contains(&assets[index]);
          assert_eq!(
            DependencyRegistrationHeaders::<Runtime>::get(source).next_index,
            before[index] + u64::from(watched)
          );
          if watched {
            assert_eq!(
              DependencyRegistrationPositions::<Runtime>::get(source, actor),
              Some(DependencyRegistrationPosition {
                page: before[index] / 32,
                slot: (before[index] % 32) as u8
              })
            );
          }
        }
      });
      // The capital recipe spaces every temporary epoch, not just each phase's first one.
      for _ in 0..4 {
        advance(&mut storage, &mut parent, &mut block, signer, &[]);
      }
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert!(
          polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get()
            <= fee_before
        );
        assert!(sources.iter().all(|source| {
          DependencyRevisions::<Runtime>::get(source)
            .scan_target
            .is_some()
        }));
      });
    }
    if turn == 31 {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        for source in &sources {
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          let scan = DependencyRevisions::<Runtime>::get(source);
          assert_eq!(
            (header.count, header.free_count, header.next_index),
            (2, 30, 32)
          );
          assert_eq!(header.first_page, Some(0));
          assert!(scan.scan_target.is_some());
          assert!(scan.scan_end > 0 && scan.scan_end <= header.next_index);
        }
      });
      println!(
        "Signed Parked churn: full-page registration frontier established; retained scan may lag"
      );
    }
    if turn == 63 {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        for source in &sources {
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          let scan = DependencyRevisions::<Runtime>::get(source);
          assert_eq!(header.first_page, Some(0));
          assert!(scan.scan_target.is_some());
          assert!(scan.scan_end >= 32 && scan.scan_end <= header.next_index);
        }
      });
      println!(
        "Signed Parked churn: full-page scan base established through the temporary-page gap phase"
      );
    }
    let birth_pair = end == SignedParkedReplayEnd::BirthPair && phase_index == 1;
    if !birth_pair {
      advance(
        &mut storage,
        &mut parent,
        &mut block,
        signer,
        &[RuntimeCall::Actors(
          pallet_deos_actors::Call::deactivate_actor {
            actor_id: active_temporary.take().unwrap(),
          },
        )],
      );
      for _ in 0..4 {
        advance(&mut storage, &mut parent, &mut block, signer, &[]);
      }
    }
    if phase_index == 0 {
      retain_current_state(&storage, &parent, "first-cohort-holes");
    }
    if bootstrap_only {
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert_eq!(
          pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count as usize,
          assets.len()
        );
        for source in &sources {
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          let scan = DependencyRevisions::<Runtime>::get(source);
          let target = scan.scan_target.expect("bootstrap scan remains active");
          assert_eq!(header.count, 1 + u32::from(birth_pair));
          assert_eq!(header.free_count, 31);
          assert_eq!(header.first_page, Some(0));
          assert_eq!(header.next_index, 32 + if phase_index == 0 {
            0
          } else {
            bootstrap_gap_epochs as u64
          });
          assert!(scan.scan_end <= header.next_index);
          if phase_index == 0 {
            assert!(scan.scan_end >= 32 || scan.revision > target);
            println!(
              "Signed Parked bootstrap input: source={source} cursor={} end={} target={target} revision={}",
              scan.scan_cursor, scan.scan_end, scan.revision
            );
          } else {
            assert!(scan.scan_end >= 32);
          }
          for offset in 0..31u64 {
            let position = DependencyRegistrationPosition {
              page: 0,
              slot: offset as u8 + 1,
            };
            assert_eq!(
              DependencyRegistrationFreePositions::<Runtime>::get(source, offset),
              Some(position)
            );
            assert_eq!(
              DependencyRegistrationFreeIndices::<Runtime>::get(source, position),
              Some(offset)
            );
          }
        }
      });
      if phase_index == 1 {
        assert_eq!(turn, 31 + bootstrap_gap_epochs);
        assert_eq!(
          block - initial_block,
          85 + 5 * assets.len() as u32 + 5 * (31 + bootstrap_gap_epochs as u32 + 2)
            - 5 * u32::from(birth_pair)
        );
        let trace = publication_trace.borrow();
        println!(
          "Signed Parked bootstrap: blocks={} gap_epochs={bootstrap_gap_epochs} producer_transfers={} maximum_gap={} maximum_block_bucket={} maximum_tick_bucket={}; all source scans have end >= 32",
          block - initial_block,
          trace.publications,
          trace.maximum_gap,
          trace.maximum_block_bucket,
          trace.maximum_tick_bucket
        );
        drop(trace);
        if matches!(
          end,
          SignedParkedReplayEnd::Birth | SignedParkedReplayEnd::BirthPair
        ) {
          let first_snapshot = externalities_with_bound_code(storage.clone()).execute_with(|| {
            (
              Actors::actor_identity(first).unwrap(),
              Actors::actor_contract(first).unwrap(),
              Actors::actor_state_hold(first).unwrap(),
              ParkedBalanceEpisodes::<Runtime>::get(first)
                .unwrap()
                .encode(),
            )
          });
          let creator = &owner_pairs[12];
          let creator_account = crate::AccountId::from(creator.public());
          let mut birth_contract = parked_contract(&assets);
          birth_contract.funding = FundingSourcePolicy::SignedAllowlist(
            owner_pairs[..16]
              .iter()
              .map(|pair| crate::AccountId::from(pair.public()))
              .collect::<BTreeSet<_>>()
              .try_into()
              .unwrap(),
          );
          let counts = externalities_with_bound_code(storage.clone()).execute_with(|| {
            assert_eq!(Actors::next_actor_id(), second);
            assert_eq!(System::account_nonce(&creator_account), 0);
            assert!(Actors::actor_identity(second).is_none());
            for source in &sources {
              let header = DependencyRegistrationHeaders::<Runtime>::get(source);
              assert_eq!(
                (header.count, header.free_count, header.next_index),
                if birth_pair { (2, 31, 64) } else { (1, 31, 64) }
              );
              assert_eq!(
                (header.first_page, header.last_page),
                (Some(0), Some(u64::from(birth_pair)))
              );
              if birth_pair {
                let retired = active_temporary.unwrap();
                assert_eq!(
                  DependencyRegistrationPositions::<Runtime>::get(source, retired),
                  Some(DependencyRegistrationPosition { page: 1, slot: 31 })
                );
                let page = DependencyRegistrationPages::<Runtime>::get(source, 1).unwrap();
                assert_eq!((page.previous, page.next), (Some(0), None));
              } else {
                assert!(DependencyRegistrationPages::<Runtime>::get(source, 1).is_none());
              }
              assert!(DependencyRegistrationPages::<Runtime>::get(source, 2).is_none());
              assert!(
                DependencyRevisions::<Runtime>::get(source)
                  .scan_target
                  .is_some()
              );
            }
            (Actors::actor_identity_count(), Actors::active_actor_count())
          });
          let create_call = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
            mutability: Mutability::Mutable,
            contract: Some(birth_contract.clone()),
          });
          if birth_pair {
            let retired = active_temporary.take().unwrap();
            let retire_call =
              RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor { actor_id: retired });
            let retire_owner = &owner_pairs[usize::from(retired == temporary[1])];
            advance_signed(
              &mut storage,
              &mut parent,
              &mut block,
              signer,
              &[(retire_owner, &retire_call), (creator, &create_call)],
            );
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              assert!(Actors::actor_identity(retired).is_some());
              assert!(Actors::actor_contract(retired).is_none());
              assert!(Actors::actor_processes(retired).is_none());
            });
          } else {
            advance(
              &mut storage,
              &mut parent,
              &mut block,
              creator,
              &[create_call],
            );
          }
          let birth_block = block;
          let inspect_birth = |storage: &Storage, now: u32| {
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              assert_eq!(Actors::next_actor_id(), second + 1);
              assert_eq!(System::account_nonce(&creator_account), 1);
              assert_eq!(
                (Actors::actor_identity_count(), Actors::active_actor_count()),
                (counts.0 + 1, counts.1 + u32::from(!birth_pair))
              );
              let identity = Actors::actor_identity(second).unwrap();
              assert_eq!(identity.owner, creator_account);
              assert_eq!(identity.actor_class, pallet_deos_actors::ActorClass::User { owner_slot: 0 });
              assert_eq!(identity.cycle_nonce, 0);
              assert_eq!(Actors::actor_contract(second).unwrap(), birth_contract);
              assert!(Actors::actor_state_hold(second).is_some());
              assert!(Actors::actor_run_state(second).is_none());
              assert!(!pallet_deos_actors::ServiceNodes::<Runtime>::contains_key(second));
              let process = Actors::actor_processes(second).unwrap();
              assert!(matches!(process.residence, Some(ProcessResidence::Parked(_))));
              assert_eq!(
                crate::configs::actor_config::TmctolAssetOps::total_balance(
                  &identity.sovereign_account, AssetKind::Native
                ),
                0
              );
              for (asset, source) in assets.iter().zip(&sources) {
                assert_eq!(
                  crate::configs::actor_config::TmctolAssetOps::total_balance(
                    &identity.sovereign_account, AssetKind::Local(*asset)
                  ),
                  0
                );
                assert_eq!(BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)), Some(*source));
                let header = DependencyRegistrationHeaders::<Runtime>::get(source);
                assert_eq!((header.count, header.free_count, header.next_index), (2, 31, 65));
                assert_eq!((header.first_page, header.last_page), (Some(0), Some(2)));
                let first_page = DependencyRegistrationPages::<Runtime>::get(source, 0).unwrap();
                let new_page = DependencyRegistrationPages::<Runtime>::get(source, 2).unwrap();
                assert_eq!((first_page.previous, first_page.next), (None, Some(2)));
                assert_eq!((new_page.previous, new_page.next), (Some(0), None));
                assert_eq!(DependencyRegistrationPositions::<Runtime>::get(source, second), Some(DependencyRegistrationPosition { page: 2, slot: 0 }));
                assert!(DependencyRegistrationPages::<Runtime>::get(source, 1).is_none());
                for offset in 0..31u64 {
                  let hole = DependencyRegistrationPosition { page: 0, slot: offset as u8 + 1 };
                  assert_eq!(DependencyRegistrationFreePositions::<Runtime>::get(source, offset), Some(hole));
                  assert_eq!(DependencyRegistrationFreeIndices::<Runtime>::get(source, hole), Some(offset));
                }
                let scan = DependencyRevisions::<Runtime>::get(source);
                assert!(scan.scan_target.is_some() && !scan.exhausted);
                assert!(scan.scan_end >= 32 && scan.scan_end <= header.next_index);
              }
              assert_eq!(pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count, watch_count);
              assert_eq!(
                (
                  Actors::actor_identity(first).unwrap(),
                  Actors::actor_contract(first).unwrap(),
                  Actors::actor_state_hold(first).unwrap(),
                  ParkedBalanceEpisodes::<Runtime>::get(first).unwrap().encode(),
                ),
                first_snapshot
              );
              let review = pallet_deos_actors::DependencyTimedReviews::<Runtime>::get(second)
                .or_else(|| pallet_deos_actors::PendingDependencyReviews::<Runtime>::get(second))
                .unwrap();
              assert!(matches!(review.deadline, WakeupKey::Block(due) if due > birth_block && due <= now + 1));
              review.deadline
            })
          };
          let initial_review = inspect_birth(&storage, block);
          let mut review_advanced = false;
          for _ in 0..4 {
            advance(&mut storage, &mut parent, &mut block, signer, &[]);
            review_advanced |= inspect_birth(&storage, block) != initial_review;
          }
          assert!(
            review_advanced,
            "the new permanent Actor receives a negative review and rearms"
          );
          assert_eq!(
            block - initial_block,
            85 + 5 * assets.len() as u32 + 5 * (31 + bootstrap_gap_epochs as u32 + 3)
              - 5 * u32::from(birth_pair)
          );
          println!(
            "Signed Parked birth: birth_block={birth_block} blocks={} actor={second} source_frontiers=65 first_page_holes=31 new_page=2 steps=12 signed_allowlist=16 creator_slot=0 review_advanced={review_advanced} paired_retirement={birth_pair}; retained authority, zero custody and {watch_count} active scans preserved",
            block - initial_block
          );
          return;
        }
        if end == SignedParkedReplayEnd::WatchMasks {
          let permanent_snapshot = |storage: &Storage| {
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              (
                Actors::actor_identity(first).unwrap(),
                Actors::actor_contract(first).unwrap(),
                Actors::actor_state_hold(first).unwrap(),
                ParkedBalanceEpisodes::<Runtime>::get(first)
                  .unwrap()
                  .encode(),
              )
            })
          };
          let permanent = permanent_snapshot(&storage);
          let mut frontiers = sources
            .iter()
            .map(|source| {
              externalities_with_bound_code(storage.clone())
                .execute_with(|| DependencyRegistrationHeaders::<Runtime>::get(source).next_index)
            })
            .collect::<Vec<_>>();
          let initial_frontier = 32 + bootstrap_gap_epochs as u64;
          assert_eq!(frontiers, vec![initial_frontier; assets.len()]);
          let inspect = |storage: &Storage,
                         current: Option<(ActorId, usize)>,
                         frontiers: &[u64]| {
            assert_eq!(permanent_snapshot(storage), permanent);
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              assert_eq!(Actors::next_actor_id(), second);
              let allocator = pallet_deos_actors::DependencySourceAllocatorState::<Runtime>::get();
              assert_eq!(allocator.next, assets.len() as u64);
              assert!(!allocator.exhausted);
              assert_eq!(
                pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().count as usize,
                assets.len()
              );
              for actor in [first, temporary[0], temporary[1]] {
                let identity = Actors::actor_identity(actor).unwrap();
                assert_eq!(identity.cycle_nonce, 0);
                assert_eq!(
                  crate::configs::actor_config::TmctolAssetOps::total_balance(
                    &identity.sovereign_account,
                    AssetKind::Native
                  ),
                  0
                );
                for asset in &assets {
                  assert_eq!(
                    crate::configs::actor_config::TmctolAssetOps::total_balance(
                      &identity.sovereign_account,
                      AssetKind::Local(*asset)
                    ),
                    0
                  );
                }
                assert!(Actors::actor_run_state(actor).is_none());
                assert!(!pallet_deos_actors::ServiceNodes::<Runtime>::contains_key(
                  actor
                ));
                if actor == first || current.is_some_and(|(active, _)| active == actor) {
                  assert!(matches!(
                    pallet_deos_actors::ActorProcesses::<Runtime>::get(actor)
                      .unwrap()
                      .residence,
                    Some(ProcessResidence::Parked(_))
                  ));
                } else {
                  assert!(!pallet_deos_actors::ActorProcesses::<Runtime>::contains_key(actor));
                  assert!(ParkedBalanceEpisodes::<Runtime>::get(actor).is_none());
                }
              }
              for (index, source) in sources.iter().enumerate() {
                assert_eq!(
                  BalanceDependencySources::<Runtime>::get(AssetKind::Local(assets[index])),
                  Some(*source)
                );
                let watched = current.is_some_and(|(_, mask)| mask & (1 << index) != 0);
                let header = DependencyRegistrationHeaders::<Runtime>::get(source);
                assert_eq!(header.next_index, frontiers[index]);
                assert_eq!(header.count, 1 + u32::from(watched));
                assert_eq!(header.free_count, 31);
                assert_eq!(header.first_page, Some(0));
                assert_eq!(
                  DependencyRegistrationPositions::<Runtime>::get(source, first),
                  Some(DependencyRegistrationPosition { page: 0, slot: 0 })
                );
                for actor in temporary {
                  let position = (watched && current.is_some_and(|(active, _)| active == actor))
                    .then_some(DependencyRegistrationPosition {
                      page: (frontiers[index] - 1) / 32,
                      slot: ((frontiers[index] - 1) % 32) as u8,
                    });
                  assert_eq!(
                    DependencyRegistrationPositions::<Runtime>::get(source, actor),
                    position
                  );
                }
                for offset in 0..31u64 {
                  let position = DependencyRegistrationPosition {
                    page: 0,
                    slot: offset as u8 + 1,
                  };
                  assert_eq!(
                    DependencyRegistrationFreePositions::<Runtime>::get(source, offset),
                    Some(position)
                  );
                  assert_eq!(
                    DependencyRegistrationFreeIndices::<Runtime>::get(source, position),
                    Some(offset)
                  );
                }
                let scan = DependencyRevisions::<Runtime>::get(source);
                assert!(scan.scan_target.is_some() && !scan.exhausted);
                assert!(scan.scan_end >= 32 && scan.scan_end <= header.next_index);
              }
            });
          };
          inspect(&storage, None, &frontiers);
          for mask in 1usize..(1 << assets.len()) {
            let actor = temporary[turn % 2];
            let watches = assets
              .iter()
              .enumerate()
              .filter_map(|(index, asset)| (mask & (1 << index) != 0).then_some(*asset))
              .collect::<Vec<_>>();
            let mut calls = Vec::new();
            if let Some(previous) = active_temporary {
              calls.push(RuntimeCall::Actors(
                pallet_deos_actors::Call::deactivate_actor { actor_id: previous },
              ));
            }
            calls.push(RuntimeCall::Actors(
              pallet_deos_actors::Call::activate_actor {
                actor_id: actor,
                contract: temporary_contract(&watches),
              },
            ));
            let multiplier = externalities_with_bound_code(storage.clone()).execute_with(|| {
              let multiplier =
                polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get();
              assert!(
                multiplier <= polkadot_sdk::pallet_transaction_payment::Multiplier::from_u32(1)
              );
              multiplier
            });
            advance(&mut storage, &mut parent, &mut block, signer, &calls);
            active_temporary = Some(actor);
            turn += 1;
            for (index, frontier) in frontiers.iter_mut().enumerate() {
              *frontier += u64::from(mask & (1 << index) != 0);
            }
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              assert_eq!(
                Actors::actor_contract(actor).unwrap(),
                temporary_contract(&watches)
              );
            });
            inspect(&storage, Some((actor, mask)), &frontiers);
            for _ in 0..4 {
              advance(&mut storage, &mut parent, &mut block, signer, &[]);
              inspect(&storage, Some((actor, mask)), &frontiers);
            }
            externalities_with_bound_code(storage.clone()).execute_with(|| {
              assert!(
                polkadot_sdk::pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get()
                  <= multiplier
              );
            });
          }
          advance(
            &mut storage,
            &mut parent,
            &mut block,
            signer,
            &[RuntimeCall::Actors(
              pallet_deos_actors::Call::deactivate_actor {
                actor_id: active_temporary.take().unwrap(),
              },
            )],
          );
          inspect(&storage, None, &frontiers);
          for _ in 0..4 {
            advance(&mut storage, &mut parent, &mut block, signer, &[]);
            inspect(&storage, None, &frontiers);
          }
          assert_eq!(
            frontiers,
            vec![initial_frontier + (1u64 << (assets.len() - 1)); assets.len()]
          );
          externalities_with_bound_code(storage.clone()).execute_with(|| {
            for source in &sources {
              let header = DependencyRegistrationHeaders::<Runtime>::get(source);
              assert_eq!(header.last_page, Some(0));
              assert!(DependencyRegistrationPages::<Runtime>::get(source, 1).is_none());
              assert!(DependencyRegistrationPages::<Runtime>::get(source, 2).is_none());
            }
            for (actor, hold) in temporary.into_iter().zip(&dormant_holds) {
              assert_eq!(Actors::actor_state_hold(actor).as_ref(), Some(hold));
            }
          });
          let trace = publication_trace.borrow();
          println!(
            "Signed Parked watch masks: masks={} blocks={} frontiers={frontiers:?} producer_transfers={} maximum_gap={}; permanent custody/episode/Contract/hold and 31 paired holes preserved; temporary pages reclaimed",
            (1usize << assets.len()) - 1,
            block - initial_block,
            trace.publications,
            trace.maximum_gap
          );
        }
        return;
      }
    }
    // The first cohort retains page 0; source-only gap epochs retain no new page.
    // Publish the second permanent cohort only after the temporary-page gap is complete.
    if phase_index == 1 {
      advance(
        &mut storage,
        &mut parent,
        &mut block,
        signer,
        &[create(
          u8::try_from(assets.len() + 1).unwrap(),
          Some(parked_contract(&assets)),
        )],
      );
      for _ in 0..4 {
        advance(&mut storage, &mut parent, &mut block, signer, &[]);
      }
    }
  }
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert_eq!(Actors::next_actor_id(), second + 1);
    assert_eq!(Actors::system_sovereign_count(), original_system_sovereigns);
    assert!(Balances::free_balance(&owner) < initial_owner_free);
    for (index, actor) in temporary.iter().enumerate() {
      assert_eq!(
        Actors::actor_state_hold(*actor),
        Some(dormant_holds[index].clone())
      );
      assert!(Actors::active_actor_state(*actor).is_none());
    }
    for source in &sources {
      let page = 2;
      let header = DependencyRegistrationHeaders::<Runtime>::get(source);
      assert_eq!(
        (header.count, header.free_count, header.next_index),
        (2, 62, 32 * (page + 1))
      );
      assert_eq!(
        DependencyRegistrationPositions::<Runtime>::get(source, second),
        Some(DependencyRegistrationPosition { page, slot: 0 })
      );
      if page == 2 {
        assert!(!DependencyRegistrationPages::<Runtime>::contains_key(
          source, 1
        ));
      }
      for offset in 0..62u64 {
        let position = DependencyRegistrationPosition {
          page: if offset < 31 { 0 } else { page },
          slot: (offset % 31 + 1) as u8,
        };
        assert_eq!(
          DependencyRegistrationFreePositions::<Runtime>::get(source, offset),
          Some(position)
        );
        assert_eq!(
          DependencyRegistrationFreeIndices::<Runtime>::get(source, position),
          Some(offset)
        );
      }
    }
    for actor in [first, second] {
      assert!(matches!(
        Actors::actor_identity(actor).unwrap().actor_class,
        ActorClass::User { .. }
      ));
      assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(actor));
      let custody = Actors::actor_identity(actor).unwrap().sovereign_account;
      assert!(assets.iter().all(
        |asset| crate::configs::actor_config::TmctolAssetOps::total_balance(
          &custody,
          AssetKind::Local(*asset)
        ) == 0
      ));
    }
    assert!(producer_ids.iter().all(|actor| {
      Actors::active_actor_state(*actor)
        .unwrap()
        .identity
        .cycle_nonce
        > 0
    }));
  });
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &[
      RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: first }),
      RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: second }),
    ],
  );
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert!(
      assets
        .iter()
        .all(|asset| !BalanceDependencySources::<Runtime>::contains_key(AssetKind::Local(*asset)))
    );
  });
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &[RuntimeCall::Actors(
      pallet_deos_actors::Call::activate_actor {
        actor_id: temporary[0],
        contract: temporary_contract(&assets),
      },
    )],
  );
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    for (index, asset) in assets.iter().enumerate() {
      let source = BalanceDependencySources::<Runtime>::get(AssetKind::Local(*asset)).unwrap();
      assert!(source > sources[index]);
      let header = DependencyRegistrationHeaders::<Runtime>::get(source);
      assert_eq!(
        (header.count, header.free_count, header.next_index),
        (1, 0, 1)
      );
    }
  });
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  advance(
    &mut storage,
    &mut parent,
    &mut block,
    signer,
    &temporary
      .map(|actor| RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: actor })),
  );
  for _ in 0..4 {
    advance(&mut storage, &mut parent, &mut block, signer, &[]);
  }
  externalities_with_bound_code(storage).execute_with(|| {
    assert_eq!(Actors::next_actor_id(), second + 1);
    assert!(
      assets
        .iter()
        .all(|asset| !BalanceDependencySources::<Runtime>::contains_key(AssetKind::Local(*asset)))
    );
    assert!(
      temporary
        .iter()
        .all(|actor| Actors::actor_state_hold(*actor).is_none())
    );
    for pair in &owner_pairs[..2] {
      assert!(
        Balances::free_balance(crate::AccountId::from(pair.public()))
          >= 30_000 * crate::UNIT + operator_epoch_cash
      );
    }
  });
  // --- Publication Trace Summary ---

  let trace = publication_trace.borrow();
  assert_eq!(trace.producers.len(), watch_count as usize);
  assert!(trace.publications > 0);
  println!(
    "Signed Parked publication trace: {} producers, {} committed Transfers, maximum gap {} blocks",
    trace.producers.len(),
    trace.publications,
    trace.maximum_gap
  );
  let setup_blocks = 85 + 5 * u32::try_from(assets.len()).unwrap();
  let paced_epoch_blocks = 5 * (31 + 32 + 31);
  // Three phase exits, the second permanent birth and three reclamation blocks.
  let paced_boundary_blocks = 5 * 7;
  assert_eq!(
    block - initial_block,
    setup_blocks + paced_epoch_blocks + paced_boundary_blocks
  );
  println!(
    "Signed Parked fee pacing: complete_blocks={} signed_parent_multiplier_upper={} boundary_mutations=7 boundary_spacers=28; every signed block starts at multiplier at most one",
    block - initial_block,
    signed_parent_multiplier_maximum.get().into_inner()
  );
}

/// Root preparation isolates the signed suffix fixture from the long governance prefix.
/// Passing this diagnostic supplies no signed-governance or full-history qualification.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_one_watch_signed_churn_suffix_reconciles_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert!(!Actors::user_creation_enabled());
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| replay_signed_parked_user_epoch_churn(&signer, parent));
    eprintln!(
      "Root-prepared signed churn suffix reconciled: {preset}; governance prefix remains independent"
    );
  }
}

/// Clock/Root preparation isolates the real scheduled hook from a long prefix.
/// The contiguous suffix is independently authored/imported, not public history to its parent.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn clock_prepared_parked_signed_setup_preserves_service_through_session_rotation() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let start = crate::configs::Period::get() - 32;
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      System::set_block_number(start);
      prepare_canonical_system_hash_window(start);
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], start);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(
        &signer,
        parent,
        SignedParkedReplayEnd::Rotation,
      )
    });
    eprintln!("Clock-prepared signed Parked rotation reconciled: {preset}");
  }
}

/// The exact setup calls run before churn, with four empty blocks after each pool batch.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_setup_keeps_fee_multiplier_bounded_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, SignedParkedReplayEnd::Setup)
    });
    eprintln!("Root-prepared signed Parked setup reconciled: {preset}");
  }
}

/// A finite one-watch gap establishes the full retained-page scan base.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_bootstrap_establishes_full_page_scans_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(
        &signer,
        parent,
        SignedParkedReplayEnd::Bootstrap,
      )
    });
    eprintln!("Root-prepared signed Parked bootstrap reconciled: {preset}");
  }
}

/// Ordinary signed creation appends a new permanent page while scans protect retained holes.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_birth_preserves_active_scans_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, SignedParkedReplayEnd::Birth)
    });
    eprintln!("Root-prepared signed Parked birth reconciled: {preset}");
  }
}

/// A maximum-width AddressEvent/window Contract stays parked through public signed birth.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_wide_birth_preserves_idle_authority_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(
        &signer,
        parent,
        SignedParkedReplayEnd::WideBirth,
      )
    });
    eprintln!("Root-prepared signed Parked wide birth reconciled: {preset}");
  }
}

/// The repeated construction retires a temporary identity and creates a permanent one together.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_birth_pair_preserves_active_scans_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(
        &signer,
        parent,
        SignedParkedReplayEnd::BirthPair,
      )
    });
    eprintln!("Root-prepared signed Parked paired birth reconciled: {preset}");
  }
}

/// Retain proof files for one preset and an explicitly selected finite replay endpoint.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "bounded retained prefix evidence; requires a fresh output directory"]
fn retained_parked_funding_recorder_selected_preset() {
  let output = std::env::var("DEOS_PROOF_PREFIX_OUTPUT").unwrap();
  let preset = std::env::var("DEOS_FUNDING_PRESET").unwrap();
  assert!(crate::genesis_config_presets::preset_names().contains(&preset));
  let end = match std::env::var("DEOS_PARKED_REPLAY_END") {
    Ok(value) if value == "complete" => SignedParkedReplayEnd::Complete,
    Err(std::env::VarError::NotPresent) => SignedParkedReplayEnd::BirthPair,
    _ => panic!("DEOS_PARKED_REPLAY_END must be unset or complete"),
  };
  let directory = std::path::Path::new(&output);
  assert!(directory.is_dir());
  assert!(std::fs::read_dir(directory).unwrap().next().is_none());
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
  ext.execute_with(|| {
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
  });
  let storage = ext.execute_with(current_top_storage);
  let parent = parent_header_for(storage, &[], 0);
  ext.execute_with(|| replay_signed_parked_user_epoch_churn_to_phase(&signer, parent, end));
  eprintln!("Retained selected Parked replay reconciled: {preset}");
}

/// Every nonempty subset of the current watch domain preserves the retained page.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn root_prepared_parked_signed_watch_masks_preserve_custody_and_sources_both_presets() {
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    ext.execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
    });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage, &[], 0);
    ext.execute_with(|| {
      replay_signed_parked_user_epoch_churn_to_phase(
        &signer,
        parent,
        SignedParkedReplayEnd::WatchMasks,
      )
    });
    eprintln!("Root-prepared signed Parked watch masks reconciled: {preset}");
  }
}

/// Native signed Governance preparation owns the prefix; this helper authors/imports only
/// the contiguous complete-block suffix from that exact prepared state.
#[cfg(not(feature = "runtime-benchmarks"))]
fn assert_frozen_ingress_candidate_rejected_by_full_executive(
  pre_state: Storage,
  parent: &Header,
  block_number: u32,
  signer: &sr25519::Pair,
) {
  let mut ext = TestExternalities::new(pre_state);
  ext.execute_with(|| {
    assert!(Actors::global_circuit_breaker());
    let signer_account = crate::AccountId::from(signer.public());
    let nonce = System::account_nonce(&signer_account);
    let fee_sink = Actors::active_actor_state(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
      .expect("declared active Fee Sink")
      .identity
      .sovereign_account;
    let mut extrinsics = inherent_data_for(parent, block_number).create_extrinsics();
    for (offset, destination) in [super::common::BOB, fee_sink.clone()]
      .into_iter()
      .enumerate()
    {
      let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
        dest: MultiAddress::Id(destination),
        value: crate::EXISTENTIAL_DEPOSIT,
      });
      extrinsics.push(actors_integration_tests::signed_extrinsic(
        signer,
        nonce + u32::try_from(offset).unwrap(),
        call,
      ));
    }
    let block = Block {
      header: Header::new(
        block_number,
        Default::default(),
        Default::default(),
        parent.hash(),
        Digest {
          logs: vec![DigestItem::PreRuntime(
            polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
            u64::from(block_number).encode(),
          )],
        },
      ),
      extrinsics,
    };
    let before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    let recipient_before = Balances::free_balance(super::common::BOB);
    let collected_before = Balances::free_balance(&fee_sink);
    let rejected =
      polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
        Executive::initialize_block(&block.header);
        for extrinsic in &block.extrinsics[..block.extrinsics.len() - 1] {
          assert_ok!(
            Executive::apply_extrinsic(extrinsic.clone()).expect("valid candidate prefix")
          );
        }
        assert_eq!(System::account_nonce(&signer_account), nonce + 1);
        assert_eq!(
          Balances::free_balance(super::common::BOB),
          recipient_before + crate::EXISTENTIAL_DEPOSIT
        );
        assert!(Balances::free_balance(&fee_sink) > collected_before);
        let result = Executive::apply_extrinsic(block.extrinsics.last().unwrap().clone());
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<_, ()>(result))
      })
      .expect("bounded authoring transaction")
      .expect("rollback result");
    assert_eq!(rejected, Err(InvalidTransaction::Custom(40).into()));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
      before
    );
    // The host import transaction owns the complete rejected block, including valid prefixes.
    let rejected =
      polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          Executive::execute_block(block.into());
        }));
        let message = result.err().map(|payload| {
          if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
          } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
          } else {
            "non-string block rejection".to_owned()
          }
        });
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<_, ()>(message))
      })
      .expect("bounded import transaction")
      .expect("rollback result");
    let expected = format!(
      "{:?}",
      polkadot_sdk::frame_executive::ExecutiveError::ApplyExtrinsic(
        InvalidTransaction::Custom(40).into()
      )
    );
    assert_eq!(rejected.as_deref(), Some(expected.as_str()));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
      before,
      "invalid certified ingress must roll back the entire candidate block"
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
pub(super) fn replay_pending_governance_thaw_blocks(
  signer: &sr25519::Pair,
  due: u32,
  domain: u32,
  item_id: u32,
) {
  assert_eq!(System::block_number(), due - 2);
  assert!(Actors::global_circuit_breaker());
  // Finish the minimal prefix's timestamp owner before entering complete block authoring.
  <crate::Timestamp as polkadot_sdk::frame_support::traits::OnFinalize<u32>>::on_finalize(
    System::block_number(),
  );
  let system_count = Actors::active_actor_count();
  let system_next = Actors::next_actor_id();
  let fee_sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let initial_actor = Actors::active_actor_state(fee_sink).unwrap();
  let initial_process = Actors::actor_processes(fee_sink);
  let initial_deadline = Actors::trigger_deadline_handles(fee_sink);
  let signer_account = crate::AccountId::from(signer.public());
  let mut storage = current_top_storage();
  let mut parent = parent_header_for(storage.clone(), &[], due - 2);
  assert_frozen_ingress_candidate_rejected_by_full_executive(
    storage.clone(),
    &parent,
    due - 1,
    signer,
  );
  for block in due - 1..=due + 2 {
    let nonce = TestExternalities::new(storage.clone())
      .execute_with(|| System::account_nonce(&signer_account));
    let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
      dest: MultiAddress::Id(super::common::BOB),
      value: crate::EXISTENTIAL_DEPOSIT,
    });
    let authored = author_complete_block_after_with_calls(
      storage,
      &[],
      &parent,
      block,
      UserDemand::ActorOnly,
      signer,
      nonce,
      &BTreeMap::new(),
      &[call],
    );
    assert_complete_block_replays_natively(&authored, &[]);
    TestExternalities::new(authored.post_state.clone()).execute_with(|| {
      assert!(!Actors::user_creation_enabled());
      assert_eq!(Actors::active_actor_count(), system_count);
      assert_eq!(Actors::next_actor_id(), system_next);
      assert_eq!(System::account_nonce(&signer_account), nonce + 1);
      if block < due {
        assert!(Actors::global_circuit_breaker());
        assert_eq!(
          Actors::active_actor_state(fee_sink),
          Some(initial_actor.clone())
        );
        assert_eq!(Actors::actor_processes(fee_sink), initial_process);
        assert_eq!(Actors::trigger_deadline_handles(fee_sink), initial_deadline);
        assert!(
          Actors::finalized_block_resource_telemetry()
            .unwrap()
            .optional_actor_work_halted()
        );
        assert_eq!(
          pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(domain, item_id),
          Some(due)
        );
      } else {
        assert!(!Actors::global_circuit_breaker());
        assert_eq!(
          crate::Governance::finalized_proposal_outcome(domain, item_id),
          Some(pallet_governance::FinalizedProposalOutcome::Approved {
            approval: pallet_governance::ProposalApproval {
              approved_epoch: due
                - crate::configs::governance_config::ProposalEnactmentDelay::get(),
              winner_count: 1
            },
            enactment: pallet_governance::ProposalEnactmentOutcome::Enacted { epoch: due },
          })
        );
        assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
        assert!(matches!(
          crate::Governance::proposal_execution_detail(domain, item_id),
          Some(pallet_governance::ProposalExecutionDetail::Succeeded(
            pallet_governance::ProposalExecutionSuccessDetail::ParameterChangeExecuted {
              surface: pallet_governance::ProposalParameterChangeSurface::ActorGlobalCircuitBreaker,
            }
          ))
        ));
      }
      if block == due + 2 {
        assert!(
          Actors::active_actor_state(fee_sink)
            .unwrap()
            .identity
            .cycle_nonce
            >= 1,
          "retained cadence becomes serviceable after thaw"
        );
      }
    });
    parent = authored.block.header;
    storage = authored.post_state;
  }
}

#[derive(Debug, Eq, PartialEq)]
struct StorageOwnerProof {
  key_paths: usize,
  node_bytes: usize,
  exclusive_node_bytes: usize,
  shared_node_bytes: usize,
}

pub(super) fn runtime_proof_storage_info() -> Vec<StorageInfo> {
  let mut info = crate::AllPalletsWithSystem::storage_info();
  // The SDK XCMP on_idle path still probes this private migration alias, absent from metadata.
  info.push(StorageInfo {
    pallet_name: b"XcmpQueue".to_vec(),
    storage_name: b"InboundXcmpStatus".to_vec(),
    prefix: polkadot_sdk::frame_support::storage::storage_prefix(
      b"XcmpQueue",
      b"InboundXcmpStatus",
    )
    .to_vec(),
    max_values: Some(1),
    max_size: None,
  });
  info
}

#[derive(Clone, Copy, Debug, Default)]
struct PersistentMutationCounts {
  inserted: u64,
  removed: u64,
  changed: u64,
}

fn actors_storage_entries(storage: &Storage) -> BTreeMap<Vec<u8>, Vec<u8>> {
  let prefixes = runtime_proof_storage_info()
    .into_iter()
    .filter(|info| info.pallet_name == b"Actors")
    .map(|info| info.prefix)
    .collect::<Vec<_>>();
  storage
    .top
    .iter()
    .filter(|(key, _)| prefixes.iter().any(|prefix| key.starts_with(prefix)))
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect()
}

fn persistent_storage_bytes(entries: &BTreeMap<Vec<u8>, Vec<u8>>) -> u64 {
  entries
    .iter()
    .map(|(key, value)| (key.len() + value.len()) as u64)
    .sum()
}

fn persistent_mutations(
  before: &BTreeMap<Vec<u8>, Vec<u8>>,
  after: &BTreeMap<Vec<u8>, Vec<u8>>,
) -> PersistentMutationCounts {
  let mut counts = PersistentMutationCounts::default();
  for (key, value) in after {
    match before.get(key) {
      None => counts.inserted = counts.inserted.saturating_add(1),
      Some(previous) if previous != value => counts.changed = counts.changed.saturating_add(1),
      Some(_) => {}
    }
  }
  counts.removed = before
    .keys()
    .filter(|key| !after.contains_key(*key))
    .count() as u64;
  counts
}

fn current_w7_deadline_snapshot(
  wasm: &[u8],
  storage: &Storage,
  actor_ids: &[ActorId],
) -> CurrentW7DeadlineSnapshot {
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    storage.clone(),
    crate::VERSION.state_version(),
  );
  ext.execute_with(|| {
    let handles = actor_ids
      .iter()
      .filter_map(|actor_id| {
        let handle = pallet_deos_actors::DeadlineHandles::<Runtime>::get(*actor_id)?;
        let WakeupKey::Block(block) = handle.key else {
          panic!("current W7 retries must retain block-clock deadlines");
        };
        let process = Actors::actor_processes(*actor_id).expect("sleeping process exists");
        let run = Actors::actor_run_state(*actor_id).expect("sleeping retry retains its Run");
        assert_eq!(process.generation, handle.actor.generation);
        assert_eq!(
          process.residence,
          Some(ProcessResidence::Deadline {
            key: handle.key,
            page: handle.page,
            slot: handle.slot,
          })
        );
        assert_eq!(run.eligible_at, block);
        assert_eq!(run.cursor, 0);
        assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
        assert_eq!(
          Actors::deadline_pages(handle.key, handle.page)
            .expect("sleeping retry owns its C32 page")
            .entries[usize::from(handle.slot)],
          Some(handle.actor)
        );
        Some((*actor_id, block))
      })
      .collect::<BTreeMap<_, _>>();
    let service = actor_ids
      .iter()
      .filter(|actor_id| {
        matches!(
          pallet_deos_actors::ActorProcesses::<Runtime>::get(**actor_id)
            .and_then(|process| process.residence),
          Some(ProcessResidence::Service(_))
        )
      })
      .copied()
      .collect::<BTreeSet<_>>();
    let suspended = actor_ids
      .iter()
      .filter(|actor_id| {
        Actors::actor_hot(**actor_id).is_some_and(|hot| hot.cycle_state == CycleState::Suspended)
      })
      .copied()
      .collect::<BTreeSet<_>>();
    let closed = actor_ids
      .iter()
      .filter(|actor_id| Actors::actor_identity(**actor_id).is_none())
      .copied()
      .collect::<BTreeSet<_>>();
    let block_headers = pallet_deos_actors::DeadlineHeaders::<Runtime>::iter()
      .filter_map(|(key, header)| match key {
        WakeupKey::Block(block) => Some((block, header.count)),
        WakeupKey::Tick(_) => None,
      })
      .collect::<BTreeMap<_, _>>();
    CurrentW7DeadlineSnapshot {
      handles,
      service,
      suspended,
      closed,
      block_headers,
      block_index_len: pallet_deos_actors::DeadlineIndexLen::<Runtime>::get(WakeupClock::Block),
    }
  })
}

#[test]
fn runtime_proof_storage_info_covers_upstream_idle_probe() {
  let key =
    polkadot_sdk::frame_support::storage::storage_prefix(b"XcmpQueue", b"InboundXcmpStatus");
  assert_eq!(
    format!(
      "{:?}",
      polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&key)
    ),
    "7b3237373ffdfeb1cab4222e3b520d6b345d8e88afa015075c945637c07e8f20",
  );
  let info = runtime_proof_storage_info();
  let matching = info
    .iter()
    .filter(|info| info.prefix == key)
    .collect::<Vec<_>>();
  assert_eq!(matching.len(), 1);
  assert_eq!(matching[0].storage_name, b"InboundXcmpStatus");
  assert!(
    !reference_genesis_storage(&[])
      .top
      .contains_key(key.as_slice())
  );
}

fn storage_proof_owners(
  pre_state_root: H256,
  paths: &BTreeMap<(H256, Vec<u8>), StorageProof>,
  storage_info: &[StorageInfo],
) -> (BTreeMap<String, StorageOwnerProof>, usize) {
  let mut owner_nodes = BTreeMap::<String, (usize, BTreeSet<&Vec<u8>>)>::new();
  for ((root, key), proof) in paths {
    let owner = if *root != pre_state_root {
      "NonTopTrie".to_owned()
    } else {
      let mut matching = storage_info
        .iter()
        .filter(|info| key.starts_with(&info.prefix));
      let owner = matching.next().map(|info| {
        format!(
          "{}.{}",
          std::str::from_utf8(&info.pallet_name).expect("pallet name is UTF-8"),
          std::str::from_utf8(&info.storage_name).expect("storage name is UTF-8"),
        )
      });
      assert!(
        matching.next().is_none(),
        "storage prefixes must have one owner"
      );
      owner.unwrap_or_else(|| {
        if key.starts_with(b":") {
          "WellKnownTop".to_owned()
        } else {
          format!(
            "UnmappedTop:0x{:?}",
            polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key),
          )
        }
      })
    };
    let (key_paths, nodes) = owner_nodes.entry(owner).or_default();
    *key_paths += 1;
    nodes.extend(proof.iter_nodes());
  }
  let mut node_owner_counts = BTreeMap::<&Vec<u8>, usize>::new();
  for (_, nodes) in owner_nodes.values() {
    for node in nodes {
      *node_owner_counts.entry(node).or_default() += 1;
    }
  }
  let cross_owner_shared_node_bytes = node_owner_counts
    .iter()
    .filter(|(_, count)| **count > 1)
    .map(|(node, _)| node.len())
    .sum();
  let owners = owner_nodes
    .into_iter()
    .map(|(owner, (key_paths, nodes))| {
      let node_bytes = nodes.iter().map(|node| node.len()).sum::<usize>();
      let exclusive_node_bytes = nodes
        .iter()
        .filter(|node| node_owner_counts[*node] == 1)
        .map(|node| node.len())
        .sum::<usize>();
      (
        owner,
        StorageOwnerProof {
          key_paths,
          node_bytes,
          exclusive_node_bytes,
          shared_node_bytes: node_bytes - exclusive_node_bytes,
        },
      )
    })
    .collect();
  (owners, cross_owner_shared_node_bytes)
}

#[test]
fn storage_proof_owners_preserve_shared_nodes_and_unmapped_roots() {
  let root = H256::repeat_byte(1);
  let child_root = H256::repeat_byte(2);
  let shared = vec![9; 3];
  let paths = [
    (
      (root, vec![1, 0]),
      StorageProof::new([vec![1; 4], shared.clone()]),
    ),
    (
      (root, vec![1, 1]),
      StorageProof::new([vec![1; 4], shared.clone()]),
    ),
    (
      (root, vec![2, 0]),
      StorageProof::new([vec![2; 5], shared.clone()]),
    ),
    (
      (child_root, vec![1, 0]),
      StorageProof::new([vec![3; 6], shared]),
    ),
    ((root, b":key".to_vec()), StorageProof::new([vec![4; 7]])),
    ((root, vec![7]), StorageProof::new([vec![5; 8]])),
  ]
  .into();
  let storage_info = [1, 2].map(|prefix| StorageInfo {
    pallet_name: b"Pallet".to_vec(),
    storage_name: format!("Item{prefix}").into_bytes(),
    prefix: vec![prefix],
    max_values: None,
    max_size: None,
  });
  let (owners, cross_owner_shared) = storage_proof_owners(root, &paths, &storage_info);
  assert_eq!(owners.len(), 5);
  assert_eq!(
    owners["Pallet.Item1"],
    StorageOwnerProof {
      key_paths: 2,
      node_bytes: 7,
      exclusive_node_bytes: 4,
      shared_node_bytes: 3,
    }
  );
  assert_eq!(owners["Pallet.Item2"].exclusive_node_bytes, 5);
  assert_eq!(owners["NonTopTrie"].exclusive_node_bytes, 6);
  assert_eq!(owners["WellKnownTop"].exclusive_node_bytes, 7);
  assert_eq!(owners["UnmappedTop:0x07"].exclusive_node_bytes, 8);
  assert_eq!(cross_owner_shared, 3);
  assert_eq!(
    owners
      .values()
      .map(|owner| owner.exclusive_node_bytes)
      .sum::<usize>()
      + cross_owner_shared,
    33,
  );
  assert_eq!(
    owners.values().map(|owner| owner.key_paths).sum::<usize>(),
    paths.len()
  );
}

fn replay_complete_block_in_wasm(
  authored: &AuthoredBlock,
  wasm: &[u8],
  label: &str,
) -> WasmProofMetrics {
  let evidence = super::wasm_replay::replay_block(
    authored.pre_state.clone(),
    wasm,
    &authored.block.encode(),
    &authored.inherent_data,
  )
  .unwrap_or_else(|error| panic!("{label} production Wasm replay failed: {error}"));
  assert_runtime_version_equivalent(&evidence.runtime_version);
  assert_eq!(evidence.block_hash, authored.block.header.hash());
  assert_eq!(
    evidence.post_state_root,
    *authored.block.header.state_root()
  );
  assert!(evidence.execute_block_result.is_empty());
  for proof in [&evidence.execution_proof, &evidence.verification_proof] {
    assert_eq!(
      proof.storage_proof_scale_bytes,
      proof.storage_proof.encoded_size()
    );
    assert_eq!(
      proof.compact_proof_scale_bytes,
      proof.compact_proof.encoded_size()
    );
    assert_eq!(proof.trie_node_count, proof.storage_proof.len());
    assert!(proof.trie_node_bytes > 0);
  }
  println!(
    "EXP_0066_WASM_BLOCK_PREP_V1 profile={label} code={:?} block={:?} execution_storage_proof_bytes={} execution_compact_proof_bytes={} execution_node_bytes={} execution_nodes={} verification_storage_proof_bytes={} verification_compact_proof_bytes={} post_execute_root_node_bytes={} proof_size_observations={:?}",
    evidence.runtime_code_hash,
    evidence.block_hash,
    evidence.execution_proof.storage_proof_scale_bytes,
    evidence.execution_proof.compact_proof_scale_bytes,
    evidence.execution_proof.trie_node_bytes,
    evidence.execution_proof.trie_node_count,
    evidence.verification_proof.storage_proof_scale_bytes,
    evidence.verification_proof.compact_proof_scale_bytes,
    evidence.post_execute_root_node_bytes,
    evidence.proof_size_observations,
  );
  let overlap = evidence.execution_key_overlap;
  assert_eq!(
    overlap.unique_node_bytes + overlap.unattributed_node_bytes,
    evidence.execution_proof.trie_node_bytes,
  );
  println!(
    "EXP_0066_WASM_KEY_PROOF_V1 profile={label} key_paths={} per_key_node_bytes={} unique_node_bytes={} shared_node_bytes={} repeated_node_bytes={} unattributed_node_bytes={}",
    evidence.execution_recorded_keys.len(),
    overlap.per_key_node_bytes,
    overlap.unique_node_bytes,
    overlap.shared_node_bytes,
    overlap.per_key_node_bytes - overlap.unique_node_bytes,
    overlap.unattributed_node_bytes,
  );
  assert_eq!(
    overlap.paths.keys().cloned().collect::<BTreeSet<_>>(),
    evidence.execution_recorded_keys,
  );
  let key_nodes = overlap
    .paths
    .values()
    .flat_map(StorageProof::iter_nodes)
    .collect::<BTreeSet<_>>();
  let execution_nodes = evidence
    .execution_proof
    .storage_proof
    .iter_nodes()
    .collect::<BTreeSet<_>>();
  let root_nodes = evidence
    .committed_root_proof
    .iter_nodes()
    .collect::<BTreeSet<_>>();
  let residual_nodes = execution_nodes
    .difference(&key_nodes)
    .copied()
    .collect::<BTreeSet<_>>();
  let root_covered_residual_node_bytes = residual_nodes
    .intersection(&root_nodes)
    .map(|node| node.len())
    .sum::<usize>();
  let remaining_residual_node_bytes = residual_nodes
    .difference(&root_nodes)
    .map(|node| node.len())
    .sum::<usize>();
  assert_eq!(
    root_covered_residual_node_bytes + remaining_residual_node_bytes,
    overlap.unattributed_node_bytes,
  );
  println!(
    "EXP_0066_WASM_ROOT_DELTA_PROOF_V1 profile={label} committed_root_node_bytes={} root_covered_residual_node_bytes={root_covered_residual_node_bytes} remaining_residual_node_bytes={remaining_residual_node_bytes}",
    root_nodes.iter().map(|node| node.len()).sum::<usize>(),
  );
  let (owners, cross_owner_shared_node_bytes) = storage_proof_owners(
    evidence.pre_state_root,
    &overlap.paths,
    &runtime_proof_storage_info(),
  );
  let exclusive_node_bytes = owners
    .values()
    .map(|owner| owner.exclusive_node_bytes)
    .sum::<usize>();
  assert_eq!(
    exclusive_node_bytes + cross_owner_shared_node_bytes,
    overlap.unique_node_bytes
  );
  assert_eq!(
    owners.values().map(|owner| owner.key_paths).sum::<usize>(),
    evidence.execution_recorded_keys.len(),
  );
  for (owner, metrics) in owners {
    println!(
      "EXP_0066_WASM_STORAGE_OWNER_V1 profile={label} owner={owner} key_paths={} node_bytes={} exclusive_node_bytes={} shared_node_bytes={}",
      metrics.key_paths,
      metrics.node_bytes,
      metrics.exclusive_node_bytes,
      metrics.shared_node_bytes,
    );
  }
  println!(
    "EXP_0066_WASM_STORAGE_PARTITION_V1 profile={label} exclusive_node_bytes={exclusive_node_bytes} cross_owner_shared_node_bytes={cross_owner_shared_node_bytes} unattributed_node_bytes={}",
    overlap.unattributed_node_bytes,
  );
  WasmProofMetrics {
    execution_storage_proof_bytes: evidence.execution_proof.storage_proof_scale_bytes as u64,
    execution_compact_proof_bytes: evidence.execution_proof.compact_proof_scale_bytes as u64,
    verification_storage_proof_bytes: evidence.verification_proof.storage_proof_scale_bytes as u64,
    verification_compact_proof_bytes: evidence.verification_proof.compact_proof_scale_bytes as u64,
    execution_recorded_keys: evidence.execution_recorded_keys,
    remaining_residual_node_bytes,
  }
}

fn assert_successful_transfer_outcomes(metrics: &FullExecutiveBlockMetrics) {
  assert_eq!(
    metrics.non_successful_steps, 0,
    "Transfer evidence rejects failed or skipped Steps"
  );
  assert_eq!(
    metrics.actor_steps, metrics.transfer_steps,
    "every Step must execute a Transfer"
  );
  assert_eq!(
    metrics.completed_cycles, metrics.transfer_steps,
    "each one-Step Transfer must complete its Cycle"
  );
}

fn assert_successful_transfer_resource_ledger(
  wasm: &[u8],
  empty: &AuthoredBlock,
  profiles: [(&str, &AuthoredBlock, u64); 3],
) {
  type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let canonical_selector =
    W::service_round_begin_populated().saturating_add(W::service_round_probe_eligible());
  let canonical_loaded_inspection =
    canonical_selector.saturating_add(W::scheduler_actor_state_probe());
  let canonical_service_suffix = W::service_round_admit_eligible().max(
    W::service_member_retire_interior()
      .max(W::service_member_retire_pair_cursor())
      .max(W::service_member_retire_singleton()),
  );
  // This fixed W0/W1 witness completes an unpredicated one-Step System Opening into passive
  // residence. The class-neutral actual selector charges the complete header owner, not the
  // separate plan/run/Queue composition. Reconstruct declarations outside execution.
  let control_components = [
    ("round-begin", W::service_round_begin_populated()),
    ("head-probe", W::service_round_probe_eligible()),
    ("loaded-classification", W::scheduler_actor_state_probe()),
    (
      "opening-completion-header-maximum",
      W::scheduler_inner_opening_user_complete_header_max(),
    ),
    ("invocation-receipt", W::action_invocation_receipt()),
    ("outer-service-suffix-maximum", canonical_service_suffix),
  ];
  let control_per_step = control_components
    .iter()
    .fold(Weight::zero(), |sum, (_, weight)| {
      sum.saturating_add(*weight)
    });
  let effect_per_step = W::task_transfer();
  let effect_io_per_step = DatabaseIo::new(19, 6);
  // Equal total Weight can hide a compensating change between RefTime and DB coefficients.
  let generated_transfer = include_str!("../weights/pallet_deos_actors.rs")
    .split_once("fn task_transfer() -> Weight {")
    .expect("generated Transfer owner exists")
    .1
    .split_once('}')
    .expect("generated Transfer owner ends")
    .0;
  for (operation, count) in [
    ("reads", effect_io_per_step.reads),
    ("writes", effect_io_per_step.writes),
  ] {
    let call = format!("T::DbWeight::get().{operation}(");
    assert_eq!(generated_transfer.matches(&call).count(), 1);
    assert!(generated_transfer.contains(&format!("{call}{count})")));
  }
  assert_production_weight_component(effect_per_step, 388_533_000, 6_196, effect_io_per_step);
  let pair = |weight: Weight| [weight.ref_time(), weight.proof_size()];
  let reference_events = |authored: &AuthoredBlock| {
    let ids = (0..ActorId::from(REFERENCE_SYSTEM_ACTOR_IDENTITIES)).collect::<Vec<_>>();
    let snapshot = actor_lifecycle_observation(wasm, &authored.post_state, &ids);
    let mut counts = BTreeMap::<String, u64>::new();
    for event in snapshot["events"]
      .as_array()
      .expect("observation has events")
    {
      *counts
        .entry(event["kind"].as_str().expect("event has a kind").to_owned())
        .or_default() += 1;
    }
    counts
  };
  let baseline_reference_events = reference_events(empty);
  assert_eq!(empty.metrics.actor_steps, 0);
  assert_eq!(empty.metrics.completed_cycles, 0);
  assert_eq!(empty.metrics.actor_effect, Weight::zero());
  let mut rows = Vec::new();
  for (label, authored, capacity_refusal_classifications) in profiles {
    assert_successful_transfer_outcomes(&authored.metrics);
    assert!(
      authored.metrics.prepass_steps <= authored.metrics.actor_steps,
      "mandatory prepass Steps remain a subset of finalized canonical Service Steps",
    );
    assert_eq!(reference_events(authored), baseline_reference_events);
    let steps = u64::from(authored.metrics.actor_steps);
    let control_delta = authored
      .metrics
      .actor_control
      .checked_sub(&empty.metrics.actor_control)
      .expect("productive canonical Service must not use less Control than the empty baseline");
    assert_ne!(
      control_delta,
      Weight::zero(),
      "productive canonical Service must add finalized Actor Control",
    );
    assert!(
      authored
        .metrics
        .prepass_actor_control
        .all_lte(authored.metrics.actor_control),
      "mandatory prepass Control remains a subset of finalized Actor Control",
    );
    assert_eq!(
      authored.metrics.actor_effect,
      effect_per_step.saturating_mul(steps)
    );
    let productive_control = control_per_step.saturating_mul(steps);
    let refused_classification =
      W::scheduler_actor_state_probe().saturating_mul(capacity_refusal_classifications);
    assert_eq!(
      control_delta,
      productive_control.saturating_add(refused_classification)
    );
    let prepass_delta = authored
      .metrics
      .prepass_actor_control
      .checked_sub(&empty.metrics.prepass_actor_control)
      .expect("this witness adds Service work above the same first-block fixed owners");
    let prepass_productive =
      control_per_step.saturating_mul(u64::from(authored.metrics.prepass_steps));
    let prepass_refusal = prepass_delta
      .checked_sub(&prepass_productive)
      .expect("Prepass covers every committed Step owner");
    assert_eq!(
      prepass_refusal,
      W::scheduler_actor_state_probe()
        .saturating_mul(u64::from(capacity_refusal_classifications > 0))
    );
    let effect_io = effect_io_per_step.saturating_mul(steps);
    rows.push(serde_json::json!({
      "profile": label, "committedSteps": steps,
      "capacityRefusalClassifications": capacity_refusal_classifications,
      "productiveControl": pair(productive_control),
      "refusedClassificationControl": pair(refused_classification),
      "prepassRefusedClassificationControl": pair(prepass_refusal),
      "canonicalSelector": pair(canonical_selector),
      "canonicalLoadedInspection": pair(canonical_loaded_inspection),
      "canonicalServiceSuffix": pair(canonical_service_suffix),
      "controlDelta": pair(control_delta),
      "effect": pair(authored.metrics.actor_effect),
      "generatedEffectIo": [effect_io.reads, effect_io.writes],
    }));
  }
  println!(
    "ACTOR_TRANSFER_RESOURCE_LEDGER_V1 {}",
    serde_json::json!({
      "scope": "W0-W1-first-block-canonical-Service-Transfer-delta",
      "effectPerStep": pair(effect_per_step),
      "generatedEffectIoPerStep": [effect_io_per_step.reads, effect_io_per_step.writes],
      "baselineControl": pair(empty.metrics.actor_control),
      "baselineReferenceEvents": baseline_reference_events,
      "baselineWorkloadSteps": 0, "baselineIoAllocatedToSteps": false,
      "controlPerCommittedStep": pair(control_per_step),
      "controlComponentsPerCommittedStep": control_components.iter().map(|(owner, weight)|
        serde_json::json!({"owner": owner, "weight": pair(*weight)})
      ).collect::<Vec<_>>(),
      "attributionBoundary": "exact generated-Weight ledger for this witness, not measured marginal trie bytes; selector work at empty/refused pass boundaries is already in baseline",
      "canonicalControlOwners": {
        "selector": pair(canonical_selector),
        "loadedInspection": pair(canonical_loaded_inspection),
        "serviceSuffix": pair(canonical_service_suffix),
      },
      "profiles": rows,
    })
  );
}

fn assert_prepared_w0_w1(wasm: &[u8], replay_wasm: bool) {
  let empty = prepare_actor_fixture(wasm, 0, WorkloadSchedule::ManualOnly);
  let w0_empty = author_complete_block(
    empty.storage,
    wasm,
    2,
    UserDemand::ActorOnly,
    &empty.signer,
    &empty.actor_profiles,
  );
  assert_eq!(w0_empty.metrics.actor_steps, 0);
  assert_complete_block_replays_natively(&w0_empty, wasm);

  let one = prepare_actor_fixture(wasm, 1, WorkloadSchedule::ManualOnly);
  let w0_one = author_complete_block(
    one.storage,
    wasm,
    2,
    UserDemand::ActorOnly,
    &one.signer,
    &one.actor_profiles,
  );
  assert_eq!(w0_one.metrics.actor_steps, 1);
  assert_eq!(w0_one.metrics.distinct_actors, 1);
  assert_successful_transfer_outcomes(&w0_one.metrics);
  assert_complete_block_replays_natively(&w0_one, wasm);

  let w1 = prepare_actor_fixture(wasm, PREPARED_W1_ACTORS, WorkloadSchedule::ManualOnly);
  let actor_only = author_complete_block(
    w1.storage.clone(),
    wasm,
    2,
    UserDemand::ActorOnly,
    &w1.signer,
    &w1.actor_profiles,
  );
  let user_demand = author_complete_block(
    w1.storage,
    wasm,
    2,
    UserDemand::ContinuousValid,
    &w1.signer,
    &w1.actor_profiles,
  );
  for authored in [&actor_only, &user_demand] {
    assert_successful_transfer_outcomes(&authored.metrics);
    assert!(authored.metrics.actor_steps > 0);
    assert!(authored.metrics.actor_steps <= w1.actor_ids.len() as u32);
    assert_eq!(
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors
    );
    assert_complete_block_replays_natively(authored, wasm);
  }
  assert_eq!(actor_only.metrics.user_calls, 0);
  assert_eq!(actor_only.metrics.user_dispatch, Weight::zero());
  assert!(user_demand.metrics.user_calls > 0);
  assert_ne!(user_demand.metrics.user_dispatch, Weight::zero());
  let next_user = user_demand
    .metrics
    .next_user_weight
    .expect("continuous demand records the first rejected frontier");
  let remaining = BlockResourceBudgetValue::get()
    .limits()
    .user_base_turn()
    .checked_sub(&user_demand.metrics.user_dispatch)
    .unwrap_or_else(Weight::zero);
  assert!(!next_user.all_lte(remaining));

  for (label, authored) in [
    ("W0-empty", &w0_empty),
    ("W0-one-step", &w0_one),
    (UserDemand::ActorOnly.label(), &actor_only),
    (UserDemand::ContinuousValid.label(), &user_demand),
  ] {
    println!(
      "EXP_0066_FULL_EXECUTIVE_PREP_V1 profile={label} block={} inherents_and_calls={} actor_steps={} distinct_actors={} user_calls={} control_ref_time={} control_proof_size={} effect_ref_time={} effect_proof_size={} user_ref_time={} user_proof_size={}",
      authored.block.header.number(),
      authored.block.extrinsics.len(),
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors,
      authored.metrics.user_calls,
      authored.metrics.actor_control.ref_time(),
      authored.metrics.actor_control.proof_size(),
      authored.metrics.actor_effect.ref_time(),
      authored.metrics.actor_effect.proof_size(),
      authored.metrics.user_dispatch.ref_time(),
      authored.metrics.user_dispatch.proof_size(),
    );
  }

  assert_successful_transfer_resource_ledger(
    wasm,
    &w0_empty,
    [
      ("one-step", &w0_one, 0),
      ("actor-only", &actor_only, 2),
      ("continuous-user", &user_demand, 2),
    ],
  );

  if replay_wasm {
    for (label, authored) in [
      ("W0-empty", w0_empty),
      ("W0-one-step", w0_one),
      (UserDemand::ActorOnly.label(), actor_only),
      (UserDemand::ContinuousValid.label(), user_demand),
    ] {
      let _ = replay_complete_block_in_wasm(&authored, wasm, label);
    }
  }
}

fn nearest_rank_percentile(values: &[u64], percentile: usize) -> u64 {
  if values.is_empty() {
    return 0;
  }
  let mut sorted = values.to_vec();
  sorted.sort_unstable();
  let rank = percentile
    .saturating_mul(sorted.len())
    .div_ceil(100)
    .saturating_sub(1)
    .min(sorted.len().saturating_sub(1));
  sorted[rank]
}

#[derive(Default)]
struct OneStepActorHistory {
  pending_signal_block: Option<u32>,
  first_signal_block: Option<u32>,
  first_step_block: Option<u32>,
  last_step_block: Option<u32>,
}

struct OneStepServiceTrace {
  actors: BTreeMap<ActorId, OneStepActorHistory>,
  signal_to_step_blocks: Vec<(ActorId, u64)>,
  inter_step_gaps: Vec<(ActorId, u64)>,
}

impl OneStepServiceTrace {
  fn new(actor_ids: &[ActorId]) -> Self {
    let actors = actor_ids
      .iter()
      .map(|id| (*id, OneStepActorHistory::default()))
      .collect::<BTreeMap<_, _>>();
    assert_eq!(
      actors.len(),
      actor_ids.len(),
      "workload Actor identities are unique"
    );
    Self {
      actors,
      signal_to_step_blocks: Vec::new(),
      inter_step_gaps: Vec::new(),
    }
  }

  fn for_schedule(actor_ids: &[ActorId], schedule: WorkloadSchedule) -> Self {
    let mut trace = Self::new(actor_ids);
    for (index, actor_id) in actor_ids.iter().enumerate() {
      if schedule.uses_manual_trigger(index as u32) {
        trace.signal(*actor_id, 1);
      }
    }
    trace
  }

  fn observe(&mut self, metrics: &FullExecutiveBlockMetrics, block: u32) {
    self.observe_with_trigger_family(metrics, block, TriggerFamily::Cadenced);
  }

  fn observe_with_trigger_family(
    &mut self,
    metrics: &FullExecutiveBlockMetrics,
    block: u32,
    trigger_family: TriggerFamily,
  ) {
    assert_eq!(
      metrics.non_successful_steps, 0,
      "service evidence rejects failed/skipped Steps"
    );
    assert_eq!(
      metrics.actor_steps, metrics.completed_cycles,
      "one completed Cycle per successful one-Step outcome"
    );
    assert_eq!(
      metrics.actor_steps,
      metrics.transfer_steps + metrics.swap_out_steps + metrics.stop_cycle_steps,
      "every service sample has a successful typed outcome"
    );
    assert!(
      metrics.continued_steps.is_empty()
        && metrics.closed_actors.is_empty()
        && metrics.suspended_steps.is_empty()
        && metrics.paused_actors.is_empty()
        && metrics.resumed_actors.is_empty()
        && metrics.failed_cycle_actors.is_empty(),
      "one-Step service observations exclude lifecycle interruptions"
    );
    assert_eq!(
      metrics
        .progressed_steps
        .iter()
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>(),
      metrics
        .completed_cycle_actors
        .iter()
        .copied()
        .collect::<BTreeSet<_>>(),
      "successful Steps and completed Cycles belong to the same Actors",
    );
    for (actor_id, family) in &metrics.trigger_occurrences {
      assert_eq!(
        *family, trigger_family,
        "only the declared automatic Trigger family arrives during this campaign"
      );
      self.signal(*actor_id, block);
    }
    for (actor_id, _) in &metrics.progressed_steps {
      self.step(*actor_id, block);
    }
  }

  fn pending_signal_count(&self) -> usize {
    self
      .actors
      .values()
      .filter(|history| history.pending_signal_block.is_some())
      .count()
  }

  fn assert_terminal_state(&self, wasm: &[u8], storage: Storage) {
    let mut ext =
      TestExternalities::new_with_code_and_state(wasm, storage, crate::VERSION.state_version());
    ext.execute_with(|| {
      for (actor_id, history) in &self.actors {
        let hot = Actors::actor_hot(*actor_id).expect("workload Actor remains active");
        assert_eq!(
          hot.pending_signal,
          history.pending_signal_block.is_some(),
          "event-derived latch matches terminal state"
        );
      }
    });
  }

  fn signal(&mut self, actor_id: ActorId, block: u32) {
    let actor = self
      .actors
      .get_mut(&actor_id)
      .expect("signal belongs to the workload");
    assert!(
      actor.pending_signal_block.is_none(),
      "one useful signal per pending latch"
    );
    assert!(
      actor
        .last_step_block
        .is_none_or(|previous| block > previous),
      "one-Step cadence rearms after prior service"
    );
    actor.pending_signal_block = Some(block);
    actor.first_signal_block.get_or_insert(block);
  }

  fn step(&mut self, actor_id: ActorId, block: u32) {
    let actor = self
      .actors
      .get_mut(&actor_id)
      .expect("Step belongs to the workload");
    let signal = actor
      .pending_signal_block
      .take()
      .expect("a Step must consume known readiness");
    assert!(
      block > signal,
      "materialized readiness has a next-block causal floor"
    );
    self
      .signal_to_step_blocks
      .push((actor_id, u64::from(block - signal)));
    actor.first_step_block.get_or_insert(block);
    if let Some(previous) = actor.last_step_block.replace(block) {
      assert!(block > previous, "Q1 permits one Step per Actor per block");
      self
        .inter_step_gaps
        .push((actor_id, u64::from(block - previous)));
    }
  }

  fn summary(&self, creation_block: u32, last_block: u32) -> serde_json::Value {
    self.summary_for(creation_block, last_block, |_| true)
  }

  fn summary_for(
    &self,
    creation_block: u32,
    last_block: u32,
    include: impl Fn(ActorId) -> bool,
  ) -> serde_json::Value {
    let actors = self
      .actors
      .iter()
      .filter(|(id, _)| include(**id))
      .map(|(_, actor)| actor)
      .collect::<Vec<_>>();
    let signal_to_step_blocks = self
      .signal_to_step_blocks
      .iter()
      .filter(|(id, _)| include(*id))
      .map(|(_, blocks)| *blocks)
      .collect::<Vec<_>>();
    let inter_step_gaps = self
      .inter_step_gaps
      .iter()
      .filter(|(id, _)| include(*id))
      .map(|(_, blocks)| *blocks)
      .collect::<Vec<_>>();
    let first_step_latencies = actors
      .iter()
      .filter_map(|actor| actor.first_step_block)
      .map(|block| {
        u64::from(
          block
            .checked_sub(creation_block)
            .expect("creation precedes service"),
        )
      })
      .collect::<Vec<_>>();
    let pending_waits = actors
      .iter()
      .filter_map(|actor| actor.pending_signal_block)
      .map(|block| {
        u64::from(
          last_block
            .checked_sub(block)
            .expect("signal is inside the observed horizon"),
        )
      })
      .collect::<Vec<_>>();
    serde_json::json!({
      "workloadActors": actors.len(),
      "creationBlock": creation_block,
      "lastObservedBlock": last_block,
      "everMaterializedActors": actors.iter().filter(|actor| actor.first_signal_block.is_some()).count(),
      "neverMaterializedActors": actors.iter().filter(|actor| actor.first_signal_block.is_none()).count(),
      "distinctProgressedActors": first_step_latencies.len(),
      "firstStepRightCensoring": {
        "actors": actors.len() - first_step_latencies.len(),
        "atBlocksSinceCreation": last_block.checked_sub(creation_block).expect("ordered horizon"),
      },
      "pendingSignals": pending_waits.len(),
      "pendingPastCausalFloor": pending_waits.iter().filter(|wait| **wait > 0).count(),
      "firstStepFromCreationBlocks": observed_distribution(&first_step_latencies),
      "materializedSignalToStepBlocks": observed_distribution(&signal_to_step_blocks),
      "pendingSignalRightCensoringBlocks": observed_distribution(&pending_waits),
      "interStepGapBlocks": observed_distribution(&inter_step_gaps),
    })
  }
}

fn observed_distribution(values: &[u64]) -> serde_json::Value {
  let observed = !values.is_empty();
  serde_json::json!({
    "samples": values.len(),
    "min": values.iter().min(),
    "mean": observed.then(|| values.iter().sum::<u64>() as f64 / values.len() as f64),
    "p50": observed.then(|| nearest_rank_percentile(values, 50)),
    "p95": observed.then(|| nearest_rank_percentile(values, 95)),
    "p99": observed.then(|| nearest_rank_percentile(values, 99)),
    "max": values.iter().max(),
  })
}

#[test]
fn one_step_service_trace_separates_observed_delays_from_censoring() {
  let mut trace = OneStepServiceTrace::new(&[15, 16, 17, 18]);
  trace.signal(15, 1);
  trace.signal(16, 2);
  trace.step(15, 3);
  trace.signal(15, 5);
  trace.step(15, 6);
  trace.signal(18, 7);
  let summary = trace.summary(1, 7);
  assert_eq!(summary["distinctProgressedActors"], 1);
  assert_eq!(summary["neverMaterializedActors"], 1);
  assert_eq!(
    summary["firstStepRightCensoring"],
    serde_json::json!({"actors": 3, "atBlocksSinceCreation": 6})
  );
  assert_eq!(summary["pendingSignals"], 2);
  assert_eq!(summary["pendingPastCausalFloor"], 1);
  assert_eq!(summary["materializedSignalToStepBlocks"]["samples"], 2);
  assert_eq!(summary["materializedSignalToStepBlocks"]["mean"], 1.5);
  assert_eq!(summary["firstStepFromCreationBlocks"]["max"], 2);
  assert_eq!(summary["interStepGapBlocks"]["max"], 3);
  assert_eq!(summary["pendingSignalRightCensoringBlocks"]["max"], 5);
  let selected = trace.summary_for(1, 7, |id| id == 15 || id == 17);
  assert_eq!(selected["workloadActors"], 2);
  assert_eq!(selected["neverMaterializedActors"], 1);
  assert_eq!(selected["firstStepRightCensoring"]["actors"], 1);
  assert_eq!(selected["pendingSignals"], 0);
  assert_eq!(selected["materializedSignalToStepBlocks"]["samples"], 2);
  assert_eq!(selected["interStepGapBlocks"]["max"], 3);
  let pending = trace.summary_for(1, 7, |id| id == 16 || id == 18);
  assert_eq!(pending["pendingSignals"], 2);
  assert_eq!(pending["pendingSignalRightCensoringBlocks"]["max"], 5);
  assert_eq!(pending["materializedSignalToStepBlocks"]["samples"], 0);
  assert!(pending["interStepGapBlocks"]["max"].is_null());
}

#[test]
fn one_step_service_trace_reports_missing_samples_as_unknown() {
  let summary = OneStepServiceTrace::new(&[15]).summary(1, 101);
  for field in [
    "firstStepFromCreationBlocks",
    "materializedSignalToStepBlocks",
    "interStepGapBlocks",
    "pendingSignalRightCensoringBlocks",
  ] {
    assert_eq!(summary[field]["samples"], 0);
    assert!(summary[field]["max"].is_null());
    assert!(summary[field]["mean"].is_null());
  }
  assert_eq!(summary["firstStepRightCensoring"]["actors"], 1);
  assert_eq!(
    summary["firstStepRightCensoring"]["atBlocksSinceCreation"],
    100
  );
}

#[test]
#[should_panic(expected = "next-block causal floor")]
fn one_step_service_trace_rejects_same_block_service() {
  let mut trace = OneStepServiceTrace::new(&[15]);
  trace.signal(15, 2);
  trace.step(15, 2);
}

#[test]
fn one_step_service_trace_matches_runtime_latches() {
  for schedule in [
    WorkloadSchedule::ManualOnly,
    WorkloadSchedule::CadencedOnly,
    WorkloadSchedule::MixedManualCadenced,
  ] {
    let fixture = prepare_actor_fixture(&[], 3, schedule);
    let mut trace = OneStepServiceTrace::for_schedule(&fixture.actor_ids, schedule);
    let mut pre_state = fixture.storage;
    let mut parent = parent_header_for(pre_state.clone(), &[], 1);
    for block in 2..=9 {
      let authored = author_complete_block_after(
        pre_state,
        &[],
        &parent,
        block,
        UserDemand::ActorOnly,
        &fixture.signer,
        0,
        &fixture.actor_profiles,
      );
      trace.observe(&authored.metrics, block);
      parent = authored.block.header;
      pre_state = authored.post_state;
    }
    trace.assert_terminal_state(&[], pre_state);
    let summary = trace.summary(1, 9);
    assert_eq!(summary["distinctProgressedActors"], 3);
    if schedule == WorkloadSchedule::ManualOnly {
      assert_eq!(summary["materializedSignalToStepBlocks"]["samples"], 3);
      assert!(summary["interStepGapBlocks"]["max"].is_null());
    } else {
      assert!(summary["interStepGapBlocks"]["samples"].as_u64().unwrap() > 0);
    }
  }
}

fn run_schedule_campaign(
  wasm: &[u8],
  workload: &str,
  demand: UserDemand,
  workload_schedule: WorkloadSchedule,
) {
  let maximum_active = <Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
  let maximum_identities = <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get();
  let (initial_active, initial_identities) = {
    let mut ext = TestExternalities::new_with_code_and_state(
      wasm,
      reference_genesis_storage(wasm),
      crate::VERSION.state_version(),
    );
    ext.execute_with(|| {
      (
        pallet_deos_actors::ActiveActorCount::<Runtime>::get(),
        pallet_deos_actors::ActorIdentityCount::<Runtime>::get(),
      )
    })
  };
  assert_eq!(
    initial_active, REFERENCE_ACTIVE_SYSTEM_ACTORS,
    "{workload} must explicitly account for the reference preset's active System Actors"
  );
  assert_eq!(
    initial_identities, REFERENCE_SYSTEM_ACTOR_IDENTITIES,
    "{workload} must explicitly account for active and dormant reference System Actor identities"
  );
  let added_actor_count = maximum_identities
    .checked_sub(initial_identities)
    .expect("reference identity population fits the production bound");
  let active_actor_count = initial_active.saturating_add(added_actor_count);
  assert!(active_actor_count <= maximum_active);
  let fixture = prepare_actor_fixture(wasm, added_actor_count, workload_schedule);
  assert_eq!(fixture.actor_ids.len() as u32, added_actor_count);

  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut signer_nonce = 0;
  let mut total_cycles = 0u32;
  let mut service = OneStepServiceTrace::for_schedule(&fixture.actor_ids, workload_schedule);
  let mut steps = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut effect_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut effect_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut user_calls = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut user_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut user_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut user_bound_blocks = 0u32;
  let mut control_remaining_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_remaining_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_bound_blocks = 0u32;
  let actor_control_limit = BlockResourceBudgetValue::get().limits().actor_control();
  let mut execution_storage_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut execution_compact_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut verification_storage_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut verification_compact_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);

  for block_number in 2..W1_TARGET_BLOCKS.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      demand,
      &fixture.signer,
      signer_nonce,
      &fixture.actor_profiles,
    );
    match demand {
      UserDemand::ActorOnly => {
        assert_eq!(authored.metrics.user_calls, 0);
        assert_eq!(authored.metrics.user_dispatch, Weight::zero());
        assert!(authored.metrics.next_user_weight.is_none());
      }
      UserDemand::ContinuousValid => {
        assert!(authored.metrics.user_calls > 0);
        let next_user = authored
          .metrics
          .next_user_weight
          .expect("continuous demand records its first inadmissible valid call");
        let remaining = BlockResourceBudgetValue::get()
          .limits()
          .user_base_turn()
          .checked_sub(&authored.metrics.user_dispatch)
          .unwrap_or_else(Weight::zero);
        assert!(
          !next_user.all_lte(remaining),
          "continuous demand must reach the User base-turn frontier"
        );
        user_bound_blocks = user_bound_blocks.saturating_add(1);
      }
      UserDemand::RefTimeHeavy | UserDemand::ContinuousShared => {
        unreachable!("the W1/W2 schedule campaign only prepares User-base-turn remarks")
      }
    }
    assert_eq!(
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors
    );
    assert_successful_transfer_outcomes(&authored.metrics);
    service.observe(&authored.metrics, block_number);
    total_cycles = total_cycles.saturating_add(authored.metrics.completed_cycles);
    steps.push(u64::from(authored.metrics.actor_steps));
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    signer_nonce = signer_nonce.saturating_add(authored.metrics.user_calls);
    user_calls.push(u64::from(authored.metrics.user_calls));
    user_ref_time.push(authored.metrics.user_dispatch.ref_time());
    user_proof_size.push(authored.metrics.user_dispatch.proof_size());
    let control_remaining = actor_control_limit
      .checked_sub(&authored.metrics.actor_control)
      .unwrap_or_else(Weight::zero);
    control_remaining_ref_time.push(control_remaining.ref_time());
    control_remaining_proof_size.push(control_remaining.proof_size());
    if !fixture.next_control_maximum.all_lte(control_remaining) {
      control_bound_blocks = control_bound_blocks.saturating_add(1);
    }
    let proof = replay_complete_block_in_wasm(
      &authored,
      wasm,
      &format!("{}-campaign-{block_number}", demand.label()),
    );
    execution_storage_proof.push(proof.execution_storage_proof_bytes);
    execution_compact_proof.push(proof.execution_compact_proof_bytes);
    verification_storage_proof.push(proof.verification_storage_proof_bytes);
    verification_compact_proof.push(proof.verification_compact_proof_bytes);
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }

  service.assert_terminal_state(wasm, pre_state);
  let service_summary = service.summary(1, W1_TARGET_BLOCKS + 1);
  let distinct_progressed = service_summary["distinctProgressedActors"]
    .as_u64()
    .expect("count is numeric");
  println!(
    "EXP_0066_SERVICE_WAITS_V1 {}",
    serde_json::json!({
      "workload": workload, "demand": demand.label(), "schedule": workload_schedule.label(),
      "workloadActorType": "System", "observations": service_summary,
    })
  );
  let step_histogram = steps
    .iter()
    .fold(BTreeMap::<u64, u32>::new(), |mut result, value| {
      result
        .entry(*value)
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
      result
    });
  let histogram = step_histogram
    .iter()
    .map(|(value, count)| format!("\"{value}\":{count}"))
    .collect::<Vec<_>>()
    .join(",");
  let minimum_steps = steps.iter().copied().min().unwrap_or(0);
  let maximum_steps = steps.iter().copied().max().unwrap_or(0);
  let target_met = total_cycles >= W1_TARGET_CYCLES && minimum_steps >= 100;
  println!(
    "EXP_0066_{workload}_V5 {{\"demand\":\"{}\",\"schedule\":\"{}\",\"referenceActiveActors\":{initial_active},\"referenceSystemActorIdentities\":{initial_identities},\"addedWorkloadActors\":{added_actor_count},\"activeActors\":{active_actor_count},\"actorIdentities\":{maximum_identities},\"targetCycles\":{W1_TARGET_CYCLES},\"eligibleBlocks\":{W1_TARGET_BLOCKS},\"committedCycles\":{total_cycles},\"successfulTransferSteps\":{total_cycles},\"failedOrSkippedSteps\":0,\"distinctProgressedActors\":{},\"meanSteps\":{:.4},\"p50Steps\":{},\"p95Steps\":{},\"p99Steps\":{},\"minSteps\":{minimum_steps},\"maxSteps\":{maximum_steps},\"stepHistogram\":{{{histogram}}},\"workloadActorType\":\"System\",\"targetMet\":{target_met},\"controlBoundBlocks\":{control_bound_blocks},\"userBoundBlocks\":{user_bound_blocks},\"totalUserCalls\":{},\"meanUserCalls\":{:.4},\"p50UserCalls\":{},\"p95UserCalls\":{},\"p99UserCalls\":{},\"minUserCalls\":{},\"maxUserCalls\":{},\"userRefTimeMin\":{},\"userRefTimeP50\":{},\"userRefTimeP95\":{},\"userRefTimeP99\":{},\"userRefTimeMax\":{},\"userProofSizeMin\":{},\"userProofSizeP50\":{},\"userProofSizeP95\":{},\"userProofSizeP99\":{},\"userProofSizeMax\":{},\"nextControlMaximumRefTime\":{},\"nextControlMaximumProofSize\":{},\"controlRemainingRefTimeMin\":{},\"controlRemainingRefTimeP50\":{},\"controlRemainingRefTimeP95\":{},\"controlRemainingRefTimeP99\":{},\"controlRemainingRefTimeMax\":{},\"controlRemainingProofSizeMin\":{},\"controlRemainingProofSizeP50\":{},\"controlRemainingProofSizeP95\":{},\"controlRemainingProofSizeP99\":{},\"controlRemainingProofSizeMax\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeP95\":{},\"controlRefTimeP99\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeP95\":{},\"controlProofSizeP99\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeP95\":{},\"effectRefTimeP99\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeP95\":{},\"effectProofSizeP99\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofP50\":{},\"executionStorageProofP95\":{},\"executionStorageProofP99\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofP50\":{},\"executionCompactProofP95\":{},\"executionCompactProofP99\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofP50\":{},\"verificationStorageProofP95\":{},\"verificationStorageProofP99\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofP50\":{},\"verificationCompactProofP95\":{},\"verificationCompactProofP99\":{},\"verificationCompactProofMax\":{}}}",
    demand.label(),
    workload_schedule.label(),
    distinct_progressed,
    total_cycles as f64 / f64::from(W1_TARGET_BLOCKS),
    nearest_rank_percentile(&steps, 50),
    nearest_rank_percentile(&steps, 95),
    nearest_rank_percentile(&steps, 99),
    user_calls.iter().sum::<u64>(),
    user_calls.iter().sum::<u64>() as f64 / f64::from(W1_TARGET_BLOCKS),
    nearest_rank_percentile(&user_calls, 50),
    nearest_rank_percentile(&user_calls, 95),
    nearest_rank_percentile(&user_calls, 99),
    user_calls.iter().copied().min().unwrap_or(0),
    user_calls.iter().copied().max().unwrap_or(0),
    user_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&user_ref_time, 50),
    nearest_rank_percentile(&user_ref_time, 95),
    nearest_rank_percentile(&user_ref_time, 99),
    user_ref_time.iter().copied().max().unwrap_or(0),
    user_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&user_proof_size, 50),
    nearest_rank_percentile(&user_proof_size, 95),
    nearest_rank_percentile(&user_proof_size, 99),
    user_proof_size.iter().copied().max().unwrap_or(0),
    fixture.next_control_maximum.ref_time(),
    fixture.next_control_maximum.proof_size(),
    control_remaining_ref_time
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&control_remaining_ref_time, 50),
    nearest_rank_percentile(&control_remaining_ref_time, 95),
    nearest_rank_percentile(&control_remaining_ref_time, 99),
    control_remaining_ref_time
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    control_remaining_proof_size
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&control_remaining_proof_size, 50),
    nearest_rank_percentile(&control_remaining_proof_size, 95),
    nearest_rank_percentile(&control_remaining_proof_size, 99),
    control_remaining_proof_size
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_ref_time, 50),
    nearest_rank_percentile(&control_ref_time, 95),
    nearest_rank_percentile(&control_ref_time, 99),
    control_ref_time.iter().copied().max().unwrap_or(0),
    control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_proof_size, 50),
    nearest_rank_percentile(&control_proof_size, 95),
    nearest_rank_percentile(&control_proof_size, 99),
    control_proof_size.iter().copied().max().unwrap_or(0),
    effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_ref_time, 50),
    nearest_rank_percentile(&effect_ref_time, 95),
    nearest_rank_percentile(&effect_ref_time, 99),
    effect_ref_time.iter().copied().max().unwrap_or(0),
    effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_proof_size, 50),
    nearest_rank_percentile(&effect_proof_size, 95),
    nearest_rank_percentile(&effect_proof_size, 99),
    effect_proof_size.iter().copied().max().unwrap_or(0),
    execution_storage_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_storage_proof, 50),
    nearest_rank_percentile(&execution_storage_proof, 95),
    nearest_rank_percentile(&execution_storage_proof, 99),
    execution_storage_proof.iter().copied().max().unwrap_or(0),
    execution_compact_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_compact_proof, 50),
    nearest_rank_percentile(&execution_compact_proof, 95),
    nearest_rank_percentile(&execution_compact_proof, 99),
    execution_compact_proof.iter().copied().max().unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_storage_proof, 50),
    nearest_rank_percentile(&verification_storage_proof, 95),
    nearest_rank_percentile(&verification_storage_proof, 99),
    verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_compact_proof, 50),
    nearest_rank_percentile(&verification_compact_proof, 95),
    nearest_rank_percentile(&verification_compact_proof, 99),
    verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
}

fn completed_manual_service_timeline(
  signal_block: u32,
  step_blocks: &[u32],
  completion_block: u32,
) -> serde_json::Value {
  let first = *step_blocks.first().expect("nonempty completed Contract");
  assert!(
    first > signal_block,
    "Manual readiness has a next-block causal floor"
  );
  assert_eq!(
    step_blocks.last(),
    Some(&completion_block),
    "completion accompanies the final Step"
  );
  let gaps = step_blocks
    .windows(2)
    .map(|pair| {
      assert!(
        pair[1] > pair[0],
        "Running progress preserves Q1 and chronology"
      );
      pair[1] - pair[0]
    })
    .collect::<Vec<_>>();
  serde_json::json!({
    "signalBlock": signal_block,
    "stepBlocks": step_blocks,
    "completionBlock": completion_block,
    "firstStepLatencyBlocks": first - signal_block,
    "completionLatencyBlocks": completion_block - signal_block,
    "runningServiceGapBlocks": gaps,
  })
}

#[test]
fn completed_manual_service_timeline_separates_first_service_completion_and_running() {
  let single = completed_manual_service_timeline(1, &[4], 4);
  assert_eq!(single["firstStepLatencyBlocks"], 3);
  assert_eq!(single["completionLatencyBlocks"], 3);
  assert_eq!(single["runningServiceGapBlocks"], serde_json::json!([]));
  let running = completed_manual_service_timeline(1, &[4, 9, 11], 11);
  assert_eq!(running["firstStepLatencyBlocks"], 3);
  assert_eq!(running["completionLatencyBlocks"], 10);
  assert_eq!(
    running["runningServiceGapBlocks"],
    serde_json::json!([5, 2])
  );
}

#[test]
#[should_panic(expected = "Q1 and chronology")]
fn completed_manual_service_timeline_rejects_duplicate_block_progress() {
  completed_manual_service_timeline(1, &[2, 2], 2);
}

#[test]
#[should_panic(expected = "completion accompanies the final Step")]
fn completed_manual_service_timeline_rejects_unmatched_completion() {
  completed_manual_service_timeline(1, &[2, 3], 4);
}

fn run_w3_opening_predicate_mixed_length_campaign(wasm: &[u8]) {
  let fixture = prepare_w3_fixture(wasm);
  let expected_steps = fixture
    .actor_profiles
    .values()
    .map(|profile| profile.step_count)
    .sum::<u32>();
  assert_eq!(fixture.actor_ids.len(), 126);
  assert_eq!(expected_steps, 252);

  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut committed_steps = 0u32;
  let mut completed_cycles = 0u32;
  let mut progressed_actors = BTreeSet::new();
  let mut progressed_step_keys = BTreeSet::new();
  let mut service_blocks = BTreeMap::<ActorId, Vec<u32>>::new();
  let mut completion_blocks = BTreeMap::new();
  let mut maximum_service_gap = 0u32;
  let mut by_predicates = BTreeMap::<u32, (u32, u32, u32, u32)>::new();
  let mut steps_per_block = Vec::new();
  let mut opening_per_block = Vec::new();
  let mut middle_per_block = Vec::new();
  let mut final_per_block = Vec::new();
  let mut control_ref_time = Vec::new();
  let mut control_proof_size = Vec::new();
  let mut effect_ref_time = Vec::new();
  let mut effect_proof_size = Vec::new();
  let mut execution_storage_proof = Vec::new();
  let mut execution_compact_proof = Vec::new();
  let mut verification_storage_proof = Vec::new();
  let mut verification_compact_proof = Vec::new();

  for block_number in 2..W3_BLOCK_LIMIT.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      UserDemand::ActorOnly,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    assert_eq!(authored.metrics.user_calls, 0);
    assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors
    );
    assert!(
      authored.metrics.actor_steps > 0,
      "W3 must make workload progress until its bounded matrix completes"
    );
    assert_eq!(
      authored.metrics.transfer_steps,
      authored.metrics.actor_steps
    );
    assert!(
      authored.metrics.closed_actors.is_empty()
        && authored.metrics.suspended_steps.is_empty()
        && authored.metrics.continued_steps.is_empty()
        && authored.metrics.paused_actors.is_empty()
        && authored.metrics.resumed_actors.is_empty()
        && authored.metrics.failed_cycle_actors.is_empty(),
      "W3 service gaps exclude lifecycle interruptions and retries"
    );
    for actor_id in &authored.metrics.completed_cycle_actors {
      assert!(
        completion_blocks.insert(*actor_id, block_number).is_none(),
        "each Manual matrix Actor completes exactly once"
      );
    }
    for (actor_id, step_index) in &authored.metrics.progressed_steps {
      assert!(
        progressed_step_keys.insert((*actor_id, *step_index)),
        "Manual-only W3 executes each matrix Step exactly once"
      );
      progressed_actors.insert(*actor_id);
      let blocks = service_blocks.entry(*actor_id).or_default();
      assert_eq!(
        *step_index as usize,
        blocks.len(),
        "W3 observes every cursor in order"
      );
      if let Some(previous) = blocks.last() {
        assert!(block_number > *previous, "W3 Running progress preserves Q1");
        maximum_service_gap = maximum_service_gap.max(block_number - *previous);
      }
      blocks.push(block_number);
      let profile = fixture
        .actor_profiles
        .get(actor_id)
        .expect("progressed W3 Actor has a matrix profile");
      let counts = by_predicates
        .entry(profile.opening_predicates_per_step)
        .or_default();
      counts.0 = counts.0.saturating_add(1);
      if *step_index == 0 {
        counts.1 = counts.1.saturating_add(1);
      } else if step_index.saturating_add(1) == profile.step_count {
        counts.3 = counts.3.saturating_add(1);
      } else {
        counts.2 = counts.2.saturating_add(1);
      }
    }
    committed_steps = committed_steps.saturating_add(authored.metrics.actor_steps);
    completed_cycles = completed_cycles.saturating_add(authored.metrics.completed_cycles);
    steps_per_block.push(u64::from(authored.metrics.actor_steps));
    opening_per_block.push(u64::from(authored.metrics.opening_steps));
    middle_per_block.push(u64::from(authored.metrics.middle_steps));
    final_per_block.push(u64::from(authored.metrics.final_steps));
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    let proof = replay_complete_block_in_wasm(
      &authored,
      wasm,
      &format!("W3-opening-predicate-mixed-length-{block_number}"),
    );
    execution_storage_proof.push(proof.execution_storage_proof_bytes);
    execution_compact_proof.push(proof.execution_compact_proof_bytes);
    verification_storage_proof.push(proof.verification_storage_proof_bytes);
    verification_compact_proof.push(proof.verification_compact_proof_bytes);
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
    if committed_steps == expected_steps {
      break;
    }
    assert!(
      committed_steps < expected_steps,
      "W3 cannot overrun its matrix"
    );
  }

  assert_eq!(
    committed_steps, expected_steps,
    "W3 completes within its block limit"
  );
  assert_eq!(completed_cycles, fixture.actor_ids.len() as u32);
  assert_eq!(progressed_actors.len(), fixture.actor_ids.len());
  assert_eq!(progressed_step_keys.len() as u32, expected_steps);
  assert_eq!(completion_blocks.len(), fixture.actor_ids.len());
  for actor_id in &fixture.actor_ids {
    let profile = &fixture.actor_profiles[actor_id];
    let blocks = &service_blocks[actor_id];
    assert_eq!(blocks.len(), profile.step_count as usize);
    println!(
      "EXP_0066_W3_SERVICE_V1 {}",
      serde_json::json!({
        "actorId": actor_id,
        "actorType": "System",
        "contractSteps": profile.step_count,
        "openingPredicatesPerStep": profile.opening_predicates_per_step,
        "timeline": completed_manual_service_timeline(1, blocks, completion_blocks[actor_id]),
      })
    );
  }
  for predicates in [0, 2, 4] {
    assert_eq!(
      by_predicates.get(&predicates),
      Some(&(84, 42, 14, 28)),
      "each predicate cohort preserves the same 1/2/3-Step matrix"
    );
  }
  let opening_steps = opening_per_block.iter().sum::<u64>();
  let middle_steps = middle_per_block.iter().sum::<u64>();
  let final_steps = final_per_block.iter().sum::<u64>();
  assert_eq!((opening_steps, middle_steps, final_steps), (126, 42, 84));
  let measured_blocks = steps_per_block.len();
  let step_histogram = steps_per_block
    .iter()
    .fold(BTreeMap::<u64, u32>::new(), |mut result, value| {
      result
        .entry(*value)
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
      result
    })
    .iter()
    .map(|(value, count)| format!("\"{value}\":{count}"))
    .collect::<Vec<_>>()
    .join(",");
  println!(
    "EXP_0066_W3_V1 {{\"schedule\":\"manual-only\",\"matrixActors\":{},\"repetitionsPerCell\":{W3_REPETITIONS_PER_CELL},\"contractLengths\":[1,2,3],\"openingPredicatesPerStep\":[0,2,4],\"measuredBlocks\":{measured_blocks},\"committedSteps\":{committed_steps},\"completedCycles\":{completed_cycles},\"distinctProgressedActors\":{},\"openingSteps\":{opening_steps},\"middleSteps\":{middle_steps},\"finalSteps\":{final_steps},\"maximumServiceGapBlocks\":{maximum_service_gap},\"meanSteps\":{:.4},\"p50Steps\":{},\"p95Steps\":{},\"p99Steps\":{},\"minSteps\":{},\"maxSteps\":{},\"stepHistogram\":{{{step_histogram}}},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeP95\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeP95\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeP95\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeP95\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofP50\":{},\"executionStorageProofP95\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofP50\":{},\"executionCompactProofP95\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofP50\":{},\"verificationStorageProofP95\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofP50\":{},\"verificationCompactProofP95\":{},\"verificationCompactProofMax\":{}}}",
    fixture.actor_ids.len(),
    progressed_actors.len(),
    f64::from(committed_steps) / measured_blocks as f64,
    nearest_rank_percentile(&steps_per_block, 50),
    nearest_rank_percentile(&steps_per_block, 95),
    nearest_rank_percentile(&steps_per_block, 99),
    steps_per_block.iter().copied().min().unwrap_or(0),
    steps_per_block.iter().copied().max().unwrap_or(0),
    control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_ref_time, 50),
    nearest_rank_percentile(&control_ref_time, 95),
    control_ref_time.iter().copied().max().unwrap_or(0),
    control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_proof_size, 50),
    nearest_rank_percentile(&control_proof_size, 95),
    control_proof_size.iter().copied().max().unwrap_or(0),
    effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_ref_time, 50),
    nearest_rank_percentile(&effect_ref_time, 95),
    effect_ref_time.iter().copied().max().unwrap_or(0),
    effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_proof_size, 50),
    nearest_rank_percentile(&effect_proof_size, 95),
    effect_proof_size.iter().copied().max().unwrap_or(0),
    execution_storage_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_storage_proof, 50),
    nearest_rank_percentile(&execution_storage_proof, 95),
    execution_storage_proof.iter().copied().max().unwrap_or(0),
    execution_compact_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_compact_proof, 50),
    nearest_rank_percentile(&execution_compact_proof, 95),
    execution_compact_proof.iter().copied().max().unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_storage_proof, 50),
    nearest_rank_percentile(&verification_storage_proof, 95),
    verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_compact_proof, 50),
    nearest_rank_percentile(&verification_compact_proof, 95),
    verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
}

#[test]
fn one_step_service_trace_tracks_heterogeneous_outcomes() {
  let fixture = prepare_w4_fixture(&[]);
  let mut service =
    OneStepServiceTrace::for_schedule(&fixture.actor_ids, WorkloadSchedule::MixedManualCadenced);
  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), &[], 1);
  for block in 2..=4 {
    let authored = author_complete_block_after(
      pre_state,
      &[],
      &parent,
      block,
      UserDemand::ActorOnly,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    service.observe(&authored.metrics, block);
    parent = authored.block.header;
    pre_state = authored.post_state;
  }
  service.assert_terminal_state(&[], pre_state);
  let mut samples = 0;
  for task in [
    WorkloadTask::Transfer,
    WorkloadTask::RouterSwapOut,
    WorkloadTask::StopCycle,
  ] {
    let summary = service.summary_for(1, 4, |id| fixture.actor_profiles[&id].task == Some(task));
    let count = summary["materializedSignalToStepBlocks"]["samples"]
      .as_u64()
      .unwrap();
    assert!(
      count > 0,
      "each W4 Task family contributes real successful service"
    );
    assert!(summary["interStepGapBlocks"]["max"].is_null());
    samples += count;
  }
  assert_eq!(
    service.summary(1, 4)["materializedSignalToStepBlocks"]["samples"],
    samples
  );
}

fn run_w4_heterogeneous_effect_campaign(wasm: &[u8], demand: UserDemand) {
  let fixture = prepare_w4_fixture(wasm);
  let population_counts = fixture.actor_profiles.values().fold(
    (0u32, 0u32, 0u32),
    |(transfer, swap_out, stop_cycle), profile| match profile.task {
      Some(WorkloadTask::Transfer) => (transfer.saturating_add(1), swap_out, stop_cycle),
      Some(WorkloadTask::RouterSwapOut) => (transfer, swap_out.saturating_add(1), stop_cycle),
      Some(WorkloadTask::StopCycle) => (transfer, swap_out, stop_cycle.saturating_add(1)),
      None => panic!("W4 profiles declare one exact task"),
    },
  );
  assert_eq!(
    population_counts,
    (W4_TRANSFER_ACTORS, W4_SWAP_OUT_ACTORS, W4_STOP_CYCLE_ACTORS)
  );

  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut signer_nonce = 0;
  let mut progressed = BTreeSet::new();
  let mut service =
    OneStepServiceTrace::for_schedule(&fixture.actor_ids, WorkloadSchedule::MixedManualCadenced);
  let mut total_cycles = 0u32;
  let mut transfer_steps = 0u32;
  let mut swap_out_steps = 0u32;
  let mut stop_cycle_steps = 0u32;
  let mut user_bound_blocks = 0u32;
  let mut pending_signals = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut steps = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut control_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut effect_ref_time = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut effect_proof_size = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut user_calls = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut execution_storage_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut execution_compact_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut verification_storage_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);
  let mut verification_compact_proof = Vec::with_capacity(W1_TARGET_BLOCKS as usize);

  for block_number in 2..W1_TARGET_BLOCKS.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      demand,
      &fixture.signer,
      signer_nonce,
      &fixture.actor_profiles,
    );
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W4 preserves Q1 in every full block"
    );
    service.observe(&authored.metrics, block_number);
    pending_signals.push(service.pending_signal_count() as u64);
    match demand {
      UserDemand::ActorOnly => {
        assert_eq!(authored.metrics.user_calls, 0);
        assert_eq!(authored.metrics.user_dispatch, Weight::zero());
        assert!(authored.metrics.next_user_weight.is_none());
      }
      UserDemand::ContinuousValid => {
        assert!(authored.metrics.user_calls > 0);
        let next_user = authored
          .metrics
          .next_user_weight
          .expect("continuous W4 demand records its first inadmissible valid call");
        let remaining = BlockResourceBudgetValue::get()
          .limits()
          .user_base_turn()
          .checked_sub(&authored.metrics.user_dispatch)
          .unwrap_or_else(Weight::zero);
        assert!(
          !next_user.all_lte(remaining),
          "continuous W4 demand reaches the User base-turn frontier"
        );
        user_bound_blocks = user_bound_blocks.saturating_add(1);
      }
      UserDemand::RefTimeHeavy | UserDemand::ContinuousShared => {
        unreachable!("the W4 campaign only prepares User-base-turn remarks")
      }
    }

    total_cycles = total_cycles.saturating_add(authored.metrics.completed_cycles);
    transfer_steps = transfer_steps.saturating_add(authored.metrics.transfer_steps);
    swap_out_steps = swap_out_steps.saturating_add(authored.metrics.swap_out_steps);
    stop_cycle_steps = stop_cycle_steps.saturating_add(authored.metrics.stop_cycle_steps);
    for (actor_id, _) in &authored.metrics.progressed_steps {
      progressed.insert(*actor_id);
    }
    steps.push(u64::from(authored.metrics.actor_steps));
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    signer_nonce = signer_nonce.saturating_add(authored.metrics.user_calls);
    user_calls.push(u64::from(authored.metrics.user_calls));
    let proof = replay_complete_block_in_wasm(
      &authored,
      wasm,
      &format!("W4-heterogeneous-{}-{block_number}", demand.label()),
    );
    execution_storage_proof.push(proof.execution_storage_proof_bytes);
    execution_compact_proof.push(proof.execution_compact_proof_bytes);
    verification_storage_proof.push(proof.verification_storage_proof_bytes);
    verification_compact_proof.push(proof.verification_compact_proof_bytes);
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }

  service.assert_terminal_state(wasm, pre_state);
  for (task, label) in [
    (WorkloadTask::Transfer, "transfer"),
    (WorkloadTask::RouterSwapOut, "swap-out"),
    (WorkloadTask::StopCycle, "stop-cycle"),
  ] {
    println!(
      "EXP_0066_W4_SERVICE_V1 {}",
      serde_json::json!({
        "demand": demand.label(), "task": label, "actorType": "System",
        "observations": service.summary_for(1, W1_TARGET_BLOCKS + 1, |id| fixture.actor_profiles[&id].task == Some(task)),
      })
    );
  }
  let committed_steps = transfer_steps
    .saturating_add(swap_out_steps)
    .saturating_add(stop_cycle_steps);
  assert_eq!(committed_steps, steps.iter().sum::<u64>() as u32);
  assert_eq!(total_cycles, committed_steps);
  assert!(transfer_steps > 0, "W4 commits Transfer effects");
  assert!(swap_out_steps > 0, "W4 commits Router SwapOut effects");
  assert!(
    stop_cycle_steps > 0,
    "W4 commits StopCycle control outcomes"
  );
  assert_eq!(
    user_bound_blocks,
    if demand == UserDemand::ContinuousValid {
      W1_TARGET_BLOCKS
    } else {
      0
    }
  );
  let minimum_steps = steps.iter().copied().min().unwrap_or(0);
  let target_met = committed_steps >= W1_TARGET_CYCLES && minimum_steps >= 100;
  assert!(
    pending_signals.iter().any(|count| *count > 0),
    "W4 observes current pending workload authority inside the campaign"
  );
  let pending_signal_zero_blocks = pending_signals.iter().filter(|count| **count == 0).count();
  println!(
    "EXP_0066_W4_V2 {{\"demand\":\"{}\",\"schedule\":\"mixed-manual-cadenced\",\"referenceSystemActorIdentities\":{REFERENCE_SYSTEM_ACTOR_IDENTITIES},\"nominalTransferSlots\":9500,\"retainedReferenceSlots\":15,\"workloadTransferActors\":{W4_TRANSFER_ACTORS},\"swapOutActors\":{W4_SWAP_OUT_ACTORS},\"stopCycleActors\":{W4_STOP_CYCLE_ACTORS},\"eligibleBlocks\":{W1_TARGET_BLOCKS},\"committedSteps\":{committed_steps},\"completedCycles\":{total_cycles},\"transferSteps\":{transfer_steps},\"swapOutSteps\":{swap_out_steps},\"stopCycleSteps\":{stop_cycle_steps},\"distinctProgressedActors\":{},\"workloadActorType\":\"System\",\"meanSteps\":{:.4},\"p50Steps\":{},\"p95Steps\":{},\"p99Steps\":{},\"minSteps\":{minimum_steps},\"maxSteps\":{},\"pendingSignalsMin\":{},\"pendingSignalsMax\":{},\"pendingSignalZeroBlocks\":{pending_signal_zero_blocks},\"targetMet\":{target_met},\"userBoundBlocks\":{user_bound_blocks},\"totalUserCalls\":{},\"nextControlMaximumRefTime\":{},\"nextControlMaximumProofSize\":{},\"nextEffectMaximumRefTime\":{},\"nextEffectMaximumProofSize\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeP95\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeP95\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeP95\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeP95\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofP50\":{},\"executionStorageProofP95\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofP50\":{},\"executionCompactProofP95\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofP50\":{},\"verificationStorageProofP95\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofP50\":{},\"verificationCompactProofP95\":{},\"verificationCompactProofMax\":{}}}",
    demand.label(),
    progressed.len(),
    f64::from(committed_steps) / f64::from(W1_TARGET_BLOCKS),
    nearest_rank_percentile(&steps, 50),
    nearest_rank_percentile(&steps, 95),
    nearest_rank_percentile(&steps, 99),
    steps.iter().copied().max().unwrap_or(0),
    pending_signals.iter().copied().min().unwrap_or(0),
    pending_signals.iter().copied().max().unwrap_or(0),
    user_calls.iter().sum::<u64>(),
    fixture.next_control_maximum.ref_time(),
    fixture.next_control_maximum.proof_size(),
    fixture.next_effect_maximum.ref_time(),
    fixture.next_effect_maximum.proof_size(),
    control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_ref_time, 50),
    nearest_rank_percentile(&control_ref_time, 95),
    control_ref_time.iter().copied().max().unwrap_or(0),
    control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_proof_size, 50),
    nearest_rank_percentile(&control_proof_size, 95),
    control_proof_size.iter().copied().max().unwrap_or(0),
    effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_ref_time, 50),
    nearest_rank_percentile(&effect_ref_time, 95),
    effect_ref_time.iter().copied().max().unwrap_or(0),
    effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_proof_size, 50),
    nearest_rank_percentile(&effect_proof_size, 95),
    effect_proof_size.iter().copied().max().unwrap_or(0),
    execution_storage_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_storage_proof, 50),
    nearest_rank_percentile(&execution_storage_proof, 95),
    execution_storage_proof.iter().copied().max().unwrap_or(0),
    execution_compact_proof.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&execution_compact_proof, 50),
    nearest_rank_percentile(&execution_compact_proof, 95),
    execution_compact_proof.iter().copied().max().unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_storage_proof, 50),
    nearest_rank_percentile(&verification_storage_proof, 95),
    verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    nearest_rank_percentile(&verification_compact_proof, 50),
    nearest_rank_percentile(&verification_compact_proof, 95),
    verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
}

fn run_current_w7_deadlines(wasm: &[u8], replay_wasm: bool) {
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let signers = (0..CURRENT_W7_SETUP_BLOCKS)
    .map(|batch| {
      let mut seed = [68u8; 32];
      seed[..4].copy_from_slice(&batch.to_le_bytes());
      sr25519::Pair::from_seed(&seed)
    })
    .collect::<Vec<_>>();
  let (actor_ids, actor_profiles, accounts, expected_deadlines) = ext.execute_with(|| {
    System::set_block_number(1);
    assert!(
      CURRENT_W7_SETUP_BATCH
        <= u32::from(<Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get())
    );
    for signer in &signers {
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(crate::AccountId::from(signer.public())),
        100_000_000 * crate::EXISTENTIAL_DEPOSIT,
      ));
    }
    let steps = BoundedVec::try_from(vec![StepOf::<Runtime> {
      precondition: None,
      task: Task::Transfer {
        to: super::common::BOB,
        asset: primitives::AssetKind::Native,
        amount: AmountResolution::Fixed(1_000_000_000 * crate::EXISTENTIAL_DEPOSIT),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    }])
    .expect("one real funding-unavailable retry Step fits");
    let mut actor_ids = Vec::new();
    let mut actor_profiles = BTreeMap::new();
    let mut accounts = BTreeMap::new();
    let mut expected_deadlines = BTreeMap::new();
    for index in 0..CURRENT_W7_SLEEPERS {
      let owner =
        crate::AccountId::from(signers[(index / CURRENT_W7_SETUP_BATCH) as usize].public());
      let attempt_block = 2 + index / CURRENT_W7_SETUP_BATCH;
      let due = CURRENT_W7_FIRST_DUE_BLOCK + index / CURRENT_W7_MEMBERS_PER_KEY;
      let mut schedule = actors_integration_tests::manual_schedule();
      schedule.cooldown_blocks = due - attempt_block;
      assert!(
        schedule.cooldown_blocks > 1,
        "every retry must sleep off-ring"
      );
      let actor_id =
        actors_integration_tests::create_user(owner.clone(), schedule, None, steps.clone());
      actors_integration_tests::fund_native(actor_id, 10_000 * crate::EXISTENTIAL_DEPOSIT);
      if index < CURRENT_W7_SETUP_BATCH {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(owner.clone()),
          actor_id
        ));
      }
      accounts.insert(
        actor_id,
        Actors::actor_identity(actor_id).unwrap().sovereign_account,
      );
      expected_deadlines.insert(actor_id, due);
      actor_ids.push(actor_id);
      actor_profiles.insert(
        actor_id,
        WorkloadActorProfile {
          step_count: 1,
          opening_predicates_per_step: 0,
          task: Some(WorkloadTask::Transfer),
        },
      );
    }
    (actor_ids, actor_profiles, accounts, expected_deadlines)
  });
  ext
    .commit_all()
    .expect("current W7 admitted fixture commits");
  let mut pre_state = ext.execute_with(current_top_storage);
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut setup_signed_calls = 0u32;
  let mut setup_control_ref_time = Vec::new();
  let mut setup_control_proof_size = Vec::new();
  let mut setup_suspensions = 0usize;
  for block in 2..CURRENT_W7_FIRST_DUE_BLOCK {
    let next_batch = ((block - 1) * CURRENT_W7_SETUP_BATCH) as usize;
    let calls = actor_ids
      .iter()
      .skip(next_batch)
      .take(CURRENT_W7_SETUP_BATCH as usize)
      .map(|id| RuntimeCall::Actors(pallet_deos_actors::Call::manual_trigger { actor_id: *id }))
      .collect::<Vec<_>>();
    let authored = author_complete_block_after_with_calls(
      pre_state,
      wasm,
      &parent,
      block,
      UserDemand::ActorOnly,
      signers.get((block - 1) as usize).unwrap_or(&signers[0]),
      0,
      &actor_profiles,
      &calls,
    );
    let attempted_batch = ((block - 2) * CURRENT_W7_SETUP_BATCH) as usize;
    assert_eq!(
      authored.metrics.suspended_steps,
      actor_ids[attempted_batch..attempted_batch + CURRENT_W7_SETUP_BATCH as usize]
        .iter()
        .map(|id| (*id, 0))
        .collect::<Vec<_>>(),
      "ordinary first Attempts create the declared sleeper batch"
    );
    assert_eq!(
      authored.metrics.actor_steps, 0,
      "funding refusal invokes no effect"
    );
    assert_eq!(authored.metrics.completed_cycles, 0);
    assert!(authored.metrics.closed_actors.is_empty());
    setup_suspensions += authored.metrics.suspended_steps.len();
    setup_control_ref_time.push(authored.metrics.actor_control.ref_time());
    setup_control_proof_size.push(authored.metrics.actor_control.proof_size());
    setup_signed_calls += calls.len() as u32;
    if replay_wasm {
      replay_complete_block_in_wasm(&authored, wasm, &format!("current-W7-setup-{block}"));
    }
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }
  assert_eq!(setup_suspensions, CURRENT_W7_SLEEPERS as usize);
  let mut previous = current_w7_deadline_snapshot(wasm, &pre_state, &actor_ids);
  assert_eq!(previous.handles, expected_deadlines);
  assert_eq!(previous.suspended.len(), CURRENT_W7_SLEEPERS as usize);
  assert!(previous.service.is_empty() && previous.closed.is_empty());
  assert_eq!(previous.block_index_len, CURRENT_W7_DEADLINE_KEYS);
  assert_eq!(
    previous.block_headers.len(),
    CURRENT_W7_DEADLINE_KEYS as usize
  );
  assert!(
    previous
      .block_headers
      .values()
      .all(|count| *count == CURRENT_W7_MEMBERS_PER_KEY)
  );
  let balances_before = {
    let mut state = TestExternalities::new_with_code_and_state(
      wasm,
      pre_state.clone(),
      crate::VERSION.state_version(),
    );
    state.execute_with(|| {
      for key in previous.block_headers.keys() {
        let header = Actors::deadline_headers(WakeupKey::Block(*key)).unwrap();
        assert_eq!(
          header.page_count, 1,
          "32 sleepers fill exactly one C32 page per key"
        );
      }
      accounts
        .iter()
        .map(|(id, account)| (*id, Balances::free_balance(account)))
        .collect::<BTreeMap<_, _>>()
    })
  };
  let initial_storage_bytes = persistent_storage_bytes(&actors_storage_entries(&pre_state));
  let mut storage_bytes = vec![initial_storage_bytes];
  let mut mutations = Vec::new();
  let mut return_blocks = BTreeMap::<ActorId, u32>::new();
  let mut retry_blocks = BTreeMap::<ActorId, u32>::new();
  let mut due_to_return = Vec::new();
  let mut return_to_attempt = Vec::new();
  let mut returned_per_block = Vec::new();
  let mut overdue_per_block = Vec::new();
  let mut control_ref_time = Vec::new();
  let mut control_proof_size = Vec::new();
  let mut effect_ref_time = Vec::new();
  let mut effect_proof_size = Vec::new();
  let mut execution_storage_proof = Vec::new();
  let mut execution_compact_proof = Vec::new();
  let mut verification_storage_proof = Vec::new();
  let mut verification_compact_proof = Vec::new();
  let arrival_end = CURRENT_W7_FIRST_DUE_BLOCK + CURRENT_W7_ARRIVAL_BLOCKS - 1;
  let final_block = arrival_end + CURRENT_W7_DRAIN_BLOCKS;
  let mut timeline = Vec::new();
  for block in CURRENT_W7_FIRST_DUE_BLOCK..=final_block {
    let before_entries = actors_storage_entries(&pre_state);
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block,
      UserDemand::ActorOnly,
      &signers[0],
      0,
      &actor_profiles,
    );
    assert_eq!(
      authored.metrics.actor_steps, 0,
      "unfunded Attempts cannot fabricate useful effects"
    );
    assert_eq!(authored.metrics.completed_cycles, 0);
    assert!(
      authored.metrics.suspended_steps.is_empty(),
      "retry exhaustion cannot re-sleep"
    );
    let current = current_w7_deadline_snapshot(wasm, &authored.post_state, &actor_ids);
    let returned = actor_ids
      .iter()
      .filter(|id| previous.handles.contains_key(id) && !current.handles.contains_key(id))
      .copied()
      .collect::<Vec<_>>();
    assert_eq!(
      returned,
      actor_ids[return_blocks.len()..return_blocks.len() + returned.len()],
      "deadline extraction preserves the due-key/C32-slot prefix"
    );
    for id in &returned {
      let due = expected_deadlines[id];
      assert!(block >= due, "no future sleeper is extracted");
      assert!(current.service.contains(id) && current.suspended.contains(id));
      assert!(
        return_blocks.insert(*id, block).is_none(),
        "one deadline extraction per sleeper"
      );
      due_to_return.push(u64::from(block - due));
    }
    let continued = authored
      .metrics
      .continued_steps
      .iter()
      .map(|(id, cursor)| {
        assert_eq!(*cursor, 0, "retry resumes the original cursor");
        let returned_at = return_blocks[id];
        assert_eq!(
          block,
          returned_at + 1,
          "returned workload receives ordinary B+1 Service"
        );
        assert!(
          retry_blocks.insert(*id, block).is_none(),
          "one terminal retry per sleeper"
        );
        return_to_attempt.push(u64::from(block - returned_at));
        *id
      })
      .collect::<Vec<_>>();
    assert_eq!(authored.metrics.failed_cycle_actors, continued);
    assert_eq!(
      authored.metrics.closed_actors,
      continued
        .iter()
        .map(|id| (*id, CloseReason::RetryAttemptsExhausted))
        .collect::<Vec<_>>()
    );
    assert_eq!(current.closed, retry_blocks.keys().copied().collect());
    assert_eq!(
      current.suspended.len(),
      current.handles.len() + current.service.len()
    );
    assert_eq!(
      current.suspended.len() + current.closed.len(),
      CURRENT_W7_SLEEPERS as usize
    );
    assert_eq!(
      current.block_headers.values().sum::<u32>() as usize,
      current.handles.len()
    );
    assert_eq!(
      current.block_index_len as usize,
      current.block_headers.len()
    );
    let overdue = current
      .handles
      .values()
      .filter(|due| **due <= block)
      .count();
    let after_entries = actors_storage_entries(&authored.post_state);
    mutations.push(persistent_mutations(&before_entries, &after_entries));
    storage_bytes.push(persistent_storage_bytes(&after_entries));
    returned_per_block.push(returned.len() as u64);
    overdue_per_block.push(overdue as u64);
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    timeline.push(serde_json::json!({
      "block": block, "newlyDue": if block <= arrival_end { CURRENT_W7_MEMBERS_PER_KEY } else { 0 },
      "returned": returned.len(), "terminalRetries": continued.len(),
      "sleeping": current.handles.len(), "overdue": overdue,
      "service": current.service.len(), "closed": current.closed.len(),
      "deadlineKeys": current.block_index_len,
    }));
    if replay_wasm {
      let proof =
        replay_complete_block_in_wasm(&authored, wasm, &format!("current-W7-deadlines-{block}"));
      execution_storage_proof.push(proof.execution_storage_proof_bytes);
      execution_compact_proof.push(proof.execution_compact_proof_bytes);
      verification_storage_proof.push(proof.verification_storage_proof_bytes);
      verification_compact_proof.push(proof.verification_compact_proof_bytes);
    }
    previous = current;
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }
  assert!(
    !return_blocks.is_empty() && !retry_blocks.is_empty(),
    "real deadline service is observed"
  );
  let mut final_state =
    TestExternalities::new_with_code_and_state(wasm, pre_state, crate::VERSION.state_version());
  final_state.execute_with(|| {
    for (id, account) in &accounts {
      assert_eq!(
        Balances::free_balance(account),
        balances_before[id],
        "non-invoked retry and close preserve exact sovereign custody"
      );
    }
  });
  let inserted = mutations.iter().map(|m| m.inserted).collect::<Vec<_>>();
  let removed = mutations.iter().map(|m| m.removed).collect::<Vec<_>>();
  let changed = mutations.iter().map(|m| m.changed).collect::<Vec<_>>();
  let censored_lateness = previous
    .handles
    .values()
    .map(|due| u64::from(final_block - due))
    .collect::<Vec<_>>();
  let target_met = previous.handles.is_empty()
    && previous.service.is_empty()
    && previous.closed.len() == CURRENT_W7_SLEEPERS as usize;
  println!(
    "CURRENT_W7_DEADLINES_V1 {}",
    serde_json::json!({
      "productionWasmReplayed": replay_wasm, "actorType": "User",
      "sleepers": CURRENT_W7_SLEEPERS, "deadlineKeys": CURRENT_W7_DEADLINE_KEYS,
      "membersPerKey": CURRENT_W7_MEMBERS_PER_KEY, "setupBlocks": CURRENT_W7_SETUP_BLOCKS,
      "setupSuspensions": setup_suspensions, "setupSignedManualCalls": setup_signed_calls,
      "owners": signers.len(), "actorsPerOwner": CURRENT_W7_SETUP_BATCH,
      "arrivalBlocks": CURRENT_W7_ARRIVAL_BLOCKS, "drainBlocks": CURRENT_W7_DRAIN_BLOCKS,
      "firstDueBlock": CURRENT_W7_FIRST_DUE_BLOCK, "finalBlock": final_block,
      "successfulEffects": 0, "completedCycles": 0, "returnedSleepers": return_blocks.len(),
      "failedCycles": retry_blocks.len(), "remainingSleeping": previous.handles.len(),
      "remainingService": previous.service.len(), "remainingDeadlineKeys": previous.block_index_len,
      "targetMet": target_met, "dueToReturnBlocks": observed_distribution(&due_to_return),
      "returnToAttemptBlocks": observed_distribution(&return_to_attempt),
      "returnedPerBlock": observed_distribution(&returned_per_block),
      "overdueSleepersPerBlock": observed_distribution(&overdue_per_block),
      "censoredDeadlineLatenessBlocks": observed_distribution(&censored_lateness),
      "setupControlRefTime": observed_distribution(&setup_control_ref_time),
      "setupControlProofSize": observed_distribution(&setup_control_proof_size),
      "controlRefTime": observed_distribution(&control_ref_time),
      "controlProofSize": observed_distribution(&control_proof_size),
      "effectRefTime": observed_distribution(&effect_ref_time),
      "effectProofSize": observed_distribution(&effect_proof_size),
      "actorsPersistentStorage": {
        "initialBytes": initial_storage_bytes, "finalBytes": storage_bytes.last(),
        "minBytes": storage_bytes.iter().min(), "maxBytes": storage_bytes.iter().max(),
      },
      "actorsPersistentKeyMutationsPerBlock": {
        "inserted": observed_distribution(&inserted), "removed": observed_distribution(&removed),
        "changed": observed_distribution(&changed),
      },
      "exactWasmProofs": {
        "executionStorage": observed_distribution(&execution_storage_proof),
        "executionCompact": observed_distribution(&execution_compact_proof),
        "verificationStorage": observed_distribution(&verification_storage_proof),
        "verificationCompact": observed_distribution(&verification_compact_proof),
      },
      "timeline": timeline,
    })
  );
}

fn actor_lifecycle_event_label(event: &Event<Runtime>) -> Option<(ActorId, &'static str)> {
  let (actor_id, kind) = match event {
    Event::CycleStarted { actor_id, .. } => (*actor_id, "CycleStarted"),
    Event::CycleSummary { actor_id, .. } => (*actor_id, "CycleSummary"),
    Event::CycleSuspended { actor_id, .. } => (*actor_id, "CycleSuspended"),
    Event::CycleContinued { actor_id, .. } => (*actor_id, "CycleContinued"),
    Event::CycleCancelled { actor_id, .. } => (*actor_id, "CycleCancelled"),
    Event::CycleStopped { actor_id, .. } => (*actor_id, "CycleStopped"),
    Event::StepSkipped { actor_id, .. } => (*actor_id, "StepSkipped"),
    Event::StepFailed { actor_id, .. } => (*actor_id, "StepFailed"),
    Event::TransferExecuted { actor_id, .. } => (*actor_id, "TransferExecuted"),
    Event::SwapExecuted { actor_id, .. } => (*actor_id, "SwapExecuted"),
    Event::ActorPaused { actor_id } => (*actor_id, "ActorPaused"),
    Event::ActorResumed { actor_id } => (*actor_id, "ActorResumed"),
    Event::ActorClosed { actor_id, .. } => (*actor_id, "ActorClosed"),
    Event::ContractUpdated { actor_id } => (*actor_id, "ContractUpdated"),
    Event::ManualTriggerSet { actor_id } => (*actor_id, "ManualTriggerSet"),
    Event::TriggerOccurrenceProcessed { actor_id, .. } => (*actor_id, "TriggerOccurrenceProcessed"),
    _ => return None,
  };
  Some((actor_id, kind))
}

fn actor_lifecycle_observation(
  wasm: &[u8],
  storage: &Storage,
  actor_ids: &[ActorId],
) -> serde_json::Value {
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    storage.clone(),
    crate::VERSION.state_version(),
  );
  ext.execute_with(|| {
    let events = System::events().into_iter().enumerate().filter_map(|(ordinal, record)| {
      let RuntimeEvent::Actors(event) = record.event else { return None; };
      let (actor_id, kind) = actor_lifecycle_event_label(&event)?;
      actor_ids.contains(&actor_id).then(|| serde_json::json!({
        "ordinal": ordinal, "actorId": actor_id, "kind": kind,
        "phase": format!("{:?}", record.phase), "details": format!("{event:?}"),
      }))
    }).collect::<Vec<_>>();
    let actors = actor_ids.iter().map(|actor_id| {
      let Some(state) = Actors::active_actor_state(*actor_id) else {
        return serde_json::json!({"actorId": actor_id, "active": false});
      };
      serde_json::json!({
        "actorId": actor_id, "active": true,
        "actorType": format!("{:?}", state.identity.actor_class.actor_type()),
        "trigger": format!("{:?}", state.contract.trigger),
        "temporalAnchorTick": state.hot.trigger_runtime_state.temporal_anchor_tick(),
        "atTimeConsumed": matches!(&state.contract.trigger, Trigger::AtTime { .. })
          .then(|| state.hot.trigger_runtime_state.temporal_occurrence_consumed()),
        "window": state.contract.window.map(|w| serde_json::json!({"start": w.start, "end": w.end})),
        "lifecycle": format!("{:?}", state.hot.lifecycle),
        "cycleState": format!("{:?}", state.hot.cycle_state),
        "pendingSignal": state.hot.pending_signal, "queueTicket": state.hot.queue_ticket,
        "wakeup": state.hot.wakeup_pointer.map(|p| format!("{:?}", p.block)),
        "triggerWakeupTick": state.hot.trigger_wakeup_pointer.map(|p| p.tick),
        "run": state.run_state.map(|run| serde_json::json!({
          "cycleNonce": run.cycle_nonce, "cursor": run.cursor,
          "eligibleAt": run.eligible_at, "lastAttemptBlock": run.last_attempt_block,
          "lastCommittedStepBlock": run.last_committed_step_block,
          "unsuccessfulAttemptsAtCursor": run.unsuccessful_attempts_at_cursor,
          "suspension": run.suspension.map(|reason| format!("{reason:?}")),
        })),
      })
    }).collect::<Vec<_>>();
    serde_json::json!({
      "block": System::block_number(), "timestampMillis": crate::Timestamp::get(),
      "cadenceTickMillis": <Runtime as pallet_deos_actors::Config>::CadenceTickMillis::get(),
      "events": events, "actors": actors,
    })
  })
}

#[test]
fn actor_lifecycle_observation_preserves_event_positions_and_filters_population() {
  let mut ext = TestExternalities::new_with_code_and_state(
    &[],
    reference_genesis_storage(&[]),
    crate::VERSION.state_version(),
  );
  let storage = ext.execute_with(|| {
    System::set_block_number(1);
    System::reset_events();
    System::deposit_event(RuntimeEvent::Actors(Event::CycleStarted {
      actor_id: 15,
      cycle_nonce: 1,
    }));
    System::deposit_event(RuntimeEvent::Actors(Event::CycleStarted {
      actor_id: 16,
      cycle_nonce: 1,
    }));
    System::deposit_event(RuntimeEvent::Actors(Event::ActorClosed {
      actor_id: 15,
      reason: CloseReason::OwnerInitiated,
    }));
    current_top_storage()
  });
  let observation = actor_lifecycle_observation(&[], &storage, &[15]);
  let events = observation["events"].as_array().unwrap();
  assert_eq!(events.len(), 2);
  assert_eq!(events[0]["ordinal"], 0);
  assert_eq!(events[0]["kind"], "CycleStarted");
  assert_eq!(events[1]["ordinal"], 2);
  assert_eq!(events[1]["kind"], "ActorClosed");
  assert_eq!(observation["actors"][0]["active"], false);
}

#[test]
fn full_executive_funded_pipeline_collection_witness() {
  use pallet_deos_actors::ActorType;
  use polkadot_sdk::frame_support::traits::Get;
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let mut rows = Vec::new();
  for actor_type in [ActorType::System, ActorType::User] {
    let mut ext = TestExternalities::new_with_code_and_state(
      &[],
      reference_genesis_storage(&[]),
      crate::VERSION.state_version(),
    );
    let (actor_id, payer, trigger_fee, payer_after_trigger, sink_after_trigger) =
      ext.execute_with(|| {
        System::set_block_number(1);
        assert!(!Actors::user_creation_enabled());
        if actor_type == ActorType::User {
          assert_ok!(Actors::set_user_creation_enabled(
            RuntimeOrigin::root(),
            true
          ));
          System::assert_last_event(RuntimeEvent::Actors(Event::UserCreationEnabledSet {
            enabled: true,
          }));
        }
        assert_eq!(
          Actors::user_creation_enabled(),
          actor_type == ActorType::User
        );
        assert_ok!(Balances::force_set_balance(
          RuntimeOrigin::root(),
          MultiAddress::Id(ALICE),
          u128::MAX / 4
        ));
        let steps = BoundedVec::try_from(vec![StepOf::<Runtime> {
          precondition: None,
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        }])
        .unwrap();
        let actor_id = match actor_type {
          ActorType::System => actors_integration_tests::create_system(
            ALICE,
            actors_integration_tests::manual_schedule(),
            None,
            steps,
          ),
          ActorType::User => actors_integration_tests::create_user(
            ALICE,
            actors_integration_tests::manual_schedule(),
            None,
            steps,
          ),
        };
        actors_integration_tests::fund_native(actor_id, 1_000 * crate::EXISTENTIAL_DEPOSIT);
        let payer = Actors::active_actor_state(actor_id)
          .unwrap()
          .identity
          .sovereign_account;
        let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
        let payer_before = Balances::free_balance(&payer);
        let sink_before = Balances::free_balance(&sink);
        assert_ok!(Actors::manual_trigger(
          match actor_type {
            ActorType::System => RuntimeOrigin::root(),
            ActorType::User => RuntimeOrigin::signed(ALICE),
          },
          actor_id
        ));
        let fees = System::events()
          .into_iter()
          .filter_map(|record| match record.event {
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id, fee, ..
            }) if id == actor_id => Some(fee),
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(fees.len(), 1);
        let payer_after = Balances::free_balance(&payer);
        let sink_after = Balances::free_balance(&sink);
        assert_eq!(payer_before - payer_after, fees[0]);
        assert_eq!(sink_after - sink_before, fees[0]);
        (actor_id, payer, fees[0], payer_after, sink_after)
      });
    ext.commit_all().unwrap();
    let storage = ext.execute_with(current_top_storage);
    let profiles = BTreeMap::from([(
      actor_id,
      WorkloadActorProfile {
        step_count: 1,
        opening_predicates_per_step: 0,
        task: Some(WorkloadTask::StopCycle),
      },
    )]);
    let authored =
      author_complete_block(storage, &[], 2, UserDemand::ActorOnly, &signer, &profiles);
    assert_eq!(authored.metrics.stop_cycle_steps, 1);
    assert_eq!(authored.metrics.completed_cycles, 1);
    assert_eq!(authored.metrics.actor_effect, Weight::zero());
    assert_complete_block_replays_natively(&authored, &[]);
    let mut after = TestExternalities::new_with_code_and_state(
      &[],
      authored.post_state,
      crate::VERSION.state_version(),
    );
    let pipeline_fee = after.execute_with(|| {
      let mut pipeline = Vec::new();
      let mut action_receipts = 0;
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: id, fee })
            if id == actor_id =>
          {
            pipeline.push(fee)
          }
          RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id: id, .. }) if id == actor_id => {
            action_receipts += 1
          }
          _ => {}
        }
      }
      assert_eq!(action_receipts, 0, "StopCycle does not invoke an Action");
      match actor_type {
        ActorType::System => {
          assert!(pipeline.is_empty());
          assert_eq!(trigger_fee, 0);
        }
        ActorType::User => {
          assert_eq!(pipeline.len(), 1);
          assert!(pipeline[0] > 0);
          assert!(trigger_fee > 0);
        }
      }
      let fee = pipeline.into_iter().sum::<u128>();
      assert_eq!(payer_after_trigger - Balances::free_balance(&payer), fee);
      assert_eq!(
        Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get())
          - sink_after_trigger,
        fee
      );
      fee
    });
    rows.push(serde_json::json!({
      "actorType": format!("{actor_type:?}"), "triggerFee": trigger_fee.to_string(), "pipelineFee": pipeline_fee.to_string(),
      "control": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
      "prepassControl": [authored.metrics.prepass_actor_control.ref_time(), authored.metrics.prepass_actor_control.proof_size()],
      "effect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
    }));
  }
  println!(
    "ACTOR_PIPELINE_COLLECTION_WITNESS_V1 {}",
    serde_json::json!({
      "mode": "native-full-executive-monetary-ledger", "task": "StopCycle", "rows": rows,
    })
  );
}

fn run_funded_action_collection_witness(wasm: &[u8], replay_wasm: bool) {
  use polkadot_sdk::frame_support::traits::Get;
  use primitives::AssetKind;

  let recipient = super::common::BOB;
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let (actor_id, payer, trigger_fee, payer_after_trigger, sink_after_trigger, recipient_before) =
    ext.execute_with(|| {
      System::set_block_number(1);
      assert!(!Actors::user_creation_enabled());
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      assert!(Actors::user_creation_enabled());
      System::assert_last_event(RuntimeEvent::Actors(Event::UserCreationEnabledSet {
        enabled: true,
      }));
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(ALICE),
        u128::MAX / 4
      ));
      let steps = BoundedVec::try_from(vec![StepOf::<Runtime> {
        precondition: None,
        task: Task::Transfer {
          to: recipient.clone(),
          asset: AssetKind::Native,
          amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }])
      .unwrap();
      let actor_id = actors_integration_tests::create_user(
        ALICE,
        actors_integration_tests::manual_schedule(),
        None,
        steps,
      );
      actors_integration_tests::fund_native(actor_id, 1_000 * crate::EXISTENTIAL_DEPOSIT);
      let payer = Actors::active_actor_state(actor_id)
        .unwrap()
        .identity
        .sovereign_account;
      let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
      let payer_before = Balances::free_balance(&payer);
      let sink_before = Balances::free_balance(&sink);
      let recipient_before = Balances::free_balance(&recipient);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let trigger_fee = System::events()
        .into_iter()
        .find_map(|record| match record.event {
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id: id, fee, ..
          }) if id == actor_id => Some(fee),
          _ => None,
        })
        .expect("funded User trigger fee");
      assert!(trigger_fee > 0);
      assert_eq!(payer_before - Balances::free_balance(&payer), trigger_fee);
      assert_eq!(Balances::free_balance(&sink) - sink_before, trigger_fee);
      (
        actor_id,
        payer.clone(),
        trigger_fee,
        Balances::free_balance(&payer),
        Balances::free_balance(&sink),
        recipient_before,
      )
    });
  ext.commit_all().unwrap();
  let storage = ext.execute_with(current_top_storage);
  let profiles = BTreeMap::from([(
    actor_id,
    WorkloadActorProfile {
      step_count: 1,
      opening_predicates_per_step: 0,
      task: Some(WorkloadTask::Transfer),
    },
  )]);
  let authored = author_complete_block(storage, wasm, 2, UserDemand::ActorOnly, &signer, &profiles);
  assert_eq!(authored.metrics.transfer_steps, 1);
  assert_eq!(authored.metrics.completed_cycles, 1);
  assert_complete_block_replays_natively(&authored, wasm);
  let proof = replay_wasm
    .then(|| replay_complete_block_in_wasm(&authored, wasm, "funded-user-action-collection"));
  let mut after = TestExternalities::new_with_code_and_state(
    wasm,
    authored.post_state,
    crate::VERSION.state_version(),
  );
  let (pipeline_fee, action_fee) = after.execute_with(|| {
    let mut pipeline = Vec::new();
    let mut action = Vec::new();
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: id, fee }) if id == actor_id => {
          pipeline.push(fee)
        }
        RuntimeEvent::Actors(Event::ActionFeeCharged {
          actor_id: id, fee, ..
        }) if id == actor_id => action.push(fee),
        _ => {}
      }
    }
    assert_eq!(pipeline.len(), 1);
    assert_eq!(action.len(), 1);
    assert!(pipeline[0] > 0 && action[0] > 0);
    let total_fees = pipeline[0] + action[0];
    assert_eq!(
      payer_after_trigger - Balances::free_balance(&payer),
      crate::EXISTENTIAL_DEPOSIT + total_fees
    );
    assert_eq!(
      Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get())
        - sink_after_trigger,
      total_fees
    );
    assert_eq!(
      Balances::free_balance(&recipient) - recipient_before,
      crate::EXISTENTIAL_DEPOSIT
    );
    (pipeline[0], action[0])
  });
  println!(
    "ACTOR_FUNDED_USER_ACTION_COLLECTION_V1 {}",
    serde_json::json!({
      "mode": if replay_wasm { "native-author-linked-to-exact-wasm" } else { "native-full-executive-monetary-ledger" },
      "triggerFee": trigger_fee.to_string(), "pipelineFee": pipeline_fee.to_string(),
      "actionFee": action_fee.to_string(),
      "control": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
      "effect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
      "executionStorageProofBytes": proof.as_ref().map(|p| p.execution_storage_proof_bytes),
      "verificationStorageProofBytes": proof.as_ref().map(|p| p.verification_storage_proof_bytes),
    })
  );
}

#[test]
fn full_executive_funded_action_collection_witness() {
  run_funded_action_collection_witness(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_funded_action_collection_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "funded User Action witness must remain bound to the accepted production Wasm"
  );
  run_funded_action_collection_witness(&wasm, true);
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FullPayloadSignedCall {
  Create,
  Activate,
  Replace,
}

#[test]
fn full_executive_user_creation_full_payload_signed_fixture() {
  assert_full_payload_signed_call(FullPayloadSignedCall::Create);
}

#[test]
fn full_executive_user_installation_full_payload_signed_fixture() {
  for kind in [
    FullPayloadSignedCall::Activate,
    FullPayloadSignedCall::Replace,
  ] {
    assert_full_payload_signed_call(kind);
  }
}

fn assert_full_payload_signed_call(kind: FullPayloadSignedCall) {
  use codec::MaxEncodedLen;
  use pallet_deos_actors::{
    ActorContract, AdmissionCertificateAuthorityProvider, AssetFilter, FundingSourcePolicy,
    Predicate, SourceFilter,
  };
  use polkadot_sdk::frame_support::traits::{Get, fungible::InspectHold};
  use primitives::AssetKind;

  // In-memory fixture key only. No network, external signing or production-Wasm claim.
  let signer = sr25519::Pair::from_seed(&[77u8; 32]);
  let owner = crate::AccountId::from(signer.public());
  let sovereign = Actors::sovereign_account_id(&owner, 0);
  let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
  let mut ext = TestExternalities::new_with_code_and_state(
    &[],
    reference_genesis_storage(&[]),
    crate::VERSION.state_version(),
  );
  let (contract, expected, actor_id, owner_before, sink_before, custody_before, held_before) = ext
    .execute_with(|| {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(owner.clone()),
        1_000_000_000_000_000_000,
      ));
      let maximum = <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize::get();
      let mut sources = vec![owner.clone()];
      sources.extend((1..maximum).map(|index| {
        crate::AccountId::new(polkadot_sdk::sp_io::hashing::blake2_256(&index.encode()))
      }));
      sources.sort_by_key(Encode::encode);
      sources.dedup();
      assert_eq!(sources.len() as u32, maximum);
      let assets = (1..=maximum).map(AssetKind::Local).collect::<Vec<_>>();
      for id in 1..=maximum {
        assert_ok!(Assets::force_create(
          RuntimeOrigin::root(),
          id,
          MultiAddress::Id(owner.clone()),
          true,
          1,
        ));
      }
      for id in 1..=maximum {
        assert_eq!(Assets::balance(id, &sovereign), 0);
      }
      let reason = crate::RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState);
      assert_eq!(Balances::balance_on_hold(&reason, &owner), 0);
      let predicates = <Runtime as pallet_deos_actors::Config>::MaxPredicatesPerStep::get();
      let mut clauses = Vec::new();
      for index in 1..=predicates {
        let input = AssetKind::Local(0x3000_0000 + index);
        let output = AssetKind::Local(0x3001_0000 + index);
        crate::configs::oracle_config::ensure_deos_router_pool_feeds(input, output).unwrap();
        let feed = crate::configs::oracle_config::deos_router_pool_feed(input, output);
        assert_eq!(
          feed.encoded_size(),
          primitives::OracleFeedId::max_encoded_len()
        );
        assert_ok!(crate::Oracle::publish(
          RuntimeOrigin::signed(crate::DeosRouter::account_id()),
          feed,
          1,
        ));
        clauses.push(
          vec![Predicate::ObservationAbove {
            feed,
            threshold: 0,
            max_age_blocks: 100,
          }]
          .try_into()
          .unwrap(),
        );
      }
      let mut precondition: pallet_deos_actors::PreconditionOf<Runtime> =
        pallet_deos_actors::Precondition {
          clauses: clauses.try_into().unwrap(),
        };
      precondition
        .clauses
        .sort_by_key(|clause| std::cmp::Reverse(clause.encode()));
      assert_eq!(precondition.predicate_count(), predicates);
      let task = Task::RemoveLiquidity {
        lp_asset: assets[0],
        asset_a: assets[0],
        asset_b: assets[0],
        lp_amount: AmountResolution::Fixed(1),
        min_amount_a: 1,
        min_amount_b: 1,
      };
      assert_eq!(
        task.encoded_size(),
        pallet_deos_actors::TaskOf::<Runtime>::max_encoded_len()
      );
      let step = StepOf::<Runtime> {
        precondition: Some(precondition),
        task,
        on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
      };
      let delay = <Runtime as pallet_deos_actors::Config>::MaxExecutionDelayBlocks::get();
      let start = 1u32.checked_add(delay).unwrap();
      let contract: pallet_deos_actors::ActorContractOf<Runtime> = ActorContract {
        trigger: Trigger::AddressEvent {
          source_filter: SourceFilter::Whitelist(sources.clone().try_into().unwrap()),
          asset_filter: AssetFilter::Whitelist(assets.try_into().unwrap()),
        },
        cooldown_blocks: delay,
        window: Some(ScheduleWindow {
          start,
          end: start
            .checked_add(<Runtime as pallet_deos_actors::Config>::MinWindowLength::get())
            .unwrap(),
        }),
        steps: vec![
          step;
          <Runtime as pallet_deos_actors::Config>::MaxContractSteps::get() as usize
        ]
        .try_into()
        .unwrap(),
        funding: FundingSourcePolicy::SignedAllowlist(
          sources
            .into_iter()
            .collect::<BTreeSet<_>>()
            .try_into()
            .unwrap(),
        ),
        on_unfunded: pallet_deos_actors::OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        auto_close_at_cycle_nonce: Some(
          <Runtime as pallet_deos_actors::Config>::MaxAutoCloseNonceHorizon::get(),
        ),
      };
      assert_eq!(
        contract.trigger.encoded_size(),
        pallet_deos_actors::TriggerOf::<Runtime>::max_encoded_len()
      );
      assert_eq!(
        contract.funding.encoded_size(),
        pallet_deos_actors::FundingSourcePolicyOf::<Runtime>::max_encoded_len()
      );
      let mut expected = contract.clone();
      for step in &mut expected.steps {
        step
          .precondition
          .as_mut()
          .unwrap()
          .clauses
          .sort_by_key(Encode::encode);
      }
      let custody = Actors::user_pipeline_machine_capacity_requirement(&expected.steps).unwrap();
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(sovereign.clone()),
        custody,
      ));
      let actor_id = Actors::next_actor_id();
      // Target and custody are complete before source installation; block 1 ages source block 0.
      if kind != FullPayloadSignedCall::Create {
        let source = (kind == FullPayloadSignedCall::Replace).then(|| ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: 0,
          window: None,
          steps: ContractSteps::<Runtime>::default(),
          funding: FundingSourcePolicy::OwnerOnly,
          on_unfunded: pallet_deos_actors::OnUnfunded::Close,
          close_when: None,
          parked_balance_activation: None,
          auto_close_at_cycle_nonce: None,
        });
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(owner.clone()),
          Mutability::Mutable,
          source.clone(),
        ));
        let identity = Actors::actor_identity(actor_id).unwrap();
        assert_eq!(identity.last_control_mutation_block, 0);
        assert_eq!(identity.actor_class.owner_slot(), Some(0));
        assert_eq!(Actors::actor_contract(actor_id), source);
        assert!(Actors::actor_run_state(actor_id).is_none());
        if kind == FullPayloadSignedCall::Replace {
          assert_eq!(
            Actors::actor_hot(actor_id).unwrap().cycle_state,
            CycleState::Idle
          );
          assert!(!Actors::actor_hot(actor_id).unwrap().pending_signal);
        }
      }
      assert!(Actors::block_resource_state().is_none());
      assert!(
        polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::get().is_none()
      );
      assert_eq!(crate::Timestamp::get(), 0);
      assert_eq!(System::account_nonce(&owner), 0);
      (
        contract,
        expected,
        actor_id,
        Balances::free_balance(&owner),
        Balances::free_balance(&sink),
        custody,
        Balances::balance_on_hold(&reason, &owner),
      )
    });
  ext.commit_all().unwrap();
  let storage = ext.execute_with(current_top_storage);
  let parent = parent_header_for(storage.clone(), &[], 0);
  let call = RuntimeCall::Actors(match kind {
    FullPayloadSignedCall::Create => pallet_deos_actors::Call::create_user_actor {
      mutability: Mutability::Mutable,
      contract: Some(contract),
    },
    FullPayloadSignedCall::Activate => {
      pallet_deos_actors::Call::activate_actor { actor_id, contract }
    }
    FullPayloadSignedCall::Replace => {
      pallet_deos_actors::Call::update_contract { actor_id, contract }
    }
  });
  let authored = author_complete_block_after_with_calls(
    storage,
    &[],
    &parent,
    1,
    UserDemand::ActorOnly,
    &signer,
    0,
    &BTreeMap::new(),
    &[call],
  );
  assert_eq!(authored.metrics.user_calls, 1);
  assert_complete_block_replays_natively(&authored, &[]);
  TestExternalities::new_with_code_and_state(
    &[],
    authored.post_state,
    crate::VERSION.state_version(),
  )
  .execute_with(|| {
    assert_eq!(System::account_nonce(&owner), 1);
    let state = Actors::active_actor_state(actor_id).unwrap();
    assert_eq!(state.contract, expected);
    assert_eq!(state.identity.owner, owner);
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    assert_eq!(state.identity.actor_class.owner_slot(), Some(0));
    assert_eq!(state.identity.mutability, Mutability::Mutable);
    assert_eq!(state.identity.cycle_nonce, 0);
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(!state.hot.pending_signal);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(Balances::free_balance(&sovereign), custody_before);
    for id in 1..=<Runtime as pallet_deos_actors::Config>::MaxWhitelistSize::get() {
      assert_eq!(Assets::balance(id, &sovereign), 0);
    }
    let pallet_deos_actors::ActorSemanticState::Active(record) =
      Actors::actor_semantic_states(actor_id).unwrap()
    else {
      panic!("active semantic authority");
    };
    assert_eq!(
      record.generation,
      if kind == FullPayloadSignedCall::Replace {
        2
      } else {
        1
      }
    );
    let certificate = record.admission;
    assert!(certificate.has_valid_identity());
    let authority =
      <Runtime as pallet_deos_actors::Config>::AdmissionCertificateAuthority::current().unwrap();
    assert_eq!(
      (
        certificate.runtime_actor_semantics_version,
        certificate.production_weight_identity,
        certificate.body_geometry_version,
        certificate.configured_bounds_commitment,
        certificate.maximum_lifecycle_weight
      ),
      (
        authority.runtime_actor_semantics_version,
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight
      )
    );
    assert_eq!(
      certificate.semantic_contract_id,
      expected.semantic_contract_id()
    );
    let head = pallet_deos_actors::ActorContractHeads::<Runtime>::get(actor_id).unwrap();
    assert_eq!(
      head.header.admission_identity,
      certificate.admission_identity
    );
    assert_eq!(head.header.body_commitment, certificate.body_commitment);
    for (_, chunk) in pallet_deos_actors::ActorContractTailChunks::<Runtime>::iter_prefix(actor_id)
    {
      assert!(chunk.authority.matches(
        &actor_id,
        &certificate.semantic_contract_id,
        &certificate.body_commitment,
        &certificate.admission_identity
      ));
    }
    let pipeline = Actors::actor_cost_quote(actor_id)
      .unwrap()
      .prospective_pipeline_fee
      .unwrap();
    assert_eq!(pipeline.admission_identity, certificate.admission_identity);
    assert_eq!(
      pipeline.pipeline_machine_fee,
      head
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper
    );
    let hold = Actors::actor_state_hold(actor_id).unwrap();
    assert_eq!(hold.owner, owner);
    let b = hold.breakdown;
    let held = b.identity + b.contract_head + b.contract_body + b.detector + b.run;
    let reason = crate::RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState);
    assert_eq!(Balances::balance_on_hold(&reason, &owner), held);
    let mut transaction_fees = Vec::new();
    let mut creations = 0;
    for event in System::events() {
      match event.event {
        RuntimeEvent::TransactionPayment(
          polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
            who,
            actual_fee,
            tip,
          },
        ) => {
          assert_eq!(who, owner);
          assert_eq!(tip, 0);
          transaction_fees.push(actual_fee);
        }
        RuntimeEvent::Actors(Event::ActorCreated { actor_id: id, .. }) if id == actor_id => {
          creations += 1
        }
        RuntimeEvent::Actors(
          Event::PipelineFeeCharged { actor_id: id, .. }
          | Event::ActionFeeCharged { actor_id: id, .. }
          | Event::TriggerOccurrenceProcessed { actor_id: id, .. },
        ) if id == actor_id => panic!("installation does not open a Cycle or charge readiness"),
        _ => {}
      }
    }
    let is_creation = kind == FullPayloadSignedCall::Create;
    assert_eq!(creations, u32::from(is_creation));
    assert_eq!(transaction_fees.len(), 1);
    assert!(transaction_fees[0] > 0);
    let creation = if is_creation {
      <Runtime as pallet_deos_actors::Config>::ActorCreationFee::get()
    } else {
      0
    };
    let hold_growth = held.checked_sub(held_before).unwrap();
    assert_eq!(
      owner_before - Balances::free_balance(&owner),
      hold_growth + creation + transaction_fees[0]
    );
    assert_eq!(
      Balances::free_balance(&sink) - sink_before,
      creation + transaction_fees[0]
    );
    assert!(Actors::block_resource_state().is_none());
    assert!(
      !Actors::finalized_block_resource_telemetry()
        .unwrap()
        .optional_actor_work_halted()
    );
    #[cfg(feature = "try-runtime")]
    <Actors as polkadot_sdk::frame_support::traits::Hooks<crate::BlockNumber>>::try_state(
      System::block_number(),
    )
    .unwrap();
  });
}

#[test]
fn full_executive_user_completion_header_domain_witness() {
  use pallet_deos_actors::{
    ActorContract, FundingSourcePolicy, Mutability, StepControlWeightContext,
    StepControlWeightProvider,
  };
  let signer = sr25519::Pair::from_seed(&[66u8; 32]);
  let mut rows = Vec::new();
  let mut geometry = Vec::new();
  for wide in [false, true] {
    let mut ext = TestExternalities::new_with_code_and_state(
      &[],
      reference_genesis_storage(&[]),
      crate::VERSION.state_version(),
    );
    let (actor_id, head_bytes, funding_bytes, context, resources) = ext.execute_with(|| {
      System::set_block_number(0);
      assert!(!Actors::user_creation_enabled());
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      assert!(Actors::user_creation_enabled());
      assert!(
        pallet_deos_actors::OwnerSlotBitmaps::<Runtime>::get(ALICE)
          .iter()
          .all(|byte| *byte == 0)
      );
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(ALICE),
        u128::MAX / 4
      ));
      let sovereign = Actors::sovereign_account_id(&ALICE, 0);
      assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        MultiAddress::Id(sovereign),
        1_000 * crate::EXISTENTIAL_DEPOSIT
      ));
      let funding = if wide {
        let maximum = <Runtime as pallet_deos_actors::Config>::MaxWhitelistSize::get() as usize;
        let mut sources = BTreeSet::from([ALICE]);
        for byte in 0..=u8::MAX {
          if sources.len() == maximum {
            break;
          }
          sources.insert(crate::AccountId::new([byte; 32]));
        }
        assert_eq!(sources.len(), maximum);
        FundingSourcePolicy::SignedAllowlist(sources.try_into().unwrap())
      } else {
        FundingSourcePolicy::OwnerOnly
      };
      let step = StepOf::<Runtime> {
        precondition: None,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      };
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: 0,
          window: None,
          steps: vec![step.clone()].try_into().unwrap(),
          funding,
          on_unfunded: pallet_deos_actors::OnUnfunded::Close,
          close_when: None,
          parked_balance_activation: None,
          auto_close_at_cycle_nonce: None,
        }),
      ));
      let head = pallet_deos_actors::ActorContractHeads::<Runtime>::get(actor_id).unwrap();
      assert_eq!(head.header.step_count, 1);
      assert_eq!(head.first_step, Some(step.clone()));
      // Source-derived context: the production constructor uses Contract and current-Step geometry, not funding policy.
      let context = StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
      };
      let resources = head.first_step_resources.unwrap();
      assert_eq!(
        resources.control,
        <Runtime as pallet_deos_actors::Config>::StepControlWeight::maximum_control_weight(
          context, &step
        )
        .unwrap()
      );
      System::set_block_number(1);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      (
        actor_id,
        head.encoded_size(),
        head.header.funding.encoded_size(),
        context,
        resources,
      )
    });
    ext.commit_all().unwrap();
    let storage = ext.execute_with(current_top_storage);
    let profiles = BTreeMap::from([(
      actor_id,
      WorkloadActorProfile {
        step_count: 1,
        opening_predicates_per_step: 0,
        task: Some(WorkloadTask::StopCycle),
      },
    )]);
    let authored =
      author_complete_block(storage, &[], 2, UserDemand::ActorOnly, &signer, &profiles);
    assert_eq!(authored.metrics.stop_cycle_steps, 1);
    assert_eq!(authored.metrics.completed_cycles, 1);
    assert_eq!(authored.metrics.actor_effect, Weight::zero());
    assert_complete_block_replays_natively(&authored, &[]);
    geometry.push((head_bytes, funding_bytes, context, resources));
    rows.push(serde_json::json!({
      "funding": if wide { "signed-allowlist-max" } else { "owner-only" },
      "headBytes": head_bytes, "fundingBytes": funding_bytes,
      "context": [context.cursor, context.steps_in_fragment, context.opening_tail_chunks,
        context.predicate_evaluation_units],
      "storedControl": [resources.control.ref_time(), resources.control.proof_size()],
      "blockControl": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
    }));
  }
  assert_eq!(geometry[0].2, geometry[1].2);
  assert_eq!(geometry[0].3, geometry[1].3);
  assert!(geometry[1].0 > geometry[0].0);
  assert_eq!(geometry[1].0 - geometry[0].0, geometry[1].1 - geometry[0].1);
  println!(
    "ACTOR_USER_COMPLETION_CONTEXT_V1 {}",
    serde_json::json!({
      "mode": "native-full-executive-source-derived-context", "rows": rows,
    })
  );
}

fn run_w5_lifecycle_retry_cleanup_campaign(wasm: &[u8], replay_wasm: bool) {
  let fixture = prepare_w5_fixture(wasm);
  let setup = actor_lifecycle_observation(wasm, &fixture.actors.storage, &fixture.actors.actor_ids);
  let mut timeline = Vec::new();
  let mut pre_state = fixture.actors.storage.clone();
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut transfer_steps = 0u32;
  let mut non_successful_steps = 0u32;
  let mut opening_steps = 0u32;
  let mut middle_steps = 0u32;
  let mut final_steps = 0u32;
  let mut completed_cycle_actors = BTreeSet::new();
  let mut failed_cycle_actors = BTreeSet::new();
  let mut suspended_steps = Vec::new();
  let mut continued_steps = Vec::new();
  let mut closed_actors = Vec::new();
  let mut steps_per_block = Vec::new();
  let mut control_ref_time = Vec::new();
  let mut control_proof_size = Vec::new();
  let mut effect_ref_time = Vec::new();
  let mut effect_proof_size = Vec::new();
  let mut execution_storage_proof = Vec::new();
  let mut execution_compact_proof = Vec::new();
  let mut verification_storage_proof = Vec::new();
  let mut verification_compact_proof = Vec::new();

  for block_number in 2..W5_BLOCK_LIMIT.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      UserDemand::ActorOnly,
      &fixture.actors.signer,
      0,
      &fixture.actors.actor_profiles,
    );
    assert_eq!(authored.metrics.user_calls, 0);
    assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W5 preserves Q1 in every full block"
    );
    transfer_steps = transfer_steps.saturating_add(authored.metrics.transfer_steps);
    non_successful_steps =
      non_successful_steps.saturating_add(authored.metrics.non_successful_steps);
    opening_steps = opening_steps.saturating_add(authored.metrics.opening_steps);
    middle_steps = middle_steps.saturating_add(authored.metrics.middle_steps);
    final_steps = final_steps.saturating_add(authored.metrics.final_steps);
    completed_cycle_actors.extend(authored.metrics.completed_cycle_actors.iter().copied());
    failed_cycle_actors.extend(authored.metrics.failed_cycle_actors.iter().copied());
    suspended_steps.extend(authored.metrics.suspended_steps.iter().copied());
    continued_steps.extend(authored.metrics.continued_steps.iter().copied());
    closed_actors.extend(authored.metrics.closed_actors.iter().copied());
    steps_per_block.push(u64::from(authored.metrics.actor_steps));
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    if replay_wasm {
      let proof = replay_complete_block_in_wasm(
        &authored,
        wasm,
        &format!("W5-lifecycle-retry-cleanup-{block_number}"),
      );
      execution_storage_proof.push(proof.execution_storage_proof_bytes);
      execution_compact_proof.push(proof.execution_compact_proof_bytes);
      verification_storage_proof.push(proof.verification_storage_proof_bytes);
      verification_compact_proof.push(proof.verification_compact_proof_bytes);
    }
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
    timeline.push(actor_lifecycle_observation(
      wasm,
      &pre_state,
      &fixture.actors.actor_ids,
    ));
    let terminal_closures = [
      (
        fixture.productive_cleanup,
        CloseReason::BehaviorPolicySatisfied,
      ),
      (fixture.apoptosis, CloseReason::CycleAdmissionInsufficient),
      (fixture.retry_prefix, CloseReason::RetryAttemptsExhausted),
    ];
    if terminal_closures
      .iter()
      .all(|expected| closed_actors.contains(expected))
      && completed_cycle_actors.contains(&fixture.zero_step)
      && completed_cycle_actors.contains(&fixture.running)
    {
      break;
    }
  }

  assert_eq!(
    steps_per_block.len(),
    3,
    "W5 converges in three linked blocks"
  );
  assert_eq!(
    transfer_steps, 5,
    "W5 commits each authored prefix exactly once"
  );
  assert_eq!(
    non_successful_steps, 2,
    "W5 attempts the retrying middle Step twice"
  );
  assert_eq!((opening_steps, middle_steps, final_steps), (3, 3, 1));
  assert!(completed_cycle_actors.contains(&fixture.zero_step));
  assert!(completed_cycle_actors.contains(&fixture.running));
  assert!(completed_cycle_actors.contains(&fixture.productive_cleanup));
  assert_eq!(
    suspended_steps,
    vec![(fixture.retry_prefix, 1)],
    "the first temporary failure creates one Actor-owned suspension"
  );
  assert_eq!(
    continued_steps,
    vec![(fixture.retry_prefix, 1)],
    "the retry resumes the same cursor exactly once"
  );
  for expected in [
    (
      fixture.productive_cleanup,
      CloseReason::BehaviorPolicySatisfied,
    ),
    (fixture.apoptosis, CloseReason::CycleAdmissionInsufficient),
    (fixture.retry_prefix, CloseReason::RetryAttemptsExhausted),
  ] {
    assert!(
      closed_actors.contains(&expected),
      "W5 observes closure {expected:?}"
    );
  }
  assert!(
    !closed_actors
      .iter()
      .any(|(actor_id, _)| *actor_id == fixture.zero_step)
  );
  assert!(
    !closed_actors
      .iter()
      .any(|(actor_id, _)| *actor_id == fixture.running)
  );

  let mut final_ext =
    TestExternalities::new_with_code_and_state(wasm, pre_state, crate::VERSION.state_version());
  final_ext.execute_with(|| {
    assert!(Actors::active_actor_state(fixture.zero_step).is_some());
    assert!(Actors::active_actor_state(fixture.running).is_some());
    assert!(Actors::active_actor_state(fixture.productive_cleanup).is_none());
    assert!(Actors::active_actor_state(fixture.apoptosis).is_none());
    assert!(Actors::active_actor_state(fixture.retry_prefix).is_none());
    assert_eq!(
      Balances::free_balance(&fixture.retry_sovereign),
      fixture
        .retry_balance_before
        .saturating_sub(crate::EXISTENTIAL_DEPOSIT),
      "retry exhaustion preserves the committed prefix and performs no compensation"
    );
  });

  println!(
    "EXP_0066_W5_TIMELINE_V1 {}",
    serde_json::json!({
      "productionWasmReplayed": replay_wasm,
      "roles": {"zeroStep": fixture.zero_step, "running": fixture.running,
        "retryPrefix": fixture.retry_prefix, "productiveCleanup": fixture.productive_cleanup,
        "admissionApoptosis": fixture.apoptosis},
      "fixtureSetup": setup, "finalizedBlocks": timeline,
    })
  );
  println!(
    "EXP_0066_W5_V1 {{\"schedule\":\"manual-only\",\"workloadActors\":{},\"measuredBlocks\":{},\"stepsPerBlock\":{:?},\"transferSteps\":{transfer_steps},\"nonSuccessfulSteps\":{non_successful_steps},\"openingAttempts\":{opening_steps},\"middleAttempts\":{middle_steps},\"finalAttempts\":{final_steps},\"completedCycles\":{},\"failedCycleSummaries\":{},\"suspensions\":{},\"continuations\":{},\"productiveCleanupClosures\":1,\"admissionApoptosisClosures\":1,\"retryExhaustionClosures\":1,\"committedPrefixAmount\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofMax\":{}}}",
    fixture.actors.actor_ids.len(),
    steps_per_block.len(),
    steps_per_block,
    completed_cycle_actors.len(),
    failed_cycle_actors.len(),
    suspended_steps.len(),
    continued_steps.len(),
    crate::EXISTENTIAL_DEPOSIT,
    control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_ref_time, 50),
    control_ref_time.iter().copied().max().unwrap_or(0),
    control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_proof_size, 50),
    control_proof_size.iter().copied().max().unwrap_or(0),
    effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_ref_time, 50),
    effect_ref_time.iter().copied().max().unwrap_or(0),
    effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_proof_size, 50),
    effect_proof_size.iter().copied().max().unwrap_or(0),
    execution_storage_proof.iter().copied().min().unwrap_or(0),
    execution_storage_proof.iter().copied().max().unwrap_or(0),
    execution_compact_proof.iter().copied().min().unwrap_or(0),
    execution_compact_proof.iter().copied().max().unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
}

fn run_w6_mixed_arrival_lifecycle_campaign(wasm: &[u8], replay_wasm: bool) {
  let fixture = prepare_w6_fixture(wasm);
  let setup = actor_lifecycle_observation(wasm, &fixture.actors.storage, &fixture.actors.actor_ids);
  let mut timeline = Vec::new();
  let mut pre_state = fixture.actors.storage.clone();
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut signer_nonce = 0;
  let mut first_progress = BTreeMap::<ActorId, u32>::new();
  let mut progress_counts = BTreeMap::<ActorId, u32>::new();
  let mut paused_events = Vec::new();
  let mut resumed_events = Vec::new();
  let mut closed_events = Vec::new();
  let mut trigger_counts = [0u32; 5];
  let mut steps_per_block = Vec::new();
  let mut control_ref_time = Vec::new();
  let mut control_proof_size = Vec::new();
  let mut effect_ref_time = Vec::new();
  let mut effect_proof_size = Vec::new();
  let mut execution_storage_proof = Vec::new();
  let mut execution_compact_proof = Vec::new();
  let mut verification_storage_proof = Vec::new();
  let mut verification_compact_proof = Vec::new();

  for block_number in 2..W6_BLOCK_LIMIT.saturating_add(2) {
    let calls = match block_number {
      2 => vec![RuntimeCall::Actors(pallet_deos_actors::Call::pause_actor {
        actor_id: fixture.paused_actor,
      })],
      5 => vec![RuntimeCall::Actors(pallet_deos_actors::Call::close_actor {
        actor_id: fixture.closed_actor,
      })],
      6 => vec![RuntimeCall::Actors(
        pallet_deos_actors::Call::resume_actor {
          actor_id: fixture.paused_actor,
        },
      )],
      _ => Vec::new(),
    };
    let authored = author_complete_block_after_with_calls(
      pre_state,
      wasm,
      &parent,
      block_number,
      UserDemand::ActorOnly,
      &fixture.actors.signer,
      signer_nonce,
      &fixture.actors.actor_profiles,
      &calls,
    );
    signer_nonce = signer_nonce.saturating_add(authored.metrics.user_calls);
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W6 preserves Q1 in every full block"
    );
    assert_eq!(
      authored.metrics.non_successful_steps, 0,
      "W6 retained arrivals complete without failed Steps"
    );
    for (actor_id, _) in &authored.metrics.progressed_steps {
      first_progress.entry(*actor_id).or_insert(block_number);
      progress_counts
        .entry(*actor_id)
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
    }
    if block_number == 3 {
      let actor_id = fixture.sparse_block[0];
      assert!(
        !authored
          .metrics
          .progressed_steps
          .iter()
          .any(|(id, _)| *id == actor_id)
      );
      TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        assert!(Actors::deadline_handles(actor_id).is_none());
        assert_eq!(Actors::service_nodes(actor_id).unwrap().eligible_from, 4);
        assert!(Actors::actor_hot(actor_id).unwrap().pending_signal);
        assert!(Actors::actor_run_state(actor_id).is_none());
      });
    }
    paused_events.extend(authored.metrics.paused_actors.iter().copied());
    resumed_events.extend(authored.metrics.resumed_actors.iter().copied());
    closed_events.extend(authored.metrics.closed_actors.iter().copied());
    for (_actor_id, family) in &authored.metrics.trigger_occurrences {
      let index = match family {
        TriggerFamily::Manual => 0,
        TriggerFamily::AddressEvent => 1,
        TriggerFamily::AtTime => 2,
        TriggerFamily::Cadenced => 3,
        TriggerFamily::ParkedBalance => 4,
      };
      trigger_counts[index] = trigger_counts[index].saturating_add(1);
    }
    steps_per_block.push(u64::from(authored.metrics.actor_steps));
    control_ref_time.push(authored.metrics.actor_control.ref_time());
    control_proof_size.push(authored.metrics.actor_control.proof_size());
    effect_ref_time.push(authored.metrics.actor_effect.ref_time());
    effect_proof_size.push(authored.metrics.actor_effect.proof_size());
    if replay_wasm {
      let proof = replay_complete_block_in_wasm(
        &authored,
        wasm,
        &format!("W6-mixed-arrival-lifecycle-{block_number}"),
      );
      execution_storage_proof.push(proof.execution_storage_proof_bytes);
      execution_compact_proof.push(proof.execution_compact_proof_bytes);
      verification_storage_proof.push(proof.verification_storage_proof_bytes);
      verification_compact_proof.push(proof.verification_compact_proof_bytes);
    }
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
    timeline.push(actor_lifecycle_observation(
      wasm,
      &pre_state,
      &fixture.actors.actor_ids,
    ));
  }

  assert_eq!(paused_events, vec![fixture.paused_actor]);
  assert_eq!(resumed_events, vec![fixture.paused_actor]);
  assert!(closed_events.contains(&(fixture.closed_actor, CloseReason::OwnerInitiated)));
  assert!(
    !first_progress.contains_key(&fixture.closed_actor),
    "closed W6 Actor unexpectedly progressed: {:?}",
    first_progress.get(&fixture.closed_actor)
  );
  assert!(
    fixture
      .dense_manual
      .iter()
      .all(|actor_id| first_progress.contains_key(actor_id))
  );
  for (index, actor_id) in fixture.sparse_block.iter().enumerate() {
    if *actor_id == fixture.closed_actor {
      continue;
    }
    let expected_start = [3, 6, 9][index];
    assert!(
      first_progress
        .get(actor_id)
        .is_some_and(|block| *block > expected_start),
      "block-clock W6 Actor preserves next-round admission after its sparse window"
    );
  }
  assert!(
    fixture
      .sparse_tick
      .iter()
      .all(|actor_id| first_progress.contains_key(actor_id))
  );
  assert!(
    fixture
      .randomized_cadenced
      .iter()
      .all(|actor_id| first_progress.contains_key(actor_id))
  );
  assert!(
    first_progress
      .get(&fixture.paused_actor)
      .is_some_and(|block| *block >= 6),
    "paused Manual Actor resumes without losing its retained arrival"
  );
  assert!(trigger_counts[2] >= fixture.sparse_tick.len() as u32);
  assert!(trigger_counts[3] >= fixture.randomized_cadenced.len() as u32);

  let mut final_ext =
    TestExternalities::new_with_code_and_state(wasm, pre_state, crate::VERSION.state_version());
  final_ext.execute_with(|| {
    assert!(Actors::active_actor_state(fixture.closed_actor).is_none());
    assert!(Actors::trigger_deadline_handles(fixture.closed_actor).is_none());
    assert!(Actors::deadline_handles(fixture.closed_actor).is_none());
  });

  println!(
    "EXP_0066_W6_TIMELINE_V1 {}",
    serde_json::json!({
      "productionWasmReplayed": replay_wasm,
      "workloadClasses": {"User": 11, "System": 1},
      "pauseControl": {"actorType": "System", "mutability": "Mutable"},
      "roles": {"denseManual": fixture.dense_manual, "windowedManual": fixture.sparse_block,
        "atTime": fixture.sparse_tick, "cadenced": fixture.randomized_cadenced,
        "cadencedPeriodsTicks": fixture.randomized_periods,
        "paused": fixture.paused_actor, "closed": fixture.closed_actor},
      "fixtureSetup": setup, "finalizedBlocks": timeline,
    })
  );
  println!(
    "EXP_0066_W6_V2 {{\"seed\":{W6_RANDOM_SEED},\"measuredBlocks\":{},\"workloadActors\":{},\"workloadUserActors\":11,\"workloadSystemActors\":1,\"pausedActorType\":\"System\",\"denseManualActors\":{},\"sparseBlockClockActors\":{},\"atTimeDeadlineActors\":{},\"randomizedCadencedActors\":{},\"randomizedCadencePeriodsTicks\":{:?},\"distinctProgressedActors\":{},\"totalSteps\":{},\"pausedEvents\":{},\"resumedEvents\":{},\"ownerCloseEvents\":{},\"atTimeOccurrences\":{},\"cadencedOccurrences\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofMax\":{}}}",
    steps_per_block.len(),
    fixture.actors.actor_ids.len(),
    fixture.dense_manual.len(),
    fixture.sparse_block.len(),
    fixture.sparse_tick.len(),
    fixture.randomized_cadenced.len(),
    fixture.randomized_periods,
    first_progress.len(),
    steps_per_block.iter().sum::<u64>(),
    paused_events.len(),
    resumed_events.len(),
    closed_events
      .iter()
      .filter(|(_, reason)| *reason == CloseReason::OwnerInitiated)
      .count(),
    trigger_counts[2],
    trigger_counts[3],
    control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_ref_time, 50),
    control_ref_time.iter().copied().max().unwrap_or(0),
    control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&control_proof_size, 50),
    control_proof_size.iter().copied().max().unwrap_or(0),
    effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_ref_time, 50),
    effect_ref_time.iter().copied().max().unwrap_or(0),
    effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&effect_proof_size, 50),
    effect_proof_size.iter().copied().max().unwrap_or(0),
    execution_storage_proof.iter().copied().min().unwrap_or(0),
    execution_storage_proof.iter().copied().max().unwrap_or(0),
    execution_compact_proof.iter().copied().min().unwrap_or(0),
    execution_compact_proof.iter().copied().max().unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
}

fn run_w7_due_only_active_frontier_campaign(
  wasm: &[u8],
  maximum_population: bool,
  replay_wasm: bool,
) -> W7CampaignResult {
  let fixture = prepare_w7_fixture(wasm, maximum_population);
  let profile = if maximum_population {
    "maximum-population"
  } else {
    "due-frontier-control"
  };
  let mut pre_state = fixture.actors.storage.clone();
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut progressed_order = Vec::new();
  let mut progressed = BTreeSet::new();
  let mut pending_frontier_blocks = 0u32;
  let mut control_bound_blocks = 0u32;
  let actor_control_limit = BlockResourceBudgetValue::get().limits().actor_control();
  let mut control_remaining_ref_time = Vec::new();
  let mut control_remaining_proof_size = Vec::new();
  let mut result = W7CampaignResult {
    steps_per_block: Vec::new(),
    control_ref_time: Vec::new(),
    control_proof_size: Vec::new(),
    effect_ref_time: Vec::new(),
    effect_proof_size: Vec::new(),
    execution_storage_proof: Vec::new(),
    execution_compact_proof: Vec::new(),
    verification_storage_proof: Vec::new(),
    verification_compact_proof: Vec::new(),
  };

  for block_number in 2..W7_BLOCK_LIMIT.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      UserDemand::ActorOnly,
      &fixture.actors.signer,
      0,
      &fixture.actors.actor_profiles,
    );
    assert_eq!(authored.metrics.user_calls, 0);
    assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W7 preserves Q1 in every full block"
    );
    assert!(
      authored
        .metrics
        .progressed_steps
        .iter()
        .all(|(actor_id, _)| fixture.due.contains(actor_id)),
      "only the declared W7 due frontier may progress"
    );
    for (actor_id, step_index) in &authored.metrics.progressed_steps {
      assert_eq!(*step_index, 0);
      assert!(progressed.insert(*actor_id), "W7 due Actor progresses once");
      progressed_order.push(*actor_id);
    }
    let control_remaining = actor_control_limit
      .checked_sub(&authored.metrics.actor_control)
      .unwrap_or_else(Weight::zero);
    control_remaining_ref_time.push(control_remaining.ref_time());
    control_remaining_proof_size.push(control_remaining.proof_size());
    if authored.metrics.queue_head < authored.metrics.queue_tail {
      pending_frontier_blocks = pending_frontier_blocks.saturating_add(1);
      assert!(
        !fixture
          .actors
          .next_control_maximum
          .all_lte(control_remaining),
        "pending W7 due work stops only at the complete-attempt Control frontier"
      );
      control_bound_blocks = control_bound_blocks.saturating_add(1);
    }
    result
      .steps_per_block
      .push(u64::from(authored.metrics.actor_steps));
    result
      .control_ref_time
      .push(authored.metrics.actor_control.ref_time());
    result
      .control_proof_size
      .push(authored.metrics.actor_control.proof_size());
    result
      .effect_ref_time
      .push(authored.metrics.actor_effect.ref_time());
    result
      .effect_proof_size
      .push(authored.metrics.actor_effect.proof_size());
    if replay_wasm {
      let proof = replay_complete_block_in_wasm(
        &authored,
        wasm,
        &format!("W7-due-only-active-frontier-{profile}-{block_number}"),
      );
      result
        .execution_storage_proof
        .push(proof.execution_storage_proof_bytes);
      result
        .execution_compact_proof
        .push(proof.execution_compact_proof_bytes);
      result
        .verification_storage_proof
        .push(proof.verification_storage_proof_bytes);
      result
        .verification_compact_proof
        .push(proof.verification_compact_proof_bytes);
    }
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
    if progressed.len() == fixture.due.len() {
      assert_eq!(authored.metrics.queue_head, authored.metrics.queue_tail);
      break;
    }
  }

  assert_eq!(progressed.len(), fixture.due.len());
  assert_eq!(progressed_order, fixture.due, "W7 due service remains FIFO");
  assert_eq!(
    result.steps_per_block.iter().sum::<u64>(),
    u64::from(W7_DUE_ACTORS)
  );
  assert_eq!(control_bound_blocks, pending_frontier_blocks);
  let mut final_ext =
    TestExternalities::new_with_code_and_state(wasm, pre_state, crate::VERSION.state_version());
  final_ext.execute_with(|| {
    assert!(fixture.future.iter().all(|actor_id| {
      Actors::active_actor_state(*actor_id).is_some_and(|state| state.identity.cycle_nonce == 0)
    }));
    assert!(fixture.unsignaled.iter().all(|actor_id| {
      Actors::active_actor_state(*actor_id).is_some_and(|state| state.identity.cycle_nonce == 0)
    }));
    assert!(fixture.unsignaled.iter().all(|actor_id| {
      Actors::service_nodes(*actor_id).is_none()
        && Actors::active_actor_state(*actor_id).is_some_and(|state| !state.hot.pending_signal)
    }));
    for actor_id in &fixture.future {
      assert_paid_window_deadline(*actor_id, W7_FUTURE_BLOCK);
    }
    assert!(fixture.future.iter().all(|actor_id| {
      Actors::active_actor_state(*actor_id)
        .is_some_and(|state| state.identity.cycle_nonce == 0 && state.hot.pending_signal)
    }));
  });

  let non_due_identities = fixture
    .reference_identities
    .saturating_add(fixture.future.len() as u32)
    .saturating_add(fixture.unsignaled.len() as u32);
  println!(
    "EXP_0066_W7_V1 {{\"profile\":\"{profile}\",\"actorIdentities\":{},\"referenceIdentities\":{},\"futureWorkloadActors\":{},\"unsignaledWorkloadActors\":{},\"nonDueIdentities\":{non_due_identities},\"dueActors\":{},\"measuredBlocks\":{},\"stepsPerBlock\":{:?},\"totalSteps\":{},\"distinctProgressedActors\":{},\"fifoOrderPreserved\":true,\"pendingFrontierBlocks\":{pending_frontier_blocks},\"controlBoundBlocks\":{control_bound_blocks},\"nextControlMaximumRefTime\":{},\"nextControlMaximumProofSize\":{},\"controlRemainingRefTimeMin\":{},\"controlRemainingRefTimeMax\":{},\"controlRemainingProofSizeMin\":{},\"controlRemainingProofSizeMax\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofMax\":{}}}",
    fixture.reference_identities as usize + fixture.actors.actor_ids.len(),
    fixture.reference_identities,
    fixture.future.len(),
    fixture.unsignaled.len(),
    fixture.due.len(),
    result.steps_per_block.len(),
    result.steps_per_block,
    result.steps_per_block.iter().sum::<u64>(),
    progressed.len(),
    fixture.actors.next_control_maximum.ref_time(),
    fixture.actors.next_control_maximum.proof_size(),
    control_remaining_ref_time
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    control_remaining_ref_time
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    control_remaining_proof_size
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    control_remaining_proof_size
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    result.control_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&result.control_ref_time, 50),
    result.control_ref_time.iter().copied().max().unwrap_or(0),
    result.control_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&result.control_proof_size, 50),
    result.control_proof_size.iter().copied().max().unwrap_or(0),
    result.effect_ref_time.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&result.effect_ref_time, 50),
    result.effect_ref_time.iter().copied().max().unwrap_or(0),
    result.effect_proof_size.iter().copied().min().unwrap_or(0),
    nearest_rank_percentile(&result.effect_proof_size, 50),
    result.effect_proof_size.iter().copied().max().unwrap_or(0),
    result
      .execution_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    result
      .execution_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    result
      .execution_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    result
      .execution_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    result
      .verification_storage_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    result
      .verification_storage_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
    result
      .verification_compact_proof
      .iter()
      .copied()
      .min()
      .unwrap_or(0),
    result
      .verification_compact_proof
      .iter()
      .copied()
      .max()
      .unwrap_or(0),
  );
  result
}

fn assert_w7_due_only_active_frontier_campaign(wasm: &[u8], replay_wasm: bool) {
  let control = run_w7_due_only_active_frontier_campaign(wasm, false, replay_wasm);
  let maximum = run_w7_due_only_active_frontier_campaign(wasm, true, replay_wasm);
  assert_eq!(
    control.steps_per_block, maximum.steps_per_block,
    "9,900 non-due identities must not change due-frontier throughput"
  );
  let control_ref_time_delta_max = control
    .control_ref_time
    .iter()
    .zip(&maximum.control_ref_time)
    .map(|(control, maximum)| control.abs_diff(*maximum))
    .max()
    .unwrap_or(0);
  let control_proof_size_delta_max = control
    .control_proof_size
    .iter()
    .zip(&maximum.control_proof_size)
    .map(|(control, maximum)| control.abs_diff(*maximum))
    .max()
    .unwrap_or(0);
  println!(
    "EXP_0066_W7_COMPARISON_V1 {{\"stepsPerBlockEqual\":true,\"controlRefTimeDeltaMax\":{control_ref_time_delta_max},\"controlProofSizeDeltaMax\":{control_proof_size_delta_max}}}"
  );
}

fn assert_passive_transfer_prefix_and_retained_suffix(
  wasm: &[u8],
  authored: &AuthoredBlock,
  actor_ids: &[ActorId],
  progressed: &[ActorId],
) -> u32 {
  use pallet_deos_actors::{CycleState, ProcessStatus};
  assert_eq!(progressed, &actor_ids[..progressed.len()]);
  assert_successful_transfer_outcomes(&authored.metrics);
  let mut pre = TestExternalities::new_with_code_and_state(
    wasm,
    authored.pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let (before, recipient_before) = pre.execute_with(|| {
    let before = actor_ids
      .iter()
      .map(|id| {
        let state = Actors::active_actor_state(*id).unwrap();
        let balance = Balances::free_balance(&state.identity.sovereign_account);
        (
          state,
          Actors::actor_processes(*id).unwrap(),
          Actors::service_nodes(*id).unwrap(),
          pallet_deos_actors::ActorStateHolds::<Runtime>::get(id),
          balance,
        )
      })
      .collect::<Vec<_>>();
    (before, Balances::free_balance(super::common::BOB))
  });
  let mut post = TestExternalities::new_with_code_and_state(
    wasm,
    authored.post_state.clone(),
    crate::VERSION.state_version(),
  );
  post.execute_with(|| {
    assert_eq!(Balances::free_balance(super::common::BOB),
      recipient_before + crate::EXISTENTIAL_DEPOSIT * progressed.len() as u128);
    let service = Actors::service_header();
    assert_eq!(service.cursor.map(|actor| actor.actor_id), actor_ids.get(progressed.len()).copied());
    assert!(service.count as usize >= actor_ids.len() - progressed.len());
    for (index, id) in actor_ids.iter().enumerate() {
      let (original, original_process, original_node, hold, balance) = &before[index];
      assert_eq!(original.identity.actor_class.actor_type(), pallet_deos_actors::ActorType::System);
      assert_eq!(original.contract.steps.len(), 1);
      assert_eq!(original.contract.steps[0].task, Task::Transfer {
        to: super::common::BOB, asset: primitives::AssetKind::Native,
        amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
      });
      assert!(original.contract.steps[0].precondition.is_none());
      assert_eq!(original.contract.trigger, Trigger::manual());
      assert!(original.contract.window.is_none());
      assert!(original.contract.close_when.is_none());
      let current = Actors::active_actor_state(*id).expect("completion retains the workload identity");
      let process = Actors::actor_processes(*id).unwrap();
      assert_eq!(process.generation, original_process.generation);
      assert_eq!(pallet_deos_actors::ActorStateHolds::<Runtime>::get(id), *hold);
      if index < progressed.len() {
        let mut identity = original.identity.clone();
        identity.cycle_nonce = identity.cycle_nonce.checked_add(1).unwrap();
        assert_eq!(current.identity, identity);
        assert_eq!(current.contract, original.contract);
        assert_eq!(current.hot.cycle_state, CycleState::Idle);
        assert!(!current.hot.pending_signal);
        assert!(current.run_state.is_none());
        assert!(Actors::service_nodes(*id).is_none());
        assert!(Actors::deadline_handles(*id).is_none());
        assert_eq!(process.last_attempted, Some(*authored.block.header.number()));
        assert_eq!(process.residence, None);
        assert!(matches!(process.status,
          ProcessStatus::Disabled(disablement)
            if disablement.cause == pallet_deos_actors::ProcessDisableCause::Protocol
              && disablement.revival_authority == pallet_deos_actors::ProcessRevivalAuthority::Protocol
              && disablement.basis == pallet_deos_actors::SuspendedProcessBasis::Idle
        ));
        assert_eq!(Balances::free_balance(&current.identity.sovereign_account),
          balance.checked_sub(crate::EXISTENTIAL_DEPOSIT).unwrap());
      } else {
        assert_eq!(&current, original, "refused suffix retains full Actor semantics");
        assert_eq!(&process, original_process);
        assert_eq!(Balances::free_balance(&current.identity.sovereign_account), *balance);
        let node = Actors::service_nodes(*id).unwrap();
        assert_eq!((node.generation, node.kind, node.eligible_from, node.last_considered),
          (original_node.generation, original_node.kind, original_node.eligible_from, original_node.last_considered));
        if index > progressed.len() {
          assert_eq!(node.previous.actor_id, actor_ids[index - 1]);
        }
        if index + 1 < actor_ids.len() {
          assert_eq!(node.next.actor_id, actor_ids[index + 1]);
        }
      }
    }
    service.count
  })
}

fn run_w8_tombstone_prefix_chunk_pressure_campaign(wasm: &[u8], replay_wasm: bool) {
  let actor_control_limit = BlockResourceBudgetValue::get().limits().actor_control();
  for tombstone_prefix in W8_TOMBSTONE_PREFIXES {
    let fixture = prepare_w8_fixture(wasm, tombstone_prefix);
    let parent = parent_header_for(fixture.actors.storage.clone(), wasm, 1);
    let authored = author_complete_block_after(
      fixture.actors.storage,
      wasm,
      &parent,
      2,
      UserDemand::ActorOnly,
      &fixture.actors.signer,
      0,
      &fixture.actors.actor_profiles,
    );
    assert_eq!(authored.metrics.user_calls, 0);
    assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W8 preserves Q1 after every closed-prefix setup"
    );
    assert!(
      authored.metrics.actor_steps > 0,
      "every bounded W8 prefix must leave capacity for a live due Actor"
    );
    assert!(authored.metrics.actor_steps < W8_DUE_ACTORS);
    assert_eq!(authored.metrics.queue_head, authored.metrics.queue_tail);
    let progressed = authored
      .metrics
      .progressed_steps
      .iter()
      .map(|(actor_id, step_index)| {
        assert_eq!(*step_index, 0);
        *actor_id
      })
      .collect::<Vec<_>>();
    assert_eq!(
      progressed,
      fixture.due[..progressed.len()],
      "W8 service preserves the live ordered prefix after setup closes removed Service members"
    );
    assert_complete_block_replays_natively(&authored, wasm);
    let service_residents = assert_passive_transfer_prefix_and_retained_suffix(
      wasm,
      &authored,
      &fixture.due,
      &progressed,
    );
    assert_eq!(
      service_residents as usize,
      fixture.due.len() - progressed.len()
    );
    let control_remaining = actor_control_limit
      .checked_sub(&authored.metrics.actor_control)
      .unwrap_or_else(Weight::zero);
    assert!(
      !fixture
        .actors
        .next_control_maximum
        .all_lte(control_remaining),
      "pending W8 live work stops only at the complete-attempt Control frontier"
    );

    let proof = replay_wasm.then(|| {
      replay_complete_block_in_wasm(
        &authored,
        wasm,
        &format!("W8-closed-during-setup-{tombstone_prefix}"),
      )
    });
    let proof = proof.as_ref();
    println!(
      "EXP_0066_W8_V1 {{\"closedPrefix\":{tombstone_prefix},\"closurePhase\":\"unmeasured-setup\",\"measuredReadyTombstoneReclamation\":false,\"dueActors\":{},\"dueSteps\":{},\"distinctProgressedActors\":{},\"dueActorsRemaining\":{},\"serviceResidents\":{},\"fifoPrefixPreserved\":true,\"q1Preserved\":true,\"controlBound\":true,\"nextControlMaximumRefTime\":{},\"nextControlMaximumProofSize\":{},\"controlRemainingRefTime\":{},\"controlRemainingProofSize\":{},\"controlRefTime\":{},\"controlProofSize\":{},\"effectRefTime\":{},\"effectProofSize\":{},\"executionStorageProofBytes\":{},\"executionCompactProofBytes\":{},\"verificationStorageProofBytes\":{},\"verificationCompactProofBytes\":{}}}",
      fixture.due.len(),
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors,
      fixture
        .due
        .len()
        .saturating_sub(authored.metrics.actor_steps as usize),
      service_residents,
      fixture.actors.next_control_maximum.ref_time(),
      fixture.actors.next_control_maximum.proof_size(),
      control_remaining.ref_time(),
      control_remaining.proof_size(),
      authored.metrics.actor_control.ref_time(),
      authored.metrics.actor_control.proof_size(),
      authored.metrics.actor_effect.ref_time(),
      authored.metrics.actor_effect.proof_size(),
      proof.map_or(0, |metrics| metrics.execution_storage_proof_bytes),
      proof.map_or(0, |metrics| metrics.execution_compact_proof_bytes),
      proof.map_or(0, |metrics| metrics.verification_storage_proof_bytes),
      proof.map_or(0, |metrics| metrics.verification_compact_proof_bytes),
    );
  }
}

fn run_w9_resource_independence_campaign(wasm: &[u8], replay_wasm: bool) {
  let user_limit = BlockResourceBudgetValue::get().limits().user_base_turn();
  let mut results = Vec::new();
  for demand in [
    UserDemand::ActorOnly,
    UserDemand::ContinuousValid,
    UserDemand::RefTimeHeavy,
  ] {
    let fixture = prepare_w9_fixture(wasm);
    let parent = parent_header_for(fixture.storage.clone(), wasm, 1);
    let authored = author_complete_block_after(
      fixture.storage,
      wasm,
      &parent,
      2,
      demand,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(
      authored.metrics.actor_steps, authored.metrics.distinct_actors,
      "W9 preserves Q1 in every demand profile"
    );
    assert!(authored.metrics.actor_steps < W9_DUE_ACTORS);
    assert_eq!(authored.metrics.queue_head, authored.metrics.queue_tail);
    let progressed = authored
      .metrics
      .progressed_steps
      .iter()
      .map(|(actor_id, step_index)| {
        assert_eq!(*step_index, 0);
        *actor_id
      })
      .collect::<Vec<_>>();
    assert_eq!(
      progressed,
      fixture.actor_ids[..progressed.len()],
      "W9 demand profiles preserve the exact live FIFO prefix"
    );
    assert_complete_block_replays_natively(&authored, wasm);
    assert_passive_transfer_prefix_and_retained_suffix(
      wasm,
      &authored,
      &fixture.actor_ids,
      &progressed,
    );

    let (next_user, user_remaining, proof_bound, ref_time_bound) = match demand {
      UserDemand::ActorOnly => {
        assert_eq!(authored.metrics.user_calls, 0);
        assert_eq!(authored.metrics.user_dispatch, Weight::zero());
        assert!(authored.metrics.next_user_weight.is_none());
        (Weight::zero(), user_limit, false, false)
      }
      UserDemand::ContinuousValid | UserDemand::RefTimeHeavy => {
        assert!(authored.metrics.user_calls > 0);
        let next_user = authored
          .metrics
          .next_user_weight
          .expect("W9 valid demand records the first inadmissible call");
        let remaining = user_limit
          .checked_sub(&authored.metrics.user_dispatch)
          .unwrap_or_else(Weight::zero);
        assert!(!next_user.all_lte(remaining));
        let proof_bound = next_user.proof_size() > remaining.proof_size();
        let ref_time_bound = next_user.ref_time() > remaining.ref_time();
        assert_eq!(
          (proof_bound, ref_time_bound),
          (
            demand == UserDemand::ContinuousValid,
            demand == UserDemand::RefTimeHeavy
          ),
          "W9 remarks and Router swaps must isolate ProofSize and RefTime refusal respectively"
        );
        (next_user, remaining, proof_bound, ref_time_bound)
      }
      UserDemand::ContinuousShared => {
        unreachable!("the W9 campaign targets the User base-turn frontier")
      }
    };
    let proof = replay_wasm
      .then(|| replay_complete_block_in_wasm(&authored, wasm, &format!("W9-{}", demand.label())));
    let proof = proof.as_ref();
    println!(
      "EXP_0066_W9_V1 {{\"profile\":\"{}\",\"dueActors\":{},\"dueSteps\":{},\"distinctProgressedActors\":{},\"fifoPrefixPreserved\":true,\"q1Preserved\":true,\"userCalls\":{},\"userRefTime\":{},\"userProofSize\":{},\"userRemainingRefTime\":{},\"userRemainingProofSize\":{},\"nextUserRefTime\":{},\"nextUserProofSize\":{},\"userProofSizeBound\":{proof_bound},\"userRefTimeBound\":{ref_time_bound},\"controlRefTime\":{},\"controlProofSize\":{},\"effectRefTime\":{},\"effectProofSize\":{},\"executionStorageProofBytes\":{},\"executionCompactProofBytes\":{},\"verificationStorageProofBytes\":{},\"verificationCompactProofBytes\":{}}}",
      demand.label(),
      fixture.actor_ids.len(),
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors,
      authored.metrics.user_calls,
      authored.metrics.user_dispatch.ref_time(),
      authored.metrics.user_dispatch.proof_size(),
      user_remaining.ref_time(),
      user_remaining.proof_size(),
      next_user.ref_time(),
      next_user.proof_size(),
      authored.metrics.actor_control.ref_time(),
      authored.metrics.actor_control.proof_size(),
      authored.metrics.actor_effect.ref_time(),
      authored.metrics.actor_effect.proof_size(),
      proof.map_or(0, |metrics| metrics.execution_storage_proof_bytes),
      proof.map_or(0, |metrics| metrics.execution_compact_proof_bytes),
      proof.map_or(0, |metrics| metrics.verification_storage_proof_bytes),
      proof.map_or(0, |metrics| metrics.verification_compact_proof_bytes),
    );
    results.push((demand, progressed, authored.metrics));
  }

  let actor_only = &results[0];
  let proof_saturated = &results[1];
  let ref_time_heavy = &results[2];
  assert!(actor_only.1.starts_with(&proof_saturated.1));
  assert!(actor_only.1.starts_with(&ref_time_heavy.1));
  assert!(results.iter().all(|(_, progressed, metrics)| {
    !progressed.is_empty()
      && metrics.actor_control != Weight::zero()
      && metrics.actor_effect != Weight::zero()
  }));
  assert!(
    ref_time_heavy.2.user_dispatch.ref_time() > proof_saturated.2.user_dispatch.ref_time(),
    "W9 Router demand must be materially more RefTime-heavy than remarks"
  );
  println!(
    "EXP_0066_W9_COMPARISON_V1 {{\"actorPrefixesCanonical\":true,\"actorServiceSurvivesEveryUserProfile\":true,\"proofSaturatedUserRefTime\":{},\"refTimeHeavyUserRefTime\":{},\"proofSaturatedUserProofSize\":{},\"refTimeHeavyUserProofSize\":{},\"userProofSizeDelta\":{},\"remarksRefusalComponent\":\"ProofSize\",\"routerRefusalComponent\":\"RefTime\"}}",
    proof_saturated.2.user_dispatch.ref_time(),
    ref_time_heavy.2.user_dispatch.ref_time(),
    proof_saturated.2.user_dispatch.proof_size(),
    ref_time_heavy.2.user_dispatch.proof_size(),
    proof_saturated
      .2
      .user_dispatch
      .proof_size()
      .abs_diff(ref_time_heavy.2.user_dispatch.proof_size()),
  );
}

fn run_control_phase_attribution_campaign(
  wasm: &[u8],
  schedule: WorkloadSchedule,
  replay_wasm: bool,
) -> ControlAttributionResult {
  let fixture = prepare_actor_fixture(wasm, CONTROL_ATTRIBUTION_ACTORS, schedule);
  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), wasm, 1);
  let mut result = ControlAttributionResult {
    steps: Vec::new(),
    prepass_steps: Vec::new(),
    trigger_occurrences: Vec::new(),
    prepass_control: Vec::new(),
    final_control: Vec::new(),
    wasm_proofs: Vec::new(),
  };

  for block_number in 2..CONTROL_ATTRIBUTION_BLOCKS.saturating_add(2) {
    let authored = author_complete_block_after(
      pre_state,
      wasm,
      &parent,
      block_number,
      UserDemand::ActorOnly,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    let trigger_occurrences = authored.metrics.trigger_occurrences.len() as u32;
    let progressed_actors = authored
      .metrics
      .progressed_steps
      .iter()
      .map(|(actor_id, _)| *actor_id)
      .collect::<BTreeSet<_>>();
    assert!(
      authored
        .metrics
        .trigger_occurrences
        .iter()
        .all(|(actor_id, _)| !progressed_actors.contains(actor_id)),
      "B+1 Service eligibility defers every newly materialized Actor to a later block"
    );
    assert_eq!(
      authored.metrics.non_successful_steps, 0,
      "Control attribution requires successful production-valid Transfer demand"
    );
    assert!(
      authored.metrics.prepass_trigger_occurrences <= trigger_occurrences,
      "mandatory Prepass trigger occurrences remain a subset of finalized block occurrences"
    );
    assert!(authored.metrics.prepass_steps <= authored.metrics.actor_steps);
    assert!(
      authored
        .metrics
        .prepass_actor_control
        .all_lte(authored.metrics.actor_control)
    );
    assert!(
      authored
        .metrics
        .prepass_actor_effect
        .all_lte(authored.metrics.actor_effect)
    );
    if replay_wasm {
      result.wasm_proofs.push(replay_complete_block_in_wasm(
        &authored,
        wasm,
        &format!("control-attribution-{}-{block_number}", schedule.label()),
      ));
    }
    result.steps.push(authored.metrics.actor_steps);
    result.prepass_steps.push(authored.metrics.prepass_steps);
    result.trigger_occurrences.push(trigger_occurrences);
    result
      .prepass_control
      .push(authored.metrics.prepass_actor_control);
    result.final_control.push(authored.metrics.actor_control);
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }

  println!(
    "EXP_0066_CONTROL_ATTRIBUTION_V1 {{\"schedule\":\"{}\",\"actors\":{CONTROL_ATTRIBUTION_ACTORS},\"blocks\":{CONTROL_ATTRIBUTION_BLOCKS},\"steps\":{:?},\"prepassSteps\":{:?},\"triggerOccurrences\":{:?},\"prepassControlRefTime\":{:?},\"prepassControlProofSize\":{:?},\"finalControlRefTime\":{:?},\"finalControlProofSize\":{:?},\"executionRecordedKeys\":{:?},\"executionStorageProofBytes\":{:?},\"executionCompactProofBytes\":{:?}}}",
    schedule.label(),
    result.steps,
    result.prepass_steps,
    result.trigger_occurrences,
    result
      .prepass_control
      .iter()
      .map(Weight::ref_time)
      .collect::<Vec<_>>(),
    result
      .prepass_control
      .iter()
      .map(Weight::proof_size)
      .collect::<Vec<_>>(),
    result
      .final_control
      .iter()
      .map(Weight::ref_time)
      .collect::<Vec<_>>(),
    result
      .final_control
      .iter()
      .map(Weight::proof_size)
      .collect::<Vec<_>>(),
    result
      .wasm_proofs
      .iter()
      .map(|proof| proof.execution_recorded_keys.len())
      .collect::<Vec<_>>(),
    result
      .wasm_proofs
      .iter()
      .map(|proof| proof.execution_storage_proof_bytes)
      .collect::<Vec<_>>(),
    result
      .wasm_proofs
      .iter()
      .map(|proof| proof.execution_compact_proof_bytes)
      .collect::<Vec<_>>(),
  );
  result
}

fn assert_production_weight_component(
  actual: Weight,
  base_ref_time: u64,
  proof_size: u64,
  io: DatabaseIo,
) {
  let database = <Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get();
  let expected = Weight::from_parts(base_ref_time, proof_size)
    .saturating_add(database.reads(io.reads))
    .saturating_add(database.writes(io.writes));
  assert_eq!(actual, expected, "production Weight component I/O drifted");
}

#[test]
#[allow(deprecated)] // The SDK's actual bare-inherent pipeline still uses ValidateUnsigned.
fn full_executive_prepass_prefix_excludes_its_provisional_reservation() {
  use polkadot_sdk::sp_runtime::traits::{Dispatchable, TransactionExtension, ValidateUnsigned};
  for actor_count in [0, 1] {
    let fixture = prepare_actor_fixture(&[], actor_count, WorkloadSchedule::ManualOnly);
    let parent = parent_header_for(fixture.storage.clone(), &[], 1);
    let mut ext = TestExternalities::new(fixture.storage);
    ext.execute_with(|| {
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
      let extrinsics = inherent_data_for(&parent, 2).create_extrinsics();
      let mut witnessed = false;
      for extrinsic in extrinsics {
        if !matches!(
          extrinsic.function,
          RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
        ) {
          assert_ok!(Executive::apply_extrinsic(extrinsic));
          continue;
        }
        assert!(!witnessed, "one runtime-created Prepass");
        witnessed = true;
        let prefix = System::block_weight().total();
        let before_root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
        let info = extrinsic.get_dispatch_info();
        let encoded = extrinsic.encode();
        let len = encoded.len();
        let base = crate::configs::RuntimeBlockWeights::get()
          .get(info.class)
          .base_extrinsic;
        let overhead = base.saturating_add(Weight::from_parts(0, len as u64));
        assert_eq!(info.extension_weight, Weight::zero());
        assert_eq!(
          info.call_weight,
          BlockResourceBudgetValue::get().maximum_block()
        );
        let guaranteed = BlockResourceBudgetValue::get().limits();
        assert!(
          info.call_weight.all_gte(
            guaranteed
              .actor_control()
              .saturating_add(guaranteed.actor_base_turn()),
          )
        );
        assert!(Actors::block_resource_state().is_none());

        // A single backend transaction exposes the SDK dispatch boundary without adding a
        // storage-backed transaction-depth key to the root being compared with Executive.
        polkadot_sdk::sp_io::storage::start_transaction();
        System::note_extrinsic(encoded);
        assert_ok!(<Runtime as ValidateUnsigned>::pre_dispatch(
          &extrinsic.function
        ));
        assert_ok!(crate::TxExtension::bare_validate_and_prepare(
          &extrinsic.function,
          &info,
          len
        ));
        let prepared = System::block_weight().total();
        assert!(
          prepared.any_gt(crate::configs::RuntimeBlockWeights::get().max_block),
          "Mandatory provisional bookkeeping is not settled block consumption",
        );
        assert_eq!(
          prepared.checked_sub(&prefix),
          info.total_weight().checked_add(&overhead)
        );
        assert_eq!(
          prepared.checked_sub(&info.total_weight()),
          prefix.checked_add(&overhead),
          "normalization must preserve base and encoded-length charges"
        );
        let dispatched = extrinsic
          .function
          .clone()
          .dispatch(RuntimeOrigin::none())
          .expect("Prepass succeeds");
        let usage = Actors::block_resource_state()
          .expect("Prepass opens state")
          .usage();
        let actual = dispatched
          .actual_weight
          .expect("Prepass reports actual Weight");
        assert_eq!(
          actual,
          usage
            .actor_control_used()
            .saturating_add(usage.actor_effect_used())
        );
        assert!(actual.all_lte(info.call_weight));
        let mut post_info = dispatched;
        assert_ok!(crate::TxExtension::bare_post_dispatch(
          &info,
          &mut post_info,
          len,
          &Ok(())
        ));
        let settled = System::block_weight().total();
        assert_eq!(
          settled,
          prefix.saturating_add(overhead).saturating_add(actual)
        );
        // The wrapper and CheckWeight both reclaim; a further identical reclaim creates no credit.
        assert_ok!(System::reclaim_weight(&info, &post_info));
        assert_eq!(System::block_weight().total(), settled);
        System::note_applied_extrinsic(&Ok(dispatched), info);
        let expected_root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
        polkadot_sdk::sp_io::storage::rollback_transaction();
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          before_root
        );
        assert_ok!(Executive::apply_extrinsic(extrinsic));
        assert_eq!(System::block_weight().total(), settled);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          expected_root
        );
        let pair = |w: Weight| [w.ref_time(), w.proof_size()];
        println!(
          "PREPASS_PREFIX_BOUNDARY_V1 {}",
          serde_json::json!({
            "preparedActors": actor_count,
            "prefix": pair(prefix),
            "dispatchMaximum": pair(info.total_weight()),
            "overhead": pair(overhead),
            "atDispatch": pair(prepared),
            "actual": pair(actual),
            "afterSettlement": pair(settled),
          })
        );
      }
      assert!(witnessed);
      // These hooks follow Prepass. Keep their present no-work binding explicit rather than
      // treating every hook as settled merely because all on_initialize calls have returned.
      {
        use polkadot_sdk::frame_support::traits::{OnPoll, PostInherents, PostTransactions};
        let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
        let mut meter = polkadot_sdk::frame_support::weights::WeightMeter::with_limit(
          crate::configs::RuntimeBlockWeights::get().max_block,
        );
        <Runtime as polkadot_sdk::frame_system::Config>::PostInherents::post_inherents();
        <Runtime as polkadot_sdk::frame_system::Config>::PostTransactions::post_transactions();
        <crate::AllPalletsWithSystem as OnPoll<u32>>::on_poll(2, &mut meter);
        assert_eq!(meter.consumed(), Weight::zero());
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          root
        );
      }
      Executive::finalize_block();
      assert!(Actors::finalized_block_resource_telemetry().is_some());
    });
  }
}

#[test]
fn full_executive_empty_workload_control_baseline_has_explicit_owners() {
  type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  control_temporal_weight_io_ledger_matches_production_selectors();
  let components = [
    (
      "prepass-admission",
      W::scheduler_on_initialize_cutoff(),
      9_498_000,
      1_560,
      DatabaseIo::new(2, 2),
      1,
    ),
    (
      "dependency-scan-probe",
      W::dependency_scan_source_probe(),
      6_635_000,
      1_498,
      DatabaseIo::new(1, 0),
      1,
    ),
    (
      "idle-base",
      W::scheduler_on_idle_base(),
      19_137_000,
      1_560,
      DatabaseIo::new(7, 2),
      1,
    ),
    (
      "prepass-service-round-open",
      W::service_round_begin_populated(),
      9_429_000,
      1_511,
      DatabaseIo::new(1, 1),
      1,
    ),
    (
      "prepass-service-frontier-probe",
      W::service_round_probe_eligible(),
      16_203_000,
      3_550,
      DatabaseIo::new(3, 0),
      1,
    ),
    (
      "canonical-deadline-frontier",
      Weight::from_parts(272_301_000, 121),
      272_301_000,
      121,
      DatabaseIo::new(0, 0),
      1,
    ),
    (
      "current-binding-deadline-selector-residual",
      Weight::from_parts(78_290_000, 13_132)
        .saturating_add(<Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get().reads(12)),
      78_290_000,
      13_132,
      DatabaseIo::new(12, 0),
      1,
    ),
    (
      "finalize",
      W::block_resource_finalize(),
      8_171_000,
      1_560,
      DatabaseIo::new(1, 2),
      1,
    ),
  ];
  let mut fixed = Weight::zero();
  let mut fixed_io = DatabaseIo::new(0, 0);
  for (_, weight, base, proof, io, frequency) in &components {
    assert_production_weight_component(*weight, *base, *proof, *io);
    fixed = fixed.saturating_add(weight.saturating_mul(*frequency));
    fixed_io = fixed_io.saturating_add(io.saturating_mul(*frequency));
  }
  assert_eq!(fixed_io, DatabaseIo::new(27, 7));
  // The first block adds one complete Fee Sink cadence occurrence above the stable baseline.
  let first_block_extra = W::cadenced_trigger_occurrence();
  let first_extra_io = DatabaseIo::new(41, 18);
  let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
  let fee_sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let before = actor_lifecycle_observation(&[], &fixture.storage, &[fee_sink]);
  assert!(before["actors"][0]["temporalAnchorTick"].is_null());
  assert_eq!(before["actors"][0]["triggerWakeupTick"], 0);
  let first = author_complete_block(
    fixture.storage,
    &[],
    2,
    UserDemand::ActorOnly,
    &fixture.signer,
    &fixture.actor_profiles,
  );
  let first_snapshot = actor_lifecycle_observation(&[], &first.post_state, &[fee_sink]);
  assert_eq!(first_snapshot["actors"][0]["temporalAnchorTick"], 24);
  assert_eq!(
    first_snapshot["actors"][0]["triggerWakeupTick"],
    24 + primitives::ecosystem::params::FEE_SINK_CADENCE_TICKS
  );
  assert_eq!(
    first.metrics.actor_control,
    fixed.saturating_add(first_block_extra)
  );
  let second = author_complete_block_after(
    first.post_state,
    &[],
    &first.block.header,
    3,
    UserDemand::ActorOnly,
    &fixture.signer,
    0,
    &fixture.actor_profiles,
  );
  assert_eq!(second.metrics.actor_control, fixed);
  let fixed_envelope = crate::configs::FixedBlockWeight::get();
  let frame_maximum = crate::configs::RuntimeBlockWeights::get().max_block;
  let first_pre_idle_charge = frame_maximum
    .checked_sub(&first.metrics.pre_idle_remaining)
    .expect("pre-idle remainder uses the FRAME maximum, not the global economic ceiling");
  let second_pre_idle_charge = frame_maximum
    .checked_sub(&second.metrics.pre_idle_remaining)
    .expect("pre-idle remainder uses the FRAME maximum, not the global economic ceiling");
  assert!(first.metrics.pre_idle_remaining.all_gt(Weight::zero()));
  assert!(second.metrics.pre_idle_remaining.all_gt(Weight::zero()));
  println!(
    "ACTOR_BASELINE_IDLE_REMAINING_V1 {}",
    serde_json::json!({
      "fixedEnvelope": [fixed_envelope.ref_time(), fixed_envelope.proof_size()],
      "maximumBlock": [crate::MAXIMUM_BLOCK_WEIGHT.ref_time(), crate::MAXIMUM_BLOCK_WEIGHT.proof_size()],
      "frameMaximum": [frame_maximum.ref_time(), frame_maximum.proof_size()],
      "firstPreIdleCharge": [first_pre_idle_charge.ref_time(), first_pre_idle_charge.proof_size()],
      "secondPreIdleCharge": [second_pre_idle_charge.ref_time(), second_pre_idle_charge.proof_size()],
      "first": [first.metrics.pre_idle_remaining.ref_time(), first.metrics.pre_idle_remaining.proof_size()],
      "second": [second.metrics.pre_idle_remaining.ref_time(), second.metrics.pre_idle_remaining.proof_size()],
    })
  );
  let second_snapshot = actor_lifecycle_observation(&[], &second.post_state, &[fee_sink]);
  assert_eq!(first_snapshot["actors"], second_snapshot["actors"]);
  for snapshot in [&first_snapshot, &second_snapshot] {
    assert!(snapshot["events"].as_array().unwrap().is_empty());
    assert_eq!(snapshot["actors"][0]["pendingSignal"], false);
    assert!(snapshot["actors"][0]["queueTicket"].is_null());
    assert!(snapshot["actors"][0]["run"].is_null());
  }
  let idle = W::scheduler_on_idle_base()
    .saturating_add(W::block_resource_finalize())
    // Source-bound canonical empty-Service discovery delta under the accepted generated binding.
    .saturating_add(Weight::from_parts(225_632_000, 5_061));
  assert_eq!(
    first
      .metrics
      .actor_control
      .checked_sub(&first.metrics.prepass_actor_control),
    Some(idle),
    "genesis cadence initialization belongs to mandatory Prepass, not Drain"
  );
  assert_eq!(
    second
      .metrics
      .actor_control
      .checked_sub(&second.metrics.prepass_actor_control),
    Some(idle)
  );
  for metrics in [&first.metrics, &second.metrics] {
    assert_eq!(metrics.actor_steps, 0);
    assert_eq!(metrics.actor_effect, Weight::zero());
  }
  println!(
    "ACTOR_BASELINE_CONTROL_LEDGER_V1 {}",
    serde_json::json!({
      "fixed": [fixed.ref_time(), fixed.proof_size()],
      "fixedGeneratedIo": [fixed_io.reads, fixed_io.writes],
      "firstExtraRefTimeSelectedIo": [first_extra_io.reads, first_extra_io.writes],
      "firstTotalRefTimeSelectedIo": [fixed_io.reads + first_extra_io.reads, fixed_io.writes + first_extra_io.writes],
      "feeSinkActor": fee_sink, "initialAnchor": before["actors"][0]["temporalAnchorTick"],
      "initializedAnchor": first_snapshot["actors"][0]["temporalAnchorTick"],
      "initializedDeadline": first_snapshot["actors"][0]["triggerWakeupTick"],
      "components": components.map(|(name, weight, _, _, io, frequency)| serde_json::json!({
        "name": name, "weight": [weight.ref_time(), weight.proof_size()],
        "reads": io.reads, "writes": io.writes, "frequency": frequency,
      })),
      "firstBlockExtra": [first_block_extra.ref_time(), first_block_extra.proof_size()],
      "firstPrepass": [first.metrics.prepass_actor_control.ref_time(), first.metrics.prepass_actor_control.proof_size()],
      "secondPrepass": [second.metrics.prepass_actor_control.ref_time(), second.metrics.prepass_actor_control.proof_size()],
    })
  );
}

#[test]
fn control_temporal_weight_io_ledger_matches_production_selectors() {
  type ProductionWeight = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

  let at_time_occurrence_io = DatabaseIo::new(35, 15);
  let cadenced_occurrence_io = DatabaseIo::new(41, 18);

  assert_production_weight_component(
    ProductionWeight::at_time_trigger_occurrence(),
    1_116_081_000,
    37_633,
    at_time_occurrence_io,
  );
  assert_production_weight_component(
    ProductionWeight::cadenced_trigger_occurrence(),
    1_222_312_000,
    46_501,
    cadenced_occurrence_io,
  );

  assert_eq!(
    DatabaseIo::new(
      cadenced_occurrence_io
        .reads
        .saturating_sub(at_time_occurrence_io.reads),
      cadenced_occurrence_io
        .writes
        .saturating_sub(at_time_occurrence_io.writes),
    ),
    DatabaseIo::new(6, 3),
    "Cadenced additionally owns populated destination rearm geometry"
  );

  let cadenced = ProductionWeight::cadenced_trigger_occurrence();
  let at_time = ProductionWeight::at_time_trigger_occurrence();
  let temporal_admission = Weight::from_parts(
    cadenced.ref_time().max(at_time.ref_time()),
    cadenced.proof_size().max(at_time.proof_size()),
  );
  assert_eq!(temporal_admission.ref_time(), cadenced.ref_time());
  assert_eq!(temporal_admission.proof_size(), cadenced.proof_size());
}

fn assert_control_phase_attribution_campaign(wasm: &[u8], replay_wasm: bool) {
  control_temporal_weight_io_ledger_matches_production_selectors();
  let manual =
    run_control_phase_attribution_campaign(wasm, WorkloadSchedule::ManualOnly, replay_wasm);
  let cadenced =
    run_control_phase_attribution_campaign(wasm, WorkloadSchedule::CadencedOnly, replay_wasm);
  if replay_wasm {
    for result in [&manual, &cadenced] {
      assert_eq!(
        result.wasm_proofs.len(),
        CONTROL_ATTRIBUTION_BLOCKS as usize
      );
      assert!(
        result
          .wasm_proofs
          .iter()
          .all(|proof| proof.remaining_residual_node_bytes == 0),
        "matched-phase proofs must be covered by recorded key paths plus committed-root reconstruction"
      );
    }
  }
  assert!(manual.steps[0] > 0 && manual.steps[0] < CONTROL_ATTRIBUTION_ACTORS);
  assert_eq!(manual.steps.iter().sum::<u32>(), CONTROL_ATTRIBUTION_ACTORS);
  assert_eq!(manual.steps.last(), Some(&0));
  assert_eq!(manual.prepass_steps, manual.steps);
  assert_eq!(manual.trigger_occurrences, vec![0; 9]);
  assert_eq!(cadenced.steps, vec![0, 0, 1, 1, 1, 1, 1, 1, 1]);
  assert_eq!(cadenced.prepass_steps, cadenced.steps);
  assert_eq!(
    cadenced.trigger_occurrences,
    vec![0, 1, 1, 1, 1, 1, 1, 1, 1]
  );
  assert!(
    manual
      .prepass_steps
      .iter()
      .zip(&manual.steps)
      .all(|(prepass, finalized)| prepass <= finalized)
  );
  assert!(manual.steps.iter().sum::<u32>() > cadenced.steps.iter().sum());
  assert!(
    cadenced
      .trigger_occurrences
      .iter()
      .zip(&cadenced.steps)
      .any(|(triggers, steps)| *triggers > 0 && *steps == 0)
  );
  println!(
    "EXP_0066_CONTROL_ATTRIBUTION_COMPARISON_V1 {{\"manualSteps\":{},\"cadencedSteps\":{},\"cadencedTriggerOccurrences\":{},\"prepassWorkIsFinalizedSubset\":true,\"deadlineMaterializationMayRunOnIdle\":true,\"materializedActorsDeferredByEligibility\":true}}",
    manual.steps.iter().sum::<u32>(),
    cadenced.steps.iter().sum::<u32>(),
    cadenced.trigger_occurrences.iter().sum::<u32>(),
  );
}

/// Author a first block from the complete reference preset, without synthetic
/// timestamp/validation/resource writes or direct Actor-hook invocation.
fn author_reference_first_block(wasm: &[u8]) -> (Storage, Block, InherentData) {
  let storage = reference_genesis_storage(wasm);
  let parent = parent_header_for(storage.clone(), wasm, 0);
  let profiles = BTreeMap::new();
  let authored = author_complete_block(
    storage,
    wasm,
    1,
    UserDemand::ActorOnly,
    &sr25519::Pair::from_seed(&[0u8; 32]),
    &profiles,
  );
  assert_eq!(*authored.block.header.parent_hash(), parent.hash());
  (authored.pre_state, authored.block, authored.inherent_data)
}

#[test]
fn runtime_prepass_api_accepts_standard_omni_inherent_data_without_storage_mutation() {
  use polkadot_sdk::sp_block_builder::runtime_decl_for_block_builder::BlockBuilder;

  let (storage, block, data) = author_reference_first_block(&[]);
  TestExternalities::new(storage).execute_with(|| {
    let mut timestamp_only = InherentData::new();
    timestamp_only
      .put_data(
        polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER,
        &data
          .get_data::<u64>(&polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER)
          .unwrap()
          .unwrap(),
      )
      .unwrap();
    let mut authoring = timestamp_only.clone();
    authoring
      .put_data(
        INHERENT_IDENTIFIER,
        &data
          .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
          .unwrap()
          .unwrap(),
      )
      .unwrap();
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let generated = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(authoring);
    assert_eq!(generated, data.create_extrinsics());
    assert_eq!(
      generated
        .iter()
        .filter(|xt| matches!(
          xt.function,
          RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
        ))
        .count(),
      1,
    );
    assert!(<Runtime as BlockBuilder<Block>>::check_inherents(block.into(), timestamp_only).ok());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before,
    );
  });
}

#[test]
fn runtime_prepass_api_preserves_explicit_invalid_provider_rejection() {
  use pallet_deos_actors::{ACTOR_PREPASS_INHERENT_IDENTIFIER, ActorPrepassInherentError};
  use polkadot_sdk::sp_block_builder::runtime_decl_for_block_builder::BlockBuilder;

  let (storage, block, data) = author_reference_first_block(&[]);
  TestExternalities::new(storage).execute_with(|| {
    let mut unsupported = data.clone();
    unsupported.replace_data(ACTOR_PREPASS_INHERENT_IDENTIFIER, &2u8);
    let mut malformed = data.clone();
    malformed.replace_data(ACTOR_PREPASS_INHERENT_IDENTIFIER, &());
    let mut trailing = data;
    trailing.replace_data(ACTOR_PREPASS_INHERENT_IDENTIFIER, &[1u8, 0u8]);
    for (provided, expected) in [
      (unsupported, ActorPrepassInherentError::UnsupportedVersion),
      (malformed, ActorPrepassInherentError::MissingData),
      (trailing, ActorPrepassInherentError::MissingData),
    ] {
      let generated = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(provided.clone());
      assert!(generated.iter().all(|xt| !matches!(
        xt.function,
        RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
      )));
      let checked =
        <Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), provided);
      assert_eq!(
        checked
          .into_errors()
          .find(|(identifier, _)| identifier == &ACTOR_PREPASS_INHERENT_IDENTIFIER)
          .map(|(_, error)| error),
        Some(expected.encode()),
      );
    }
  });
}

#[test]
fn runtime_prepass_api_keeps_missing_call_fatal_with_standard_omni_data() {
  use pallet_deos_actors::{ACTOR_PREPASS_INHERENT_IDENTIFIER, ActorPrepassInherentError};
  use polkadot_sdk::sp_block_builder::runtime_decl_for_block_builder::BlockBuilder;

  let (storage, mut block, data) = author_reference_first_block(&[]);
  TestExternalities::new(storage).execute_with(|| {
    block.extrinsics.retain(|xt| {
      !matches!(
        xt.function,
        RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
      )
    });
    let mut timestamp_only = InherentData::new();
    timestamp_only
      .put_data(
        polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER,
        &data
          .get_data::<u64>(&polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER)
          .unwrap()
          .unwrap(),
      )
      .unwrap();
    let checked = <Runtime as BlockBuilder<Block>>::check_inherents(block.into(), timestamp_only);
    assert!(checked.fatal_error());
    assert_eq!(
      checked
        .into_errors()
        .find(|(identifier, _)| identifier == &ACTOR_PREPASS_INHERENT_IDENTIFIER)
        .map(|(_, error)| error),
      Some(ActorPrepassInherentError::MissingCall.encode()),
    );
  });
}

#[test]
fn reference_full_block_fixture_replays_through_executive() {
  let (storage, block, data) = author_reference_first_block(&[]);
  let authored = AuthoredBlock {
    pre_state: storage.clone(),
    post_state: storage,
    block,
    inherent_data: data,
    execution_backend_keys: BTreeSet::new(),
    execution_overlay_keys: BTreeSet::new(),
    #[cfg(not(feature = "runtime-benchmarks"))]
    execution_recorded_proof_bytes: None,
    metrics: FullExecutiveBlockMetrics {
      actor_steps: 0,
      distinct_actors: 0,
      progressed_steps: Vec::new(),
      opening_steps: 0,
      middle_steps: 0,
      final_steps: 0,
      transfer_steps: 0,
      swap_out_steps: 0,
      stop_cycle_steps: 0,
      non_successful_steps: 0,
      completed_cycles: 0,
      completed_cycle_actors: Vec::new(),
      failed_cycle_actors: Vec::new(),
      suspended_steps: Vec::new(),
      continued_steps: Vec::new(),
      closed_actors: Vec::new(),
      paused_actors: Vec::new(),
      resumed_actors: Vec::new(),
      trigger_occurrences: Vec::new(),
      user_calls: 0,
      next_user_weight: None,
      user_admission_refusal: None,
      prepass_steps: 0,
      prepass_trigger_occurrences: 0,
      prepass_actor_control: Weight::zero(),
      prepass_actor_effect: Weight::zero(),
      pre_idle_remaining: Weight::zero(),
      actor_control: Weight::zero(),
      actor_effect: Weight::zero(),
      user_dispatch: Weight::zero(),
      queue_head: 0,
      queue_tail: 0,
    },
  };
  assert_complete_block_replays_natively(&authored, &[]);
}

#[test]
fn full_executive_step_observations_preserve_ticket_order_not_actor_id_order() {
  let mut ext = TestExternalities::new_with_code_and_state(
    &[],
    reference_genesis_storage(&[]),
    crate::VERSION.state_version(),
  );
  let (actor_ids, profiles) = ext.execute_with(|| {
    System::set_block_number(1);
    let steps = actors_integration_tests::transfer_contract_steps(
      super::common::BOB,
      primitives::AssetKind::Native,
      crate::EXISTENTIAL_DEPOSIT,
    );
    let actor_ids = (0..2)
      .map(|_| {
        let actor_id = actors_integration_tests::create_system(
          ALICE,
          actors_integration_tests::manual_schedule(),
          None,
          steps.clone(),
        );
        actors_integration_tests::fund_native(actor_id, 1_000 * crate::EXISTENTIAL_DEPOSIT);
        actor_id
      })
      .collect::<Vec<_>>();
    assert!(actor_ids[0] < actor_ids[1]);
    for actor_id in actor_ids.iter().rev() {
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), *actor_id));
    }
    let profiles = actor_ids
      .iter()
      .map(|actor_id| {
        (
          *actor_id,
          WorkloadActorProfile {
            step_count: 1,
            opening_predicates_per_step: 0,
            task: Some(WorkloadTask::Transfer),
          },
        )
      })
      .collect();
    (actor_ids, profiles)
  });
  ext.commit_all().expect("reversed-ticket fixture commits");
  let storage = ext.execute_with(current_top_storage);
  let parent = parent_header_for(storage.clone(), &[], 1);
  let signer = sr25519::Pair::from_seed(&[66; 32]);
  let authored = author_complete_block_after(
    storage,
    &[],
    &parent,
    2,
    UserDemand::ActorOnly,
    &signer,
    0,
    &profiles,
  );
  assert_successful_transfer_outcomes(&authored.metrics);
  assert_eq!(
    authored.metrics.progressed_steps,
    vec![(actor_ids[1], 0), (actor_ids[0], 0)],
    "Step observations must retain actual FIFO event chronology"
  );
}

#[test]
fn full_executive_manual_phase_control_stops_use_live_head_and_stage_budget() {
  let snapshot = || {
    assert!(Actors::prepass_execution_cutoff().is_none());
    assert!(!pallet_deos_actors::GlobalCircuitBreaker::<Runtime>::get());
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    let header = Actors::service_header();
    let head = header.cursor.map(|actor| {
      let node =
        Actors::service_nodes(actor.actor_id).expect("canonical Service cursor owns its node");
      assert_eq!(node.generation, actor.generation);
      let process =
        Actors::actor_processes(actor.actor_id).expect("canonical Service cursor owns its process");
      assert_eq!(process.generation, actor.generation);
      assert_eq!(
        process.residence,
        Some(pallet_deos_actors::ProcessResidence::Service(node.kind))
      );
      let state = Actors::active_actor_state(actor.actor_id)
        .expect("canonical Service cursor owns active state");
      assert_eq!(
        state.identity.actor_class.actor_type(),
        pallet_deos_actors::ActorType::System
      );
      let contract = pallet_deos_actors::ActorContractHeads::<Runtime>::get(actor.actor_id)
        .expect("ordinary canonical Service member has its Contract head");
      assert_eq!(contract.header.trigger, Trigger::Manual);
      assert_eq!(contract.header.step_count, 1);
      assert_eq!(contract.header.has_close_when, false);
      assert!(contract.header.window.is_none());
      assert!(contract.header.auto_close_at_cycle_nonce.is_none());
      assert_eq!(state.hot.unsuccessful_attempt_streak, 0);
      assert!(<Runtime as pallet_deos_actors::Config>::MaxConsecutiveFailures::get() > 1);
      assert_eq!(
        contract.first_step.as_ref().expect("one Step").on_error,
        StepErrorPolicy::AbortCycle
      );
      (actor, node.eligible_from, contract.first_step_resources)
    });
    assert_eq!(header.cursor.is_none(), header.count == 0);
    (header.count, head)
  };
  let fixture = prepare_actor_fixture(
    &[],
    CONTROL_ATTRIBUTION_ACTORS,
    WorkloadSchedule::ManualOnly,
  );
  let mut pre_state = fixture.storage;
  let mut parent = parent_header_for(pre_state.clone(), &[], 1);
  let limits = BlockResourceBudgetValue::get().limits();
  let pair = |w: Weight| [w.ref_time(), w.proof_size()];
  let mut rows = Vec::new();
  for block in 2..3 {
    let authored = author_complete_block_after(
      pre_state,
      &[],
      &parent,
      block,
      UserDemand::ActorOnly,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    assert_successful_transfer_outcomes(&authored.metrics);
    assert!(
      authored.metrics.prepass_steps <= authored.metrics.actor_steps,
      "mandatory prepass Steps remain a subset of finalized canonical Service Steps",
    );
    assert_eq!(authored.block.extrinsics.len(), 3);

    // Replay only the exact inherent prefix on a disposable clone. Observation reads must not
    // enter the original block's recorder or influence subsequent extrinsics/idle execution.
    let mut prefix = TestExternalities::new_with_code_and_state(
      &[],
      authored.pre_state.clone(),
      crate::VERSION.state_version(),
    );
    let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    prefix.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    let (cutoff, head) = prefix.execute_with_recorder(recorder, || {
      Executive::initialize_block(&authored.block.header);
      for extrinsic in &authored.block.extrinsics {
        Executive::apply_extrinsic(extrinsic.clone())
          .expect("prefix is valid")
          .expect("inherent succeeds");
      }
      let state = Actors::block_resource_state().expect("Prepass state exists");
      assert_eq!(
        state.usage().actor_control_used(),
        authored.metrics.prepass_actor_control
      );
      assert_eq!(
        state.usage().actor_effect_used(),
        authored.metrics.prepass_actor_effect
      );
      assert_eq!(state.outstanding_reservations(), 0);
      snapshot()
    });
    let mut finalized = TestExternalities::new_with_code_and_state(
      &[],
      authored.post_state.clone(),
      crate::VERSION.state_version(),
    );
    let (final_count, final_head) = finalized.execute_with(snapshot);
    let prepass_remaining = limits
      .actor_control()
      .checked_sub(&authored.metrics.prepass_actor_control)
      .expect("mandatory prepass Control fits its limit");
    let final_remaining = limits
      .actor_control()
      .checked_sub(&authored.metrics.actor_control)
      .expect("finalized Actor Control fits its limit");
    assert!(final_remaining.all_lte(prepass_remaining));
    let later_phase_control = authored
      .metrics
      .actor_control
      .checked_sub(&authored.metrics.prepass_actor_control)
      .expect("mandatory prepass Control is a subset of finalized Actor Control");
    assert_ne!(
      later_phase_control,
      Weight::zero(),
      "idle and finalization own nonzero canonical control work"
    );
    type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
    let stored = head
      .as_ref()
      .and_then(|(_, _, resources)| *resources)
      .expect("saturated Manual head retains its certified Step envelope");
    let step_maximum_components = [
      (
        "head-plan-maximum",
        W::current_step_plan_opening_head().max(W::current_step_plan_suspended_head()),
      ),
      (
        "run-commit-maximum",
        W::run_progress()
          .max(W::run_suspend())
          .max(W::run_complete()),
      ),
      (
        "placement-maximum",
        W::service_round_admit_eligible().max(W::service_member_to_deadline_new_key()),
      ),
      ("fee-collection-allowance", W::fee_collection()),
      ("invocation-receipt", W::action_invocation_receipt()),
    ];
    assert_eq!(
      stored.control,
      step_maximum_components
        .iter()
        .fold(Weight::zero(), |sum, (_, weight)| sum
          .saturating_add(*weight)),
      "the shared composed maximum dominates the alternative complete-owner profiles for this witness"
    );
    let inspection = W::service_round_begin_populated()
      .saturating_add(W::service_round_probe_eligible())
      .saturating_add(W::scheduler_actor_state_probe());
    let suffix = W::service_round_admit_eligible().max(
      W::service_member_retire_interior()
        .max(W::service_member_retire_pair_cursor())
        .max(W::service_member_retire_singleton()),
    );
    // The snapshot excludes every terminal-control condition for this persistent System Step.
    let next_admission_control = inspection
      .saturating_add(stored.control)
      .saturating_add(suffix);
    assert!(next_admission_control.ref_time() <= prepass_remaining.ref_time());
    assert!(next_admission_control.proof_size() > prepass_remaining.proof_size());
    assert!(!next_admission_control.all_lte(final_remaining));
    assert_eq!(
      head, final_head,
      "both phases retain the same refused Service member"
    );
    let prepass_head = format!("{head:?}");
    let final_head = format!("{final_head:?}");
    rows.push(serde_json::json!({
      "block": block,
      "prepassSteps": authored.metrics.prepass_steps,
      "finalizedSteps": authored.metrics.actor_steps,
      "prepassServiceCount": cutoff,
      "finalServiceCount": final_count,
      "prepassHead": prepass_head,
      "finalHead": final_head,
      "prepassRemainingControl": pair(prepass_remaining),
      "finalRemainingControl": pair(final_remaining),
      "laterPhaseControl": pair(later_phase_control),
      "storedStepControlMaximum": pair(stored.control),
      "stepMaximumComponents": step_maximum_components.iter().map(|(owner, weight)|
        serde_json::json!({"owner": owner, "weight": pair(*weight)})
      ).collect::<Vec<_>>(),
      "storedStepEffectMaximum": pair(stored.effect),
      "inspectionControl": pair(inspection),
      "outerSuffixMaximum": pair(suffix),
      "nextAdmissionControl": pair(next_admission_control),
      "prepassAdmissionDeficit": pair(next_admission_control.saturating_sub(prepass_remaining)),
      "finalAdmissionDeficit": pair(next_admission_control.saturating_sub(final_remaining)),
    }));
    parent = authored.block.header.clone();
    pre_state = authored.post_state;
  }
  assert_eq!(rows.len(), 1);
  println!(
    "ACTOR_MANUAL_PHASE_STOPS_V1 {}",
    serde_json::json!({
      "mode": "canonical-Service-inherent-prefix-and-final-state", "blocks": rows,
    })
  );
}

#[test]
fn full_executive_control_frontier_separates_capacity_from_service_eligibility() {
  for (count, schedule) in [
    (0, WorkloadSchedule::ManualOnly),
    (128, WorkloadSchedule::ManualOnly),
    (128, WorkloadSchedule::CadencedOnly),
    (128, WorkloadSchedule::MixedManualCadenced),
  ] {
    let fixture = prepare_actor_fixture(&[], count, schedule);
    let authored = author_complete_block(
      fixture.storage,
      &[],
      2,
      UserDemand::ActorOnly,
      &fixture.signer,
      &fixture.actor_profiles,
    );
    assert_successful_transfer_outcomes(&authored.metrics);
    assert_complete_block_replays_natively(&authored, &[]);
    let remaining = BlockResourceBudgetValue::get()
      .limits()
      .actor_control()
      .checked_sub(&authored.metrics.actor_control)
      .expect("finalized Control usage fits its limit");
    let mut ext = TestExternalities::new_with_code_and_state(
      &[],
      authored.post_state,
      crate::VERSION.state_version(),
    );
    ext.execute_with(|| {
      assert!(Actors::prepass_execution_cutoff().is_none());
      assert_eq!(Actors::combined_queue_occupancy(), 0);
      assert_eq!(Actors::queue_head(), Actors::queue_tail());
      if count == 0 {
        assert_eq!(authored.metrics.actor_steps, 0);
        return;
      }

      let header = Actors::service_header();
      let Some(actor) = header.cursor else {
        assert_eq!(header.count, 0);
        assert!(authored.metrics.trigger_occurrences.is_empty());
        let manual_ids = fixture.actor_ids.iter().enumerate().filter_map(|(index, id)| {
          schedule.uses_manual_trigger(u32::try_from(index).unwrap()).then_some(*id)
        }).collect::<Vec<_>>();
        assert_eq!(authored.metrics.completed_cycle_actors, manual_ids);
        assert_eq!(authored.metrics.actor_steps as usize, manual_ids.len());
        assert!(authored.metrics.closed_actors.is_empty());
        // Empty Service can mean that the ready Manual prefix completed and left passive Idle,
        // while the independent Cadenced detector still owns an unmaterialized occurrence.
        for (index, id) in fixture.actor_ids.iter().enumerate() {
          let manual = schedule.uses_manual_trigger(u32::try_from(index).unwrap());
          let state = Actors::active_actor_state(*id).expect("workload identity is retained");
          assert_eq!(state.contract.trigger, if manual { Trigger::manual() } else { Trigger::cadenced(1) });
          assert_eq!(state.identity.cycle_nonce, u64::from(manual));
          assert_eq!(state.hot.cycle_state, pallet_deos_actors::CycleState::Idle);
          assert!(!state.hot.pending_signal);
          assert!(state.run_state.is_none());
          assert!(Actors::service_nodes(*id).is_none());
          assert!(Actors::deadline_handles(*id).is_none());
          let process = Actors::actor_processes(*id).unwrap();
          assert_eq!(process.generation, 1);
          assert_eq!(process.last_attempted, manual.then_some(2));
          assert_eq!(process.residence, None);
          assert!(matches!(process.status,
            pallet_deos_actors::ProcessStatus::Disabled(disablement)
              if disablement.cause == pallet_deos_actors::ProcessDisableCause::Protocol
                && disablement.revival_authority == pallet_deos_actors::ProcessRevivalAuthority::Protocol
                && disablement.basis == pallet_deos_actors::SuspendedProcessBasis::Idle
          ));
          let detector = pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::get(id);
          if manual {
            assert!(detector.is_none());
          } else {
            let detector = detector.expect("unmaterialized cadence retains its source");
            assert_eq!(detector.actor.actor_id, *id);
            assert_eq!(detector.actor.generation, process.generation);
            assert_eq!(detector.key, WakeupKey::Tick(1));
          }
        }
        return;
      };
      assert!(header.count > 0);
      let node = Actors::service_nodes(actor.actor_id)
        .expect("canonical Service head owns its generation-bound node");
      assert_eq!(node.generation, actor.generation);
      let process = Actors::actor_processes(actor.actor_id)
        .expect("canonical Service head owns its process publication");
      assert_eq!(process.generation, actor.generation);
      assert_eq!(
        process.residence,
        Some(pallet_deos_actors::ProcessResidence::Service(node.kind))
      );

      // Eligibility and complete-attempt capacity are independent frontiers. The finalized block can
      // retain a canonical Service member even when its remaining Control budget cannot admit the
      // runtime's maximum next attempt; no frozen ticket cutoff is a second source of authority.
      let required = fixture.next_control_maximum;
      let deficit = required.saturating_sub(remaining);
      assert!(!required.all_lte(remaining));
      assert!(deficit.ref_time() > 0 || deficit.proof_size() > 0);
      assert_eq!(
        authored.metrics.actor_steps > 0,
        schedule != WorkloadSchedule::CadencedOnly
      );
      println!(
        "ACTOR_CONTROL_FRONTIER_V1 {}",
        serde_json::json!({
          "mode": "native-finalized-snapshot",
          "schedule": schedule.label(), "block": 2, "actors": count,
          "steps": authored.metrics.actor_steps,
          "serviceCount": header.count, "serviceActor": actor.actor_id,
          "eligibleFrom": node.eligible_from,
          "ordinaryControlRequirement": [required.ref_time(), required.proof_size()],
          "finalizedControlRemaining": [remaining.ref_time(), remaining.proof_size()],
          "finalizedControlDeficit": [deficit.ref_time(), deficit.proof_size()],
        })
      );
    });
  }
}

#[test]
fn full_executive_w0_read_write_key_owners_are_observed_before_diagnostic_state_scan() {
  let storage_info = runtime_proof_storage_info();
  let owner_for_key = |key: &[u8]| {
    let mut matching = storage_info
      .iter()
      .filter(|info| key.starts_with(&info.prefix));
    let owner = matching
      .next()
      .map(|info| {
        format!(
          "{}.{}",
          String::from_utf8_lossy(&info.pallet_name),
          String::from_utf8_lossy(&info.storage_name),
        )
      })
      .unwrap_or_else(|| {
        if key.starts_with(b":") {
          "WellKnownTop"
        } else {
          "UnmappedTop"
        }
        .to_owned()
      });
    assert!(matching.next().is_none(), "one owner per storage prefix");
    owner
  };
  for actors in [0, 1] {
    let fixture = prepare_actor_fixture(&[], actors, WorkloadSchedule::ManualOnly);
    let authored = author_complete_block(
      fixture.storage,
      &[],
      2,
      UserDemand::ActorOnly,
      &fixture.signer,
      &fixture.actor_profiles,
    );
    assert_eq!(authored.metrics.actor_steps, actors);
    let roots = authored
      .execution_backend_keys
      .iter()
      .map(|(root, _)| *root)
      .collect::<BTreeSet<_>>();
    assert_eq!(roots.len(), 1, "W0 uses only the top trie");
    assert!(
      authored
        .execution_overlay_keys
        .iter()
        .all(|(child, _)| child.is_none()),
      "W0 has no child-trie writes"
    );
    let mut read_owners = BTreeMap::<String, usize>::new();
    for (_, key) in &authored.execution_backend_keys {
      *read_owners.entry(owner_for_key(key)).or_default() += 1;
    }
    let mut write_owners = BTreeMap::<String, usize>::new();
    for (_, key) in &authored.execution_overlay_keys {
      *write_owners.entry(owner_for_key(key)).or_default() += 1;
    }
    assert_eq!(
      read_owners.values().sum::<usize>(),
      authored.execution_backend_keys.len()
    );
    assert_eq!(
      write_owners.values().sum::<usize>(),
      authored.execution_overlay_keys.len()
    );
    println!(
      "ACTOR_BASELINE_NATIVE_BLOCK_KEYS_V1 {}",
      serde_json::json!({
        "preparedActors": actors,
        "committedSteps": authored.metrics.actor_steps,
        "distinctBackendReadPaths": authored.execution_backend_keys.len(),
        "readOwners": read_owners,
        "distinctCommittedOverlayWriteKeys": authored.execution_overlay_keys.len(),
        "writeOwners": write_owners,
        "scope": "whole authored block; surviving overlay write identities and backend trie read paths, not per-Step attribution or access frequency",
      })
    );
  }
}

#[test]
fn full_executive_native_proof_owner_partition() {
  let storage_info = runtime_proof_storage_info();
  for actors in [0, 1, PREPARED_W1_ACTORS] {
    let fixture = prepare_actor_fixture(&[], actors, WorkloadSchedule::ManualOnly);
    let authored = author_complete_block(
      fixture.storage,
      &[],
      2,
      UserDemand::ActorOnly,
      &fixture.signer,
      &fixture.actor_profiles,
    );
    assert_successful_transfer_outcomes(&authored.metrics);
    if actors <= 1 {
      assert_eq!(authored.metrics.actor_steps, actors);
    } else {
      assert!(authored.metrics.actor_steps > 0 && authored.metrics.actor_steps < actors);
    }
    let mut replay = TestExternalities::new_with_code_and_state(
      &[],
      authored.pre_state.clone(),
      crate::VERSION.state_version(),
    );
    let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    replay.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    // Record only Executive execution, not authoring, inherent checks, or diagnostic queries.
    replay.execute_with_recorder(recorder.clone(), || {
      Executive::execute_block(authored.block.clone().into());
    });
    let proof = recorder.to_storage_proof();
    let overlap = super::wasm_replay::key_proof_overlap(&recorder)
      .expect("every recorded access reconstructs from execution proof with the same access kind");
    let roots = overlap
      .paths
      .keys()
      .map(|(root, _)| *root)
      .collect::<BTreeSet<_>>();
    assert_eq!(
      roots.len(),
      1,
      "this workload accesses only the pre-state top trie"
    );
    let pre_state_root = *roots.first().expect("Executive reads pre-state");
    let (owners, shared_bytes) =
      storage_proof_owners(pre_state_root, &overlap.paths, &storage_info);
    let exclusive_bytes = owners
      .values()
      .map(|owner| owner.exclusive_node_bytes)
      .sum::<usize>();
    let node_bytes = proof.iter_nodes().map(Vec::len).sum::<usize>();
    assert_eq!(exclusive_bytes + shared_bytes, overlap.unique_node_bytes);
    assert_eq!(
      overlap.unique_node_bytes + overlap.unattributed_node_bytes,
      node_bytes
    );
    assert_eq!(
      owners.values().map(|owner| owner.key_paths).sum::<usize>(),
      overlap.paths.len()
    );
    assert!(
      owners
        .keys()
        .all(|owner| !owner.starts_with("UnmappedTop") && owner != "NonTopTrie")
    );
    let service_read_actors = fixture
      .actor_profiles
      .keys()
      .copied()
      .filter(|actor_id| {
        overlap.paths.contains_key(&(
          pre_state_root,
          pallet_deos_actors::ServiceNodes::<Runtime>::hashed_key_for(actor_id),
        ))
      })
      .collect::<BTreeSet<_>>();
    let progressed_actors = authored
      .metrics
      .progressed_steps
      .iter()
      .map(|(actor_id, _)| *actor_id)
      .collect::<BTreeSet<_>>();
    assert!(progressed_actors.is_subset(&service_read_actors));
    let service_read_without_step = service_read_actors
      .difference(&progressed_actors)
      .copied()
      .collect::<Vec<_>>();
    let owner_rows = owners
      .iter()
      .map(|(owner, metrics)| {
        let info = storage_info.iter().find(|info| {
          format!(
            "{}.{}",
            String::from_utf8_lossy(&info.pallet_name),
            String::from_utf8_lossy(&info.storage_name)
          ) == *owner
        });
        // Encoded pre-state values are context, not an additive attribution of trie proof bytes.
        let values = overlap
          .paths
          .keys()
          .filter_map(|(_, key)| {
            let matches = info.map_or_else(
              || owner == "WellKnownTop" && key.starts_with(b":"),
              |info| key.starts_with(&info.prefix),
            );
            matches.then(|| authored.pre_state.top.get(key).map(Vec::len))
          })
          .collect::<Vec<_>>();
        assert_eq!(values.len(), metrics.key_paths);
        serde_json::json!({
          "owner": owner,
          "keyPaths": metrics.key_paths,
          "exclusiveNodeBytes": metrics.exclusive_node_bytes,
          "sharedNodeBytes": metrics.shared_node_bytes,
          "presentPreStateValues": values.iter().flatten().count(),
          "absentPreStateValues": values.iter().filter(|value| value.is_none()).count(),
          "encodedPreStateValueBytes": values.iter().flatten().sum::<usize>(),
        })
      })
      .collect::<Vec<_>>();
    println!(
      "ACTOR_BASELINE_NATIVE_PROOF_OWNERS_V1 {}",
      serde_json::json!({
        "preparedActors": actors,
        "committedSteps": authored.metrics.actor_steps,
        "preStateRoot": format!("{pre_state_root:?}"),
        "blockHash": format!("{:?}", authored.block.header.hash()),
        "chargedControl": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
        "chargedEffect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
        "storageProofScaleBytes": proof.encoded_size(),
        "trieNodes": proof.len(),
        "trieNodeBytes": node_bytes,
        "workloadServiceNodesRead": service_read_actors.len(),
        "workloadServiceReadsWithoutCommittedStep": service_read_without_step,
        "exclusiveNodeBytes": exclusive_bytes,
        "crossOwnerSharedNodeBytes": shared_bytes,
        "unattributedNodeBytes": overlap.unattributed_node_bytes,
        "owners": owner_rows,
        "scope": "native full Executive replay; whole-block trie proof, not per-Step marginal, FRAME discount, parachain PoV, or production-Wasm evidence",
      })
    );
    // State inspection stays outside the recorder and checks that diagnostics changed no behavior.
    assert_eq!(
      replay.execute_with(current_top_storage).top,
      authored.post_state.top
    );
  }
}

#[test]
fn full_executive_governance_expiry_competes_with_actor_and_user_demand() {
  for participants in [0u32, 512] {
    // Four block-sized cohorts keep eligible work live through the expiry block.
    let mut fixture =
      prepare_actor_fixture(&[], 4 * PREPARED_W1_ACTORS, WorkloadSchedule::ManualOnly);
    let mut setup = TestExternalities::new_with_code_and_state(
      &[],
      fixture.storage.clone(),
      crate::VERSION.state_version(),
    );
    setup.execute_with(|| {
      assert_eq!(System::block_number(), 1);
      for (batch, start) in (0..participants).step_by(256).enumerate() {
        let winners = (start..(start + 256).min(participants))
          .map(|account| {
            crate::AccountId::new(polkadot_sdk::sp_io::hashing::blake2_256(
              &(b"governance-expiry", account).encode(),
            ))
          })
          .collect::<Vec<_>>();
        assert_ok!(crate::Governance::record_winning_vote_batch(
          RuntimeOrigin::root(),
          0,
          10_000 + batch as u32,
          BoundedVec::try_from(winners).expect("one bounded winning batch"),
        ));
      }
      assert_eq!(
        pallet_governance::ExpiryBuckets::<Runtime>::get(4).len(),
        participants as usize
      );
    });
    setup
      .commit_all()
      .expect("Governance ingress commits before authoring");
    fixture.storage = setup.execute_with(current_top_storage);
    let mut parent = parent_header_for(fixture.storage.clone(), &[], 1);
    for block in 2..=5 {
      let authored = author_complete_block_after(
        fixture.storage.clone(),
        &[],
        &parent,
        block,
        if block == 5 {
          UserDemand::ContinuousShared
        } else {
          UserDemand::ActorOnly
        },
        &fixture.signer,
        0,
        &fixture.actor_profiles,
      );
      assert_complete_block_replays_natively(&authored, &[]);
      if block == 5 {
        assert_successful_transfer_outcomes(&authored.metrics);
        assert!(authored.metrics.actor_steps > 0);
        assert!(authored.metrics.user_calls > 0);
        // Custom(44) is the runtime economic meter's reservation-rejected code, not FRAME.
        assert_eq!(
          authored.metrics.user_admission_refusal,
          Some(InvalidTransaction::Custom(44).into()),
        );
        let mut post = TestExternalities::new_with_code_and_state(
          &[],
          authored.post_state.clone(),
          crate::VERSION.state_version(),
        );
        post.execute_with(|| {
          assert!(pallet_governance::ExpiryBuckets::<Runtime>::get(4).is_empty());
          let evicted = System::events()
            .iter()
            .filter(|record| {
              matches!(
                record.event,
                RuntimeEvent::Governance(pallet_governance::Event::WinningVoteWindowEvicted { .. })
              )
            })
            .count();
          assert_eq!(evicted, participants as usize);
        });
        let limits = BlockResourceBudgetValue::get().limits();
        let shared_used = authored
          .metrics
          .actor_effect
          .saturating_add(authored.metrics.user_dispatch);
        let shared_remaining = limits
          .shared_economic()
          .checked_sub(&shared_used)
          .expect("economic usage fits its ledger");
        let next = authored
          .metrics
          .next_user_weight
          .expect("the rejected candidate is recorded");
        assert!(next.proof_size() > shared_remaining.proof_size());
        assert!(next.all_lte(authored.metrics.pre_idle_remaining));
        let pair = |weight: Weight| [weight.ref_time(), weight.proof_size()];
        println!(
          "GOVERNANCE_MIXED_EXECUTIVE_V1 {}",
          serde_json::json!({
            "participants": participants, "block": block,
            "actorSteps": authored.metrics.actor_steps, "userCalls": authored.metrics.user_calls,
            "refusal": format!("{:?}", authored.metrics.user_admission_refusal),
            "control": pair(authored.metrics.actor_control),
            "effect": pair(authored.metrics.actor_effect),
            "userDispatch": pair(authored.metrics.user_dispatch),
            "sharedRemaining": pair(shared_remaining),
            "preIdleRemaining": pair(authored.metrics.pre_idle_remaining),
            "scope": "native full Executive, real governance expiry and signed remarks; generated charges, not production Wasm or measured PoV",
          })
        );
      }
      parent = authored.block.header;
      fixture.storage = authored.post_state;
    }
  }
}

#[test]
fn full_executive_w0_w1_fixture_covers_actor_only_and_valid_user_demand() {
  assert_prepared_w0_w1(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w0_w1_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload preparation must remain bound to the accepted production Wasm"
  );
  assert_prepared_w0_w1(&wasm, true);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w1_actor_only_100_block_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W1 must remain bound to the accepted production Wasm"
  );
  run_schedule_campaign(
    &wasm,
    "W1",
    UserDemand::ActorOnly,
    WorkloadSchedule::MixedManualCadenced,
  );
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w1_continuous_valid_user_100_block_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Signed-demand linkage must use the accepted production Wasm"
  );
  run_schedule_campaign(
    &wasm,
    "W1",
    UserDemand::ContinuousValid,
    WorkloadSchedule::MixedManualCadenced,
  );
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w2_manual_and_cadenced_only_100_block_campaigns_replay_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W2 must remain bound to the accepted production Wasm"
  );
  for schedule in [WorkloadSchedule::ManualOnly, WorkloadSchedule::CadencedOnly] {
    run_schedule_campaign(&wasm, "W2", UserDemand::ActorOnly, schedule);
  }
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w3_opening_predicate_mixed_length_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W3 must remain bound to the accepted production Wasm"
  );
  run_w3_opening_predicate_mixed_length_campaign(&wasm);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w4_heterogeneous_effect_campaigns_replay_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W4 must remain bound to the accepted production Wasm"
  );
  for demand in [UserDemand::ActorOnly, UserDemand::ContinuousValid] {
    run_w4_heterogeneous_effect_campaign(&wasm, demand);
  }
}

#[test]
#[ignore = "128-block deadline profile; run through actors-assurance"]
fn full_executive_current_w7_deadline_fixture() {
  run_current_w7_deadlines(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_current_w7_deadlines_replay_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "current W7 must remain bound to the accepted production Wasm"
  );
  run_current_w7_deadlines(&wasm, true);
}

#[test]
fn full_executive_w5_lifecycle_retry_cleanup_fixture_preserves_prefixes() {
  let wasm = std::env::var_os("DEOS_PRODUCTION_WASM")
    .map(|path| {
      let code = std::fs::read(path).expect("genesis code is readable");
      assert_eq!(
        polkadot_sdk::sp_io::hashing::sha2_256(&code),
        ACCEPTED_PRODUCTION_WASM_SHA256
      );
      code
    })
    .unwrap_or_default();
  run_w5_lifecycle_retry_cleanup_campaign(&wasm, false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w5_lifecycle_retry_cleanup_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W5 must remain bound to the accepted production Wasm"
  );
  run_w5_lifecycle_retry_cleanup_campaign(&wasm, true);
}

#[test]
fn full_executive_w6_mixed_arrival_lifecycle_fixture_handles_stale_generations() {
  run_w6_mixed_arrival_lifecycle_campaign(&[], false);
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
#[ignore = "requires hash-bound diagnostic Wasm via DEOS_DIAGNOSTIC_WASM and DEOS_DIAGNOSTIC_WASM_SHA256"]
fn full_executive_w6_mixed_arrival_lifecycle_replays_hash_bound_diagnostic_wasm() {
  let path = std::env::var_os("DEOS_DIAGNOSTIC_WASM")
    .expect("DEOS_DIAGNOSTIC_WASM must explicitly select the diagnostic artifact");
  let expected: H256 = std::env::var("DEOS_DIAGNOSTIC_WASM_SHA256")
    .expect("DEOS_DIAGNOSTIC_WASM_SHA256 must bind the selected artifact")
    .parse()
    .expect("diagnostic artifact SHA-256 is a 32-byte hex value");
  let wasm = std::fs::read(path).expect("selected diagnostic Wasm is readable");
  assert_eq!(
    H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&wasm)),
    expected,
    "W6 diagnostic replay must remain bound to the caller-qualified artifact"
  );
  run_w6_mixed_arrival_lifecycle_campaign(&wasm, true);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w6_mixed_arrival_lifecycle_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W6 must remain bound to the accepted production Wasm"
  );
  run_w6_mixed_arrival_lifecycle_campaign(&wasm, true);
}

#[test]
fn full_executive_w7_due_only_active_frontier_fixture_scales_independently() {
  assert_w7_due_only_active_frontier_campaign(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w7_due_only_active_frontier_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W7 must remain bound to the accepted production Wasm"
  );
  assert_w7_due_only_active_frontier_campaign(&wasm, true);
}

#[test]
fn full_executive_w8_tombstone_prefix_chunk_pressure_fixture_preserves_fifo() {
  run_w8_tombstone_prefix_chunk_pressure_campaign(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w8_tombstone_prefix_chunk_pressure_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W8 must remain bound to the accepted production Wasm"
  );
  run_w8_tombstone_prefix_chunk_pressure_campaign(&wasm, true);
}

#[test]
fn full_executive_w9_resource_independence_fixture_preserves_actor_service() {
  run_w9_resource_independence_campaign(&[], false);
}

#[test]
fn full_executive_control_phase_attribution_fixture_isolates_materialization() {
  assert_control_phase_attribution_campaign(&[], false);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_control_phase_attribution_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload Control attribution must remain bound to the accepted production Wasm"
  );
  assert_control_phase_attribution_campaign(&wasm, true);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn full_executive_w9_resource_independence_campaign_replays_exact_production_wasm() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the accepted production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "Workload W9 must remain bound to the accepted production Wasm"
  );
  run_w9_resource_independence_campaign(&wasm, true);
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn reference_full_block_replays_in_production_wasm_with_verified_storage_proof() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the current production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256,
    "reference replay must bind the accepted production Wasm"
  );
  let (storage, block, data) = author_reference_first_block(&wasm);
  let evidence = super::wasm_replay::replay_block(storage, &wasm, &block.encode(), &data)
    .expect("production Wasm and independent proof-only execution agree");
  assert_runtime_version_equivalent(&evidence.runtime_version);
  assert_eq!(evidence.block_hash, block.header.hash());
  assert_eq!(evidence.post_state_root, *block.header.state_root());
  assert!(evidence.execute_block_result.is_empty());
  for proof in [&evidence.execution_proof, &evidence.verification_proof] {
    assert_eq!(
      proof.storage_proof_scale_bytes,
      proof.storage_proof.encoded_size()
    );
    assert_eq!(
      proof.compact_proof_scale_bytes,
      proof.compact_proof.encoded_size()
    );
    assert_eq!(proof.trie_node_count, proof.storage_proof.len());
    assert!(proof.trie_node_bytes > 0);
  }
  println!(
    "WASM_BLOCK_REPLAY_V1 code={:?} block={:?} preRoot={:?} postRoot={:?} executionStorageProofBytes={} executionCompactProofBytes={} executionNodeBytes={} executionNodes={} verificationStorageProofBytes={} verificationCompactProofBytes={} proofSizeObservations={:?}",
    evidence.runtime_code_hash,
    evidence.block_hash,
    evidence.pre_state_root,
    evidence.post_state_root,
    evidence.execution_proof.storage_proof_scale_bytes,
    evidence.execution_proof.compact_proof_scale_bytes,
    evidence.execution_proof.trie_node_bytes,
    evidence.execution_proof.trie_node_count,
    evidence.verification_proof.storage_proof_scale_bytes,
    evidence.verification_proof.compact_proof_scale_bytes,
    evidence.proof_size_observations,
  );
}

/// A legal signed prefix exercises Close's event append through native block finalization
/// and import. Root preparation is outside the encoded block; this is no cost bound.
#[test]
fn signed_dormant_close_preserves_event_prefix_through_complete_native_block() {
  use polkadot_sdk::frame_support::traits::fungible::InspectHold;

  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let reason = crate::RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState);
  for preset in crate::genesis_config_presets::preset_names() {
    let mut ext = TestExternalities::new(preset_genesis_storage(&[], &preset));
    let (actor_id, sovereign, custody, held_before, release, identities_before) =
      ext.execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Actors::set_user_creation_enabled(
          RuntimeOrigin::root(),
          true
        ));
        let actor_id = Actors::next_actor_id();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(owner.clone()),
          Mutability::Mutable,
          None,
        ));
        let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
        let custody = 7 * crate::EXISTENTIAL_DEPOSIT;
        assert_ok!(Balances::transfer_keep_alive(
          RuntimeOrigin::signed(owner.clone()),
          MultiAddress::Id(sovereign.clone()),
          custody,
        ));
        let hold = Actors::actor_state_hold(actor_id).unwrap().breakdown;
        let release =
          hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
        assert!(release > 0);
        (
          actor_id,
          sovereign.clone(),
          Balances::free_balance(&sovereign),
          Balances::balance_on_hold(&reason, &owner),
          release,
          Actors::actor_identity_count(),
        )
      });
    let storage = ext.execute_with(current_top_storage);
    let parent = parent_header_for(storage.clone(), &[], 1);
    let nonce =
      TestExternalities::new(storage.clone()).execute_with(|| System::account_nonce(&owner));
    let mut calls = (0u8..64)
      .map(|index| {
        RuntimeCall::System(polkadot_sdk::frame_system::Call::remark_with_event {
          remark: vec![index],
        })
      })
      .collect::<Vec<_>>();
    calls.push(RuntimeCall::Actors(pallet_deos_actors::Call::close_actor {
      actor_id,
    }));
    let authored = author_complete_block_after_with_calls(
      storage,
      &[],
      &parent,
      2,
      UserDemand::ActorOnly,
      &signer,
      nonce,
      &BTreeMap::new(),
      &calls,
    );
    assert_complete_block_replays_natively(&authored, &[]);
    TestExternalities::new(authored.post_state.clone()).execute_with(|| {
      assert!(Actors::actor_identity(actor_id).is_none());
      assert_eq!(Actors::actor_identity_count(), identities_before - 1);
      assert_eq!(Balances::free_balance(&sovereign), custody);
      assert_eq!(
        Balances::balance_on_hold(&reason, &owner),
        held_before - release
      );
      assert!(!pallet_deos_actors::ActorStateHolds::<Runtime>::contains_key(actor_id));
      assert_eq!(System::account_nonce(&owner), nonce + 65);
      let events = System::events();
      let close_index = u32::try_from(authored.block.extrinsics.len() - 1).unwrap();
      let close_position = events
        .iter()
        .position(|record| {
          matches!(record.event,
            RuntimeEvent::Actors(Event::ActorClosed {
              actor_id: closed, reason: CloseReason::OwnerInitiated,
            }) if closed == actor_id)
        })
        .expect("signed owner Close emits its terminal event");
      assert_eq!(
        events[close_position].phase,
        polkadot_sdk::frame_system::Phase::ApplyExtrinsic(close_index),
      );
      let remarks = events[..close_position]
        .iter()
        .filter_map(|record| match &record.event {
          RuntimeEvent::System(polkadot_sdk::frame_system::Event::Remarked { sender, hash }) => {
            assert_eq!(sender, &owner);
            Some((record.phase.clone(), *hash))
          }
          _ => None,
        })
        .collect::<Vec<_>>();
      assert_eq!(remarks.len(), 64);
      for (index, (phase, hash)) in remarks.iter().enumerate() {
        assert_eq!(
          *phase,
          polkadot_sdk::frame_system::Phase::ApplyExtrinsic(close_index - 64 + index as u32),
        );
        assert_eq!(*hash, BlakeTwo256::hash(&[index as u8]));
      }
      #[cfg(feature = "try-runtime")]
      <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(2).unwrap();
      eprintln!(
        "Signed dormant Close event prefix imported: {preset}; 64 remarks, {} events",
        events.len(),
      );
    });
  }
}
