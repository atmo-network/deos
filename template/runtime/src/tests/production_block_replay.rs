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
  ActorId, AmountResolution, CloseReason, CompletionPolicy, ContractSteps, CycleResult, CycleState,
  Event, InputLimit, ProcessResidence, ScheduleWindow, StepErrorPolicy, StepOf, Task, Trigger,
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
  },
  sp_trie::{
    StorageProof,
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
  sp_weights::WeightToFee,
};
use std::collections::{BTreeMap, BTreeSet};

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
/// SHA-256 of the production Wasm accepted for the retained release tree, rebuilt by
/// `scripts/03-build-runtime.sh`. Every exact-Wasm regression profile binds this identity and
/// fails closed when the caller selects different bytes; update it together with the accepted
/// production binding.
const ACCEPTED_PRODUCTION_WASM_SHA256: [u8; 32] = [
  0x26, 0x2f, 0x5a, 0x54, 0x7d, 0xaa, 0xdb, 0xa8, 0x5c, 0x2b, 0x4d, 0x55, 0xfe, 0x2a, 0xed, 0xa0,
  0xe6, 0x10, 0xdb, 0x80, 0x19, 0xae, 0x75, 0x5b, 0x41, 0x1e, 0xf3, 0x6d, 0x48, 0xdd, 0x36, 0x4d,
];
const REFERENCE_ACTIVE_SYSTEM_ACTORS: u32 = 3;
const REFERENCE_SYSTEM_ACTOR_IDENTITIES: u32 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UserDemand {
  ActorOnly,
  ContinuousValid,
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
      Self::RefTimeHeavy => "ref-time-heavy-valid-user-demand",
    }
  }
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
  prepass_steps: u32,
  prepass_trigger_occurrences: u32,
  prepass_actor_control: Weight,
  prepass_actor_effect: Weight,
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

fn reference_genesis_storage(wasm: &[u8]) -> Storage {
  let preset = crate::genesis_config_presets::get_preset(
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  )
  .expect("Development is a declared runtime preset");
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

fn current_top_storage() -> Storage {
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
    let productive_cleanup = actors_integration_tests::create_system_with_completion(
      ALICE,
      actors_integration_tests::manual_schedule(),
      None,
      cleanup_steps,
      CompletionPolicy::CloseAfterProductiveCycle,
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
    let mut add_actor = |schedule, window, trigger_manual| {
      let actor_id =
        actors_integration_tests::create_user(owner.clone(), schedule, window, steps.clone());
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
      .map(|_| add_actor(actors_integration_tests::manual_schedule(), None, true))
      .collect::<Vec<_>>();
    let sparse_block = [3, 6, 9]
      .into_iter()
      .map(|start| {
        add_actor(
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
          actors_integration_tests::cadenced_schedule(*period),
          None,
          false,
        )
      })
      .collect::<Vec<_>>();
    let paused_actor = sparse_block[1];
    let closed_actor = sparse_block[2];
    assert_eq!(
      Actors::service_nodes(closed_actor)
        .expect("future-window W6 Actor has canonical Service residence")
        .eligible_from,
      2
    );
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
        assert!(future.iter().all(|actor_id| {
          Actors::service_nodes(*actor_id).is_some_and(|node| node.eligible_from == 2)
        }));
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
          max_pov_size: 5_000_000,
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
) -> (u32, Weight) {
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
    let next_weight = extrinsic.get_dispatch_info().total_weight();
    let state = Actors::block_resource_state().expect("Actor Prepass opens resource state");
    let remaining = user_limit
      .checked_sub(&state.usage().user_dispatch_used())
      .unwrap_or_else(Weight::zero);
    if !next_weight.all_lte(remaining) {
      assert!(
        calls > 0,
        "continuous demand must admit at least one valid call"
      );
      return (calls, next_weight);
    }
    Executive::apply_extrinsic(extrinsic.clone())
      .unwrap_or_else(|error| panic!("valid user call rejected: {error:?}"))
      .unwrap_or_else(|error| panic!("valid user call failed dispatch: {error:?}"));
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
    let next_weight = extrinsic.get_dispatch_info().total_weight();
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
  prepass_steps: u32,
  prepass_trigger_occurrences: u32,
  prepass_actor_control: Weight,
  prepass_actor_effect: Weight,
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
    prepass_steps,
    prepass_trigger_occurrences,
    prepass_actor_control,
    prepass_actor_effect,
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
  let inherent_data = inherent_data_for(parent, block_number);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  let proof_size = RecordingProofSizeProvider::new(recorder.clone());
  ext.register_extension(ProofSizeExt::new(proof_size));
  let (block, metrics, post_state) = ext.execute_with_recorder(recorder, || {
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
    let signed_call_count = u32::try_from(signed_calls.len()).expect("bounded W6 call count fits");
    let mut next_nonce = signer_nonce;
    for (call_index, call) in signed_calls.iter().enumerate() {
      let extrinsic = actors_integration_tests::signed_extrinsic(signer, next_nonce, call.clone());
      Executive::apply_extrinsic(extrinsic.clone())
        .unwrap_or_else(|error| panic!("valid scripted call {call_index} rejected: {error:?}"))
        .unwrap_or_else(|error| {
          panic!("valid scripted call {call_index} failed dispatch: {error:?}")
        });
      extrinsics.push(extrinsic);
      next_nonce = next_nonce.saturating_add(1);
    }
    let (demand_calls, next_user_weight) = match demand {
      UserDemand::ActorOnly => (0, None),
      UserDemand::ContinuousValid => {
        let (calls, next) =
          saturate_user_dispatch(signer, block_number, next_nonce, &mut extrinsics);
        (calls, Some(next))
      }
      UserDemand::RefTimeHeavy => {
        let (calls, next) = saturate_user_dispatch_with_router_swaps(
          signer,
          block_number,
          next_nonce,
          &mut extrinsics,
        );
        (calls, Some(next))
      }
    };
    let user_calls = signed_call_count.saturating_add(demand_calls);
    let block = Block {
      header: Executive::finalize_block(),
      extrinsics,
    };
    let metrics = authored_metrics(
      workload_profiles,
      user_calls,
      next_user_weight,
      prepass_steps,
      prepass_trigger_occurrences,
      prepass_usage.actor_control_used(),
      prepass_usage.actor_effect_used(),
    );
    let post_state = current_top_storage();
    (block, metrics, post_state)
  });
  AuthoredBlock {
    pre_state,
    post_state,
    block,
    inherent_data,
    metrics,
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

#[derive(Debug, Eq, PartialEq)]
struct StorageOwnerProof {
  key_paths: usize,
  node_bytes: usize,
  exclusive_node_bytes: usize,
  shared_node_bytes: usize,
}

fn runtime_proof_storage_info() -> Vec<StorageInfo> {
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
  let effect_per_step = W::task_transfer();
  let effect_io_per_step = DatabaseIo::new(20, 6);
  assert_production_weight_component(effect_per_step, 363_533_000, 6_196, effect_io_per_step);
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
  for (label, authored, legacy_extra_probes) in profiles {
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
    let effect_io = effect_io_per_step.saturating_mul(steps);
    rows.push(serde_json::json!({
      "profile": label, "committedSteps": steps,
      "retiredLegacyExtraProbeAssumption": legacy_extra_probes,
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
      UserDemand::RefTimeHeavy => {
        unreachable!("the W1/W2 schedule campaign does not prepare Router demand")
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
      UserDemand::RefTimeHeavy => {
        unreachable!("the W4 campaign does not prepare signed Router demand")
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

#[test]
fn full_executive_user_completion_header_domain_witness() {
  use pallet_deos_actors::{
    ActorContract, CompletionPolicy, FundingSourcePolicy, Mutability, StepControlWeightContext,
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
          completion: CompletionPolicy::Persistent,
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
        CloseReason::ProductiveCycleCompleted,
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
      CloseReason::ProductiveCycleCompleted,
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
  let mut trigger_counts = [0u32; 4];
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
    paused_events.extend(authored.metrics.paused_actors.iter().copied());
    resumed_events.extend(authored.metrics.resumed_actors.iter().copied());
    closed_events.extend(authored.metrics.closed_actors.iter().copied());
    for (_actor_id, family) in &authored.metrics.trigger_occurrences {
      let index = match family {
        TriggerFamily::Manual => 0,
        TriggerFamily::AddressEvent => 1,
        TriggerFamily::AtTime => 2,
        TriggerFamily::Cadenced => 3,
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
        .is_some_and(|block| *block >= expected_start),
      "block-clock W6 Actor cannot progress before its sparse window"
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
      "roles": {"denseManual": fixture.dense_manual, "windowedManual": fixture.sparse_block,
        "atTime": fixture.sparse_tick, "cadenced": fixture.randomized_cadenced,
        "cadencedPeriodsTicks": fixture.randomized_periods,
        "paused": fixture.paused_actor, "closed": fixture.closed_actor},
      "fixtureSetup": setup, "finalizedBlocks": timeline,
    })
  );
  println!(
    "EXP_0066_W6_V2 {{\"seed\":{W6_RANDOM_SEED},\"measuredBlocks\":{},\"workloadActors\":{},\"denseManualActors\":{},\"sparseBlockClockActors\":{},\"atTimeDeadlineActors\":{},\"randomizedCadencedActors\":{},\"randomizedCadencePeriodsTicks\":{:?},\"distinctProgressedActors\":{},\"totalSteps\":{},\"pausedEvents\":{},\"resumedEvents\":{},\"ownerCloseEvents\":{},\"atTimeOccurrences\":{},\"cadencedOccurrences\":{},\"controlRefTimeMin\":{},\"controlRefTimeP50\":{},\"controlRefTimeMax\":{},\"controlProofSizeMin\":{},\"controlProofSizeP50\":{},\"controlProofSizeMax\":{},\"effectRefTimeMin\":{},\"effectRefTimeP50\":{},\"effectRefTimeMax\":{},\"effectProofSizeMin\":{},\"effectProofSizeP50\":{},\"effectProofSizeMax\":{},\"executionStorageProofMin\":{},\"executionStorageProofMax\":{},\"executionCompactProofMin\":{},\"executionCompactProofMax\":{},\"verificationStorageProofMin\":{},\"verificationStorageProofMax\":{},\"verificationCompactProofMin\":{},\"verificationCompactProofMax\":{}}}",
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
    assert!(fixture.future.iter().all(|actor_id| {
      Actors::service_nodes(*actor_id).is_some_and(|node| node.eligible_from == 2)
    }));
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
      "W8 preserves Q1 under every tombstone-prefix profile"
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
      "W8 service remains the exact live FIFO prefix after tombstone reclamation"
    );
    let mut post = TestExternalities::new_with_code_and_state(
      wasm,
      authored.post_state.clone(),
      crate::VERSION.state_version(),
    );
    post.execute_with(|| {
      let service = Actors::service_header();
      assert_eq!(service.count, W8_DUE_ACTORS);
      assert_eq!(
        service.cursor.map(|actor| actor.actor_id),
        fixture.due.get(progressed.len()).copied(),
        "W8 cursor advances to the first unserved live Actor"
      );
      assert!(
        fixture
          .due
          .iter()
          .all(|actor_id| Actors::service_nodes(*actor_id).is_some())
      );
    });
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
        &format!("W8-tombstone-prefix-{tombstone_prefix}"),
      )
    });
    let proof = proof.as_ref();
    println!(
      "EXP_0066_W8_V1 {{\"closedPrefix\":{tombstone_prefix},\"dueActors\":{},\"dueSteps\":{},\"distinctProgressedActors\":{},\"dueActorsRemaining\":{},\"serviceResidents\":{},\"fifoPrefixPreserved\":true,\"q1Preserved\":true,\"controlBound\":true,\"nextControlMaximumRefTime\":{},\"nextControlMaximumProofSize\":{},\"controlRemainingRefTime\":{},\"controlRemainingProofSize\":{},\"controlRefTime\":{},\"controlProofSize\":{},\"effectRefTime\":{},\"effectProofSize\":{},\"executionStorageProofBytes\":{},\"executionCompactProofBytes\":{},\"verificationStorageProofBytes\":{},\"verificationCompactProofBytes\":{}}}",
      fixture.due.len(),
      authored.metrics.actor_steps,
      authored.metrics.distinct_actors,
      fixture
        .due
        .len()
        .saturating_sub(authored.metrics.actor_steps as usize),
      W8_DUE_ACTORS,
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
    let mut post = TestExternalities::new_with_code_and_state(
      wasm,
      authored.post_state.clone(),
      crate::VERSION.state_version(),
    );
    post.execute_with(|| {
      let service = Actors::service_header();
      assert!(service.count >= W9_DUE_ACTORS);
      assert_eq!(
        service.cursor.map(|actor| actor.actor_id),
        fixture.actor_ids.get(progressed.len()).copied(),
        "W9 canonical Service cursor advances to the first unserved Actor"
      );
      assert!(
        fixture
          .actor_ids
          .iter()
          .all(|actor_id| Actors::service_nodes(*actor_id).is_some())
      );
      assert!(
        fixture.actor_ids[progressed.len()..]
          .iter()
          .all(|actor_id| {
            Actors::service_nodes(*actor_id).is_some_and(|node| node.eligible_from == 2)
          })
      );
    });

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
        assert!(
          proof_bound,
          "retained W9 valid demand must exhaust User ProofSize"
        );
        assert!(
          !ref_time_bound,
          "W9 fallback must not mislabel a ProofSize frontier as RefTime saturation"
        );
        (next_user, remaining, proof_bound, ref_time_bound)
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
    "EXP_0066_W9_COMPARISON_V1 {{\"actorPrefixesCanonical\":true,\"actorServiceSurvivesEveryUserProfile\":true,\"proofSaturatedUserRefTime\":{},\"refTimeHeavyUserRefTime\":{},\"proofSaturatedUserProofSize\":{},\"refTimeHeavyUserProofSize\":{},\"userProofSizeDelta\":{},\"refTimeFrontierFallback\":\"highest-valid-business-call-profile-remains-proof-size-bound\"}}",
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
      "the captured FIFO cutoff defers every newly materialized Actor to a later block"
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
  assert_eq!(manual.steps, vec![15, 16, 16, 16, 16, 16, 5, 0, 0]);
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
fn reference_full_block_fixture_replays_through_executive() {
  let (storage, block, data) = author_reference_first_block(&[]);
  let authored = AuthoredBlock {
    pre_state: storage.clone(),
    post_state: storage,
    block,
    inherent_data: data,
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
      prepass_steps: 0,
      prepass_trigger_occurrences: 0,
      prepass_actor_control: Weight::zero(),
      prepass_actor_effect: Weight::zero(),
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
      assert_eq!(contract.header.completion, CompletionPolicy::Persistent);
      assert!(contract.header.window.is_none());
      assert!(contract.header.auto_close_at_cycle_nonce.is_none());
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
        assert_eq!(schedule, WorkloadSchedule::CadencedOnly);
        assert_eq!(header.count, 0);
        assert_eq!(authored.metrics.actor_steps, 0);
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
