//! Independent executable oracle for the current-state Actors service contract.
//!
//! This model deliberately imports no pallet type or scheduler implementation. It decides logical
//! residence, block-round mutation, Q1, retry/recurrence, parking invalidation, disablement, and
//! generation-safe retirement. Production implementations must reproduce these outcomes without
//! treating this test's in-memory collections as prescribed storage geometry.

use std::collections::{BTreeMap, VecDeque};

type ActorId = u8;
type Generation = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Residence {
  Live,
  Sleeping { due: u32 },
  Parked { watched_revision: u32 },
  Pending { observed_revision: u32 },
  Disabled,
  Retired { reclaim_cursor: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceDimension {
  RefTime,
  ProofSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Process {
  Idle,
  Running { cursor: u8 },
  Retry { cursor: u8, due: u32, attempts: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Actor {
  generation: Generation,
  residence: Residence,
  process: Process,
  admitted_in: u32,
  last_turn: Option<u32>,
  last_commit: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParkedBalance {
  generation: Generation,
  episode: u32,
  anchor: u64,
  current: u64,
  threshold: u64,
  revision: u32,
}

#[derive(Default)]
struct Oracle {
  block: u32,
  ring: VecDeque<(ActorId, Generation)>,
  actors: BTreeMap<ActorId, Actor>,
  parked_balances: BTreeMap<ActorId, ParkedBalance>,
  cursor: Option<(ActorId, Generation)>,
  round: VecDeque<(ActorId, Generation)>,
}

impl Oracle {
  fn insert(&mut self, id: ActorId, actor: Actor) {
    self.actors.insert(id, actor);
    if actor.residence == Residence::Live {
      let member = (id, actor.generation);
      self.insert_before_cursor(member);
      self.cursor.get_or_insert(member);
    }
  }

  fn begin_block(&mut self, block: u32) {
    assert!(block > self.block);
    self.block = block;
    self.round = self.ring.iter().copied().collect();
    if let Some(cursor) = self.cursor
      && let Some(position) = self.round.iter().position(|member| *member == cursor)
    {
      self.round.rotate_left(position);
    }
  }

  /// Observes the next eligible candidate without consuming its turn or moving the cursor.
  fn peek(&mut self) -> Option<ActorId> {
    loop {
      let (id, generation) = *self.round.front()?;
      let Some(actor) = self.actors.get(&id) else {
        self.round.pop_front();
        continue;
      };
      if actor.generation != generation
        || actor.residence != Residence::Live
        || actor.admitted_in >= self.block
        || actor.last_turn == Some(self.block)
      {
        self.round.pop_front();
        continue;
      }
      return Some(id);
    }
  }

  /// Commits one admitted semantic turn. Resource refusal calls only `peek` and changes nothing.
  fn admit(&mut self, id: ActorId) {
    let member = self
      .round
      .pop_front()
      .expect("admission requires a candidate");
    assert_eq!(member.0, id, "admission must consume the observed head");
    let actor = self.actors.get_mut(&id).expect("candidate remains current");
    assert_eq!(actor.generation, member.1);
    assert_eq!(actor.residence, Residence::Live);
    assert_ne!(actor.last_turn, Some(self.block));
    actor.last_turn = Some(self.block);
    self.cursor = self.successor(member);
  }

  fn next(&mut self) -> Option<ActorId> {
    let id = self.peek()?;
    self.admit(id);
    Some(id)
  }

  fn refuse(&mut self, id: ActorId, _: ResourceDimension) {
    assert_eq!(self.peek(), Some(id));
  }

  fn successor(&self, member: (ActorId, Generation)) -> Option<(ActorId, Generation)> {
    let position = self.ring.iter().position(|entry| *entry == member)?;
    self.ring.get((position + 1) % self.ring.len()).copied()
  }

  fn commit_step(&mut self, id: ActorId, next_cursor: u8) {
    let actor = self.actors.get_mut(&id).unwrap();
    assert_eq!(actor.last_turn, Some(self.block));
    assert_ne!(actor.last_commit, Some(self.block), "Q1 duplicate commit");
    actor.last_commit = Some(self.block);
    actor.process = Process::Running {
      cursor: next_cursor,
    };
    // Resident continuation retains its ring membership and receives no second round entry.
  }

  fn retry(&mut self, id: ActorId, cursor: u8, due: u32, attempts: u8) {
    let adjacent_round = due <= self.block.saturating_add(1);
    if !adjacent_round {
      self.detach(id);
    }
    let actor = self.actors.get_mut(&id).unwrap();
    actor.process = Process::Retry {
      cursor,
      due,
      attempts,
    };
    if !adjacent_round {
      actor.residence = Residence::Sleeping { due };
    }
  }

  fn complete_level_sensitive(&mut self, id: ActorId, still_true: bool, revision: u32) {
    let actor = self.actors.get_mut(&id).unwrap();
    actor.process = Process::Idle;
    if still_true {
      // It remains live, but the same block's turn/commit guards survive completion.
      assert_eq!(actor.residence, Residence::Live);
    } else {
      self.detach(id);
      self.actors.get_mut(&id).unwrap().residence = Residence::Parked {
        watched_revision: revision,
      };
    }
  }

  fn arm_parked_balance(&mut self, id: ActorId, current: u64, threshold: u64, episode: u32) {
    assert!(threshold > 0);
    self.detach(id);
    let generation = self.actors[&id].generation;
    self.actors.get_mut(&id).unwrap().residence = Residence::Parked {
      watched_revision: 0,
    };
    self.parked_balances.insert(
      id,
      ParkedBalance {
        generation,
        episode,
        anchor: current,
        current,
        threshold,
        revision: 0,
      },
    );
  }

  fn change_parked_balance(&mut self, id: ActorId, current: u64) {
    let Some(watch) = self.parked_balances.get_mut(&id) else {
      return; // Busy, disabled, retired, and unconfigured Actors own no watch.
    };
    if self.actors[&id].generation != watch.generation
      || !matches!(
        self.actors[&id].residence,
        Residence::Parked { .. } | Residence::Pending { .. }
      )
    {
      return;
    }
    watch.current = current;
    watch.revision = watch.revision.checked_add(1).unwrap();
    let revision = watch.revision;
    let qualified = current.abs_diff(watch.anchor) >= watch.threshold;
    if qualified || matches!(self.actors[&id].residence, Residence::Pending { .. }) {
      self.invalidate(id, revision);
    }
  }

  fn check_parked_balance(&mut self, id: ActorId, start_condition: bool) {
    let watch = self.parked_balances[&id];
    let qualified = watch.current.abs_diff(watch.anchor) >= watch.threshold;
    self.check_pending(id, qualified && start_condition, watch.revision);
    if self.actors[&id].residence == Residence::Live {
      self.parked_balances.remove(&id);
    }
  }

  fn wake_due(&mut self, id: ActorId) {
    let actor = self.actors.get(&id).copied().unwrap();
    let Residence::Sleeping { due } = actor.residence else {
      panic!("not sleeping")
    };
    assert!(due <= self.block);
    self.admit_live(id);
  }

  fn invalidate(&mut self, id: ActorId, revision: u32) {
    let actor = self.actors.get_mut(&id).unwrap();
    match actor.residence {
      Residence::Parked { watched_revision } if revision > watched_revision => {
        actor.residence = Residence::Pending {
          observed_revision: revision,
        };
      }
      Residence::Pending { observed_revision } if revision > observed_revision => {
        actor.residence = Residence::Pending {
          observed_revision: revision,
        };
      }
      Residence::Parked { .. } | Residence::Pending { .. } => {}
      Residence::Disabled | Residence::Retired { .. } => {}
      Residence::Live | Residence::Sleeping { .. } => {
        // Busy/current continuation acquires no future-cycle promise.
      }
    }
  }

  fn check_pending(&mut self, id: ActorId, condition: bool, covered_revision: u32) {
    let actor = self.actors.get(&id).copied().unwrap();
    let Residence::Pending { observed_revision } = actor.residence else {
      panic!("not pending")
    };
    assert!(covered_revision <= observed_revision);
    if covered_revision < observed_revision {
      return; // A later invalidation remains owed: never clear it as covered.
    }
    if condition {
      self.admit_live(id);
    } else {
      self.actors.get_mut(&id).unwrap().residence = Residence::Parked {
        watched_revision: covered_revision,
      };
    }
  }

  fn disable(&mut self, id: ActorId) {
    self.detach(id);
    self.actors.get_mut(&id).unwrap().residence = Residence::Disabled;
  }

  fn resume(&mut self, id: ActorId) {
    assert_eq!(self.actors[&id].residence, Residence::Disabled);
    self.admit_live(id);
  }

  fn retire(&mut self, id: ActorId) -> Generation {
    self.detach(id);
    let actor = self.actors.get_mut(&id).unwrap();
    let retired = actor.generation;
    actor.residence = Residence::Retired { reclaim_cursor: 0 };
    retired
  }

  fn recreate(&mut self, id: ActorId) {
    let old = self.actors[&id];
    let generation = old.generation.checked_add(1).unwrap();
    self.actors.insert(
      id,
      Actor {
        generation,
        residence: Residence::Live,
        process: Process::Idle,
        admitted_in: self.block,
        last_turn: None,
        last_commit: None,
      },
    );
    let member = (id, generation);
    self.insert_before_cursor(member);
    self.cursor.get_or_insert(member);
  }

  fn reclaim(&mut self, id: ActorId, generation: Generation) -> bool {
    let actor = self.actors.get_mut(&id).unwrap();
    if actor.generation != generation || !matches!(actor.residence, Residence::Retired { .. }) {
      return false;
    }
    actor.residence = Residence::Retired { reclaim_cursor: 1 };
    true
  }

  fn admit_live(&mut self, id: ActorId) {
    let actor = self.actors.get_mut(&id).unwrap();
    actor.residence = Residence::Live;
    actor.admitted_in = self.block;
    let member = (id, actor.generation);
    assert!(!self.ring.contains(&member), "duplicate live residence");
    self.insert_before_cursor(member);
  }

  /// Appends behind every current resident in encounter order, immediately before the cursor.
  fn insert_before_cursor(&mut self, member: (ActorId, Generation)) {
    if let Some(cursor) = self.cursor
      && let Some(position) = self.ring.iter().position(|entry| *entry == cursor)
    {
      self.ring.insert(position, member);
    } else {
      self.ring.push_back(member);
    }
  }

  fn detach(&mut self, id: ActorId) {
    let member = (id, self.actors[&id].generation);
    if self.cursor == Some(member) {
      self.cursor = if self.ring.len() == 1 {
        None
      } else {
        self.successor(member)
      };
    }
    self.ring.retain(|entry| *entry != member);
    self.round.retain(|entry| *entry != member);
    self.parked_balances.remove(&id);
  }
}

fn live(generation: Generation) -> Actor {
  Actor {
    generation,
    residence: Residence::Live,
    process: Process::Idle,
    admitted_in: 0,
    last_turn: None,
    last_commit: None,
  }
}

#[test]
fn mutable_round_preserves_survivor_order_and_defers_new_or_reentered_members() {
  let mut o = Oracle::default();
  for id in 1..=4 {
    o.insert(id, live(0));
  }
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.disable(2); // remove the next member
  o.disable(4); // remove the tail
  o.insert(
    5,
    Actor {
      admitted_in: 1,
      ..live(0)
    },
  ); // new member is outside this round
  o.disable(1);
  o.resume(1); // reentry cannot donate/reset a same-block turn
  assert_eq!(o.next(), Some(3));
  assert_eq!(o.next(), None);
  o.begin_block(2);
  assert_eq!([o.next(), o.next(), o.next()], [Some(5), Some(1), Some(3)]);
}

#[test]
fn partial_round_admission_appends_behind_all_current_residents() {
  let mut o = Oracle::default();
  for id in 1..=3 {
    o.insert(id, live(0));
  }
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.insert(
    4,
    Actor {
      admitted_in: 1,
      ..live(0)
    },
  );
  // The partial round stops here; admission does not splice ahead of B, C, or wrapped A.
  o.begin_block(2);
  assert_eq!(
    [o.next(), o.next(), o.next(), o.next()],
    [Some(2), Some(3), Some(1), Some(4)]
  );
}

#[test]
fn production_round_trace_preserves_partial_continuity_and_refused_head() {
  let mut o = Oracle::default();
  for id in 120..=122 {
    o.insert(id, live(0));
  }

  let mut block = 0;
  for row in include_str!("fixtures/current_state_round_trace_v1.tsv").lines() {
    if row.starts_with('#') || row.is_empty() {
      continue;
    }
    let mut fields = row.split('\t');
    let next_block = fields.next().unwrap().parse::<u32>().unwrap();
    let expected = fields.next().unwrap().parse::<u8>().unwrap();
    let action = fields.next().unwrap();
    assert!(fields.next().is_none());
    if next_block != block {
      o.begin_block(next_block);
      block = next_block;
    }
    assert_eq!(o.peek(), Some(expected));
    match action {
      "admit" => o.admit(expected),
      "refuse_ref_time" => o.refuse(expected, ResourceDimension::RefTime),
      "refuse_proof_size" => o.refuse(expected, ResourceDimension::ProofSize),
      _ => panic!("unknown trace action: {action}"),
    }
  }
}

#[test]
fn partial_round_continues_between_passes_without_reopening_the_block() {
  let mut o = Oracle::default();
  for id in 1..=3 {
    o.insert(id, live(0));
  }

  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  // A later pass in the same immutable round resumes from the retained snapshot frontier.
  assert_eq!(o.next(), Some(2));
  assert_eq!(o.next(), Some(3));
  assert_eq!(o.next(), None);
}

#[test]
fn resource_refusal_preserves_the_candidate_and_next_round_priority() {
  for dimension in [ResourceDimension::RefTime, ResourceDimension::ProofSize] {
    let mut o = Oracle::default();
    for id in 1..=3 {
      o.insert(id, live(0));
    }
    o.begin_block(1);
    assert_eq!(o.next(), Some(1));
    assert_eq!(o.peek(), Some(2));
    o.refuse(2, dimension);
    assert_eq!(o.peek(), Some(2), "same-block pass cannot bypass B");

    o.begin_block(2);
    assert_eq!(o.peek(), Some(2), "next block cannot begin at C");
    o.admit(2);
    assert_eq!(o.next(), Some(3));
  }
}

#[test]
fn resident_multiblock_pipeline_and_level_recurrence_obey_q1() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.commit_step(1, 1);
  assert_eq!(o.next(), None);
  o.begin_block(2);
  assert_eq!(o.next(), Some(1));
  o.commit_step(1, 2);
  o.complete_level_sensitive(1, true, 7);
  assert_eq!(o.next(), None);
  o.begin_block(3);
  assert_eq!(o.next(), Some(1));
}

#[test]
fn adjacent_round_retry_remains_resident_with_the_same_cursor_and_attempt_count() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.insert(2, live(0));
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.retry(1, 2, 2, 1);
  assert_eq!(o.actors[&1].residence, Residence::Live);
  assert_eq!(o.next(), Some(2));
  o.begin_block(2);
  assert_eq!(o.next(), Some(1));
  assert_eq!(
    o.actors[&1].process,
    Process::Retry {
      cursor: 2,
      due: 2,
      attempts: 1,
    }
  );
}

#[test]
fn later_retry_sleeps_then_returns_without_resetting_cursor_or_attempts() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.retry(1, 2, 4, 1);
  o.begin_block(3);
  assert_eq!(o.next(), None);
  o.begin_block(4);
  o.wake_due(1);
  assert_eq!(o.next(), None); // wake/reentry in B is eligible in B+1
  o.begin_block(5);
  assert_eq!(o.next(), Some(1));
  assert_eq!(
    o.actors[&1].process,
    Process::Retry {
      cursor: 2,
      due: 4,
      attempts: 1
    }
  );
}

#[test]
fn parking_invalidation_coalesces_and_cannot_lose_a_later_revision() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.begin_block(1);
  assert_eq!(o.next(), Some(1));
  o.complete_level_sensitive(1, false, 10);
  o.invalidate(1, 11);
  o.invalidate(1, 12);
  o.check_pending(1, false, 11); // stale evaluation cannot acknowledge revision 12
  assert_eq!(
    o.actors[&1].residence,
    Residence::Pending {
      observed_revision: 12
    }
  );
  o.check_pending(1, true, 12);
  assert_eq!(o.actors[&1].residence, Residence::Live);
  assert_eq!(o.next(), None);
  o.begin_block(2);
  assert_eq!(o.next(), Some(1));
}

#[test]
fn parked_balance_keeps_a_fixed_anchor_and_tracks_only_parked_episodes() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.arm_parked_balance(1, 1_000, 100, 1);
  o.change_parked_balance(1, 1_060);
  assert!(matches!(o.actors[&1].residence, Residence::Parked { .. }));
  o.change_parked_balance(1, 1_100);
  assert!(matches!(o.actors[&1].residence, Residence::Pending { .. }));

  // A negative start decision preserves the final-cycle anchor, not the latest sample.
  o.check_parked_balance(1, false);
  assert!(matches!(o.actors[&1].residence, Residence::Parked { .. }));
  assert_eq!(o.parked_balances[&1].anchor, 1_000);
  o.change_parked_balance(1, 1_120);
  o.check_parked_balance(1, true);
  assert_eq!(o.actors[&1].residence, Residence::Live);
  assert!(!o.parked_balances.contains_key(&1));

  // Busy-period changes have no baseline, accumulator, or future-cycle authority.
  o.change_parked_balance(1, 2_000);
  assert_eq!(o.actors[&1].residence, Residence::Live);
}

#[test]
fn parked_balance_reversal_and_generation_replacement_do_not_leak_authority() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.arm_parked_balance(1, 500, 100, 7);
  o.change_parked_balance(1, 600);
  o.change_parked_balance(1, 500);
  assert!(matches!(o.actors[&1].residence, Residence::Pending { .. }));
  o.check_parked_balance(1, true);
  assert!(matches!(o.actors[&1].residence, Residence::Parked { .. }));
  assert_eq!(o.parked_balances[&1].episode, 7);

  o.retire(1);
  o.recreate(1);
  o.change_parked_balance(1, 700);
  assert_eq!(o.actors[&1].residence, Residence::Live);
}

#[test]
fn disabled_and_retired_generations_ignore_ordinary_wakes_and_stale_cleanup() {
  let mut o = Oracle::default();
  o.insert(1, live(0));
  o.disable(1);
  o.invalidate(1, 1);
  assert_eq!(o.actors[&1].residence, Residence::Disabled);
  o.begin_block(1);
  o.resume(1);
  assert_eq!(o.next(), None); // resumed in this block
  let retired = o.retire(1);
  o.recreate(1);
  assert!(
    !o.reclaim(1, retired),
    "old cleanup must not mutate the new generation"
  );
  o.invalidate(1, 2);
  assert_eq!(o.actors[&1].residence, Residence::Live);
}
