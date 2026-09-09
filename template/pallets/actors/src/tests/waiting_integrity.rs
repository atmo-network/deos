//! Focused corruption and ownership witnesses for the canonical Waiting container.

use super::*;
use crate::{
  ActorControlCellOf, ActorControlLocation, ActorControlLocators, ActorUnsignaledControlCells,
  ActorWaitingEntry, ActorWaitingFrameChunks, ActorWaitingOccupancies,
  scheduler::{ActorControlTransitionError, ActorWaitingAuthority},
};
use polkadot_sdk::sp_runtime::StateVersion;

#[test]
fn mixed_waiting_pages_reclaim_both_entry_kinds_through_real_close() {
  for primary_first in [true, false] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let primaries = (0..33)
        .map(|_| create_system_with(ALICE, timer_schedule(200), None, inert_contract_steps()))
        .collect::<Vec<_>>();
      let step = inert_contract_steps()[0].clone();
      let steps = BoundedVec::try_from(vec![step.clone(), step]).expect("two Steps fit");
      let references = (0..33)
        .map(|_| create_system_with(ALICE, timer_schedule(100), None, steps.clone()))
        .collect::<Vec<_>>();
      for block in 101..=111 {
        frame_system::Pallet::<Test>::set_block_number(block);
        Actors::on_idle(block, Weight::MAX);
      }
      let key = WakeupKey::Tick(201);
      for (index, actor) in references.iter().enumerate() {
        let run = Actors::actor_run_state(*actor).expect("real Opening retains a suffix");
        assert_eq!(run.cursor, 1);
        assert_eq!(run.last_committed_step_block, Some(101 + index as u64 / 3));
        let pointer = Actors::actor_hot(*actor)
          .expect("Running Actor")
          .trigger_wakeup_pointer
          .expect("Running cadence is rearmed");
        assert_eq!(pointer.tick, 201);
        let page =
          ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id)).expect("rearmed page");
        assert!(matches!(&page.entries[pointer.slot as usize],
          Some(ActorWaitingEntry::Reference(reference)) if reference.actor_id == *actor));
      }
      // Append after real rearming has settled: lifecycle churn need not compact earlier holes.
      let later_primaries = (0..33)
        .map(|_| create_system_with(ALICE, timer_schedule(90), None, inert_contract_steps()))
        .collect::<Vec<_>>();
      let middle = ActorWaitingFrameChunks::<Test>::get((key, 3)).expect("mixed middle page");
      assert!(matches!(&middle.entries[0],
        Some(ActorWaitingEntry::Reference(reference)) if reference.actor_id == references[32]));
      for (entry, actor) in middle.entries.iter().skip(1).zip(&later_primaries[..31]) {
        assert!(matches!(entry, Some(ActorWaitingEntry::Primary(cell)) if cell.actor_id == *actor));
      }
      let assert_directory = |pages: &[(u64, u32)], total, tail| {
        assert_eq!(ActorWaitingOccupancies::<Test>::get(key), total);
        assert_eq!(crate::ActorWaitingTails::<Test>::get(key), tail);
        for (index, (page_id, live)) in pages.iter().enumerate() {
          let page =
            ActorWaitingFrameChunks::<Test>::get((key, *page_id)).expect("linked live page");
          assert_eq!(page.live_entries, *live);
          assert_eq!(
            page.entries.iter().filter(|entry| entry.is_some()).count(),
            *live as usize
          );
          assert_eq!(page.scan_slot, 0);
          assert_eq!(
            page.previous_page,
            index.checked_sub(1).map(|previous| pages[previous].0)
          );
          assert_eq!(page.next_page, pages.get(index + 1).map(|next| next.0));
        }
        assert_eq!(crate::ActorWaitingHeads::<Test>::get(key), 0);
        assert!(crate::ActorWaitingCursorIndices::<Test>::contains_key(key));
        assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), Some(key));
        #[cfg(feature = "try-runtime")]
        {
          let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          assert_ok!(Actors::do_try_state());
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
        }
      };
      assert_directory(&[(0, 32), (1, 1), (2, 32), (3, 32), (4, 2)], 99, 130);
      let mut middle_removals = later_primaries[..31].to_vec();
      if primary_first {
        middle_removals.push(references[32]);
      } else {
        middle_removals.insert(0, references[32]);
      }
      for (index, actor) in middle_removals.into_iter().enumerate() {
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor));
        let remaining = 31 - index as u32;
        if remaining > 0 {
          assert_directory(
            &[(0, 32), (1, 1), (2, 32), (3, remaining), (4, 2)],
            67 + remaining,
            130,
          );
        }
      }
      assert!(!ActorWaitingFrameChunks::<Test>::contains_key((key, 3)));
      assert_directory(&[(0, 32), (1, 1), (2, 32), (4, 2)], 67, 130);
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::root(),
        later_primaries[31]
      ));
      assert_directory(&[(0, 32), (1, 1), (2, 32), (4, 1)], 66, 130);
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::root(),
        later_primaries[32]
      ));
      assert!(!ActorWaitingFrameChunks::<Test>::contains_key((key, 4)));
      assert_directory(&[(0, 32), (1, 1), (2, 32)], 65, 96);
      for (index, actor) in references[..32].iter().enumerate() {
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), *actor));
        if index < 31 {
          assert_directory(
            &[(0, 32), (1, 1), (2, 31 - index as u32)],
            64 - index as u32,
            96,
          );
        }
      }
      assert!(!ActorWaitingFrameChunks::<Test>::contains_key((key, 2)));
      assert_directory(&[(0, 32), (1, 1)], 33, 64);
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), primaries[32]));
      assert!(!ActorWaitingFrameChunks::<Test>::contains_key((key, 1)));
      assert_directory(&[(0, 32)], 32, 32);
      for (index, actor) in primaries[..32].iter().enumerate() {
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), *actor));
        if index < 31 {
          assert_directory(&[(0, 31 - index as u32)], 31 - index as u32, 32);
        }
      }
      assert!(!ActorWaitingFrameChunks::<Test>::contains_key((key, 0)));
      assert!(!ActorWaitingOccupancies::<Test>::contains_key(key));
      assert!(!crate::ActorWaitingHeads::<Test>::contains_key(key));
      assert!(!crate::ActorWaitingTails::<Test>::contains_key(key));
      assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(key));
      assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn primary_inline_authority_matches_canonical_cursor_and_resources() {
  for zero_step in [false, true] {
    for corruption in 0..3 {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let steps = if zero_step {
          BoundedVec::default()
        } else {
          inert_contract_steps()
        };
        let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        assert_ok!(Actors::do_try_state());
        mutate_primary_control_cell(actor_id, |cell| match corruption {
          0 => cell.cursor += 1,
          1 => {
            cell.resources.control = cell
              .resources
              .control
              .saturating_add(Weight::from_parts(1, 0))
          }
          _ => {
            cell.resources.effect = cell
              .resources
              .effect
              .saturating_add(Weight::from_parts(0, 1))
          }
        });
        let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        let expected = if corruption == 0 {
          "ActorControl primary cursor disagrees with canonical Run authority"
        } else {
          "ActorControl primary resources disagree with canonical Step authority"
        };
        assert_eq!(
          Actors::do_try_state(),
          Err(polkadot_sdk::sp_runtime::TryRuntimeError::Other(expected))
        );
        assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
      });
    }
  }
}

fn temporal_primary() -> (
  ActorId,
  ActorControlCellOf<Test>,
  WakeupKey<MockBlockNumber>,
) {
  let actor_id = create_system_with(ALICE, timer_schedule(5), None, inert_contract_steps());
  let (location, cell) = Actors::actor_control_cell(actor_id)
    .expect("ordinary temporal creation installs canonical control authority");
  let ActorControlLocation::Waiting { key, .. } = location else {
    panic!("an unlatched temporal Actor must own a Waiting primary");
  };
  assert!(matches!(key, WakeupKey::Tick(_)));
  (actor_id, cell, key)
}

fn temporal_reference() -> (
  ActorId,
  ActorControlCellOf<Test>,
  WakeupKey<MockBlockNumber>,
) {
  let (actor_id, _, key) = temporal_primary();
  let mut cell = Actors::remove_primary_control_cell_inner(actor_id)
    .expect("fixture moves primary ownership before installing a separate reference");
  cell.hot.trigger_wakeup_pointer = None;
  let cell = Actors::control_schedule_fresh_wakeup_reference(cell, key)
    .expect("one fresh reference retains the same temporal obligation");
  ActorUnsignaledControlCells::<Test>::insert(actor_id, &cell);
  ActorControlLocators::<Test>::insert(actor_id, ActorControlLocation::Unsignaled);
  assert!(!ActorIdentities::<Test>::contains_key(actor_id));
  (actor_id, cell, key)
}

#[test]
fn stale_ready_plan_rolls_back_waiting_primary_demotion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, cell, key) = temporal_primary();
    let location = ActorControlLocators::<Test>::get(actor_id).unwrap();
    let pointer = cell.hot.trigger_wakeup_pointer.unwrap();
    let mut hot = Actors::actor_hot(actor_id).unwrap();
    hot.pending_signal = true;
    let plan = Actors::preflight_paged_enqueue_cohort_with_authority(vec![(actor_id, hot)])
      .expect("retained temporal primary admits a prepared Ready publication");
    let old_tail = Actors::queue_tail();
    let other = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), other));
    assert_eq!(Actors::queue_tail(), old_tail + 1);
    let page_before = ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id)).unwrap();
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let result =
      polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
        let result = Actors::commit_paged_enqueue(plan);
        assert_eq!(
          result,
          Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
        );
        // The stale-tail guard follows source detachment: prove this rollback exercises demotion.
        assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
        let page = ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id)).unwrap();
        let Some(ActorWaitingEntry::Reference(reference)) = &page.entries[pointer.slot as usize]
        else {
          panic!("retained trigger obligation must survive transient primary detachment");
        };
        assert_eq!(reference.actor_id, actor_id);
        assert_eq!(
          reference.admission_identity,
          cell.admission.admission_identity
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
      })
      .expect("transaction nesting is available");
    assert_eq!(
      result,
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert_eq!(ActorControlLocators::<Test>::get(actor_id), Some(location));
    assert_eq!(Actors::actor_control_cell(actor_id), Some((location, cell)));
    assert_eq!(
      ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id)),
      Some(page_before)
    );
  });
}

#[test]
fn waiting_append_rejects_an_existing_primary_without_mutation() {
  for retain_locator in [true, false] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let (actor_id, cell, key) = temporal_primary();
      if !retain_locator {
        // Even a missing locator must not permit overwriting an occupied primary slot.
        ActorControlLocators::<Test>::remove(actor_id);
      }
      let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Actors::control_append_waiting(cell, key, ActorWaitingAuthority::Trigger),
        Err(ActorControlTransitionError::Invariant)
      );
      assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    });
  }
}

#[test]
fn waiting_reference_cannot_collapse_while_another_primary_is_live() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, cell, key) = temporal_reference();
    let pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("reference pointer exists");
    assert!(matches!(
      ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id)).and_then(|page| page
        .entries
        .get(pointer.slot as usize)
        .cloned()
        .flatten()),
      Some(ActorWaitingEntry::Reference(_))
    ));
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::control_append_waiting(cell, key, ActorWaitingAuthority::Trigger),
      Err(ActorControlTransitionError::Invariant)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert_eq!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(ActorControlLocation::Unsignaled)
    );
  });
}

#[test]
fn waiting_removal_rejects_page_and_global_occupancy_disagreement_without_mutation() {
  for (live_entries, corrupt_occupancy) in [(1, 2), (2, 1)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let (actor_id, _, key) = temporal_primary();
      if live_entries == 2 {
        let (_, _, second_key) = temporal_primary();
        assert_eq!(second_key, key);
      }
      assert_eq!(ActorWaitingOccupancies::<Test>::get(key), live_entries);
      ActorWaitingOccupancies::<Test>::insert(key, corrupt_occupancy);
      let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Actors::remove_primary_control_cell_inner(actor_id),
        Err(ActorControlTransitionError::Invariant)
      );
      assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    });
  }
}

#[test]
fn waiting_reference_with_wrong_admission_cannot_resolve_or_latch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, cell, key) = temporal_reference();
    let pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("reference pointer exists");
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
    ActorWaitingFrameChunks::<Test>::mutate((key, pointer.page_id), |page| {
      let entry = page
        .as_mut()
        .and_then(|page| page.entries.get_mut(pointer.slot as usize))
        .and_then(Option::as_mut)
        .expect("reference slot exists");
      let ActorWaitingEntry::Reference(reference) = entry else {
        panic!("fixture owns a separate reference");
      };
      assert_eq!(reference.actor_id, actor_id);
      assert_eq!(
        reference.admission_identity,
        cell.admission.admission_identity
      );
      reference.admission_identity[0] ^= 1;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    assert_eq!(
      Actors::control_due_wakeup_primary(key, 1, pointer.tick),
      Err(ActorControlTransitionError::Invariant)
    );
    assert_eq!(
      Actors::control_latch_due_temporal_reference(key, 1, pointer.tick),
      Err(ActorControlTransitionError::Invariant)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert!(
      !ActorUnsignaledControlCells::<Test>::get(actor_id)
        .expect("original primary remains")
        .hot
        .pending_signal
    );
  });
}

#[test]
fn waiting_primary_pointer_must_address_its_own_slot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let (actor_id, cell, key) = temporal_primary();
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
    let pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("Tick primary pointer exists");
    let reference_slot = pointer.slot + 1;
    ActorWaitingFrameChunks::<Test>::mutate((key, pointer.page_id), |page| {
      let page = page.as_mut().expect("primary page exists");
      assert!(page.entries[reference_slot as usize].is_none());
      page.entries[reference_slot as usize] =
        Some(ActorWaitingEntry::Reference(crate::ActorWakeupReference {
          actor_id,
          admission_identity: cell.admission.admission_identity,
        }));
      page.live_entries += 1;
    });
    ActorWaitingOccupancies::<Test>::mutate(key, |count| *count += 1);
    mutate_primary_control_cell(actor_id, |primary| {
      primary
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("primary pointer exists")
        .slot = reference_slot;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}
