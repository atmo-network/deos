//! One native complete-Prepass consumer. Instrumented wall time and proof increments are not prices.

use super::common;
use crate::{Actors, Balances, Runtime, RuntimeOrigin, System};
use codec::Encode;
use pallet_deos_actors::{
  ActorContract, ActorContractHeads, AmountResolution, AssetFilter, AssetOps, BenchmarkHelper,
  Config, ContractSteps, CycleState, FundingSourcePolicy, Mutability, OnUnfunded, SourceFilter,
  Step, StepErrorPolicy, Task, Trigger,
  benchmark_observation::{self, Phase},
};
use polkadot_sdk::{
  frame_support::{
    assert_ok,
    traits::{Currency, Get, Hooks},
  },
  sp_core::{
    Blake2Hasher, H256, Hasher,
    storage::{ChildInfo, Storage, TrackedStorageKey},
  },
  sp_externalities::{Extension, ExtensionStore, Externalities, MultiRemovalResults},
  sp_runtime::{StateVersion, traits::BlakeTwo256},
  sp_state_machine::{Backend, Ext, OverlayedChanges},
  sp_trie::{
    EMPTY_PREFIX, HashDBT, LayoutV1, MemoryDB, StorageProof, Trie, TrieConfiguration,
    TrieDBBuilder, delta_trie_root,
  },
};
use std::{
  any::{Any, TypeId},
  cell::RefCell,
  collections::{BTreeMap, BTreeSet},
  time::Instant,
};

type State = BTreeMap<Vec<u8>, Vec<u8>>;
type DbCount = (u32, u32, u32, u32);
type Host = sc_client_db::BenchmarkingState<BlakeTwo256>;
type Assets = <Runtime as Config>::AssetOps;
type Helper = <Runtime as Config>::BenchmarkHelper;
type Asset = <Runtime as Config>::AssetId;
type Layout = LayoutV1<Blake2Hasher>;

#[path = "actor_service_observation/admitted_source.rs"]
mod admitted_source;

#[path = "actor_service_observation/calibration.rs"]
mod calibration;

#[derive(Clone)]
struct Fixture {
  actor: u64,
  payer: crate::AccountId,
  lp: Asset,
  a: Asset,
  b: Asset,
  amount: crate::Balance,
  spend: crate::Balance,
  sink: crate::AccountId,
  native: Asset,
  sink_before: crate::Balance,
  pipeline_fee: crate::Balance,
  inspection_predecessors: Option<u32>,
  paid_prefix: Option<u64>,
}

#[derive(Debug)]
struct Operation {
  phase: Option<Phase>,
  method: &'static str,
  key: Vec<u8>,
  overlay: bool,
  observer: bool,
  value: Option<Vec<u8>>,
  db: DbCount,
}

struct Observation {
  enabled: bool,
  recording: bool,
  diagnostic: bool,
  refuse: bool,
  fault_applied: bool,
  fixture: Fixture,
  recorder: polkadot_sdk::sp_trie::recorder::Recorder<BlakeTwo256>,
  stack: Vec<Phase>,
  markers: Vec<(Phase, bool)>,
  last: Instant,
  wall_ns: BTreeMap<Option<Phase>, u128>,
  first_seen: BTreeMap<Option<Phase>, BTreeSet<Vec<u8>>>,
  seen: BTreeSet<Vec<u8>>,
  operations: Vec<Operation>,
  transactions: Vec<(&'static str, usize, Option<Phase>)>,
  after_task: Option<(crate::Balance, crate::Balance, crate::Balance)>,
  refusal_roots: [Option<H256>; 2],
}

std::thread_local! {
  static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}

fn state<R>(run: impl FnOnce(&mut Observation) -> R) -> R {
  ACTIVE.with_borrow_mut(|active| run(active.as_mut().expect("active physical consumer")))
}

fn snapshot(observation: &mut Observation) {
  if !observation.enabled {
    return;
  }
  let phase = observation.stack.last().copied();
  let now = Instant::now();
  *observation.wall_ns.entry(phase).or_default() += now.duration_since(observation.last).as_nanos();
  observation.last = now;
  for node in observation.recorder.to_storage_proof().iter_nodes() {
    if observation.seen.insert(node.clone()) {
      observation
        .first_seen
        .entry(phase)
        .or_default()
        .insert(node.clone());
    }
  }
}

fn notice(phase: Phase, entering: bool) {
  let (probe, root_probe) = state(|observation| {
    snapshot(observation);
    observation.markers.push((phase, entering));
    if entering {
      observation.stack.push(phase);
    } else {
      assert_eq!(observation.stack.pop(), Some(phase));
    }
    if let Some(predecessors) = observation.fixture.inspection_predecessors {
      let encounters = observation
        .markers
        .iter()
        .filter(|marker| **marker == (Phase::Encounter, true))
        .count();
      if encounters
        == predecessors as usize + 1 + usize::from(observation.fixture.paid_prefix.is_some())
      {
        let root_probe = if phase == Phase::Encounter && entering {
          Some(0)
        } else if phase == Phase::Encounter && !entering {
          Some(1)
        } else {
          None
        };
        if root_probe.is_some() {
          observation.diagnostic = true;
        }
        return (None, root_probe);
      }
      return (None, None);
    }
    if phase == Phase::Collection && entering {
      assert!(observation.markers.contains(&(Phase::Task, false)));
      assert!(observation.markers.contains(&(Phase::Retirement, false)));
      observation.diagnostic = true;
      (
        Some((observation.fixture.clone(), observation.refuse)),
        None,
      )
    } else {
      (None, None)
    }
  });
  if let Some(index) = root_probe {
    let root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1));
    state(|observation| {
      observation.refusal_roots[index] = Some(root);
      observation.diagnostic = false;
    });
  }
  if let Some((fixture, refuse)) = probe {
    // Explicit observer probes/fault, tagged separately from production operations.
    let balances = (
      Assets::balance(&fixture.payer, fixture.lp),
      Assets::balance(&fixture.payer, fixture.a),
      Assets::balance(&fixture.payer, fixture.b),
    );
    if refuse {
      // Keep the native provider alive; setting zero can be a no-op with live asset consumers.
      drop(
        <Balances as Currency<crate::AccountId>>::make_free_balance_be(
          &fixture.payer,
          <Balances as Currency<crate::AccountId>>::minimum_balance(),
        ),
      );
      assert!(
        Assets::balance(&fixture.payer, fixture.native)
          < <Runtime as Config>::MinUserBalance::get(),
        "late fault must actually remove fee capacity"
      );
    }
    state(|observation| {
      observation.after_task = Some(balances);
      observation.fault_applied = refuse;
      observation.diagnostic = false;
    });
  }
}

/// Delegates the exact SDK externalities; the journal observes lineage, never drives execution.
struct HostRecorder<'a> {
  inner: &'a mut dyn Externalities,
  original: &'a State,
  written: BTreeSet<Vec<u8>>,
  savepoints: Vec<BTreeSet<Vec<u8>>>,
}

impl HostRecorder<'_> {
  fn record(&self, method: &'static str, key: &[u8], value: Option<Vec<u8>>, before: DbCount) {
    let after = self.inner.read_write_count();
    let db = (
      after.0 - before.0,
      after.1 - before.1,
      after.2 - before.2,
      after.3 - before.3,
    );
    state(|observation| {
      if observation.enabled && observation.recording {
        let population = observation
          .fixture
          .inspection_predecessors
          .map_or(1, |n| n as usize + 1);
        assert!(observation.operations.len() < 30_000 * population);
        let overlay = self.written.contains(key);
        if !overlay && method == "get" {
          assert_eq!(
            value.as_ref(),
            self.original.get(key),
            "original/overlay lineage mismatch"
          );
        }
        observation.operations.push(Operation {
          phase: observation.stack.last().copied(),
          method,
          key: key.to_vec(),
          overlay,
          observer: observation.diagnostic,
          value,
          db,
        });
      }
    });
  }

  fn transaction(&self, method: &'static str) {
    state(|observation| {
      if observation.enabled && observation.recording {
        observation.transactions.push((
          method,
          self.savepoints.len(),
          observation.stack.last().copied(),
        ));
      }
    });
  }
}

impl ExtensionStore for HostRecorder<'_> {
  fn extension_by_type_id(&mut self, id: TypeId) -> Option<&mut dyn Any> {
    self.inner.extension_by_type_id(id)
  }
  fn register_extension_with_type_id(
    &mut self,
    id: TypeId,
    ext: Box<dyn Extension>,
  ) -> Result<(), polkadot_sdk::sp_externalities::Error> {
    self.inner.register_extension_with_type_id(id, ext)
  }
  fn deregister_extension_by_type_id(
    &mut self,
    id: TypeId,
  ) -> Result<(), polkadot_sdk::sp_externalities::Error> {
    self.inner.deregister_extension_by_type_id(id)
  }
}

impl Externalities for HostRecorder<'_> {
  fn storage(&mut self, key: &[u8]) -> Option<Vec<u8>> {
    let before = self.inner.read_write_count();
    let value = self.inner.storage(key);
    self.record("get", key, value.clone(), before);
    value
  }
  fn storage_hash(&mut self, key: &[u8]) -> Option<Vec<u8>> {
    let before = self.inner.read_write_count();
    let value = self.inner.storage_hash(key);
    self.record("hash", key, value.clone(), before);
    value
  }
  fn exists_storage(&mut self, key: &[u8]) -> bool {
    let before = self.inner.read_write_count();
    let value = self.inner.exists_storage(key);
    self.record("exists", key, Some(vec![u8::from(value)]), before);
    value
  }
  fn place_storage(&mut self, key: Vec<u8>, value: Option<Vec<u8>>) {
    let before = self.inner.read_write_count();
    self.inner.place_storage(key.clone(), value.clone());
    self.record("put", &key, value, before);
    self.written.insert(key);
  }
  fn storage_append(&mut self, key: Vec<u8>, value: Vec<u8>) {
    let before = self.inner.read_write_count();
    self.inner.storage_append(key.clone(), value.clone());
    self.record("append", &key, Some(value), before);
    self.written.insert(key);
  }
  fn storage_root(&mut self, version: StateVersion) -> Vec<u8> {
    let before = self.inner.read_write_count();
    let root = self.inner.storage_root(version);
    self.record("root", &[], Some(root.clone()), before);
    root
  }
  fn storage_start_transaction(&mut self) {
    self.inner.storage_start_transaction();
    self.savepoints.push(self.written.clone());
    self.transaction("start");
  }
  fn storage_rollback_transaction(&mut self) -> Result<(), ()> {
    self.inner.storage_rollback_transaction()?;
    self.written = self
      .savepoints
      .pop()
      .expect("observed SDK rollback has matching start");
    self.transaction("rollback");
    Ok(())
  }
  fn storage_commit_transaction(&mut self) -> Result<(), ()> {
    self.inner.storage_commit_transaction()?;
    self
      .savepoints
      .pop()
      .expect("observed SDK commit has matching start");
    self.transaction("commit");
    Ok(())
  }
  fn next_storage_key(&mut self, key: &[u8]) -> Option<Vec<u8>> {
    let before = self.inner.read_write_count();
    let value = self.inner.next_storage_key(key);
    self.record("next", key, value.clone(), before);
    value
  }
  fn next_child_storage_key(&mut self, _: &ChildInfo, _: &[u8]) -> Option<Vec<u8>> {
    panic!("child storage outside selected consumer")
  }
  fn child_storage_hash(&mut self, _: &ChildInfo, _: &[u8]) -> Option<Vec<u8>> {
    panic!("child storage outside selected consumer")
  }
  fn child_storage(&mut self, _: &ChildInfo, _: &[u8]) -> Option<Vec<u8>> {
    panic!("child storage outside selected consumer")
  }
  fn exists_child_storage(&mut self, _: &ChildInfo, _: &[u8]) -> bool {
    panic!("child storage outside selected consumer")
  }
  fn place_child_storage(&mut self, _: &ChildInfo, _: Vec<u8>, _: Option<Vec<u8>>) {
    panic!("child storage outside selected consumer")
  }
  fn child_storage_root(&mut self, _: &ChildInfo, _: StateVersion) -> Vec<u8> {
    panic!("child storage outside selected consumer")
  }
  fn kill_child_storage(
    &mut self,
    _: &ChildInfo,
    _: Option<u32>,
    _: Option<&[u8]>,
  ) -> MultiRemovalResults {
    panic!("child storage outside selected consumer")
  }
  fn clear_prefix(
    &mut self,
    prefix: &[u8],
    limit: Option<u32>,
    cursor: Option<&[u8]>,
  ) -> MultiRemovalResults {
    // Full Executive clears the empty EventTopics namespace. Broader removal stays unqualified.
    assert!(!self.original.keys().any(|key| key.starts_with(prefix)));
    assert!(!self.written.iter().any(|key| key.starts_with(prefix)));
    let before = self.inner.read_write_count();
    let result = self.inner.clear_prefix(prefix, limit, cursor);
    self.record("clear-empty-prefix", prefix, Some(result.encode()), before);
    assert_eq!(result.backend, 0);
    assert_eq!(result.unique, 0);
    assert!(result.maybe_cursor.is_none());
    result
  }
  fn clear_child_prefix(
    &mut self,
    _: &ChildInfo,
    _: &[u8],
    _: Option<u32>,
    _: Option<&[u8]>,
  ) -> MultiRemovalResults {
    panic!("child storage outside selected consumer")
  }
  fn set_offchain_storage(&mut self, _: &[u8], _: Option<&[u8]>) {
    panic!("offchain storage outside selected consumer")
  }
  fn wipe(&mut self) {
    panic!("no wipe inside physical consumer")
  }
  fn commit(&mut self) {
    panic!("no intermediate commit inside physical consumer")
  }
  fn read_write_count(&self) -> DbCount {
    self.inner.read_write_count()
  }
  fn reset_read_write_count(&mut self) {
    panic!("no counter reset inside physical consumer")
  }
  fn get_whitelist(&self) -> Vec<TrackedStorageKey> {
    self.inner.get_whitelist()
  }
  fn set_whitelist(&mut self, _: Vec<TrackedStorageKey>) {
    panic!("no whitelist mutation inside physical consumer")
  }
  fn proof_size(&self) -> Option<u32> {
    self.inner.proof_size()
  }
  fn get_read_and_written_keys(&self) -> Vec<(Vec<u8>, u32, u32, bool)> {
    self.inner.get_read_and_written_keys()
  }
}

fn prepare() -> (Storage, Fixture) {
  let mut source = common::synthetic_actor_test_ext();
  let fixture = source.execute_with(|| {
    assert_ok!(Helper::advance_to_scheduler_tick(12));
    <Actors as Hooks<crate::BlockNumber>>::on_initialize(System::block_number());
    assert_ok!(Helper::prepare_actor_prepass_context());
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    System::note_finished_initialize();
    let owner = common::ALICE;
    let (lp, a, b, amount) = Helper::setup_remove_liquidity_consumer_outputs(&owner)
      .unwrap()
      .unwrap();
    let spend = amount / 2;
    let steps: ContractSteps<Runtime> = vec![Step {
      precondition: None,
      task: Task::RemoveLiquidity {
        lp_asset: lp,
        asset_a: a,
        asset_b: b,
        lp_amount: AmountResolution::Fixed(spend),
        min_amount_a: 1,
        min_amount_b: 1,
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    }]
    .try_into()
    .unwrap();
    super::actors_integration_tests::prefund_active_user_creation(&owner, &steps);
    let actor = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(owner.clone()),
      Mutability::Mutable,
      Some(ActorContract {
        trigger: Trigger::AddressEvent {
          source_filter: SourceFilter::Any,
          asset_filter: AssetFilter::Any
        },
        cooldown_blocks: 3,
        window: None,
        funding: FundingSourcePolicy::SignedAllowlist(
          vec![owner.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .try_into()
            .unwrap()
        ),
        on_unfunded: OnUnfunded::Close,
        close_when: None,
        parked_balance_activation: None,
        auto_close_at_cycle_nonce: Some(2),
        steps,
      })
    ));
    let payer = Actors::active_actor_state(actor)
      .unwrap()
      .identity
      .sovereign_account;
    drop(<Balances as Currency<crate::AccountId>>::deposit_creating(
      &payer,
      common::INITIAL_BALANCE,
    ));
    let pipeline_fee = Actors::actor_cost_quote(actor)
      .unwrap()
      .prospective_pipeline_fee
      .unwrap()
      .total_fee;
    assert_ok!(Helper::transfer_signed(&owner, &payer, lp, amount));
    assert_eq!(
      Actors::service_nodes(actor).unwrap().eligible_from,
      System::block_number() + 1
    );
    System::note_finished_extrinsics();
    <Actors as Hooks<crate::BlockNumber>>::on_idle(
      System::block_number(),
      <Runtime as Config>::ActorOnIdleReserve::get(),
    );
    <Actors as Hooks<crate::BlockNumber>>::on_finalize(System::block_number());
    assert_ok!(Helper::finalize_scheduler_clock());
    assert_ok!(Helper::advance_to_scheduler_tick(24));
    <Actors as Hooks<crate::BlockNumber>>::on_initialize(System::block_number());
    assert_ok!(Helper::prepare_actor_prepass_context());
    assert_eq!(Actors::service_header().count, 1);
    let sink = <Runtime as Config>::FeeSink::get();
    let native = <Runtime as Config>::FeeNativeAssetId::get();
    Fixture {
      actor,
      payer,
      lp,
      a,
      b,
      amount,
      spend,
      sink_before: Assets::balance(&sink, native),
      sink,
      native,
      pipeline_fee,
      inspection_predecessors: None,
      paid_prefix: None,
    }
  });
  (common::committed_fixture_storage(source), fixture)
}

fn prepare_inspection_refusal() -> (Storage, Fixture) {
  prepare_inspection_refusal_at_cut(false)
}

fn prepare_inspection_refusal_at_cut(before_initialize: bool) -> (Storage, Fixture) {
  use pallet_deos_actors::TaskEffectWeightProvider;
  let mut source = common::synthetic_actor_test_ext();
  let fixture = source.execute_with(|| {
    let enter_block = |tick| {
      assert_ok!(Helper::advance_to_scheduler_tick(tick));
      <Actors as Hooks<crate::BlockNumber>>::on_initialize(System::block_number());
      assert_ok!(Helper::prepare_actor_prepass_context());
    };
    let finish_block = || {
      System::note_finished_extrinsics();
      <Actors as Hooks<crate::BlockNumber>>::on_idle(System::block_number(), <Runtime as Config>::ActorOnIdleReserve::get());
      <Actors as Hooks<crate::BlockNumber>>::on_finalize(System::block_number());
      assert_ok!(Helper::finalize_scheduler_clock());
    };
    let mut tick = 12;
    enter_block(tick);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    System::note_finished_initialize();
    let owner = common::ALICE;
    let (lp, a, b, _) = Helper::setup_remove_liquidity_consumer_outputs(&owner).unwrap().unwrap();
    let amount = 1_000_000u128;
    let task = Task::AddLiquidity { asset_a: a, asset_b: b, amount_a: AmountResolution::Fixed(amount), amount_b: AmountResolution::Fixed(amount), min_lp_out: 1 };
    let effect = <Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&task).unwrap();
    let base = <Runtime as Config>::BlockResourceBudget::get().limits().actor_base_turn();
    assert!(effect.ref_time() > 0 && effect.proof_size() > 0);
    let predecessors = (base.ref_time() / effect.ref_time()).min(base.proof_size() / effect.proof_size());
    assert!((1..=128).contains(&predecessors), "finite public source exceeds selected population: {predecessors}");
    let funding = amount + 2;
    for asset in [a, b] {
      assert_ok!(Assets::mint(&owner, asset, funding.checked_mul(u128::from(predecessors + 2)).unwrap()));
    }
    let create = |steps: ContractSteps<Runtime>, system: bool| {
      if !system {
        super::actors_integration_tests::prefund_active_user_creation(&owner, &steps);
      }
      let actor = Actors::next_actor_id();
      let contract = Some(ActorContract {
        trigger: Trigger::AddressEvent { source_filter: SourceFilter::Any, asset_filter: AssetFilter::Any },
        cooldown_blocks: 3, window: None,
        funding: FundingSourcePolicy::SignedAllowlist(vec![owner.clone()].into_iter().collect::<BTreeSet<_>>().try_into().unwrap()),
        on_unfunded: OnUnfunded::Close, close_when: None, parked_balance_activation: None,
        auto_close_at_cycle_nonce: Some(2), steps,
      });
      if system {
        assert_ok!(Actors::create_system_actor(RuntimeOrigin::root(), owner.clone(), Mutability::Mutable, contract));
      } else {
        assert_ok!(Actors::create_user_actor(RuntimeOrigin::signed(owner.clone()), Mutability::Mutable, contract));
      }
      let payer = Actors::active_actor_state(actor).unwrap().identity.sovereign_account;
      drop(<Balances as Currency<crate::AccountId>>::deposit_creating(&payer, common::INITIAL_BALANCE));
      for asset in [a, b] {
        assert_ok!(Helper::transfer_signed(&owner, &payer, asset, funding));
      }
      assert_eq!(Actors::service_nodes(actor).unwrap().eligible_from, System::block_number() + 1);
      actor
    };
    let transfer = Step { precondition: None, task: Task::Transfer { to: common::BOB, asset: a, amount: AmountResolution::Fixed(1) }, on_error: StepErrorPolicy::AbortCycle };
    let effect_step = Step { precondition: None, task, on_error: StepErrorPolicy::RetryLater { max_attempts: 2 } };
    let mut peers = Vec::new();
    // Public System pause stages real Running continuations one at a time. No User pause,
    // resource-state injection, priority, synthetic Run or scheduler replacement is used.
    for _ in 0..predecessors {
      let actor = create(vec![transfer.clone(), effect_step.clone()].try_into().unwrap(), true);
      finish_block();
      tick += 12;
      enter_block(tick);
      assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
      System::note_finished_initialize();
      assert_eq!(Actors::active_actor_state(actor).unwrap().run_state.unwrap().cursor, 1);
      assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor));
      peers.push(actor);
    }
    let paid_prefix = create(vec![transfer, Step { precondition: None, task: Task::StopCycle, on_error: StepErrorPolicy::AbortCycle }].try_into().unwrap(), false);
    finish_block();
    tick += 12;
    enter_block(tick);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    System::note_finished_initialize();
    let paid = Actors::active_actor_state(paid_prefix).unwrap();
    assert_eq!(paid.hot.cycle_state, CycleState::Running);
    let run = paid.run_state.unwrap();
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(System::block_number()));
    for peer in &peers {
      assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), *peer));
      assert_eq!(Actors::active_actor_state(*peer).unwrap().run_state.unwrap().cursor, 1);
    }
    let actor = create(vec![effect_step].try_into().unwrap(), false);
    finish_block();
    tick += 12;
    if !before_initialize {
      enter_block(tick);
    }
    let payer = Actors::active_actor_state(actor).unwrap().identity.sovereign_account;
    assert_eq!(Actors::service_header().count as u64, predecessors + 2);
    assert_eq!(Actors::active_actor_state(paid_prefix).unwrap().run_state.unwrap().cursor, 1);
    let sink = <Runtime as Config>::FeeSink::get();
    let native = <Runtime as Config>::FeeNativeAssetId::get();
    println!("ACTOR_I_PUBLIC_SOURCE predecessors={predecessors} base={base:?} step_effect={effect:?} paid_prefix={paid_prefix} ledger_injection=false");
    Fixture { actor, payer, lp, a, b, amount: funding, spend: 1,
      sink_before: Assets::balance(&sink, native), sink, native, pipeline_fee: 0,
      inspection_predecessors: Some(predecessors as u32), paid_prefix: Some(paid_prefix) }
  });
  (common::committed_fixture_storage(source), fixture)
}

fn replay(
  parent_root: H256,
  proof: &StorageProof,
  parent: &State,
  actual: &State,
  operations: &[Operation],
) -> Result<H256, &'static str> {
  replay_db(
    parent_root,
    proof.to_memory_db::<Blake2Hasher>(),
    parent,
    actual,
    operations,
  )
}

fn replay_db(
  parent_root: H256,
  mut db: MemoryDB<Blake2Hasher>,
  parent: &State,
  actual: &State,
  operations: &[Operation],
) -> Result<H256, &'static str> {
  {
    let trie = TrieDBBuilder::<Layout>::new(&db, &parent_root).build();
    for op in operations.iter().filter(|op| !op.overlay && !op.observer) {
      if op.method == "get" && trie.get(&op.key).map_err(|_| "missing original read")? != op.value {
        return Err("wrong original read");
      }
      if op.method == "exists"
        && trie
          .get_hash(&op.key)
          .map_err(|_| "missing original path")?
          .is_some()
          != (op.value.as_ref().unwrap()[0] != 0)
      {
        return Err("wrong original existence");
      }
      if op.method == "hash"
        && trie
          .get_hash(&op.key)
          .map_err(|_| "missing original hash")?
          .map(|hash| hash.as_bytes().to_vec())
          != op.value
      {
        return Err("wrong original hash");
      }
    }
  }
  let delta = parent
    .keys()
    .chain(actual.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|key| parent.get(*key) != actual.get(*key))
    .map(|key| (key.clone(), actual.get(key).cloned()))
    .collect::<Vec<_>>();
  delta_trie_root::<Layout, _, _, _, _, _>(&mut db, parent_root, delta, None, None)
    .map_err(|_| "missing repair input")
}

struct ResultRow {
  top: State,
  root: H256,
  proof: StorageProof,
  body_proof: StorageProof,
  db: DbCount,
  db_after_commit: DbCount,
  elapsed_ns: u128,
  root_ns: u128,
  commit_ns: u128,
  capture_ns: u128,
  diagnostic_callback: bool,
  observation: Observation,
}

fn run(parent: &Storage, fixture: &Fixture, observe: bool, refuse: bool) -> ResultRow {
  run_call(parent, fixture, observe, refuse, notice, false, || {
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
  })
}

fn run_call(
  parent: &Storage,
  fixture: &Fixture,
  observe: bool,
  refuse: bool,
  observer: fn(Phase, bool),
  record_proof_size: bool,
  runtime_call: impl FnOnce(),
) -> ResultRow {
  run_call_captured(
    parent,
    fixture,
    observe,
    refuse,
    Some(observer),
    record_proof_size,
    runtime_call,
  )
}

fn checked_capture_interval_ps(ns: u128) -> Result<u64, &'static str> {
  if ns == 0 {
    return Err("zero clock interval");
  }
  let ps = ns.checked_mul(1000).ok_or("clock interval ps overflow")?;
  u64::try_from(ps).map_err(|_| "clock interval exceeds RefTime range")
}

fn checked_capture_interval_ns(start: Instant, end: Instant) -> Result<u128, &'static str> {
  let ns = end
    .checked_duration_since(start)
    .ok_or("monotonic clock moved backward")?
    .as_nanos();
  checked_capture_interval_ps(ns)?;
  Ok(ns)
}

fn run_call_captured(
  parent: &Storage,
  fixture: &Fixture,
  observe: bool,
  refuse: bool,
  observer: Option<fn(Phase, bool)>,
  record_proof_size: bool,
  runtime_call: impl FnOnce(),
) -> ResultRow {
  assert!(
    observer.is_some() || (!observe && !refuse),
    "no-callback capture cannot journal or inject a diagnostic fault"
  );
  if observer.is_none() {
    benchmark_observation::assert_unobserved();
  }
  assert!(parent.children_default.is_empty());
  let host = Host::new(parent.clone(), None, true, true).unwrap();
  let recorder = host.recorder().unwrap();
  assert!(recorder.to_storage_proof().is_empty());
  assert_eq!(host.read_write_count(), (0, 0, 0, 0));
  ACTIVE.with_borrow_mut(|active| {
    assert!(active.is_none());
    *active = Some(Observation {
      enabled: observe,
      recording: true,
      diagnostic: false,
      refuse,
      fault_applied: false,
      fixture: fixture.clone(),
      recorder: recorder.clone(),
      stack: Vec::new(),
      markers: Vec::new(),
      last: Instant::now(),
      wall_ns: BTreeMap::new(),
      first_seen: BTreeMap::new(),
      seen: BTreeSet::new(),
      operations: Vec::new(),
      transactions: Vec::new(),
      after_task: None,
      refusal_roots: [None; 2],
    });
  });
  let mut overlay = OverlayedChanges::default();
  let mut extensions = polkadot_sdk::sp_externalities::Extensions::default();
  if record_proof_size {
    use polkadot_sdk::sp_trie::proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider};
    extensions.register(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
  }
  let mut ext = Ext::new(
    &mut overlay,
    &host,
    record_proof_size.then_some(&mut extensions),
  );
  let mut capture_start = None;
  let (elapsed_ns, root_ns, root, body_proof) = {
    let mut adapter = HostRecorder {
      inner: &mut ext,
      original: &parent.top,
      written: BTreeSet::new(),
      savepoints: Vec::new(),
    };
    let call = || {
      let start = Instant::now();
      capture_start = Some(start);
      if let Some(observer) = observer {
        state(|observation| observation.last = start);
        benchmark_observation::with_observer(observer, runtime_call);
      } else {
        runtime_call();
      }
      let elapsed = checked_capture_interval_ns(start, Instant::now())
        .expect("invalid caller-body clock interval");
      state(|observation| {
        if observer.is_some() {
          snapshot(observation);
        }
        assert!(observation.stack.is_empty());
        observation.recording = false;
      });
      let body = recorder.to_storage_proof();
      let start = Instant::now();
      let root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1));
      let root_ns = checked_capture_interval_ns(start, Instant::now())
        .expect("invalid post-call root clock interval");
      (elapsed, root_ns, root, body)
    };
    if observe {
      let result =
        polkadot_sdk::sp_externalities::set_and_run_with_externalities(&mut adapter, call);
      assert!(
        adapter.savepoints.is_empty(),
        "complete caller must close every SDK transaction"
      );
      result
    } else {
      polkadot_sdk::sp_externalities::set_and_run_with_externalities(adapter.inner, call)
    }
  };
  assert_eq!(
    host.recorder().unwrap().to_storage_proof(),
    recorder.to_storage_proof()
  );
  let proof = recorder.to_storage_proof();
  let db = host.read_write_count();
  // Freeze the original witness/counts before commit resets the recorder; retain new counts separately.
  let commit_start = Instant::now();
  polkadot_sdk::sp_externalities::set_and_run_with_externalities(&mut ext, || {
    polkadot_sdk::frame_benchmarking::benchmarking::commit_db();
  });
  let commit_ns = checked_capture_interval_ns(commit_start, Instant::now())
    .expect("invalid post-capture commit clock interval");
  // Inclusive capture also contains proof snapshots/glue; it is not a disjoint production price.
  let capture_ns = checked_capture_interval_ns(capture_start.unwrap(), Instant::now())
    .expect("invalid inclusive capture clock interval");
  let db_after_commit = host.read_write_count();
  let top = host
    .pairs(Default::default())
    .unwrap()
    .map(Result::unwrap)
    .collect::<State>();
  let observation = ACTIVE.with_borrow_mut(|active| active.take().unwrap());
  ResultRow {
    top,
    root,
    proof,
    body_proof,
    db,
    db_after_commit,
    elapsed_ns,
    root_ns,
    commit_ns,
    capture_ns,
    diagnostic_callback: observer.is_some(),
    observation,
  }
}

fn export_case(
  output: &std::path::Path,
  label: &str,
  parent_root: H256,
  observed: &ResultRow,
  compact: &polkadot_sdk::sp_trie::CompactProof,
) {
  let observation = &observed.observation;
  let case = output.join(label);
  std::fs::create_dir(&case).unwrap();
  std::fs::write(case.join("post.scale"), observed.top.encode()).unwrap();
  std::fs::write(case.join("witness.scale"), observed.proof.encode()).unwrap();
  std::fs::write(case.join("compact.scale"), compact.encode()).unwrap();
  let operations = observation
    .operations
    .iter()
    .map(|op| {
      serde_json::json!({
        "phase": format!("{:?}", op.phase), "method": op.method, "key": op.key,
        "overlay": op.overlay, "observer": op.observer, "value": op.value, "sdkDbDelta": op.db
      })
    })
    .collect::<Vec<_>>();
  let trace = serde_json::json!({
    "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root),
    "nativeInstrumentedWallNs": observed.elapsed_ns.to_string(), "postCallRootWallNs": observed.root_ns.to_string(),
    "postCaptureCommitWallNs": observed.commit_ns.to_string(), "inclusiveCaptureWallNs": observed.capture_ns.to_string(),
    "diagnosticCallbackInstalled": observed.diagnostic_callback, "inclusiveCaptureContainsDiagnosticGlue": true,
    "captureClockIntervalsChecked": true,
    "operations": operations, "markers": format!("{:?}", observation.markers),
    "refusalRoots": observation.refusal_roots.map(|root| root.map(|value| format!("{value:?}"))),
    "transactions": format!("{:?}", observation.transactions),
    "phaseWallNs": format!("{:?}", observation.wall_ns), "sdkDbBeforeCommit": observed.db,
    "sdkDbAfterCommit": observed.db_after_commit,
    "conservativeWeightBound": false, "phaseProofAdditive": false
  });
  std::fs::write(
    case.join("trace.json"),
    serde_json::to_vec_pretty(&trace).unwrap(),
  )
  .unwrap();
}

fn economic_state(top: &State) -> State {
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  let account_prefix = polkadot_sdk::frame_system::Account::<Runtime>::final_prefix();
  let prefixes = [
    b"Balances".as_slice(),
    b"Assets".as_slice(),
    b"AssetConversion".as_slice(),
  ]
  .map(polkadot_sdk::sp_io::hashing::twox_128);
  top
    .iter()
    .filter(|(key, _)| {
      key.starts_with(&account_prefix) || prefixes.iter().any(|prefix| key.starts_with(prefix))
    })
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect()
}

/// Re-executes the same caller on proof-only SDK storage, including hash/iterator/overlay reads.
/// This is a correctness oracle, not another physical sample or a copied transition algorithm.
fn replay_caller(
  parent_root: H256,
  db: MemoryDB<Blake2Hasher>,
  fixture: &Fixture,
  refuse: bool,
  recorder: polkadot_sdk::sp_trie::recorder::Recorder<BlakeTwo256>,
  expected_refusal_roots: [Option<H256>; 2],
) -> H256 {
  replay_caller_captured(
    parent_root,
    db,
    fixture,
    refuse,
    recorder,
    expected_refusal_roots,
    Some(notice),
  )
}

fn replay_caller_captured(
  parent_root: H256,
  db: MemoryDB<Blake2Hasher>,
  fixture: &Fixture,
  refuse: bool,
  recorder: polkadot_sdk::sp_trie::recorder::Recorder<BlakeTwo256>,
  expected_refusal_roots: [Option<H256>; 2],
  observer: Option<fn(Phase, bool)>,
) -> H256 {
  assert!(observer.is_some() || !refuse);
  if observer.is_none() {
    benchmark_observation::assert_unobserved();
  }
  let backend = polkadot_sdk::sp_state_machine::TrieBackendBuilder::new(db, parent_root).build();
  let mut overlay = OverlayedChanges::default();
  let mut ext = Ext::new(&mut overlay, &backend, None);
  ACTIVE.with_borrow_mut(|active| {
    assert!(active.is_none());
    *active = Some(Observation {
      enabled: false,
      recording: false,
      diagnostic: false,
      refuse,
      fault_applied: false,
      fixture: fixture.clone(),
      recorder,
      stack: Vec::new(),
      markers: Vec::new(),
      last: Instant::now(),
      wall_ns: BTreeMap::new(),
      first_seen: BTreeMap::new(),
      seen: BTreeSet::new(),
      operations: Vec::new(),
      transactions: Vec::new(),
      after_task: None,
      refusal_roots: [None; 2],
    });
  });
  let root = polkadot_sdk::sp_externalities::set_and_run_with_externalities(&mut ext, || {
    let call = || Actors::actor_prepass(RuntimeOrigin::none());
    if let Some(observer) = observer {
      assert_ok!(benchmark_observation::with_observer(observer, call));
    } else {
      assert_ok!(call());
    }
    H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1))
  });
  let oracle = ACTIVE.with_borrow_mut(|active| active.take().unwrap());
  assert!(oracle.stack.is_empty());
  assert_eq!(oracle.fault_applied, refuse);
  assert_eq!(oracle.refusal_roots, expected_refusal_roots);
  root
}

fn refusal_state_domain(
  top: &BTreeMap<Vec<u8>, Vec<u8>>,
  fixture: &Fixture,
) -> BTreeMap<&'static str, Vec<u8>> {
  let mut ext = polkadot_sdk::sp_io::TestExternalities::new(Storage {
    top: top.clone(),
    ..Default::default()
  });
  ext.execute_with(|| {
    let resources = Actors::block_resource_state().unwrap();
    let predecessors = fixture.inspection_predecessors.unwrap();
    let peer_states = ((fixture.actor - u64::from(predecessors) - 1)..(fixture.actor - 1))
      .map(|id| {
        let actor = Actors::active_actor_state(id).unwrap();
        (
          actor.encode(),
          Assets::balance(&actor.identity.sovereign_account, fixture.lp),
        )
      })
      .collect::<Vec<_>>();
    let actor_events = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        crate::RuntimeEvent::Actors(event) => Some(event.encode()),
        _ => None,
      })
      .collect::<Vec<_>>();
    let control = resources.usage().actor_control_used();
    let effect = resources.usage().actor_effect_used();
    BTreeMap::from([
      (
        "target",
        Actors::active_actor_state(fixture.actor).unwrap().encode(),
      ),
      (
        "targetBalances",
        [fixture.native, fixture.a, fixture.b, fixture.lp]
          .map(|asset| Assets::balance(&fixture.payer, asset))
          .encode(),
      ),
      (
        "hold",
        polkadot_sdk::sp_io::storage::get(
          &pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(fixture.actor),
        )
        .map(|v| v.to_vec())
        .encode(),
      ),
      ("peerStatesAndLp", peer_states.encode()),
      (
        "paidPrefix",
        Actors::active_actor_state(fixture.paid_prefix.unwrap())
          .unwrap()
          .encode(),
      ),
      ("service", Actors::service_header().encode()),
      (
        "recipientAndSink",
        (
          Assets::balance(&common::BOB, fixture.a),
          Assets::balance(&fixture.sink, fixture.native),
        )
          .encode(),
      ),
      (
        "actorControlUsed",
        [control.ref_time(), control.proof_size()].encode(),
      ),
      (
        "actorEffectUsed",
        [effect.ref_time(), effect.proof_size()].encode(),
      ),
      (
        "reservationsAndHalt",
        (
          resources.outstanding_reservations(),
          resources.optional_actor_work_halted(),
        )
          .encode(),
      ),
      (
        "resourceLedger",
        top
          .get(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key().as_slice())
          .unwrap()
          .clone(),
      ),
      ("actorEvents", actor_events.encode()),
    ])
  })
}

#[test]
#[ignore = "requires retained authenticated native and framed Prepass states"]
fn retained_prepass_event_suffixes_match_typed() {
  use codec::DecodeAll;
  use polkadot_sdk::frame_system::{Event, EventRecord, Phase};
  type Record = EventRecord<crate::RuntimeEvent, H256>;
  let events = |variable: &str, expected_root: &str| {
    let path = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} required"));
    let bytes = std::fs::read(path).unwrap();
    let top = BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(&mut bytes.as_slice()).unwrap();
    assert_eq!(
      Layout::trie_root(top.clone()),
      expected_root.parse::<H256>().unwrap()
    );
    let key = polkadot_sdk::frame_support::storage::storage_prefix(b"System", b"Events");
    let raw = top.get(key.as_slice()).unwrap();
    let records = Vec::<Record>::decode_all(&mut raw.as_slice()).unwrap();
    assert_eq!(
      records.encode(),
      *raw,
      "typed event records must round-trip exactly"
    );
    records
  };
  let native_parent = events(
    "DEOS_SERVICE_NATIVE_REFUSAL_PARENT",
    "0xf9f02cf8328d3953f6d3519e8ac952945af8d7bb3a0c84b4d8a3ced892c1cac8",
  );
  let native_post = events(
    "DEOS_SERVICE_NATIVE_REFUSAL_POST",
    "0xb8b4469a39ac5b9f224de3e525d38bc5547c15d8c4b3301b9b81711fc395d4b6",
  );
  let framed_before = events(
    "DEOS_SERVICE_FRAMED_BEFORE_PREPASS",
    "0x070b48d5a9bfd40b48cb4171ba35709cfd8b3cbd35eca8c3586179dc3cfd9485",
  );
  let framed_after = events(
    "DEOS_SERVICE_FRAMED_AFTER_PREPASS",
    "0xb2ee3fe44d1f4dd0f041eefd6ec424324b000625ff4d4a6e1f9ea8d620bd12da",
  );
  assert_eq!(
    native_post.get(..native_parent.len()),
    Some(native_parent.as_slice())
  );
  assert_eq!(
    framed_after.get(..framed_before.len()),
    Some(framed_before.as_slice())
  );
  let native_suffix = &native_post[native_parent.len()..];
  let framed_suffix = &framed_after[framed_before.len()..];
  let (framing, framed_payloads) = framed_suffix.split_last().unwrap();
  let payloads = |records: &[Record]| {
    records
      .iter()
      .map(|record| record.event.encode())
      .collect::<Vec<_>>()
  };
  let actors = |records: &[Record]| {
    records
      .iter()
      .filter_map(|record| match &record.event {
        crate::RuntimeEvent::Actors(event) => Some(event.encode()),
        _ => None,
      })
      .collect::<Vec<_>>()
  };
  let native_actors = actors(native_suffix);
  let framed_actors = actors(framed_payloads);
  let payloads_equal = payloads(native_suffix) == payloads(framed_payloads);
  let actor_payloads_equal = native_actors == framed_actors;
  let topics_equal = native_suffix
    .iter()
    .map(|r| &r.topics)
    .eq(framed_payloads.iter().map(|r| &r.topics));
  let phases = |records: &[Record]| {
    records
      .iter()
      .map(|r| format!("{:?}", r.phase))
      .collect::<BTreeSet<_>>()
  };
  let report = serde_json::json!({
    "scope": "typed comparison of immutable retained event suffixes; no execution or state repair",
    "nativeParentCount": native_parent.len(), "nativePostCount": native_post.len(),
    "framedBeforeCount": framed_before.len(), "framedAfterCount": framed_after.len(),
    "nativeSuffixCount": native_suffix.len(), "framedSuffixCount": framed_suffix.len(),
    "nativeActorCount": native_actors.len(), "framedActorCount": framed_actors.len(),
    "payloadsAndOrderEqual": payloads_equal, "actorPayloadsAndOrderEqual": actor_payloads_equal,
    "topicsEqual": topics_equal, "nativePhases": phases(native_suffix),
    "framedPhases": phases(framed_suffix), "extraEvent": format!("{:?}", framing.event),
    "extraPhase": format!("{:?}", framing.phase),
    "nativeActorSuffixSha256": format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&native_actors.encode()))),
    "framedActorSuffixSha256": format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&framed_actors.encode()))),
    "wholeHistoricalBuffersEqual": native_post == framed_after,
    "stateAlignment": false, "physicalPricingQualified": false,
  });
  if let Some(path) = std::env::var_os("DEOS_SERVICE_OBSERVATION_OUTPUT") {
    std::fs::create_dir(&path).unwrap();
    std::fs::write(
      std::path::Path::new(&path).join("typed-events.json"),
      serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
  }
  println!("ACTOR_TYPED_PREPASS_EVENTS {report}");
  assert_eq!(framed_suffix.len(), native_suffix.len() + 1);
  assert!(
    matches!(
      framing.event,
      crate::RuntimeEvent::System(Event::ExtrinsicSuccess { .. })
    ),
    "only the final real Executive success receipt may be additional"
  );
  assert!(framing.topics.is_empty());
  assert!(
    framed_suffix
      .iter()
      .all(|r| r.phase == Phase::ApplyExtrinsic(2)),
    "every framed event must retain the real Prepass extrinsic index"
  );
  assert!(
    payloads_equal,
    "all non-framing RuntimeEvent payloads and ordering must match"
  );
  assert!(
    actor_payloads_equal,
    "Actor-event suffixes must match byte-for-byte in order"
  );
  assert!(topics_equal, "topic metadata must not be normalized away");
}

#[test]
#[ignore = "requires explicit hash-bound ordinary Wasm artifact"]
fn ordinary_wasm_prepass_framing_probe() {
  use crate::InherentDataExt;
  use codec::DecodeAll;
  use polkadot_sdk::{
    sp_core::storage::well_known_keys,
    sp_runtime::{Digest, DigestItem, traits::Header as HeaderT},
    sp_trie::proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
  };
  let wasm_path =
    std::env::var("DEOS_ORDINARY_WASM").expect("explicit ordinary Wasm path required");
  let wasm = std::fs::read(&wasm_path).unwrap();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    super::production_block_replay::ACCEPTED_PRODUCTION_WASM_SHA256,
    "ordinary artifact must equal the existing accepted image, not native benchmark Wasm"
  );
  let historical_path = std::env::var("DEOS_SERVICE_NATIVE_REFUSAL_POST")
    .expect("explicit retained native refusal post required");
  let historical: BTreeMap<Vec<u8>, Vec<u8>> =
    DecodeAll::decode_all(&mut std::fs::read(historical_path).unwrap().as_slice()).unwrap();
  assert_eq!(
    Layout::trie_root(historical.clone()),
    "0xb8b4469a39ac5b9f224de3e525d38bc5547c15d8c4b3301b9b81711fc395d4b6"
      .parse::<H256>()
      .unwrap(),
    "retained native post must authenticate its original accepted root"
  );
  let (parent, fixture) = prepare_inspection_refusal_at_cut(true);
  let parent_root = Layout::trie_root(parent.top.clone());
  let mut inspection = polkadot_sdk::sp_io::TestExternalities::new(parent.clone());
  let (number, para_id) =
    inspection.execute_with(|| (System::block_number(), crate::ParachainInfo::parachain_id()));
  let parent_header = crate::Header::new(
    number,
    Default::default(),
    parent_root,
    Default::default(),
    Digest::default(),
  );
  let header = crate::Header::new(
    number + 1,
    Default::default(),
    Default::default(),
    parent_header.hash(),
    Digest {
      logs: vec![DigestItem::PreRuntime(
        polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
        u64::from(number + 1).encode(),
      )],
    },
  );
  let mut extrinsics = Vec::<crate::UncheckedExtrinsic>::new();
  let mut calls = vec![("Core_initialize_block".to_owned(), header.encode())];
  let recorder = polkadot_sdk::sp_trie::recorder::Recorder::<Blake2Hasher>::default();
  let mut native = polkadot_sdk::sp_io::TestExternalities::new(parent.clone());
  native.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  let mut native_phases = Vec::new();
  let mut construction_error = None;
  let mut index = 0;
  while index < calls.len() {
    let method = calls[index].0.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      native.execute_with_recorder(recorder.clone(), || {
        if index == 0 {
          (crate::Executive::initialize_block(&header).encode(), true)
        } else if index <= extrinsics.len() {
          let applied = crate::Executive::apply_extrinsic(extrinsics[index - 1].clone());
          let succeeded = applied.as_ref().is_ok_and(|dispatch| dispatch.is_ok());
          (applied.encode(), succeeded)
        } else {
          (crate::Executive::finalize_block().encode(), true)
        }
      })
    }));
    let (output, succeeded, panicked) = match outcome {
      Ok((bytes, succeeded)) => (Ok(bytes), succeeded, false),
      Err(error) => (
        Err(
          error
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| error.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "native panic without text".to_owned()),
        ),
        false,
        true,
      ),
    };
    let top = native
      .execute_with(super::production_block_replay::current_top_storage)
      .top;
    let root = Layout::trie_root(top.clone());
    native_phases.push((
      method,
      output,
      top,
      root,
      recorder.to_storage_proof(),
      succeeded,
      panicked,
    ));
    if !succeeded {
      break;
    }
    if index == 0 {
      let construction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let before = native.execute_with(super::production_block_replay::current_top_storage);
        let generated: Vec<crate::UncheckedExtrinsic> = native.execute_with(|| {
          super::production_block_replay::inherent_data_for_para(
            &parent_header,
            number + 1,
            para_id,
          )
          .create_extrinsics()
        });
        assert_eq!(
          native
            .execute_with(super::production_block_replay::current_top_storage)
            .top,
          before.top,
          "inherent construction must not mutate the source"
        );
        assert_eq!(generated.len(), 3);
        assert!(matches!(
          generated[2].function,
          crate::RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
        ));
        generated
      }));
      match construction {
        Ok(generated) => extrinsics = generated,
        Err(error) => {
          construction_error = Some(
            error
              .downcast_ref::<String>()
              .cloned()
              .or_else(|| error.downcast_ref::<&str>().map(|s| (*s).to_owned()))
              .unwrap_or_else(|| "inherent construction panic without text".to_owned()),
          );
          break;
        }
      }
      calls.extend(
        extrinsics
          .iter()
          .map(|xt| ("BlockBuilder_apply_extrinsic".to_owned(), xt.encode())),
      );
      calls.push(("BlockBuilder_finalize_block".to_owned(), Vec::new()));
    }
    index += 1;
  }
  let replay =
    super::wasm_replay::replay_prepass_framing(parent.clone(), &wasm, parent_header.hash(), &calls)
      .unwrap();
  let guest = &replay.phases;
  let mut original = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(
    Layout::trie_build(&mut original, parent.top.clone()),
    parent_root
  );
  let summaries = guest.iter().enumerate().map(|(index, phase)| {
    assert_eq!(Layout::trie_root(phase.top.clone()), phase.root, "literal overlay diagnostic must match actual host root");
    assert!(phase.proof.iter_nodes().all(|node| HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)), "all recorded nodes must belong to the unmodified original parent");
    let native_phase = native_phases.get(index);
    serde_json::json!({
      "phase": index, "method": phase.method, "wasmSucceeded": phase.succeeded,
      "extrinsicKind": index.checked_sub(1).and_then(|i| extrinsics.get(i)).map(|xt| match &xt.function { crate::RuntimeCall::Timestamp(_) => "Timestamp", crate::RuntimeCall::ParachainSystem(_) => "ParachainSystem", crate::RuntimeCall::Actors(_) => "ActorPrepass", _ => "Unexpected" }),
      "differentKeys": native_phase.map(|p| p.2.keys().chain(phase.top.keys()).collect::<BTreeSet<_>>().into_iter().filter(|key| p.2.get(*key) != phase.top.get(*key)).map(|key| format!("{key:02x?}")).collect::<Vec<_>>()),
      "wasmOutput": format!("{:?}", phase.output), "wasmRoot": format!("{:?}", phase.root),
      "nativeOutput": native_phase.map(|p| format!("{:?}", p.1)), "nativeRoot": native_phase.map(|p| format!("{:?}", p.3)),
      "nativeSucceeded": native_phase.map(|p| p.5), "nativePanicked": native_phase.map(|p| p.6),
      "rootsEqual": native_phase.is_some_and(|p| p.3 == phase.root),
      "economicsEqual": native_phase.is_some_and(|p| economic_state(&p.2) == economic_state(&phase.top)),
      "resourceLedgerEqual": native_phase.is_some_and(|p| p.2.get(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key().as_slice()) == phase.top.get(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key().as_slice())),
      "bodyWitnessNodes": phase.body_proof.len(), "rootWitnessNodes": phase.proof.len(),
    })
  }).collect::<Vec<_>>();
  let complete =
    calls.len() == 5 && guest.len() == calls.len() && guest.iter().all(|p| p.succeeded);
  let historical_domain = refusal_state_domain(&historical, &fixture);
  let framed_domain = guest
    .get(3)
    .filter(|phase| phase.succeeded)
    .map(|phase| refusal_state_domain(&phase.top, &fixture));
  let historical_comparison = framed_domain.as_ref().map(|framed| historical_domain.iter().map(|(name, value)| {
    (*name, serde_json::json!({ "equal": framed.get(name) == Some(value), "nativeSha256": format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(value))), "framedSha256": framed.get(name).map(|v| format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(v)))), "nativeEncoded": (value.len() <= 64).then(|| format!("{value:02x?}")), "framedEncoded": framed.get(name).filter(|v| v.len() <= 64).map(|v| format!("{v:02x?}")) }))
  }).collect::<BTreeMap<_, _>>());
  let same_outcome = framed_domain.as_ref().is_some_and(|framed| {
    historical_domain.iter().all(|(name, value)| {
      matches!(*name, "resourceLedger" | "actorControlUsed") || framed.get(name) == Some(value)
    })
  });
  let report = serde_json::json!({
    "source": "literal capacity fixture cut before final initialize; not full Executive ancestry",
    "ordinaryWasmSha256": format!("{:?}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&wasm))),
    "parentHasCode": parent.top.contains_key(well_known_keys::CODE), "codeInserted": false,
    "parentCodeLength": parent.top.get(well_known_keys::CODE).map(Vec::len),
    "parentCodeMatchesOrdinary": parent.top.get(well_known_keys::CODE).is_some_and(|code| code == &wasm),
    "relayInputParaId": u32::from(para_id),
    "parentRoot": format!("{parent_root:?}"), "parentBlock": number, "target": fixture.actor,
    "framingComplete": complete, "phases": summaries,
    "prepassDispatchSucceeded": guest.len() >= 4 && guest[3].succeeded,
    "originalRefusalQualified": false,
    "originalRefusalOutcomeEquivalent": same_outcome,
    "historicalRefusalComparison": historical_comparison,
    "stateAlignment": false, "physicalPricingQualified": false,
    "inherentConstructionError": construction_error,
    "compactProofReplay": replay.proof_verification, "emptyWitnessCheck": replay.empty_witness_check,
  });
  if let Some(output) =
    std::env::var_os("DEOS_SERVICE_OBSERVATION_OUTPUT").map(std::path::PathBuf::from)
  {
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("parent.scale"), parent.top.encode()).unwrap();
    std::fs::write(output.join("calls.scale"), calls.encode()).unwrap();
    std::fs::write(
      output.join("report.json"),
      serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    for (index, native_phase) in native_phases.iter().enumerate() {
      std::fs::write(
        output.join(format!("phase-{index}-native-post.scale")),
        native_phase.2.encode(),
      )
      .unwrap();
      std::fs::write(
        output.join(format!("phase-{index}-native-witness.scale")),
        native_phase.4.encode(),
      )
      .unwrap();
    }
    for (index, phase) in guest.iter().enumerate() {
      std::fs::write(
        output.join(format!("phase-{index}-post.scale")),
        phase.top.encode(),
      )
      .unwrap();
      std::fs::write(
        output.join(format!("phase-{index}-witness.scale")),
        phase.proof.encode(),
      )
      .unwrap();
      match phase.proof.to_compact_proof::<Blake2Hasher>(parent_root) {
        Ok(compact) => std::fs::write(
          output.join(format!("phase-{index}-compact.scale")),
          compact.encode(),
        )
        .unwrap(),
        Err(error) => std::fs::write(
          output.join(format!("phase-{index}-compact-error.txt")),
          format!("{error:?}"),
        )
        .unwrap(),
      }
    }
  }
  println!("ORDINARY_PREPASS_FRAMING {report}");
  assert!(
    replay.proof_verification.is_ok(),
    "proof-only replay failed; collected observations were exported: {:?}",
    replay.proof_verification
  );
  assert!(
    replay.empty_witness_check.is_err(),
    "empty witness must fail closed"
  );
  assert!(
    construction_error.is_none(),
    "inherent construction failed; initialized prefix was exported: {construction_error:?}"
  );
}

#[test]
fn complete_service_inspection_refusal_host_observation() {
  use pallet_deos_actors::{TaskEffectWeightProvider, WeightInfo};
  use polkadot_sdk::sp_weights::Weight;
  let (parent, fixture) = prepare_inspection_refusal();
  let predecessors = fixture.inspection_predecessors.unwrap();
  let parent_root = Layout::trie_root(parent.top.clone());
  let mut source = polkadot_sdk::sp_io::TestExternalities::new(parent.clone());
  let (before_actor, before_balances, before_recipient, event_start, expected_effect) = source
    .execute_with(|| {
      let actor = Actors::active_actor_state(fixture.actor).unwrap();
      assert!(actor.run_state.is_none());
      assert_eq!(actor.hot.cycle_state, CycleState::Idle);
      assert!(actor.hot.pending_signal);
      assert_eq!(actor.identity.cycle_nonce, 0);
      let effect =
        <Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&actor.contract.steps[0].task)
          .unwrap();
      (
        actor.encode(),
        [
          Assets::balance(&fixture.payer, fixture.native),
          Assets::balance(&fixture.payer, fixture.lp),
          Assets::balance(&fixture.payer, fixture.a),
          Assets::balance(&fixture.payer, fixture.b),
        ],
        Assets::balance(&common::BOB, fixture.a),
        System::events().len(),
        effect,
      )
    });
  assert!(
    before_recipient > 0,
    "previous block's real Transfer prefix must exist"
  );
  let plain = run(&parent, &fixture, false, false);
  let observed = run(&parent, &fixture, true, false);
  assert_eq!(observed.top, plain.top);
  assert_eq!(observed.root, plain.root);
  assert_eq!(observed.body_proof, plain.body_proof);
  assert_eq!(observed.proof, plain.proof);
  assert_eq!(observed.db, plain.db);
  assert_eq!(observed.root, Layout::trie_root(observed.top.clone()));
  let observation = &observed.observation;
  assert!(!observation.fault_applied);
  println!(
    "ACTOR_I_REACHED encounters={} tasks={} last_markers={:?}",
    observation
      .markers
      .iter()
      .filter(|marker| **marker == (Phase::Encounter, true))
      .count(),
    observation
      .markers
      .iter()
      .filter(|marker| **marker == (Phase::Task, true))
      .count(),
    &observation.markers[observation.markers.len().saturating_sub(8)..]
  );
  assert!(observation.refusal_roots.iter().all(Option::is_some));
  assert_eq!(
    observation.refusal_roots[0], observation.refusal_roots[1],
    "failed paired booking must restore exact provisional TOP/fees/events/holds after committed peers"
  );
  assert_eq!(observation.refusal_roots, plain.observation.refusal_roots);
  assert_eq!(observation.markers, plain.observation.markers);
  assert_eq!(
    observation
      .markers
      .iter()
      .filter(|marker| **marker == (Phase::Preflight, true))
      .count(),
    predecessors as usize + 2
  );
  assert_eq!(
    observation
      .markers
      .iter()
      .filter(|marker| **marker == (Phase::Task, true))
      .count(),
    predecessors as usize + 1,
    "only predecessor LP Tasks and the paid User's authored StopCycle may be invoked"
  );
  let booking = observation
    .markers
    .iter()
    .filter_map(|(phase, entering)| match (*phase, *entering) {
      (phase @ Phase::Booking { actor, .. }, true) if actor == fixture.actor => Some(phase),
      _ => None,
    })
    .collect::<Vec<_>>();
  assert_eq!(booking.len(), 1);
  let booking_exit = observation
    .markers
    .iter()
    .position(|marker| *marker == (booking[0], false))
    .unwrap();
  assert!(
    observation.markers[booking_exit + 1..]
      .iter()
      .all(|marker| *marker != (Phase::Task, true)),
    "the refused target must never invoke a Task after its actual paired booking"
  );
  let Phase::Booking {
    ledger_control,
    caller_control,
    ledger_effect,
    required_effect,
    ..
  } = booking[0]
  else {
    unreachable!()
  };
  let weight = |[time, proof]: [u64; 2]| Weight::from_parts(time, proof);
  let (ledger_control, caller_control, ledger_effect, required_effect) = (
    weight(ledger_control),
    weight(caller_control),
    weight(ledger_effect),
    weight(required_effect),
  );
  assert_eq!(required_effect, expected_effect);
  assert_eq!(
    ledger_effect,
    expected_effect.saturating_mul(predecessors.into())
  );
  let base = <Runtime as Config>::BlockResourceBudget::get()
    .limits()
    .actor_base_turn();
  assert!(ledger_effect.all_lte(base));
  assert!(
    ledger_effect
      .checked_add(&required_effect)
      .unwrap()
      .any_gt(base),
    "real prefix must exhaust an effect axis before the target booking"
  );
  assert!(
    observation
      .transactions
      .iter()
      .any(|(kind, _, phase)| *kind == "rollback" && *phase == Some(Phase::Encounter))
  );
  let mut original = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(
    Layout::trie_build(&mut original, parent.top.clone()),
    parent_root
  );
  assert!(
    observed
      .body_proof
      .iter_nodes()
      .all(|node| observed.proof.iter_nodes().any(|value| value == node))
  );
  assert!(observed.proof.iter_nodes().all(|node| {
    HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)
  }));
  assert_eq!(
    replay(
      parent_root,
      &observed.proof,
      &parent.top,
      &observed.top,
      &observation.operations
    ),
    Ok(observed.root)
  );
  assert!(
    replay(
      parent_root,
      &StorageProof::empty(),
      &parent.top,
      &observed.top,
      &observation.operations
    )
    .is_err()
  );
  let compact = observed
    .proof
    .to_compact_proof::<Blake2Hasher>(parent_root)
    .unwrap();
  let (decoded, decoded_root) = compact.to_memory_db::<Blake2Hasher>(&parent_root).unwrap();
  assert_eq!(decoded_root, parent_root);
  assert_eq!(
    replay_caller(
      parent_root,
      decoded.clone(),
      &fixture,
      false,
      observation.recorder.clone(),
      observation.refusal_roots,
    ),
    observed.root
  );
  assert_eq!(
    replay_db(
      parent_root,
      decoded,
      &parent.top,
      &observed.top,
      &observation.operations
    ),
    Ok(observed.root)
  );
  let mut final_ext = polkadot_sdk::sp_io::TestExternalities::new(Storage {
    top: observed.top.clone(),
    ..Default::default()
  });
  final_ext.execute_with(|| {
    let after = Actors::active_actor_state(fixture.actor).unwrap();
    assert_eq!(after.encode(), before_actor, "original paid Idle activation, nonce and current certificate remain exact");
    assert_eq!([Assets::balance(&fixture.payer, fixture.native), Assets::balance(&fixture.payer, fixture.lp), Assets::balance(&fixture.payer, fixture.a), Assets::balance(&fixture.payer, fixture.b)], before_balances);
    let hold_key = pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(fixture.actor);
    assert_eq!(observed.top.get(&hold_key), parent.top.get(&hold_key));
    let resources = Actors::block_resource_state().unwrap();
    assert_eq!(resources.outstanding_reservations(), 0);
    assert!(!resources.optional_actor_work_halted());
    assert_eq!(resources.usage().actor_effect_used(), ledger_effect,
      "failed atomic pair books no target effect in either axis");
    type W = <Runtime as Config>::WeightInfo;
    let inspection = W::service_round_begin_populated().saturating_add(W::service_round_probe_eligible()).saturating_add(W::scheduler_actor_state_probe());
    let before_refusal_control = ledger_control.checked_sub(&caller_control).unwrap();
    assert_eq!(resources.usage().actor_control_used().checked_sub(&before_refusal_control), Some(inspection),
      "pass releases its Control maximum but retains complete declared inspection/refusal work in both axes");
    assert!(inspection.ref_time() > 0 && inspection.proof_size() > 0);
    assert!(resources.usage().actor_control_used().all_lte(resources.budget().unwrap().limits().actor_control()));
    for peer in (fixture.actor - u64::from(predecessors) - 1)..(fixture.actor - 1) {
      let peer = Actors::active_actor_state(peer).unwrap();
      assert_eq!(peer.hot.cycle_state, CycleState::Idle);
      assert!(peer.run_state.is_none());
      assert_eq!(peer.identity.cycle_nonce, 1);
      assert!(Assets::balance(&peer.identity.sovereign_account, fixture.lp) > 0,
        "each booked predecessor must have real LP output, not only an Invoked receipt");
    }
    assert_eq!(Actors::service_header().count, 1);
    assert_eq!(Actors::service_header().cursor.unwrap().actor_id, fixture.actor);
    assert_eq!(Assets::balance(&common::BOB, fixture.a), before_recipient, "previously committed economic prefix must survive the refused turn");
    let paid_prefix = fixture.paid_prefix.unwrap();
    let paid = Actors::active_actor_state(paid_prefix).unwrap();
    assert_eq!(paid.identity.cycle_nonce, 1);
    assert_eq!(paid.hot.cycle_state, CycleState::Idle);
    assert!(paid.run_state.is_none());
    let events = System::events();
    let mut actions = 0;
    let mut pipelines = 0;
    let mut fees = 0;
    for record in &events[event_start..] {
      match &record.event {
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::ActionFeeCharged { actor_id, fee, .. }) => {
          assert_ne!(*actor_id, fixture.actor);
          assert_ne!(*actor_id, paid_prefix, "control-only StopCycle has no invocation receipt");
          assert_eq!(*fee, 0, "System invocation receipts do not collect a User Action fee");
          actions += 1;
          fees += *fee;
        }
        crate::RuntimeEvent::Actors(pallet_deos_actors::Event::PipelineFeeCharged { actor_id, fee, .. }) => {
          assert_ne!(*actor_id, fixture.actor);
          assert_ne!(*actor_id, paid_prefix, "previously paid Run must not buy another Pipeline");
          pipelines += 1;
          fees += *fee;
        }
        _ => {}
      }
    }
    assert_eq!(actions, predecessors, "each actual System Task has one zero-fee invocation receipt");
    assert_eq!(pipelines, 0, "System effects reuse unmetered rights; the User continuation is already paid");
    assert_eq!(Assets::balance(&fixture.sink, fixture.native), fixture.sink_before + fees);
  });
  let db_sum = observation.operations.iter().fold((0, 0, 0, 0), |sum, op| {
    (
      sum.0 + op.db.0,
      sum.1 + op.db.1,
      sum.2 + op.db.2,
      sum.3 + op.db.3,
    )
  });
  assert_eq!(db_sum, observed.db);
  if let Some(output) =
    std::env::var_os("DEOS_SERVICE_OBSERVATION_OUTPUT").map(std::path::PathBuf::from)
  {
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("parent.scale"), parent.top.encode()).unwrap();
    export_case(
      &output,
      "inspection-refusal",
      parent_root,
      &observed,
      &compact,
    );
  }
  println!(
    "ACTOR_COMPLETE_I_REFUSAL {}",
    serde_json::json!({
      "predecessors": predecessors, "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root),
      "rollbackRoot": format!("{:?}", observation.refusal_roots[0].unwrap()), "effectBefore": [ledger_effect.ref_time(), ledger_effect.proof_size()],
      "sdkDbBeforeCommit": observed.db, "operations": observation.operations.len(), "bodyWitnessNodes": observed.body_proof.len(),
      "rootWitnessNodes": observed.proof.len(), "rawWitnessBytes": observed.proof.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
      "instrumentedNativeWallNs": observed.elapsed_ns.to_string(), "productionWeightQualified": false, "proofPhaseAdditive": false,
      "paidPrefixPreserved": true, "ledgerInjection": false
    })
  );
}

#[test]
fn complete_service_opening_host_observation() {
  let (parent, fixture) = prepare();
  let parent_root = Layout::trie_root(parent.top.clone());
  let output = std::env::var_os("DEOS_SERVICE_OBSERVATION_OUTPUT").map(std::path::PathBuf::from);
  if let Some(output) = &output {
    std::fs::create_dir(output).expect("observation output must be a new directory");
    std::fs::write(output.join("parent.scale"), parent.top.encode()).unwrap();
  }
  let mut original = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(
    Layout::trie_build(&mut original, parent.top.clone()),
    parent_root
  );
  for refuse in [false, true] {
    let plain = run(&parent, &fixture, false, refuse);
    let observed = run(&parent, &fixture, true, refuse);
    assert_eq!(
      observed.top, plain.top,
      "observer must preserve complete TOP"
    );
    assert_eq!(observed.root, plain.root);
    assert_eq!(observed.body_proof, plain.body_proof);
    assert_eq!(observed.proof, plain.proof);
    assert_eq!(observed.db, plain.db);
    assert_eq!(observed.root, Layout::trie_root(observed.top.clone()));
    assert_ne!(
      observed.root, parent_root,
      "Prepass must persist its resource settlement"
    );
    let observation = &observed.observation;
    assert_eq!(observation.fault_applied, refuse);
    assert_eq!(observation.markers, plain.observation.markers);
    for phase in [
      Phase::Pass,
      Phase::Encounter,
      Phase::Preflight,
      Phase::Task,
      Phase::Retirement,
      Phase::Settlement,
      Phase::Collection,
    ] {
      assert!(
        observation.markers.contains(&(phase, true)),
        "missing {phase:?}"
      );
      assert!(
        observation.markers.contains(&(phase, false)),
        "unterminated {phase:?}"
      );
    }
    assert_eq!(
      observation
        .markers
        .iter()
        .filter(|entry| **entry == (Phase::Task, true))
        .count(),
      1
    );
    assert_eq!(
      observation.markers.contains(&(Phase::Authentication, true)),
      !refuse
    );
    let (lp, a, b) = observation.after_task.unwrap();
    assert_eq!(lp, fixture.amount - fixture.spend);
    assert!(
      a > 0 && b > 0,
      "real LP Task outputs precede collection/refusal"
    );
    assert!(
      observation
        .operations
        .iter()
        .any(|op| op.phase == Some(Phase::Task) && op.method == "put" && !op.observer)
    );
    assert!(
      observation
        .operations
        .iter()
        .any(|op| op.phase == Some(Phase::Preflight) && op.method == "get" && !op.observer)
    );
    assert!(
      observation
        .operations
        .iter()
        .any(|op| op.overlay && op.method == "get" && !op.observer)
    );
    assert!(
      observation
        .operations
        .iter()
        .any(|op| !op.overlay && op.method == "get" && !op.observer)
    );
    assert_eq!(
      observation
        .transactions
        .iter()
        .any(|(method, _, _)| *method == "rollback"),
      refuse
    );
    assert!(
      observation
        .transactions
        .iter()
        .any(|(method, _, phase)| *method == "commit" && *phase == Some(Phase::Task))
    );
    assert!(
      observed
        .body_proof
        .iter_nodes()
        .all(|node| observed.proof.iter_nodes().any(|v| v == node))
    );
    assert!(observed.proof.iter_nodes().all(|node| {
      HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)
    }));
    assert_eq!(
      replay(
        parent_root,
        &observed.proof,
        &parent.top,
        &observed.top,
        &observation.operations
      ),
      Ok(observed.root)
    );
    assert!(
      replay(
        parent_root,
        &StorageProof::empty(),
        &parent.top,
        &observed.top,
        &observation.operations
      )
      .is_err()
    );
    let compact = observed
      .proof
      .to_compact_proof::<Blake2Hasher>(parent_root)
      .unwrap();
    let (decoded, decoded_root) = compact.to_memory_db::<Blake2Hasher>(&parent_root).unwrap();
    assert_eq!(decoded_root, parent_root);
    assert_eq!(
      replay_caller(
        parent_root,
        decoded.clone(),
        &fixture,
        refuse,
        observation.recorder.clone(),
        observation.refusal_roots,
      ),
      observed.root,
      "compact witness must execute the actual complete caller, including iterator semantics"
    );
    if refuse {
      assert_eq!(
        economic_state(&observed.top),
        economic_state(&parent.top),
        "late refusal must restore all native accounts, balances/holds/issuance, assets and pools"
      );
      let hold = pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(fixture.actor);
      assert_eq!(observed.top.get(&hold), parent.top.get(&hold));
    }
    assert_eq!(
      replay_db(
        parent_root,
        decoded,
        &parent.top,
        &observed.top,
        &observation.operations
      ),
      Ok(observed.root)
    );
    let mut final_ext = polkadot_sdk::sp_io::TestExternalities::new(Storage {
      top: observed.top.clone(),
      ..Default::default()
    });
    final_ext.execute_with(|| {
      let actor = Actors::active_actor_state(fixture.actor).unwrap();
      let usage = Actors::block_resource_state().unwrap();
      assert!(usage.usage().actor_effect_used().ref_time() > 0);
      assert!(usage.usage().actor_effect_used().proof_size() > 0);
      let budget = <Runtime as Config>::BlockResourceBudget::get();
      assert!(
        usage
          .usage()
          .actor_effect_used()
          .all_lte(budget.limits().shared_economic())
      );
      assert!(
        usage
          .usage()
          .actor_control_used()
          .all_lte(budget.limits().actor_control())
      );
      if refuse {
        assert_eq!(actor.identity.cycle_nonce, 0);
        assert!(actor.hot.pending_signal);
        assert_eq!(Assets::balance(&fixture.payer, fixture.lp), fixture.amount);
        assert_eq!(Assets::balance(&fixture.payer, fixture.a), 0);
        assert_eq!(Assets::balance(&fixture.payer, fixture.b), 0);
        assert_eq!(
          Assets::balance(&fixture.sink, fixture.native),
          fixture.sink_before
        );
        assert!(Actors::service_nodes(fixture.actor).is_some());
        assert!(usage.optional_actor_work_halted());
      } else {
        assert_eq!(actor.identity.cycle_nonce, 1);
        assert_eq!(actor.hot.cycle_state, CycleState::Idle);
        assert_eq!(
          Assets::balance(&fixture.payer, fixture.lp),
          fixture.amount - fixture.spend
        );
        assert!(
          Assets::balance(&fixture.payer, fixture.a) > 0
            && Assets::balance(&fixture.payer, fixture.b) > 0
        );
        assert!(Actors::service_nodes(fixture.actor).is_none());
        let action_fees = System::events()
          .iter()
          .filter_map(|record| match &record.event {
            crate::RuntimeEvent::Actors(pallet_deos_actors::Event::ActionFeeCharged {
              actor_id,
              fee,
              ..
            }) if *actor_id == fixture.actor => Some(*fee),
            _ => None,
          })
          .sum::<crate::Balance>();
        assert!(action_fees > 0);
        assert_eq!(
          Assets::balance(&fixture.sink, fixture.native),
          fixture.sink_before + fixture.pipeline_fee + action_fees
        );
      }
      assert!(ActorContractHeads::<Runtime>::contains_key(fixture.actor));
    });
    let attributed = observation
      .first_seen
      .values()
      .flat_map(|nodes| nodes.iter())
      .collect::<BTreeSet<_>>();
    assert_eq!(attributed.len(), observed.body_proof.len());
    let db_sum = observation.operations.iter().fold((0, 0, 0, 0), |sum, op| {
      (
        sum.0 + op.db.0,
        sum.1 + op.db.1,
        sum.2 + op.db.2,
        sum.3 + op.db.3,
      )
    });
    if let Some(output) = &output {
      export_case(
        output,
        if refuse { "refusal" } else { "success" },
        parent_root,
        &observed,
        &compact,
      );
    }
    println!(
      "ACTOR_SERVICE_HOST_OBSERVATION {}",
      serde_json::json!({
        "refusal": refuse, "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root),
        "nativeInstrumentedWallNs": observed.elapsed_ns.to_string(), "postCallRootWallNs": observed.root_ns.to_string(),
        "bodyWitnessNodes": observed.body_proof.len(), "rootWitnessNodes": observed.proof.len(),
        "rawWitnessBytes": observed.proof.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
        "sdkDbBeforeCommit": observed.db, "attributedSdkDb": db_sum,
        "logicalWritesIncludingRolledBack": observation.operations.iter().filter(|op| op.method == "put" || op.method == "append").count(),
        "transactions": observation.transactions.len(), "phaseWallNs": format!("{:?}", observation.wall_ns),
      "methodCounts": observation.operations.iter().fold(BTreeMap::<&str, usize>::new(), |mut counts, op| { *counts.entry(op.method).or_default() += 1; counts }),
        "observerProbesAndFaultTagged": true, "proofPhaseIncrementsAreAdditiveBounds": false,
        "productionWeightQualified": false, "defaultWasmOrPvfTransferQualified": false
      })
    );
  }
}
