//! Boundary checks for the finite native H1 corpus, not production maxima or trie proofs.

use crate::{Actors, Runtime};
use codec::Encode;
use pallet_deos_actors::{self as actors, GenesisSystemActors, ProcessResidence, ProcessStatus};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ACTORS: usize = 105;
const MAX_STEPS: u32 = 12;

fn bounded<I: Iterator>(iter: I, maximum: usize) -> Vec<I::Item> {
  let rows = iter.take(maximum + 1).collect::<Vec<_>>();
  assert!(rows.len() <= maximum, "outside the qualified H1 geometry");
  rows
}

/// Called outside every event-observer scope; all iteration is native test-only validation.
pub(super) fn check(now: u32) -> Value {
  assert!((2..=14).contains(&now));
  type Genesis = <Runtime as actors::Config>::GenesisSystemActors;
  let first_id = Genesis::system_actors()
    .into_iter()
    .map(|(id, _, _, _)| id)
    .chain(
      Genesis::dormant_system_actors()
        .into_iter()
        .map(|(id, _, _)| id),
    )
    .max()
    .map_or(0, |id| id.checked_add(1).unwrap());
  assert!((first_id..=first_id + MAX_ACTORS as u64).contains(&Actors::next_actor_id()));
  let semantic = bounded(actors::ActorSemanticStates::<Runtime>::iter(), MAX_ACTORS);
  let processes = bounded(actors::ActorProcesses::<Runtime>::iter(), MAX_ACTORS)
    .into_iter()
    .collect::<BTreeMap<_, _>>();
  let heads = bounded(actors::ActorContractHeads::<Runtime>::iter(), MAX_ACTORS)
    .into_iter()
    .collect::<BTreeMap<_, _>>();
  let nodes = bounded(actors::ServiceNodes::<Runtime>::iter(), MAX_ACTORS)
    .into_iter()
    .collect::<BTreeMap<_, _>>();
  let runs = bounded(actors::ActorRunStateStore::<Runtime>::iter(), MAX_ACTORS)
    .into_iter()
    .collect::<BTreeMap<_, _>>();
  let chunks = bounded(
    actors::ActorContractTailChunks::<Runtime>::iter(),
    3 * MAX_ACTORS,
  );
  let handles = bounded(actors::DeadlineHandles::<Runtime>::iter(), 1);
  let holds = bounded(actors::ActorStateHolds::<Runtime>::iter(), 2);
  let sovereigns = bounded(actors::SovereignIndex::<Runtime>::iter(), MAX_ACTORS);
  assert_eq!(Actors::active_actor_count() as usize, semantic.len());
  assert_eq!(Actors::actor_identity_count() as usize, semantic.len());
  assert_eq!(processes.len(), semantic.len());
  assert_eq!(heads.len(), semantic.len());
  assert_eq!(sovereigns.len(), semantic.len());
  let mut expected_chunks = BTreeSet::new();
  let mut expected_nodes = BTreeSet::new();
  let mut expected_deadlines = BTreeSet::new();
  let mut expected_holds = BTreeSet::new();
  let mut maximum_semantic_bytes = 0;
  for (id, state) in &semantic {
    assert!((*id >= first_id) && *id < Actors::next_actor_id());
    maximum_semantic_bytes = maximum_semantic_bytes.max(state.encoded_size());
    let actors::ActorSemanticState::Active(record) = state else {
      panic!("H1 has no dormant Actor");
    };
    assert_eq!(record.generation, 1);
    assert_eq!(record.identity.mutability, actors::Mutability::Mutable);
    assert_eq!(
      actors::SovereignIndex::<Runtime>::get(&record.identity.sovereign_account),
      Some(*id)
    );
    if record.identity.actor_class.actor_type() == actors::ActorType::User {
      expected_holds.insert(*id);
    }
    let head = &heads[id];
    assert!((1..=MAX_STEPS).contains(&head.header.step_count));
    assert_eq!(head.header.trigger, actors::Trigger::manual());
    assert_eq!(head.header.cooldown_blocks, 0);
    assert!(head.header.window.is_none());
    assert!(!head.header.has_close_when);
    assert!(head.header.parked_balance_activation.is_none());
    assert_eq!(head.header.on_unfunded, actors::OnUnfunded::Close);
    assert_eq!(head.header.funding, actors::FundingSourcePolicy::OwnerOnly);
    assert!(matches!(
      head.header.auto_close_at_cycle_nonce,
      None | Some(1)
    ));
    assert_eq!(
      head.header.semantic_contract_id,
      record.admission.semantic_contract_id
    );
    assert_eq!(
      head.header.body_commitment,
      record.admission.body_commitment
    );
    assert_eq!(
      head.header.admission_identity,
      record.admission.admission_identity
    );
    assert!(head.first_step.as_ref().unwrap().precondition.is_none());
    assert!(head.first_step_resources.is_some());
    for chunk in 0..(head.header.step_count - 1).div_ceil(actors::MAX_STEPS_PER_TAIL_CHUNK) {
      expected_chunks.insert((*id, chunk));
    }
    let process = processes[id];
    assert_eq!(process.generation, record.generation);
    match (process.status, process.residence) {
      (ProcessStatus::Serving, Some(ProcessResidence::Service(kind))) => {
        expected_nodes.insert(*id);
        let node = nodes[id];
        assert_eq!(node.generation, record.generation);
        assert_eq!(node.kind, kind);
        assert!(node.last_considered <= now && node.eligible_from <= now + 1);
      }
      (ProcessStatus::Serving, Some(ProcessResidence::Deadline { key, page, slot })) => {
        expected_deadlines.insert(*id);
        let (_, handle) = handles.iter().find(|(owner, _)| owner == id).unwrap();
        assert_eq!(
          (handle.actor.actor_id, handle.actor.generation),
          (*id, record.generation)
        );
        assert_eq!((handle.key, handle.page, handle.slot), (key, page, slot));
      }
      (ProcessStatus::Disabled(disabled), None) => {
        assert_eq!(disabled.cause, actors::ProcessDisableCause::Protocol);
        assert!(!runs.contains_key(id));
      }
      _ => panic!("unexpected H1 process domain"),
    }
    if let Some(run) = runs.get(id) {
      assert!(run.cursor < head.header.step_count);
      assert_eq!(run.cycle_nonce, 1);
      assert!(run.balance_snapshot.is_none());
      assert!(run.unsuccessful_attempts_at_cursor <= 2);
      assert!(run.eligible_at <= 15);
      assert_eq!(
        run.contract_authority.semantic_contract_id,
        head.header.semantic_contract_id
      );
      assert_eq!(
        run.contract_authority.body_commitment,
        head.header.body_commitment
      );
      assert_eq!(
        run.contract_authority.admission_identity,
        head.header.admission_identity
      );
    }
  }
  assert!(runs.keys().all(|id| heads.contains_key(id)));
  assert_eq!(
    nodes.keys().copied().collect::<BTreeSet<_>>(),
    expected_nodes
  );
  assert_eq!(
    handles.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>(),
    expected_deadlines
  );
  assert_eq!(
    holds.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>(),
    expected_holds
  );
  assert_eq!(
    chunks
      .iter()
      .map(|(id, index, _)| (*id, *index))
      .collect::<BTreeSet<_>>(),
    expected_chunks
  );
  for (id, index, chunk) in &chunks {
    let head = &heads[id];
    assert_eq!(
      chunk.first_step_index,
      1 + index * actors::MAX_STEPS_PER_TAIL_CHUNK
    );
    assert_eq!(chunk.steps.len(), chunk.step_resources.len());
    assert_eq!(
      chunk.steps.len() as u32,
      (head.header.step_count - chunk.first_step_index).min(actors::MAX_STEPS_PER_TAIL_CHUNK)
    );
    assert_eq!(chunk.authority.actor_id, *id);
    assert_eq!(
      chunk.authority.semantic_contract_id,
      head.header.semantic_contract_id
    );
    assert_eq!(chunk.authority.body_commitment, head.header.body_commitment);
    assert_eq!(
      chunk.authority.admission_identity,
      head.header.admission_identity
    );
    assert!(chunk.steps.iter().all(|step| step.precondition.is_none()));
  }
  let ring = Actors::service_header();
  assert_eq!(ring.count as usize, nodes.len());
  assert_eq!(ring.cursor.is_none(), nodes.is_empty());
  if let Some(start) = ring.cursor {
    let mut current = start;
    let mut seen = BTreeSet::new();
    for _ in 0..nodes.len() {
      assert!(seen.insert(current.actor_id));
      let node = nodes[&current.actor_id];
      assert_eq!(node.generation, current.generation);
      let previous = nodes[&node.previous.actor_id];
      let next = nodes[&node.next.actor_id];
      assert_eq!(previous.generation, node.previous.generation);
      assert_eq!(next.generation, node.next.generation);
      assert_eq!(previous.next, current);
      assert_eq!(next.previous, current);
      current = node.next;
    }
    assert_eq!(current, start);
    assert_eq!(seen.len(), nodes.len());
  }
  let headers = bounded(actors::DeadlineHeaders::<Runtime>::iter(), 1);
  let pages = bounded(actors::DeadlinePages::<Runtime>::iter(), 1);
  let index_pages = bounded(actors::DeadlineIndexPages::<Runtime>::iter(), 1);
  let positions = bounded(actors::DeadlineIndexPositions::<Runtime>::iter(), 1);
  let key = actors::WakeupKey::Block(7);
  assert_eq!(Actors::deadline_index_len(actors::WakeupClock::Tick), 0);
  assert_eq!(
    Actors::deadline_index_len(actors::WakeupClock::Block),
    handles.len() as u32
  );
  if let Some((_, handle)) = handles.first() {
    assert_eq!((handle.key, handle.page, handle.slot), (key, 0, 0));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, key);
    let header = headers[0].1;
    assert_eq!(
      (header.first_page, header.last_page, header.next_page),
      (0, 0, 1)
    );
    assert_eq!(
      (header.count, header.page_count, header.first_vacant_page),
      (1, 1, Some(0))
    );
    assert_eq!(pages.len(), 1);
    assert_eq!((pages[0].0, pages[0].1), (key, 0));
    let page = &pages[0].2;
    assert_eq!(page.entries.len(), 32);
    assert_eq!(page.live_entries, 1);
    assert_eq!(page.entries[0], Some(handle.actor));
    assert!(page.entries[1..].iter().all(Option::is_none));
    assert_eq!(
      (
        page.previous_page,
        page.next_page,
        page.previous_vacant_page,
        page.next_vacant_page
      ),
      (None, None, None, None)
    );
    assert_eq!(positions, vec![(key, 0)]);
    assert_eq!(index_pages.len(), 1);
    assert_eq!(
      (index_pages[0].0, index_pages[0].1),
      (actors::WakeupClock::Block, 0)
    );
    assert_eq!(index_pages[0].2.as_slice(), &[key]);
  } else {
    assert!(
      headers.is_empty() && pages.is_empty() && index_pages.is_empty() && positions.is_empty()
    );
  }
  macro_rules! empty {
    ($($map:ident),+ $(,)?) => { $(
      assert!(actors::$map::<Runtime>::iter_keys().next().is_none(), stringify!($map));
    )+ };
  }
  empty!(
    ActorIdentities,
    ActorBehaviorPolicies,
    ActorBehaviorMonitors,
    ActorFinancialWaits,
    TriggerDeadlineHandles,
    ActorControlLocators,
    ActorUnsignaledControlCells,
    ActorReadyFrameChunks,
    ActorWaitingFrameChunks,
    ActorWaitingHeads,
    ActorWaitingTails,
    ActorWaitingOccupancies,
    ActorWaitingCursorIndices,
    ParkedBalanceEpisodes,
    DependencyPlans,
    DependencyTimedReviews,
    PendingDependencyEvents,
    PendingDependencyReviews,
    PendingCheckOwners,
    DependencyRegistrations,
    DependencyRegistrationHeaders,
    DependencyRegistrationPages,
    DependencyRegistrationPositions,
    DependencyRegistrationFreePositions,
    DependencyRegistrationFreeIndices,
    BalanceDependencySources,
    DependencySourceBalances,
    DependencyRevisions,
    DependencyScanSourceNodes
  );
  assert_eq!(actors::ActorReadyOccupancy::<Runtime>::get(), 0);
  assert_eq!(
    actors::ActorReadyHead::<Runtime>::get(),
    actors::ActorReadyTail::<Runtime>::get()
  );
  json!({
    "live_actors": semantic.len(), "generation": if semantic.is_empty() { None } else { Some(1) },
    "first_id": first_id, "next_id": Actors::next_actor_id(),
    "service_nodes": nodes.len(), "runs": runs.len(), "holds": holds.len(),
    "heads": heads.len(), "tail_chunks": chunks.len(), "retry_deadlines": handles.len(),
    "max_semantic_bytes": maximum_semantic_bytes,
    "max_head_bytes": heads.values().map(Encode::encoded_size).max().unwrap_or(0),
    "max_tail_bytes": chunks.iter().map(|(_, _, chunk)| chunk.encoded_size()).max().unwrap_or(0),
    "max_run_bytes": runs.values().map(Encode::encoded_size).max().unwrap_or(0),
    "deadline_page_bytes": pages.first().map(|(_, _, page)| page.encoded_size()).unwrap_or(0),
    "deadline_header_bytes": headers.first().map(|(_, header)| header.encoded_size()).unwrap_or(0),
    "optional_and_legacy_maps_empty": true,
  })
}
