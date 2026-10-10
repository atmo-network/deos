//! Public native preparation for the fixed retention corpus.
//! Source topology remains projected outside the encoded chain.

mod block;
pub(super) use block::{
  contract, create_user, inherents, native_transfer, preset_storage, signed_lifecycle,
};

use super::actor_retention_corpus::{PAGES, WATCHES};
use codec::Encode;
use deos_runtime::{
  AccountId, Actors, AssetConversion, Assets, Balances, DeosRouter, Runtime, RuntimeOrigin, System,
};
use pallet_deos_actors::{
  ActorContractOf, ActorId, ActorIdentityOf, AmountResolution, AssetOps, Config, DependencyPlans,
  DependencyRegistrationFreeIndices, DependencyRegistrationFreePositions,
  DependencyRegistrationHeader, DependencyRegistrationHeaders, DependencyRegistrationPage,
  DependencyRegistrationPages, DependencyRegistrationPosition, DependencyRegistrationPositions,
  DependencyRegistrations, DependencySourceAllocatorState, DependencyTimedReviews,
  ParkedBalanceActivationOf, ParkedBalanceRule, PendingDependencyEvents, StepErrorPolicy, StepOf,
  Task, Trigger, WakeupKey,
};
use polkadot_sdk::{
  frame_support::{BoundedVec, assert_ok},
  sp_core::{Blake2Hasher, Pair, storage::Storage},
  sp_genesis_builder::PresetId,
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::MultiAddress,
  sp_state_machine::Backend,
  sp_trie::{LayoutV1, StorageProof, Trie, TrieDBBuilder, recorder::Recorder},
};
use primitives::AssetKind;
use std::collections::BTreeSet;
type Ops = <Runtime as Config>::AssetOps;
type Query = (Vec<u8>, Option<Vec<u8>>);

// Both named presets start with 15 identities and three Active Actors. Preparation
// adds the permanent cohorts, one producer per watch and two temporary identities,
// of which one is Active at the prepared boundary.
pub(super) const PREPARED_IDENTITIES: u32 = 15 + PAGES as u32 + WATCHES as u32 + 2;
pub(super) const PREPARED_ACTIVE_ACTORS: u32 = 3 + PAGES as u32 + WATCHES as u32 + 1;

pub(super) struct Prepared {
  pub(super) storage: Storage,
  #[allow(
    dead_code,
    reason = "only the admission diagnostic retains a committed backend"
  )]
  pub(super) committed: Option<TestExternalities>,
  pub(super) target: ActorId,
  pub(super) identity: ActorIdentityOf<Runtime>,
  pub(super) contract: ActorContractOf<Runtime>,
  pub(super) sources: Vec<u64>,
  pub(super) queries: Vec<Query>,
  pub(super) first_asset: u32,
  pub(super) temporary: [ActorId; 2],
  pub(super) temporary_contract: ActorContractOf<Runtime>,
  pub(super) dormant_holds: [Vec<u8>; 2],
  pub(super) native_before: u128,
  pub(super) allocator: Vec<u8>,
}

pub(super) fn owners() -> Vec<AccountId> {
  let mut owners = Sr25519Keyring::well_known()
    .map(|key| key.to_account_id())
    .collect::<Vec<_>>();
  assert_eq!(owners.len(), 12);
  assert_eq!(owners[0], Sr25519Keyring::Alice.to_account_id());
  assert_eq!(owners[1], Sr25519Keyring::Bob.to_account_id());
  for index in 0..28 {
    owners.push(AccountId::from(Sr25519Keyring::numeric(index).public()));
  }
  assert_eq!(
    owners.iter(/* deos-bypass: bounded-iter -- twelve well-known and 28 fixed numeric owners. */).collect::<BTreeSet<_>>().len(),
    40
  );
  owners
}

pub(super) fn prepare(
  preset: &PresetId,
  pages: &[Vec<u64>],
  target_contract_shape: Option<fn(&[u32], &mut ActorContractOf<Runtime>)>,
) -> Prepared {
  prepare_with_target_funding(preset, pages, target_contract_shape, true)
}

pub(super) fn prepare_with_target_funding(
  preset: &PresetId,
  pages: &[Vec<u64>],
  target_contract_shape: Option<fn(&[u32], &mut ActorContractOf<Runtime>)>,
  fund_target: bool,
) -> Prepared {
  prepare_state(preset, pages, target_contract_shape, fund_target, false)
}

#[allow(
  dead_code,
  reason = "only the admission diagnostic consumes committed preparation"
)]
pub(super) fn prepare_committed(preset: &PresetId, pages: &[Vec<u64>]) -> Prepared {
  prepare_state(preset, pages, None, false, true)
}

fn prepare_state(
  preset: &PresetId,
  pages: &[Vec<u64>],
  target_contract_shape: Option<fn(&[u32], &mut ActorContractOf<Runtime>)>,
  fund_target: bool,
  retain_backend: bool,
) -> Prepared {
  assert_eq!(
    PAGES as u32 + 32,
    <Runtime as Config>::MaxActiveActors::get()
  );
  assert_eq!(
    WATCHES as u32,
    <Runtime as Config>::MaxParkedBalanceWatches::get(),
    "fixed corpus watch domain differs from the configured parked-balance limit"
  );
  let genesis = preset_storage(preset);
  assert!(genesis.children_default.is_empty());
  let mut ext = TestExternalities::new(genesis);
  let (target, identity, parked_contract, sources, queries, first_asset, temporary, temporary_contract, dormant_holds, native_before, allocator) = ext
    .execute_with(|| {
      assert_eq!(
        (Actors::actor_identity_count(), Actors::active_actor_count()),
        (15, 3)
      );
      assert!(!Actors::user_creation_enabled());
      // This diagnostic's Root grant is outside its encoded block suffix. The real
      // signed Genesis-to-L2 chronology has its independent qualification owner.
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      let owners = owners();
      let alice = &owners[0];
      for donor in &owners[2..8] {
        native_transfer(donor, alice, 900_000 * deos_runtime::UNIT);
      }
      native_transfer(&owners[8], &owners[1], 200_000 * deos_runtime::UNIT);
      for owner in &owners[2..] {
        native_transfer(alice, owner, 30_000 * deos_runtime::UNIT);
      }
      // Match the ordinary asset composition of the signed suffix: successive
      // public pools supply LP ledgers without Root-created or minted watch assets.
      let mut assets = Vec::new();
      let mut paired = AssetKind::Local(0);
      for _ in 0..WATCHES {
        assert_ok!(DeosRouter::create_pool(
          RuntimeOrigin::signed(alice.clone()),
          AssetKind::Native,
          paired,
        ));
        assert_ok!(AssetConversion::add_liquidity(
          RuntimeOrigin::signed(alice.clone()),
          Box::new(AssetKind::Native),
          Box::new(paired),
          deos_runtime::UNIT,
          deos_runtime::UNIT / 4,
          1,
          1,
          alice.clone(),
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
      let mut producer_custodies = Vec::new();
      for asset in &assets {
        let producer = create_user(
          alice,
          Some(contract(
            Trigger::cadenced(1),
            vec![Task::Transfer {
              to: alice.clone(),
              asset: AssetKind::Local(*asset),
              amount: AmountResolution::Fixed(1),
            }],
          )),
        );
        let custody = Actors::actor_identity(producer).unwrap().sovereign_account;
        native_transfer(alice, &custody, 320_000 * deos_runtime::UNIT);
        assert_ok!(Assets::transfer(
          RuntimeOrigin::signed(alice.clone()),
          *asset,
          MultiAddress::Id(custody.clone()),
          10_000_000_000,
        ));
        producer_custodies.push(custody);
      }
      let temporary = [create_user(alice, None), create_user(&owners[1], None)];
      let dormant_holds = temporary.map(|actor| Actors::actor_state_hold(actor).unwrap().encode());
      System::set_block_number(1);
      let mut parked_contract = contract(Trigger::manual(), vec![Task::StopCycle; <Runtime as Config>::MaxContractSteps::get() as usize]);
      parked_contract.parked_balance_activation = Some(
        ParkedBalanceActivationOf::<Runtime>::try_from_rules(
          assets
            .iter() // deos-bypass: bounded-iter -- the four fixed reference watches
            .map(|asset| ParkedBalanceRule {
              asset: AssetKind::Local(*asset),
              authored_min_delta: 100,
            })
            .collect(),
        )
        .unwrap(),
      );
      let local = PAGES / 2;
      let mut target_contract = parked_contract.clone();
      if let Some(shape) = target_contract_shape {
        shape(&assets, &mut target_contract);
      }
      let mut actor_ids = vec![0; PAGES];
      let mut owner_counts = [0u32; 40];
      owner_counts[0] = WATCHES as u32 + 1;
      owner_counts[1] = 1;
      // Give the target the first due-review position. Source positions below are
      // synthetic recipe projection, not an assertion about this creation order.
      for (ordinal, cohort) in std::iter::once(local)
        .chain((0..PAGES).filter(|cohort| *cohort != local))
        .enumerate()
      {
        let owner = owner_counts
          .iter() // deos-bypass: bounded-iter -- forty fixed funded owners
          .enumerate()
          .min_by_key(|(_, count)| **count)
          .unwrap()
          .0;
        let installed = if cohort == local {
          target_contract.clone()
        } else {
          parked_contract.clone()
        };
        actor_ids[cohort] = create_user(&owners[owner], Some(installed));
        if cohort == local && target_contract_shape.is_some() {
          assert_eq!(Actors::actor_contract(actor_ids[cohort]), Some(target_contract.clone()));
        }
        owner_counts[owner] += 1;
        assert!(owner_counts[owner] <= u32::from(<Runtime as Config>::MaxOwnerSlots::get()));
        if ordinal.is_multiple_of(1000) {
          println!(
            "admission preparation: preset={preset} user_cohorts={}",
            ordinal + 1
          );
        }
      }
      assert_eq!(
        owner_counts.iter(/* deos-bypass: bounded-iter -- forty fixed owner counters. */).sum::<u32>(),
        (PAGES + WATCHES + 2) as u32
      );
      assert_eq!(*owner_counts.iter(/* deos-bypass: bounded-iter -- forty fixed owner counters. */).max().unwrap(), 250);
      let target = actor_ids[local];
      let sources = DependencyPlans::<Runtime>::get(target)
        .into_iter()
        .map(|entry| entry.source)
        .collect::<Vec<_>>();
      assert_eq!(sources, (0..WATCHES as u64).collect::<Vec<_>>());
      let count = 31 * PAGES as u64;
      let mut queries = Vec::new();
      for (source_index, source) in sources.iter(/* deos-bypass: bounded-iter -- four fixed watch sources. */).copied().enumerate() {
        let old_header = DependencyRegistrationHeaders::<Runtime>::get(source);
        assert_eq!((old_header.count, old_header.free_count), (PAGES as u32, 0));
        for page in 0..=old_header.last_page.unwrap() {
          DependencyRegistrationPages::<Runtime>::remove(source, page);
        }
        let selected = &pages[source_index];
        for (cohort, page) in selected.iter(/* deos-bypass: bounded-iter -- exactly 9968 hash-bound pages. */).copied().enumerate() {
          let actor = actor_ids[cohort];
          let handle = DependencyRegistrations::<Runtime>::get(source, actor).unwrap();
          let mut entries = BoundedVec::try_from(vec![None; 32]).unwrap();
          entries[0] = Some(handle);
          DependencyRegistrationPages::<Runtime>::insert(
            source,
            page,
            DependencyRegistrationPage {
              entries,
              previous: cohort.checked_sub(1).map(|index| selected[index]),
              next: selected.get(cohort + 1).copied(),
            },
          );
          DependencyRegistrationPositions::<Runtime>::insert(
            source,
            actor,
            DependencyRegistrationPosition { page, slot: 0 },
          );
          for slot in 1..32u8 {
            let index = 31 * cohort as u64 + u64::from(slot - 1);
            let position = DependencyRegistrationPosition { page, slot };
            DependencyRegistrationFreePositions::<Runtime>::insert(source, index, position);
            DependencyRegistrationFreeIndices::<Runtime>::insert(source, position, index);
          }
        }
        DependencyRegistrationHeaders::<Runtime>::insert(
          source,
          DependencyRegistrationHeader {
            count: PAGES as u32,
            free_count: count,
            next_index: (selected[PAGES - 1] + 1) * 32,
            first_page: Some(selected[0]),
            last_page: Some(selected[PAGES - 1]),
          },
        );
        for index in (31 * local as u64..31 * local as u64 + 31).chain(count - 31..count) {
          let position = DependencyRegistrationPosition {
            page: selected[(index / 31) as usize],
            slot: (index % 31 + 1) as u8,
          };
          queries.push((
            DependencyRegistrationFreePositions::<Runtime>::hashed_key_for(source, index),
            Some(position.encode()),
          ));
          queries.push((
            DependencyRegistrationFreeIndices::<Runtime>::hashed_key_for(source, position),
            Some(index.encode()),
          ));
        }
        queries.push((
          DependencyRegistrationFreePositions::<Runtime>::hashed_key_for(source, count),
          None,
        ));
        queries.push((
          DependencyRegistrationFreeIndices::<Runtime>::hashed_key_for(
            source,
            DependencyRegistrationPosition {
              page: selected[local],
              slot: 0,
            },
          ),
          None,
        ));
        println!(
          "admission projection: preset={preset} source={source} pages={PAGES} holes={count}"
        );
      }
      // Match the paid producers' real Transfer direction using existing LP accounts.
      // Every source has current scan authority; funding optionally qualifies the target.
      for (asset, producer) in assets.iter(/* deos-bypass: bounded-iter -- four fixed LP sources. */).zip(&producer_custodies) {
        assert_ok!(Ops::transfer(producer, alice, AssetKind::Local(*asset), 1));
      }
      let custody = Actors::actor_identity(target).unwrap().sovereign_account;
      if fund_target {
        // LP ledgers are not sufficient assets: establish the native provider first.
        native_transfer(alice, &custody, deos_runtime::UNIT);
        assert_ok!(Ops::transfer(
          alice,
          &custody,
          AssetKind::Local(assets[0]),
          117,
        ));
      }
      let mut temporary_contract = parked_contract.clone();
      temporary_contract.steps = vec![StepOf::<Runtime> {
        precondition: None,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .unwrap();
      assert_ok!(Actors::activate_actor(
        RuntimeOrigin::signed(alice.clone()),
        temporary[0],
        temporary_contract.clone()
      ));
      assert_eq!(
        (Actors::actor_identity_count(), Actors::active_actor_count()),
        (PREPARED_IDENTITIES, PREPARED_ACTIVE_ACTORS)
      );
      for source in &sources {
        let header = DependencyRegistrationHeaders::<Runtime>::get(source);
        assert_eq!((header.count, header.free_count), (PAGES as u32 + 1, count));
      }
      assert!(!PendingDependencyEvents::<Runtime>::contains_key(target));
      assert_eq!(
        DependencyTimedReviews::<Runtime>::get(target)
          .unwrap()
          .deadline,
        WakeupKey::Block(2)
      );
      #[cfg(feature = "try-runtime")]
      {
        println!("admission preparation try-state begins: preset={preset}");
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(1).unwrap();
        println!("admission preparation try-state passed: preset={preset}");
      }
      (
        target,
        Actors::actor_identity(target).unwrap(),
        target_contract,
        sources,
        queries,
        assets[0],
        temporary,
        temporary_contract,
        dormant_holds,
        Balances::free_balance(&custody),
        DependencySourceAllocatorState::<Runtime>::get().encode(),
      )
    });
  println!("admission preparation trie commit begins: preset={preset}");
  ext.commit_all().unwrap();
  println!("admission preparation snapshot begins: preset={preset}");
  let top: std::collections::BTreeMap<Vec<u8>, Vec<u8>> = ext
    .backend
    .pairs(Default::default())
    .unwrap()
    .map(Result::unwrap)
    .collect();
  assert!(!top.keys().any(|key| key.starts_with(b":child_storage:")));
  println!(
    "admission preparation snapshot complete: preset={preset} keys={}",
    top.len()
  );
  let committed = retain_backend.then_some(ext);
  Prepared {
    committed,
    storage: Storage {
      top,
      children_default: Default::default(),
    },
    target,
    identity,
    contract: parked_contract,
    sources,
    queries,
    first_asset,
    temporary,
    temporary_contract,
    dormant_holds,
    native_before,
    allocator,
  }
}

pub(super) fn query_proof(ext: &mut TestExternalities, queries: &[Query]) -> StorageProof {
  // The independent importer's current pre-state cannot prime the author recorder.
  let recorder = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    for (key, expected) in queries {
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(key).map(|value| value.to_vec()),
        *expected
      );
    }
  });
  let proof = recorder.to_storage_proof();
  let root = *ext.as_backend().root();
  let database = proof.clone().into_memory_db::<Blake2Hasher>();
  let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&database, &root).build();
  for (key, expected) in queries {
    assert_eq!(trie.get(key).unwrap(), *expected);
  }
  proof
}

pub(super) fn recorded_nodes(recorder: &Recorder<Blake2Hasher>) -> BTreeSet<Vec<u8>> {
  recorder.to_storage_proof().iter_nodes().cloned().collect()
}

pub(super) fn recorded_keys(recorder: &Recorder<Blake2Hasher>) -> BTreeSet<Vec<u8>> {
  recorder
    .recorded_keys()
    .into_values()
    .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
    .collect()
}
