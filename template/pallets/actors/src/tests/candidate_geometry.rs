use super::*;
use crate::{
  ActiveActorCount, ActorContractHeads, ActorIdentityCount, ActorStateHolds, QueueTicket,
};
use codec::{Compact, Encode, MaxEncodedLen};
use polkadot_sdk::sp_weights::Weight;

const ACTOR_CONTROL_TRANSITION_ORACLE: &str =
  include_str!("fixtures/actor_control_transition_oracle_v1.tsv");

fn emit_baseline_oracle<T: std::fmt::Debug>(name: &str, value: &T) {
  let trace = format!("{value:?}");
  let digest = polkadot_sdk::sp_io::hashing::blake2_256(trace.as_bytes());
  let actual = digest
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
  let expected = ACTOR_CONTROL_TRANSITION_ORACLE
    .lines()
    .filter(|line| !line.starts_with('#'))
    .filter_map(|line| line.split_once('|'))
    .find_map(|(scenario, digest)| (scenario == name).then_some(digest))
    .unwrap_or_else(|| panic!("missing immutable reference oracle scenario: {name}"));
  assert_eq!(actual, expected, "immutable reference oracle drift: {name}");
}

#[test]
fn control_baseline_transition_oracle_fixture_is_complete_and_unique() {
  let expected = [
    "address_event_collection_failure",
    "address_event_success",
    "at_time_collection_failure",
    "at_time_success",
    "cadenced_success",
    "direct_funding_unavailable",
    "direct_stop_cycle",
    "direct_temporary_failure",
    "direct_transfer_p0_match_true",
    "direct_transfer_p2_match_false",
    "direct_transfer_p2_match_true",
    "direct_transfer_p4_match_true",
    "funding_retry_fails_false",
    "funding_retry_fails_true",
    "manual_collection_failure",
    "manual_success",
    "middle_running_step",
    "observation_change_collection_failure",
    "observation_change_success",
    "observation_crossing_collection_failure",
    "observation_crossing_success",
    "temporary_retry_fails_false",
    "temporary_retry_fails_true",
    "terminal_running_step",
    "zero_step_user_opening",
  ];
  let entries = ACTOR_CONTROL_TRANSITION_ORACLE
    .lines()
    .filter(|line| !line.starts_with('#'))
    .map(|line| line.split_once('|').expect("oracle row has one delimiter"))
    .collect::<Vec<_>>();
  assert_eq!(entries.len(), expected.len());
  assert_eq!(
    entries.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
    expected
  );
  for (_, digest) in entries {
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
  }
}

/// Minimum frame frame-owned strip needed to reject a non-executable FIFO head before loading its
/// Contract, funding, Predicate, run payload, or adapter state. This is an experiment-only encoded
/// geometry model, not accepted runtime storage.
#[derive(Clone, Encode, MaxEncodedLen)]
struct AdmissionCellModel {
  actor_id: ActorId,
  sovereign_account: [u8; 32],
  actor_type: ActorType,
  lifecycle: ActiveLifecycle,
  cycle_state: CycleState,
  cycle_nonce: u64,
  cursor: u32,
  eligible_at: u32,
  contract_commitment: crate::ActorContractCommitment<[u8; 32]>,
  resources: crate::ActorStepResourceEnvelope,
  terminal_at: Option<u32>,
  last_cycle_block: Option<u32>,
  last_attempt_block: Option<u32>,
  pending_signal: bool,
}

fn bounded_vec_prefix_bytes(bound: u32) -> usize {
  Compact(bound).encoded_size()
}

#[test]
fn control_frame_admission_strip_chunk_sweep_preserves_target_headroom() {
  const A0_QUEUE_PAGE_SIZE: u32 = 64;
  const A0_QUEUE_PAGE_MAX_BYTES: usize = 6_170;
  const TRIE_PROOF_OVERHEAD_BYTES: usize = 8_645 - A0_QUEUE_PAGE_MAX_BYTES;
  const TARGET_PROOF_PER_COMMIT: usize = 6_361;
  const TARGET_COMMITS: usize = 100;

  let baseline_cell_bytes = crate::QueueEntry::<u32>::max_encoded_len();
  let baseline_modeled_page_bytes = bounded_vec_prefix_bytes(A0_QUEUE_PAGE_SIZE)
    .saturating_add(baseline_cell_bytes.saturating_mul(A0_QUEUE_PAGE_SIZE as usize));

  let frame_cell_bytes = AdmissionCellModel::max_encoded_len();
  let full_cell_bytes = crate::ActorControlCell::<
    [u8; 32],
    u32,
    crate::ActorAdmissionCertificate<()>,
  >::max_encoded_len();
  assert!(
    frame_cell_bytes <= 256,
    "minimum frame admission strip already exceeds its prepared compactness ceiling"
  );
  assert!(
    full_cell_bytes <= 400,
    "full single-owner frame cell exceeds its prototype compactness ceiling"
  );

  for chunk_size in [4usize, 8, 16, 32, 64] {
    let chunk_bytes = bounded_vec_prefix_bytes(chunk_size as u32)
      .saturating_add(frame_cell_bytes.saturating_mul(chunk_size));
    let estimated_chunk_proof = TRIE_PROOF_OVERHEAD_BYTES.saturating_add(chunk_bytes);
    let full_chunk_proof_per_cell = estimated_chunk_proof.div_ceil(chunk_size);
    let chunks_for_target = TARGET_COMMITS.div_ceil(chunk_size);
    let conservative_target_window_proof = estimated_chunk_proof.saturating_mul(chunks_for_target);
    let conservative_target_proof_per_commit =
      conservative_target_window_proof.div_ceil(TARGET_COMMITS);

    assert!(full_chunk_proof_per_cell < TARGET_PROOF_PER_COMMIT);
    assert!(conservative_target_proof_per_commit < TARGET_PROOF_PER_COMMIT);
    println!(
      "ACTOR_C1_STATIC_GEOMETRY_V1 chunkSize={chunk_size} a0CellBytes={baseline_cell_bytes} a0ModeledPageBytes={baseline_modeled_page_bytes} a0GeneratedPageMaxBytes={A0_QUEUE_PAGE_MAX_BYTES} c1CellBytes={frame_cell_bytes} fullCellBytes={full_cell_bytes} chunkBytes={chunk_bytes} estimatedChunkProof={estimated_chunk_proof} fullChunkProofPerCell={full_chunk_proof_per_cell} targetWindowChunks={chunks_for_target} targetWindowProofPerCommit={conservative_target_proof_per_commit}"
    );
  }
}

#[test]
fn control_ticket_address_is_total_contiguous_and_order_preserving() {
  for chunk_size in [4u64, 8, 16, 32, 64] {
    for base_ticket in [0u64, 1, 63, 64, 65, 9_999, u32::MAX as u64] {
      for offset in 0..=128u64 {
        let ticket = base_ticket.saturating_add(offset);
        let chunk = ticket / chunk_size;
        let slot = ticket % chunk_size;
        assert_eq!(
          chunk
            .checked_mul(chunk_size)
            .and_then(|value| value.checked_add(slot)),
          Some(ticket)
        );
        assert!(slot < chunk_size);
      }
    }
  }
}

type C1Cell = crate::ActorControlCellOf<Test>;
type C1Location = crate::ActorControlLocation<MockBlockNumber>;

fn frame_cell(actor_id: ActorId, eligible_at: MockBlockNumber) -> C1Cell {
  crate::ActorControlCell {
    actor_id,
    identity: crate::ActorControlIdentity {
      owner: actor_id.saturating_add(2_000_000),
      actor_class: ActorClass::User { owner_slot: 0 },
      mutability: Mutability::Mutable,
      cycle_nonce: 0,
      last_control_mutation_block: 0,
    },
    hot: crate::ActorControlHotState {
      lifecycle: ActiveLifecycle::Active,
      cycle_state: CycleState::Idle,
      trigger_runtime_state: TriggerRuntimeState::Stateless,
      unsuccessful_attempt_streak: 0,
      pending_signal: true,
      wakeup_pointer: None,
      trigger_wakeup_pointer: None,
      terminal_at: None,
      schedule_anchor: 0,
      last_cycle_block: None,
    },
    cursor: 0,
    eligible_at: Some(eligible_at),
    admission: crate::ActorAdmissionCertificate::<crate::ActorAdmissionResourcesOf<Test>>::new(
      [1u8; 32],
      [2u8; 32],
      1,
      [3u8; 32],
      1,
      [4u8; 32],
      Weight::from_parts(1, 1),
    ),
    resources: crate::ActorStepResourceEnvelope {
      control: Weight::from_parts(10, 10),
      effect: Weight::zero(),
    },
  }
}

fn frame_install_direct_ready(step: StepOf<Test>, count: u32) -> Vec<ActorId> {
  frame_system::Pallet::<Test>::set_block_number(1);
  let mut actor_ids = Vec::with_capacity(count as usize);
  for _ in 0..count {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step.clone()),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("direct-prefix User exists")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    actor_ids.push(actor_id);
  }
  for actor_id in actor_ids.iter().copied() {
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("direct-prefix Ready state is coherent");
    let queue_ticket = state.hot.queue_ticket.expect("direct-prefix ticket exists");
    let ticket = Actors::build_actor_step_ticket(
      actor_id,
      queue_ticket,
      1,
      &state.identity,
      &state.hot,
      state.run_state.as_ref(),
      &admission,
    )
    .expect("direct-prefix Step ticket builds");
    let cell = Actors::control_opening_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &ticket,
      &loaded_step,
    )
    .expect("direct-prefix Ready cell projects");
    assert_eq!(
      Actors::actor_control_cell(actor_id).map(|(_, stored)| stored),
      Some(cell)
    );
  }
  assert_eq!(ActorReadyOccupancy::<Test>::get(), count);
  actor_ids
}

fn frame_install_direct_stop_cycle_ready(count: u32) -> Vec<ActorId> {
  frame_install_direct_ready(make_step(Task::StopCycle), count)
}

fn frame_install_direct_zero_step_ready(
  auto_close_at_cycle_nonce: Option<u64>,
) -> (ActorId, QueueTicket) {
  frame_system::Pallet::<Test>::set_block_number(1);
  let steps = BoundedVec::default();
  prefund_active_user_creation(ALICE, &steps);
  let actor_id = Actors::next_actor_id();
  let mut contract =
    user_active_contract(manual_schedule(), None, steps).expect("zero-Step User Contract exists");
  contract.auto_close_at_cycle_nonce = auto_close_at_cycle_nonce;
  assert_ok!(Actors::create_user_actor(
    RuntimeOrigin::signed(ALICE),
    Mutability::Mutable,
    Some(contract),
  ));
  age_fixture_control_clock(actor_id);
  let sovereign = Actors::active_actor_view(actor_id)
    .expect("zero-Step frame User exists")
    .sovereign_account;
  let _ = <Test as crate::Config>::AssetOps::mint(
    &sovereign,
    <Test as crate::Config>::FeeNativeAssetId::get(),
    u128::from(u64::MAX / 4),
  );
  assert_ok!(Actors::manual_trigger(
    RuntimeOrigin::signed(ALICE),
    actor_id
  ));
  let (state, admission, loaded_step) = Actors::load_frame_actor_service_state(actor_id)
    .expect("zero-Step frame Ready state is coherent");
  assert!(loaded_step.is_none());
  let queue_ticket = state
    .hot
    .queue_ticket
    .expect("zero-Step frame ticket exists");
  let ticket = Actors::build_actor_step_ticket(
    actor_id,
    queue_ticket,
    1,
    &state.identity,
    &state.hot,
    state.run_state.as_ref(),
    &admission,
  )
  .expect("zero-Step frame ticket builds");
  let cell = Actors::control_zero_step_opening_cell_from_scalar(
    actor_id,
    state.identity,
    state.hot,
    admission,
    &ticket,
  )
  .expect("zero-Step frame Ready cell projects");
  assert_eq!(
    Actors::actor_control_cell(actor_id).map(|(_, stored)| stored),
    Some(cell)
  );
  assert_eq!(ActorReadyOccupancy::<Test>::get(), 1);
  (actor_id, queue_ticket)
}

#[test]
fn control_zero_step_user_opening_completes_without_action_or_scalar_owner() {
  new_test_ext().execute_with(|| {
    let (actor_id, ticket) = frame_install_direct_zero_step_ready(None);
    clear_fee_collections();
    System::reset_events();

    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (ticket.saturating_add(1)).min(head_before_report.saturating_add(1)),
    );
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);

    assert_eq!(fee_collections().len(), 1);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted {
        actor_id: id,
        cycle_nonce: 1,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Completed,
        outcomes,
      } if *id == actor_id && *outcomes == OutcomeTotals::default()
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActionFeeCharged { actor_id: id, .. } if *id == actor_id
    )));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(
      ActorFunding::<Test>::get(actor_id)
        .expect("zero-Step funding remains")
        .funding_accumulated
        .is_empty()
    );
    let (location, identity, hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("zero-Step output retains sole frame authority");
    assert_eq!(location, C1Location::Unsignaled);
    assert_eq!(identity.cycle_nonce, 1);
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(!hot.pending_signal);
    assert_eq!(hot.queue_ticket, None);
    assert_eq!(hot.last_cycle_block, Some(1));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    frame_assert_single_owner();
  });
}

#[test]
fn control_zero_step_auto_close_removes_frame_authority_and_preserves_cycle_events() {
  new_test_ext().execute_with(|| {
    let (actor_id, ticket) = frame_install_direct_zero_step_ready(Some(1));
    clear_fee_collections();
    System::reset_events();

    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (ticket.saturating_add(1)).min(head_before_report.saturating_add(1)),
    );
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);

    assert_eq!(fee_collections().len(), 1);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted {
        actor_id: id,
        cycle_nonce: 1,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::AutoCloseNonceReached,
      } if *id == actor_id
    )));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!ActorFunding::<Test>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ActorStateHolds::<Test>::contains_key(actor_id));
    assert_eq!(ActiveActorCount::<Test>::get(), 0);
    assert_eq!(ActorIdentityCount::<Test>::get(), 0);
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    frame_assert_single_owner();
  });
}

#[test]
fn control_immutable_zero_step_at_time_closes_after_frame_owned_temporal_path() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::default();
    prefund_active_user_creation(ALICE, &steps);
    let actor_id = Actors::next_actor_id();
    let mut contract = user_active_contract(at_time_schedule(1), None, steps)
      .expect("immutable zero-Step AtTime Contract exists");
    contract.auto_close_at_cycle_nonce = Some(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Immutable,
      Some(contract),
    ));
    age_fixture_control_clock(actor_id);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("immutable zero-Step AtTime Actor exists")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let residual_asset = TestAsset::Local(9);
    set_asset_balance(&sovereign, residual_asset, 919);
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("reference AtTime reference invalidates before frame projection");
    let (state, admission, loaded_step) = Actors::load_frame_actor_service_state(actor_id)
      .expect("immutable zero-Step AtTime state remains coherent");
    assert!(loaded_step.is_none());
    let cell = Actors::control_zero_step_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
    )
    .expect("immutable zero-Step AtTime projects Unsignaled");
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, WakeupKey::Tick(2))
      .expect("immutable zero-Step AtTime reference schedules");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    clear_fee_collections();
    System::reset_events();

    frame_system::Pallet::<Test>::set_block_number(2);
    let (_, waiting) = Actors::control_latch_due_temporal_reference(WakeupKey::Tick(2), 2, 2)
      .expect("immutable zero-Step AtTime occurrence latches");
    let C1Location::Waiting {
      key: WakeupKey::Block(3),
      page,
      ..
    } = waiting
    else {
      panic!("immutable zero-Step AtTime must enter exact N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(3);
    let promoted = Actors::control_promote_due_waiting_page(3, page, 3)
      .expect("immutable zero-Step AtTime promotes at N+1");
    assert_eq!(promoted.len(), 1);
    let ticket = promoted[0].1;
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (ticket.saturating_add(1)).min(head_before_report.saturating_add(1)),
    );
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    assert_eq!(
      fee_collections(),
      vec![
        at_time_trigger_fee(),
        pipeline_opening_fee(&crate::ContractSteps::<Test>::default()),
      ]
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::AutoCloseNonceReached,
      } if *id == actor_id
    )));
    assert_eq!(asset_balance(&sovereign, residual_asset), 919);
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorFunding::<Test>::contains_key(actor_id));
    assert!(!ActorStateHolds::<Test>::contains_key(actor_id));
    frame_assert_single_owner();
  });
}

#[test]
fn control_zero_and_one_step_cadenced_recurrence_rearms_from_frame_authority() {
  for step_count in [0, 1] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let steps = if step_count == 0 {
        BoundedVec::default()
      } else {
        contract_steps_with_step(make_step(Task::StopCycle))
      };
      let opening_fee = pipeline_opening_fee(&steps);
      let action_fee = steps.first().map(user_step_fee);
      prefund_active_user_creation(ALICE, &steps);
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(timer_schedule(1), None, steps),
      ));
      age_fixture_control_clock(actor_id);
      let installed_hold = Actors::actor_state_hold(actor_id).expect("Cadenced hold exists");
      assert!(installed_hold.breakdown.detector > 0);
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("zero-Step Cadenced Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
        .expect("reference Cadenced reference invalidates before frame projection");
      let (state, admission, loaded_step) = Actors::load_frame_actor_service_state(actor_id)
        .expect("zero-Step Cadenced state remains coherent");
      let cell = match loaded_step {
        None => Actors::control_zero_step_unsignaled_cell_from_scalar(
          actor_id,
          state.identity,
          state.hot,
          admission,
        ),
        Some(step) => Actors::control_unsignaled_cell_from_scalar(
          actor_id,
          state.identity,
          state.hot,
          admission,
          &step,
        ),
      }
      .expect("Cadenced authority projects Unsignaled");
      let cell = Actors::control_schedule_fresh_wakeup_reference(cell, WakeupKey::Tick(2))
        .expect("zero-Step Cadenced source occurrence schedules");
      crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
      crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
      clear_fee_collections();
      System::reset_events();

      let execute_cycle = |due_tick: u64, due_block: u64, ready_block: u64| {
        frame_system::Pallet::<Test>::set_block_number(due_block);
        let (_, waiting) = Actors::control_latch_due_temporal_reference(
          WakeupKey::Tick(due_tick),
          due_block,
          due_tick,
        )
        .expect("zero-Step Cadenced occurrence latches");
        let C1Location::Waiting {
          key: WakeupKey::Block(eligible_at),
          page,
          ..
        } = waiting
        else {
          panic!("zero-Step Cadenced occurrence must enter N+1 Waiting");
        };
        assert_eq!(eligible_at, ready_block);
        frame_system::Pallet::<Test>::set_block_number(ready_block);
        let promoted = Actors::control_promote_due_waiting_page(ready_block, page, ready_block)
          .expect("zero-Step Cadenced occurrence promotes at N+1");
        assert_eq!(promoted.len(), 1);
        let ticket = promoted[0].1;
        let head_before_report = Actors::queue_head();
        frame_system::Pallet::<Test>::set_block_number(ready_block);
        Actors::execute_cycle_to_cutoff(
          Weight::MAX,
          (ticket.saturating_add(1)).min(head_before_report.saturating_add(1)),
        );
        assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
        assert_eq!(
          Actors::actor_state_hold(actor_id).expect("Cadenced hold survives recurrence"),
          installed_hold
        );
      };

      execute_cycle(2, 2, 3);
      let first = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
        .expect("zero-Step Cadenced recurrence returns Unsignaled");
      assert_eq!(first.identity.cycle_nonce, 1);
      assert!(!first.hot.pending_signal);
      assert_eq!(
        first.hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
        Some(4)
      );
      assert_eq!(
        Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
        Some(WakeupKey::Tick(4))
      );

      execute_cycle(4, 4, 5);
      let second = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
        .expect("second zero-Step Cadenced recurrence returns Unsignaled");
      assert_eq!(second.identity.cycle_nonce, 2);
      assert_eq!(
        second
          .hot
          .trigger_wakeup_pointer
          .map(|pointer| pointer.tick),
        Some(6)
      );
      let mut expected_fees = Vec::new();
      for _ in 0..2 {
        expected_fees.extend([cadenced_trigger_fee(), opening_fee]);
        if let Some(fee) = action_fee.filter(|fee| *fee > 0) {
          expected_fees.push(fee);
        }
      }
      assert_eq!(fee_collections(), expected_fees);
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Unsignaled)
      );
      assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
      frame_assert_single_owner();
    });
  }
}

#[test]
fn control_zero_step_user_opening_matches_immutable_oracle_logical_authority_and_fees() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let (actor_id, _ticket) = {
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          BoundedVec::default(),
        );
        let sovereign = Actors::active_actor_view(actor_id)
          .expect("reference zero-Step User exists")
          .sovereign_account;
        let _ = <Test as crate::Config>::AssetOps::mint(
          &sovereign,
          <Test as crate::Config>::FeeNativeAssetId::get(),
          u128::from(u64::MAX / 4),
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        let ticket = Actors::active_actor_view(actor_id)
          .and_then(|view| view.queue_ticket)
          .expect("reference zero-Step ticket exists");
        (actor_id, ticket)
      };
      let sovereign = {
        Actors::active_actor_view(actor_id)
          .expect("reference zero-Step Actor exists before execution")
          .sovereign_account
      };
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();

      {
        Actors::on_idle(1, Weight::MAX);
      }

      let (identity, hot, admission) = {
        let (identity, hot) =
          Actors::load_control_head(actor_id).expect("reference zero-Step output authority exists");
        let admission =
          Actors::load_control_admission(actor_id).expect("reference zero-Step admission remains");
        (identity, hot, admission)
      };
      let event_counts = (
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: id, .. })
                if *id == actor_id
            )
          })
          .count(),
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. })
                if *id == actor_id
            )
          })
          .count(),
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::CycleSummary { actor_id: id, .. })
                if *id == actor_id
            )
          })
          .count(),
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id: id, .. })
                if *id == actor_id
            )
          })
          .count(),
      );
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        identity.cycle_nonce,
        hot.cycle_state,
        hot.pending_signal,
        hot.queue_ticket,
        hot.last_cycle_block,
        admission,
        ActorFunding::<Test>::get(actor_id)
          .expect("zero-Step differential funding remains")
          .funding_accumulated,
        ActorRunStateStore::<Test>::contains_key(actor_id),
        event_counts,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("zero_step_user_opening", &baseline);
}

fn control_running_branch_snapshot(step_count: u32) -> Vec<Vec<u8>> {
  new_test_ext().execute_with(|| {
    assert!(matches!(step_count, 2 | 3));
    frame_system::Pallet::<Test>::set_block_number(0);
    let transfer = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    });
    let mut steps = vec![transfer.clone(), make_step(Task::StopCycle)];
    if step_count == 3 {
      steps.insert(1, transfer);
    }
    let steps = BoundedVec::try_from(steps).expect("Running differential Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    assert_eq!(
      Actors::load_actor_contract(actor_id)
        .expect("Running differential Contract exists")
        .steps
        .len(),
      step_count as usize,
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("Running differential User exists")
      .sovereign_account;
    let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
    let _ =
      <Test as crate::Config>::AssetOps::mint(&sovereign, native_asset, u128::from(u64::MAX / 4));
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::reset_events();
    {
      Actors::on_idle(1, Weight::MAX);
    }
    let run = ActorRunStateStore::<Test>::get(actor_id).unwrap_or_else(|| {
      panic!(
        "{} Running state persists after Step 0; view={:?}; events={:?}",
        { "reference" },
        Actors::active_actor_view(actor_id),
        System::events(),
      )
    });
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(1));

    frame_system::Pallet::<Test>::set_block_number(2);
    {
      Actors::on_initialize(2);
      run_prepass();
    }
    {
      Actors::on_idle(2, Weight::MAX);
    }

    let (identity, hot, admission) = {
      (
        Actors::actor_identity(actor_id).expect("canonical Running identity exists"),
        Actors::actor_hot(actor_id).expect("reference Running hot state exists"),
        Actors::load_control_admission(actor_id).expect("canonical Running admission exists"),
      )
    };
    vec![
      identity.encode(),
      hot.encode(),
      admission.encode(),
      ActorFunding::<Test>::get(actor_id).encode(),
      ActorRunStateStore::<Test>::get(actor_id).encode(),
      crate::ActorStateHolds::<Test>::get(actor_id)
        .map(|hold| hold.owner)
        .encode(),
      <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset).encode(),
      <Test as crate::Config>::AssetOps::balance(&BOB, native_asset).encode(),
      <Test as crate::Config>::AssetOps::balance(
        &<Test as crate::Config>::FeeSink::get(),
        native_asset,
      )
      .encode(),
      System::events().encode(),
    ]
  })
}

#[test]
fn control_running_continuation_and_terminal_step_matches_immutable_oracle() {
  let baseline = control_running_branch_snapshot(2);
  emit_baseline_oracle("terminal_running_step", &baseline);
}

#[test]
fn control_running_middle_step_continuation_matches_immutable_oracle() {
  let baseline = control_running_branch_snapshot(3);
  emit_baseline_oracle("middle_running_step", &baseline);
}

#[test]
fn control_stable_scalar_seams_update_an_existing_frame_primary_in_benchmark_builds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("scalar benchmark Actor remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("scalar authority projects to one Unsignaled primary");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);

    mutate_actor_hot_coherent(actor_id, |hot| hot.unsuccessful_attempt_streak = 1);
    mutate_actor_identity_coherent(actor_id, |identity| {
      identity.last_control_mutation_block = 7;
    });

    let (location, identity, hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("stable scalar seams retain the frame primary");
    assert_eq!(location, C1Location::Unsignaled);
    assert_eq!(identity.last_control_mutation_block, 7);
    assert_eq!(hot.unsuccessful_attempt_streak, 1);
  });
}

fn frame_install_temporal_system_unsignaled(count: u32) -> Vec<ActorId> {
  frame_system::Pallet::<Test>::set_block_number(1);
  let mut actor_ids = Vec::with_capacity(count as usize);
  for _ in 0..count {
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(5),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("reference Trigger wakeup invalidates before candidate projection");
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("temporal System Actor remains coherent");
    assert!(state.hot.trigger_wakeup_pointer.is_none());
    let unsignaled = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("temporal System authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, &unsignaled);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    actor_ids.push(actor_id);
  }
  actor_ids
}

#[test]
fn control_fresh_trigger_reference_round_trip_uses_no_scalar_hot_owner() {
  new_test_ext().execute_with(|| {
    let actor_id = frame_install_temporal_system_unsignaled(1)[0];
    let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("candidate Unsignaled cell exists");
    let key = WakeupKey::Tick(9);
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, key)
      .expect("fresh Trigger reference schedules");
    let pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("frame-owned hot state receives Trigger pointer");
    assert_eq!(pointer.tick, 9);
    assert!(
      crate::ActorWaitingFrameChunks::<Test>::get((key, pointer.page_id))
        .and_then(|chunk| chunk.entries.get(pointer.slot as usize).cloned().flatten())
        .is_some_and(
          |entry| matches!(entry, crate::ActorWaitingEntry::Reference(reference)
          if reference.actor_id == actor_id
            && reference.admission_identity == cell.admission.admission_identity)
        )
    );
    assert!(matches!(
      Actors::control_schedule_fresh_wakeup_reference(cell.clone(), key),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 1);
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    frame_assert_single_owner();
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    frame_assert_single_owner();

    let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("scheduled candidate cell remains primary");
    assert!(matches!(
      Actors::control_due_wakeup_reference(key, 1, 8),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert!(matches!(
      Actors::control_consume_due_wakeup_reference(cell.clone(), key, 1, 8),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert!(crate::ActorWaitingFrameChunks::<Test>::contains_key((
      key,
      pointer.page_id
    )));
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), Some(key));
    frame_assert_single_owner();
    let (due_actor, due_pointer) = Actors::control_due_wakeup_reference(key, 1, 9)
      .expect("due candidate Trigger reference is discoverable");
    assert_eq!(due_actor, actor_id);
    assert_eq!(due_pointer.block, key);
    assert_eq!(due_pointer.page_id, pointer.page_id);
    assert_eq!(due_pointer.slot, pointer.slot);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Ready { ticket: 999 });
    assert!(matches!(
      Actors::control_due_wakeup_primary(key, 1, 9),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert!(crate::ActorWaitingFrameChunks::<Test>::contains_key((
      key,
      pointer.page_id
    )));
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), Some(key));
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let (due_location, due_cell) = Actors::control_due_wakeup_primary(key, 1, 9)
      .expect("due reference resolves the primary frame cell");
    assert_eq!(due_location, C1Location::Unsignaled);
    assert_eq!(due_cell, cell);
    frame_assert_single_owner();
    let cell = Actors::control_consume_due_wakeup_reference(cell, key, 1, 9)
      .expect("due candidate Trigger reference consumes");
    assert!(cell.hot.trigger_wakeup_pointer.is_none());
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    assert!(!crate::ActorWaitingFrameChunks::<Test>::contains_key((
      key,
      pointer.page_id
    )));
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(key));
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    frame_assert_single_owner();
  });
}

#[test]
fn control_due_temporal_reference_latches_into_n_plus_one_process_waiting_atomically() {
  new_test_ext().execute_with(|| {
    let actor_id = frame_install_temporal_system_unsignaled(1)[0];
    let source_key = WakeupKey::Tick(9);
    let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("candidate temporal Unsignaled cell exists");
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, source_key)
      .expect("candidate temporal Trigger reference schedules");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    System::reset_events();

    assert!(matches!(
      Actors::control_latch_due_temporal_reference(source_key, 1, 8),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    assert!(crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(source_key)
    );

    let source_cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("source cell survives early rejection");
    let mut occupied_destination =
      BoundedVec::try_from(vec![None; 32]).expect("collision C32 fits");
    occupied_destination[0] = Some(crate::ActorWaitingEntry::Primary(source_cell.clone()));
    crate::ActorWaitingFrameChunks::<Test>::insert(
      (WakeupKey::Block(2), 0),
      crate::ActorWaitingPageOf::<Test> {
        entries: occupied_destination,
        live_entries: 1,
        scan_slot: 0,
        previous_page: None,
        next_page: None,
      },
    );
    assert!(matches!(
      Actors::control_latch_due_temporal_reference(source_key, 1, 9),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    assert_eq!(
      crate::ActorUnsignaledControlCells::<Test>::get(actor_id),
      Some(source_cell)
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(source_key)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
    crate::ActorWaitingFrameChunks::<Test>::remove((WakeupKey::Block(2), 0));

    let (latched_actor, destination) =
      Actors::control_latch_due_temporal_reference(source_key, 1, 9)
        .expect("due temporal reference latches");
    assert_eq!(latched_actor, actor_id);
    let C1Location::Waiting { key, page, slot } = destination else {
      panic!("latched temporal reference must enter process Waiting");
    };
    assert_eq!(key, WakeupKey::Block(2));
    let cell = crate::ActorWaitingFrameChunks::<Test>::get((key, page))
      .and_then(|chunk| chunk.entries.get(slot as usize).cloned().flatten())
      .and_then(crate::ActorWaitingEntry::into_primary)
      .expect("latched process-primary cell exists");
    assert_eq!(cell.actor_id, actor_id);
    assert!(cell.hot.pending_signal);
    assert_eq!(cell.hot.cycle_state, CycleState::Idle);
    assert_eq!(cell.eligible_at, Some(2));
    assert!(cell.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(
      cell.hot.wakeup_pointer.map(|pointer| pointer.block),
      Some(key)
    );
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(destination)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Block),
      Some(key)
    );
    frame_assert_single_owner();
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        fee,
      } if *id == actor_id && *fee == 0
    )));
    frame_assert_single_owner();

    frame_system::Pallet::<Test>::set_block_number(2);
    let promoted = Actors::control_promote_due_waiting_page(2, page, 2)
      .expect("N+1 process Waiting promotes to Ready");
    assert_eq!(promoted, vec![(actor_id, 0)]);
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);

    let recurrent = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("completed Cadenced Actor returns to Unsignaled");
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    let recurrent_pointer = recurrent
      .hot
      .trigger_wakeup_pointer
      .expect("completed Cadenced Actor rearms Trigger reference");
    assert_eq!(recurrent_pointer.tick, 6);
    let recurrent_key = WakeupKey::Tick(recurrent_pointer.tick);
    assert!(
      crate::ActorWaitingFrameChunks::<Test>::get((recurrent_key, recurrent_pointer.page_id))
        .and_then(|chunk| chunk
          .entries
          .get(recurrent_pointer.slot as usize)
          .cloned()
          .flatten())
        .is_some_and(
          |entry| matches!(entry, crate::ActorWaitingEntry::Reference(reference)
          if reference.actor_id == actor_id
            && reference.admission_identity == recurrent.admission.admission_identity)
        )
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(recurrent_key)
    );
    frame_assert_single_owner();
  });
}

fn frame_install_temporal_system_unsignaled_with_steps(
  steps: crate::ContractSteps<Test>,
  fund: bool,
) -> ActorId {
  frame_system::Pallet::<Test>::set_block_number(1);
  let actor_id = create_system_with(ALICE, timer_schedule(5), None, steps);
  if fund {
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("temporal System Actor exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
  }
  Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
    .expect("reference System Trigger wakeup invalidates before candidate projection");
  let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
    .expect("temporal System Actor remains coherent");
  let unsignaled = Actors::control_unsignaled_cell_from_scalar(
    actor_id,
    state.identity,
    state.hot,
    admission,
    &loaded_step,
  )
  .expect("temporal System authority projects Unsignaled");
  crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, unsignaled);
  crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
  actor_id
}

fn frame_promote_temporal_actor_to_ready(actor_id: ActorId, source_tick: u64) -> u64 {
  let source_key = WakeupKey::Tick(source_tick);
  let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
    .expect("candidate temporal Unsignaled cell exists");
  let cell = Actors::control_schedule_fresh_wakeup_reference(cell, source_key)
    .expect("candidate temporal Trigger reference schedules");
  crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
  let (_, destination) =
    Actors::control_latch_due_temporal_reference(source_key, source_tick, source_tick)
      .expect("candidate temporal occurrence latches");
  let C1Location::Waiting { page, .. } = destination else {
    panic!("latched temporal occurrence enters process Waiting");
  };
  let opening_block = source_tick.saturating_add(1);
  frame_system::Pallet::<Test>::set_block_number(opening_block);
  assert_eq!(
    Actors::control_promote_due_waiting_page(opening_block, page, opening_block)
      .expect("N+1 process Waiting promotes"),
    vec![(actor_id, 0)]
  );
  opening_block
}

#[test]
fn control_cadenced_running_primary_retains_lightweight_trigger_rearm() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::StopCycle),
    ])
    .expect("two-Step temporal Contract fits");
    let actor_id = frame_install_temporal_system_unsignaled_with_steps(steps, true);
    let opening_block = frame_promote_temporal_actor_to_ready(actor_id, 6);

    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(opening_block);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let location = crate::ActorControlLocators::<Test>::get(actor_id)
      .expect("Running Cadenced Actor has a primary locator");
    assert_eq!(location, C1Location::Ready { ticket: 1 });
    let cell = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Running Cadenced primary cell exists");
    assert_eq!(cell.hot.cycle_state, CycleState::Running);
    assert_eq!(cell.cursor, 1);
    assert_eq!(cell.eligible_at, Some(8));
    let trigger_pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("Running primary retains cadence re-arm pointer");
    assert_eq!(trigger_pointer.tick, 11);
    let trigger_key = WakeupKey::Tick(trigger_pointer.tick);
    assert!(
      crate::ActorWaitingFrameChunks::<Test>::get((trigger_key, trigger_pointer.page_id))
        .and_then(|chunk| chunk
          .entries
          .get(trigger_pointer.slot as usize)
          .cloned()
          .flatten())
        .is_some_and(
          |entry| matches!(entry, crate::ActorWaitingEntry::Reference(reference)
          if reference.actor_id == actor_id
            && reference.admission_identity == cell.admission.admission_identity)
        )
    );
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(trigger_key)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    System::reset_events();
    let (deferred_actor, deferred_location) =
      Actors::control_latch_due_temporal_reference(trigger_key, opening_block, 11)
        .expect("due cadence latches while Running");
    assert_eq!(deferred_actor, actor_id);
    assert_eq!(deferred_location, location);
    let deferred = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Running primary survives deferred latch");
    assert_eq!(deferred.hot.cycle_state, CycleState::Running);
    assert!(deferred.hot.pending_signal);
    assert_eq!(deferred.eligible_at, Some(8));
    assert!(deferred.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        fee,
      } if *id == actor_id && *fee == 0
    )));

    frame_system::Pallet::<Test>::set_block_number(8);
    let head_before_completion = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(8);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (2).min(head_before_completion.saturating_add(1)),
    );
    assert_eq!(
      Actors::queue_head().saturating_sub(head_before_completion),
      1
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Ready { ticket: 2 })
    );
    let deferred_opening = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(2).cloned().flatten())
      .expect("completed current cycle queues deferred Opening");
    assert_eq!(deferred_opening.hot.cycle_state, CycleState::Idle);
    assert!(deferred_opening.hot.pending_signal);
    assert_eq!(deferred_opening.cursor, 0);
    assert_eq!(deferred_opening.eligible_at, Some(9));
    assert!(deferred_opening.hot.trigger_wakeup_pointer.is_none());
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(9);
    let head_before_next_cycle = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(9);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (3).min(head_before_next_cycle.saturating_add(1)),
    );
    assert_eq!(
      Actors::queue_head().saturating_sub(head_before_next_cycle),
      1
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Ready { ticket: 3 })
    );
    let reopened = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(3).cloned().flatten())
      .expect("deferred next Cycle opens at exact N+1");
    assert_eq!(reopened.hot.cycle_state, CycleState::Running);
    assert!(!reopened.hot.pending_signal);
    assert_eq!(reopened.cursor, 1);
    assert_eq!(reopened.eligible_at, Some(10));
    assert_eq!(
      reopened
        .hot
        .trigger_wakeup_pointer
        .map(|pointer| pointer.tick),
      Some(11)
    );
    let reopened_run = ActorRunStateStore::<Test>::get(actor_id)
      .expect("deferred next Cycle owns a new Running state");
    assert_eq!(reopened_run.cycle_nonce, 2);
    frame_assert_single_owner();
  });
}

#[test]
fn control_cadenced_suspended_primary_retains_lightweight_trigger_rearm() {
  new_test_ext().execute_with(|| {
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(u128::from(u64::MAX)),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
    let actor_id =
      frame_install_temporal_system_unsignaled_with_steps(contract_steps_with_step(step), true);
    let opening_block = frame_promote_temporal_actor_to_ready(actor_id, 6);

    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(opening_block);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let location = crate::ActorControlLocators::<Test>::get(actor_id)
      .expect("Suspended Cadenced Actor has a primary locator");
    assert_eq!(location, C1Location::Ready { ticket: 1 });
    let cell = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Suspended Cadenced primary cell exists");
    assert_eq!(cell.hot.cycle_state, CycleState::Suspended);
    assert_eq!(cell.cursor, 0);
    assert_eq!(cell.eligible_at, Some(8));
    let trigger_pointer = cell
      .hot
      .trigger_wakeup_pointer
      .expect("Suspended primary retains cadence re-arm pointer");
    assert_eq!(trigger_pointer.tick, 11);
    let trigger_key = WakeupKey::Tick(trigger_pointer.tick);
    assert!(
      crate::ActorWaitingFrameChunks::<Test>::get((trigger_key, trigger_pointer.page_id))
        .and_then(|chunk| chunk
          .entries
          .get(trigger_pointer.slot as usize)
          .cloned()
          .flatten())
        .is_some_and(
          |entry| matches!(entry, crate::ActorWaitingEntry::Reference(reference)
          if reference.actor_id == actor_id
            && reference.admission_identity == cell.admission.admission_identity)
        )
    );
    let run =
      ActorRunStateStore::<Test>::get(actor_id).expect("Suspended Cadenced run state persists");
    assert_eq!(run.cursor, 0);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(trigger_key)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    System::reset_events();
    let (deferred_actor, deferred_location) =
      Actors::control_latch_due_temporal_reference(trigger_key, opening_block, 11)
        .expect("due cadence latches while Suspended");
    assert_eq!(deferred_actor, actor_id);
    assert_eq!(deferred_location, location);
    let deferred = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Suspended primary survives deferred latch");
    assert_eq!(deferred.hot.cycle_state, CycleState::Suspended);
    assert!(deferred.hot.pending_signal);
    assert_eq!(deferred.eligible_at, Some(8));
    assert!(deferred.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        fee,
      } if *id == actor_id && *fee == 0
    )));

    frame_system::Pallet::<Test>::set_block_number(8);
    let head_before_retry = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(8);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (2).min(head_before_retry.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_retry), 1);
    let resuspension_key = WakeupKey::Block(10);
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Waiting {
        key: resuspension_key,
        page: 0,
        slot: 0,
      })
    );
    let resuspended = crate::ActorWaitingFrameChunks::<Test>::get((resuspension_key, 0))
      .and_then(|chunk| chunk.entries.first().cloned().flatten())
      .and_then(crate::ActorWaitingEntry::into_primary)
      .expect("Cadenced retry remains one Suspended primary");
    assert_eq!(resuspended.hot.cycle_state, CycleState::Suspended);
    assert!(resuspended.hot.pending_signal);
    assert_eq!(resuspended.cursor, 0);
    assert_eq!(resuspended.eligible_at, Some(10));
    assert!(resuspended.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Block),
      Some(resuspension_key)
    );
    frame_assert_single_owner();
  });
}

#[test]
fn control_underfunded_at_time_closes_from_frame_authority_without_custody_movement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("AtTime User Actor exists before candidate projection")
      .sovereign_account;
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    let custody_before = native_balance(&sovereign);
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("reference AtTime wakeup invalidates before candidate projection");
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("underfunded AtTime Actor remains structurally coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("underfunded AtTime authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let source_key = WakeupKey::Tick(2);
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, source_key)
      .expect("candidate AtTime reference schedules");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    clear_fee_collections();
    System::reset_events();

    let (closed_actor, closed_location) =
      Actors::control_latch_due_temporal_reference(source_key, 2, 2)
        .expect("underfunded AtTime selects minimal apoptosis");
    assert_eq!(closed_actor, actor_id);
    assert_eq!(closed_location, C1Location::Unsignaled);
    assert!(fee_collections().is_empty());
    assert_eq!(native_balance(&sovereign), custody_before);
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorFunding::<Test>::contains_key(actor_id));
    assert!(!crate::ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    frame_assert_single_owner();
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::TriggerAdmissionInsufficient,
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
    frame_assert_single_owner();
  });
}

#[test]
fn control_funded_at_time_latch_rolls_back_collection_failure_and_completes_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      at_time_schedule(1),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("funded AtTime User exists before candidate projection")
      .sovereign_account;
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("reference AtTime wakeup invalidates before candidate projection");
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("funded AtTime Actor remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("funded AtTime authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let source_key = WakeupKey::Tick(2);
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, source_key)
      .expect("candidate funded AtTime reference schedules");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    let custody_before = native_balance(&sovereign);
    clear_fee_collections();
    System::reset_events();

    set_fail_fee_sink_transfer(true);
    assert!(matches!(
      Actors::control_latch_due_temporal_reference(source_key, 2, 2),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    assert_eq!(native_balance(&sovereign), custody_before);
    assert_eq!(
      crate::ActorUnsignaledControlCells::<Test>::get(actor_id),
      Some(cell)
    );
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(source_key)
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    let (_, destination) = Actors::control_latch_due_temporal_reference(source_key, 2, 2)
      .expect("funded AtTime occurrence latches exactly once");
    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::AtTime,
        fee,
      } if *id == actor_id && *fee == at_time_trigger_fee()
    )));
    let C1Location::Waiting { page, .. } = destination else {
      panic!("funded AtTime latch enters N+1 process Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_eq!(
      Actors::control_promote_due_waiting_page(3, page, 3)
        .expect("funded AtTime N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let completed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("completed AtTime Actor returns to sole Unsignaled primary");
    assert_eq!(completed.hot.cycle_state, CycleState::Idle);
    assert!(!completed.hot.pending_signal);
    assert!(completed.eligible_at.is_none());
    assert!(completed.hot.wakeup_pointer.is_none());
    assert!(completed.hot.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      completed.hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    frame_assert_single_owner();
  });
}

#[test]
fn control_manual_latch_charges_once_rolls_back_collection_failure_and_coalesces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("Manual User exists before candidate projection")
      .sovereign_account;
    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("Manual User remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("Manual authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let custody_before = native_balance(&sovereign);
    clear_fee_collections();
    System::reset_events();

    set_fail_fee_sink_transfer(true);
    assert!(matches!(
      Actors::control_latch_manual_occurrence(actor_id, 1),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    ));
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    assert_eq!(native_balance(&sovereign), custody_before);
    assert_eq!(
      crate::ActorUnsignaledControlCells::<Test>::get(actor_id),
      Some(cell)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    let destination = Actors::control_latch_manual_occurrence(actor_id, 1)
      .expect("funded Manual occurrence commits")
      .expect("first Manual occurrence latches");
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Manual,
        fee,
      } if *id == actor_id && *fee == manual_trigger_fee()
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ManualTriggerSet { actor_id: id } if *id == actor_id
    )));
    assert_eq!(
      Actors::control_latch_manual_occurrence(actor_id, 1)
        .expect("latched Manual occurrence coalesces"),
      None
    );
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    let processed_count = System::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id: id,
            trigger_family: TriggerFamily::Manual,
            ..
          }) if *id == actor_id
        )
      })
      .count();
    assert_eq!(processed_count, 1);

    let C1Location::Waiting { page, .. } = destination else {
      panic!("Manual latch enters N+1 process Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2).expect("Manual N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let completed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("completed Manual Actor returns to Unsignaled");
    assert_eq!(completed.hot.cycle_state, CycleState::Idle);
    assert!(!completed.hot.pending_signal);
    assert!(completed.eligible_at.is_none());
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    frame_assert_single_owner();
  });
}

#[test]
fn control_manual_due_while_running_latches_one_deferred_next_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::StopCycle),
    ])
    .expect("two-Step Manual Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("Manual Running fixture exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("Manual Running fixture remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("Manual Running authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);

    let first = Actors::control_latch_manual_occurrence(actor_id, 1)
      .expect("first Manual occurrence commits")
      .expect("first Manual occurrence latches");
    let C1Location::Waiting { page, .. } = first else {
      panic!("first Manual occurrence enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2)
        .expect("first Manual N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_opening = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_opening.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_opening), 1);
    let running_location = C1Location::Ready { ticket: 1 };
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(running_location)
    );
    let running = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Manual cycle owns one Running primary");
    assert_eq!(running.hot.cycle_state, CycleState::Running);
    assert!(!running.hot.pending_signal);
    assert_eq!(running.cursor, 1);
    assert_eq!(running.eligible_at, Some(3));
    assert!(running.hot.trigger_wakeup_pointer.is_none());

    clear_fee_collections();
    System::reset_events();
    assert_eq!(
      Actors::control_latch_manual_occurrence(actor_id, 2).expect("busy Manual occurrence commits"),
      Some(running_location)
    );
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    let deferred = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("busy Manual latch preserves Running primary");
    assert_eq!(deferred.hot.cycle_state, CycleState::Running);
    assert!(deferred.hot.pending_signal);
    assert_eq!(deferred.cursor, 1);
    assert_eq!(deferred.eligible_at, Some(3));
    assert_eq!(
      Actors::control_latch_manual_occurrence(actor_id, 2)
        .expect("second busy Manual occurrence coalesces"),
      None
    );
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);

    frame_system::Pallet::<Test>::set_block_number(3);
    let head_before_completion = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (2).min(head_before_completion.saturating_add(1)),
    );
    assert_eq!(
      Actors::queue_head().saturating_sub(head_before_completion),
      1
    );
    let next_opening = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(2).cloned().flatten())
      .expect("current Manual cycle hands deferred latch to N+1");
    assert_eq!(next_opening.hot.cycle_state, CycleState::Idle);
    assert!(next_opening.hot.pending_signal);
    assert_eq!(next_opening.cursor, 0);
    assert_eq!(next_opening.eligible_at, Some(4));

    frame_system::Pallet::<Test>::set_block_number(4);
    let head_before_reopened = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (3).min(head_before_reopened.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_reopened), 1);
    let next_running = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(3).cloned().flatten())
      .expect("deferred Manual next Cycle opens exactly once");
    assert_eq!(next_running.hot.cycle_state, CycleState::Running);
    assert!(!next_running.hot.pending_signal);
    assert_eq!(next_running.cursor, 1);
    assert_eq!(next_running.eligible_at, Some(5));
    assert!(next_running.hot.trigger_wakeup_pointer.is_none());
    frame_assert_single_owner();
  });
}

#[test]
fn control_manual_due_while_suspended_survives_block_waiting_replacement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(u128::from(u64::MAX)),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("Manual Suspended fixture exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("Manual Suspended fixture remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("Manual Suspended authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);

    let first = Actors::control_latch_manual_occurrence(actor_id, 1)
      .expect("first Manual occurrence commits")
      .expect("first Manual occurrence latches");
    let C1Location::Waiting { page, .. } = first else {
      panic!("first Manual occurrence enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2)
        .expect("first Manual N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_opening = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_opening.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_opening), 1);
    let suspended_location = C1Location::Ready { ticket: 1 };
    let suspended = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Manual cycle owns one Suspended primary");
    assert_eq!(suspended.hot.cycle_state, CycleState::Suspended);
    assert!(!suspended.hot.pending_signal);
    assert_eq!(suspended.eligible_at, Some(3));

    clear_fee_collections();
    assert_eq!(
      Actors::control_latch_manual_occurrence(actor_id, 2)
        .expect("busy Suspended Manual occurrence commits"),
      Some(suspended_location)
    );
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    let deferred = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("Manual deferred latch preserves Suspended primary");
    assert_eq!(deferred.hot.cycle_state, CycleState::Suspended);
    assert!(deferred.hot.pending_signal);
    assert_eq!(deferred.eligible_at, Some(3));
    assert_eq!(
      Actors::control_latch_manual_occurrence(actor_id, 2)
        .expect("repeated Suspended Manual occurrence coalesces"),
      None
    );
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);

    frame_system::Pallet::<Test>::set_block_number(3);
    let head_before_retry = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (2).min(head_before_retry.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_retry), 1);
    let waiting_key = WakeupKey::Block(5);
    let waiting_location = C1Location::Waiting {
      key: waiting_key,
      page: 0,
      slot: 0,
    };
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(waiting_location)
    );
    let resuspended = crate::ActorWaitingFrameChunks::<Test>::get((waiting_key, 0))
      .and_then(|chunk| chunk.entries.first().cloned().flatten())
      .and_then(crate::ActorWaitingEntry::into_primary)
      .expect("Manual deferred latch survives Block Waiting replacement");
    assert_eq!(resuspended.hot.cycle_state, CycleState::Suspended);
    assert!(resuspended.hot.pending_signal);
    assert_eq!(resuspended.cursor, 0);
    assert_eq!(resuspended.eligible_at, Some(5));
    assert!(resuspended.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Block),
      Some(waiting_key)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), None);
    frame_assert_single_owner();
  });
}

#[test]
fn ordinary_fifo_preserves_ingress_latched_by_an_earlier_step() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let recipient = create_system_with(
        ALICE,
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        None,
        BoundedVec::try_from(vec![
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          }),
          make_step(Task::StopCycle),
        ])
        .unwrap(),
      );
      let recipient_account = Actors::active_actor_view(recipient)
        .unwrap()
        .sovereign_account;
      assert_ok!(<Test as crate::Config>::AssetOps::mint(
        &recipient_account,
        TestAsset::Native,
        1_000_000,
      ));
      assert_ok!(Actors::notify_address_event(
        recipient,
        TestAsset::Native,
        1,
        &ALICE
      ));
      assert_eq!(Actors::actor_hot(recipient).unwrap().queue_ticket, Some(0));
      let sender = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::Transfer {
          to: recipient_account,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(7),
        })),
      );
      let sender_account = Actors::active_actor_view(sender).unwrap().sovereign_account;
      assert_ok!(<Test as crate::Config>::AssetOps::mint(
        &sender_account,
        TestAsset::Native,
        1_000_000,
      ));
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), sender));
      let head_before_opening = Actors::queue_head();
      frame_system::Pallet::<Test>::set_block_number(1);
      Actors::execute_cycle_to_cutoff(
        Weight::MAX,
        (Actors::queue_tail()).min(head_before_opening.saturating_add(1)),
      );
      assert_eq!(Actors::queue_head().saturating_sub(head_before_opening), 1);
      let hot = Actors::actor_hot(recipient).unwrap();
      assert_eq!(hot.cycle_state, CycleState::Running);
      assert!(!hot.pending_signal);
      assert_eq!(Actors::actor_hot(sender).unwrap().queue_ticket, Some(1));
      assert_eq!(hot.queue_ticket, Some(2));
      <crate::mock::MockBenchmarkHelper as crate::BenchmarkHelper<
        AccountId,
        TestAsset,
        Balance,
        u32,
      >>::enable_asset_ops_ingress();
      frame_system::Pallet::<Test>::set_block_number(2);
      System::reset_events();

      Actors::on_idle(2, Weight::MAX);

      assert!(
        System::events().iter().any(|record| matches!(
          record.event,
          RuntimeEvent::Actors(crate::Event::TriggerOccurrenceProcessed {
            actor_id, trigger_family: TriggerFamily::AddressEvent, ..
          }) if actor_id == recipient
        )),
        "the earlier Transfer must traverse the certified AddressEvent ingress"
      );
      let state = Actors::active_actor_view(recipient).unwrap();
      assert_eq!(state.cycle_nonce, 1);
      assert_eq!(state.cycle_state, CycleState::Idle);
      state.pending_signal
    })
  };
  let ordinary = execute();
  assert!(
    ordinary,
    "ordinary FIFO retains the newly paid deferred latch"
  );
}

#[test]
fn control_address_event_commits_funding_independently_and_latches_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_filter = AssetFilter::Whitelist(
      BoundedVec::try_from(vec![TestAsset::Native]).expect("one AddressEvent asset filter fits"),
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::OwnerOnly, asset_filter),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("AddressEvent User exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("AddressEvent User remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("AddressEvent authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let provenance = crate::FundingProvenance::Signed;
    clear_fee_collections();
    System::reset_events();

    assert_eq!(
      Actors::control_apply_address_event(
        actor_id,
        TestAsset::Native,
        10,
        Some(&BOB),
        Some(&provenance),
        1,
      )
      .expect("unmatched AddressEvent is a balance-only consequence"),
      None
    );
    assert!(
      ActorFunding::<Test>::get(actor_id)
        .expect("unmatched AddressEvent funding state remains")
        .funding_accumulated
        .is_empty()
    );
    assert!(fee_collections().is_empty());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    set_fail_fee_sink_transfer(true);
    assert_eq!(
      Actors::control_apply_address_event(
        actor_id,
        TestAsset::Native,
        100,
        Some(&ALICE),
        Some(&provenance),
        1,
      )
      .expect("failed Trigger collection retains independent funding consequence"),
      None
    );
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    assert_eq!(
      ActorFunding::<Test>::get(actor_id)
        .expect("AddressEvent funding remains")
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
    assert_eq!(
      crate::ActorUnsignaledControlCells::<Test>::get(actor_id),
      Some(cell)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::FundingAccumulated {
        actor_id: id,
        asset: TestAsset::Native,
        added: 100,
        accumulated: 100,
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    let destination = Actors::control_apply_address_event(
      actor_id,
      TestAsset::Native,
      50,
      Some(&ALICE),
      Some(&provenance),
      1,
    )
    .expect("funded AddressEvent transition commits")
    .expect("matched AddressEvent latches");
    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    assert_eq!(
      ActorFunding::<Test>::get(actor_id)
        .expect("AddressEvent funding remains")
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&150)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::AddressEvent,
        fee,
      } if *id == actor_id && *fee == address_event_trigger_fee()
    )));

    assert_eq!(
      Actors::control_apply_address_event(
        actor_id,
        TestAsset::Native,
        25,
        Some(&ALICE),
        Some(&provenance),
        1,
      )
      .expect("latched AddressEvent still commits funding"),
      None
    );
    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    assert_eq!(
      ActorFunding::<Test>::get(actor_id)
        .expect("coalesced AddressEvent funding remains")
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&175)
    );
    let C1Location::Waiting { page, .. } = destination else {
      panic!("matched AddressEvent enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2)
        .expect("AddressEvent N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let completed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("AddressEvent one-Step cycle completes");
    assert_eq!(completed.hot.cycle_state, CycleState::Idle);
    assert!(!completed.hot.pending_signal);
    assert!(completed.hot.trigger_wakeup_pointer.is_none());
    frame_assert_single_owner();
  });
}

#[test]
fn control_address_event_due_while_running_accumulates_and_defers_in_place() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      }),
      make_step(Task::StopCycle),
    ])
    .expect("two-Step AddressEvent Contract fits");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      steps,
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("AddressEvent Running fixture exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("AddressEvent Running fixture remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("AddressEvent Running authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    let provenance = crate::FundingProvenance::Signed;

    let first = Actors::control_apply_address_event(
      actor_id,
      TestAsset::Native,
      100,
      Some(&ALICE),
      Some(&provenance),
      1,
    )
    .expect("first AddressEvent transition commits")
    .expect("first AddressEvent latches");
    let C1Location::Waiting { page, .. } = first else {
      panic!("first AddressEvent enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2)
        .expect("first AddressEvent N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_opening = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_opening.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_opening), 1);
    let running_location = C1Location::Ready { ticket: 1 };
    let running = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("AddressEvent cycle owns one Running primary");
    assert_eq!(running.hot.cycle_state, CycleState::Running);
    assert!(!running.hot.pending_signal);
    assert_eq!(running.cursor, 1);
    assert_eq!(running.eligible_at, Some(3));
    assert!(
      ActorFunding::<Test>::get(actor_id)
        .expect("Opening funding state remains")
        .funding_accumulated
        .is_empty()
    );

    clear_fee_collections();
    assert_eq!(
      Actors::control_apply_address_event(
        actor_id,
        TestAsset::Native,
        25,
        Some(&ALICE),
        Some(&provenance),
        2,
      )
      .expect("busy AddressEvent transition commits"),
      Some(running_location)
    );
    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    let deferred = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(1).cloned().flatten())
      .expect("busy AddressEvent preserves Running primary");
    assert_eq!(deferred.hot.cycle_state, CycleState::Running);
    assert!(deferred.hot.pending_signal);
    assert_eq!(deferred.cursor, 1);
    assert_eq!(deferred.eligible_at, Some(3));
    assert_eq!(
      Actors::control_apply_address_event(
        actor_id,
        TestAsset::Native,
        5,
        Some(&ALICE),
        Some(&provenance),
        2,
      )
      .expect("latched AddressEvent still commits funding"),
      None
    );
    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    assert_eq!(
      ActorFunding::<Test>::get(actor_id)
        .expect("busy AddressEvent funding remains")
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&30)
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    let head_before_completion = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (2).min(head_before_completion.saturating_add(1)),
    );
    assert_eq!(
      Actors::queue_head().saturating_sub(head_before_completion),
      1
    );
    let next_opening = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.get(2).cloned().flatten())
      .expect("AddressEvent deferred latch enters exact N+1");
    assert_eq!(next_opening.hot.cycle_state, CycleState::Idle);
    assert!(next_opening.hot.pending_signal);
    assert_eq!(next_opening.cursor, 0);
    assert_eq!(next_opening.eligible_at, Some(4));
    assert!(next_opening.hot.trigger_wakeup_pointer.is_none());
    frame_assert_single_owner();
  });
}

#[test]
fn control_observation_change_disables_on_latch_and_rearms_at_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 7;
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      observation_schedule(vec![feed]),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("ObservationChange User exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let subscription = crate::ObservationSubscriptionSlot::<Test>::get(actor_id)
      .expect("ObservationChange subscription exists");
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("ObservationChange User remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("ObservationChange authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    clear_fee_collections();
    System::reset_events();

    set_fail_fee_sink_transfer(true);
    assert_eq!(
      Actors::control_latch_observation_change_occurrence(actor_id, feed, 1)
        .expect("failed ObservationChange collection is nonfatal"),
      None
    );
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    assert_eq!(
      crate::ActorUnsignaledControlCells::<Test>::get(actor_id),
      Some(cell)
    );
    assert!(!crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    let destination = Actors::control_latch_observation_change_occurrence(actor_id, feed, 1)
      .expect("funded ObservationChange occurrence commits")
      .expect("ObservationChange occurrence latches");
    assert_eq!(fee_collections(), vec![observation_change_trigger_fee()]);
    assert!(crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::ObservationChange,
        fee,
      } if *id == actor_id && *fee == observation_change_trigger_fee()
    )));
    assert_eq!(
      Actors::control_latch_observation_change_occurrence(actor_id, feed, 1)
        .expect("disabled ObservationChange occurrence coalesces"),
      None
    );
    assert_eq!(fee_collections(), vec![observation_change_trigger_fee()]);

    let C1Location::Waiting { page, .. } = destination else {
      panic!("ObservationChange latch enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2)
        .expect("ObservationChange N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let completed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("ObservationChange one-Step cycle completes");
    assert_eq!(completed.hot.cycle_state, CycleState::Idle);
    assert!(!completed.hot.pending_signal);
    assert!(!crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::ObservationSubscriptionSlot::<Test>::get(actor_id),
      Some(subscription)
    );
    assert!(completed.hot.trigger_wakeup_pointer.is_none());
    frame_assert_single_owner();
  });
}

#[test]
fn control_observation_change_fanout_consumes_one_bounded_subscription_page() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 7;
    let mut actor_ids = Vec::new();
    let mut page_id = None;
    for owner in [ALICE, BOB] {
      let actor_id = create_user_with(
        owner,
        Mutability::Mutable,
        observation_schedule(vec![feed]),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("ObservationChange cohort member exists before projection")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let slot = crate::ObservationSubscriptionSlot::<Test>::get(actor_id)
        .expect("ObservationChange cohort member owns one subscription slot");
      let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
      let actor_page = slot / page_size;
      if let Some(expected) = page_id {
        assert_eq!(actor_page, expected);
      } else {
        page_id = Some(actor_page);
      }
      let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
        .expect("ObservationChange cohort member remains coherent");
      let cell = Actors::control_unsignaled_cell_from_scalar(
        actor_id,
        state.identity,
        state.hot,
        admission,
        &loaded_step,
      )
      .expect("ObservationChange cohort authority projects Unsignaled");
      crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
      crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
      actor_ids.push(actor_id);
    }
    let page_id = page_id.expect("ObservationChange cohort page exists");
    let page_before = crate::ObservationSubscriberPages::<Test>::get(feed, page_id)
      .expect("ObservationChange canonical subscriber page exists");
    clear_fee_collections();
    System::reset_events();

    let outcomes = Actors::control_latch_observation_change_page(feed, page_id, 1)
      .expect("one bounded ObservationChange page fanout commits");
    assert_eq!(outcomes.len(), actor_ids.len());
    assert!(outcomes.iter().all(|(_, outcome)| outcome.is_some()));
    assert_eq!(
      fee_collections(),
      vec![
        observation_change_trigger_fee(),
        observation_change_trigger_fee(),
      ]
    );
    assert_eq!(
      crate::ObservationSubscriberPages::<Test>::get(feed, page_id),
      Some(page_before)
    );
    for actor_id in actor_ids {
      assert!(crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
      assert!(matches!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Waiting {
          key: WakeupKey::Block(2),
          ..
        })
      ));
      frame_assert_single_owner();
    }
    let processed = System::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            trigger_family: TriggerFamily::ObservationChange,
            ..
          })
        )
      })
      .count();
    assert_eq!(processed, 2);
    frame_assert_single_owner();
  });
}

#[test]
fn control_observation_crossing_preserves_phase_on_fee_failure_and_rearms_at_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("ObservationCrossing User exists before candidate projection")
      .sovereign_account;
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      u128::from(u64::MAX / 4),
    );
    let initial_membership = crate::CrossingMemberships::<Test>::get(actor_id)
      .expect("ObservationCrossing Armed membership exists");
    assert_eq!(initial_membership.key.threshold, 100);
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("ObservationCrossing User remains coherent");
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("ObservationCrossing authority projects Unsignaled");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
    clear_fee_collections();
    System::reset_events();

    set_fail_fee_sink_transfer(true);
    assert_eq!(
      Actors::control_latch_observation_crossing_fire(
        actor_id,
        crate::ObservationTransition {
          revision: 2,
          previous: Some(50),
          current: 150,
        },
        1,
      )
      .expect("failed Crossing fee still commits hysteresis phase"),
      None
    );
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    let after_failed_fire = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("failed Crossing fee leaves sole Unsignaled primary");
    assert!(matches!(
      after_failed_fire.hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        ..
      }
    ));
    assert_eq!(
      crate::CrossingMemberships::<Test>::get(actor_id)
        .expect("failed Crossing fee retains rearm membership")
        .key
        .threshold,
      80
    );
    assert!(!after_failed_fire.hot.pending_signal);
    assert!(!crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    Actors::control_apply_observation_crossing_rearm(
      actor_id,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(150),
        current: 70,
      },
    )
    .expect("Crossing hysteresis rearm commits without Trigger occurrence");
    let rearmed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("rearmed Crossing primary remains Unsignaled");
    assert!(matches!(
      rearmed.hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      }
    ));
    assert_eq!(
      crate::CrossingMemberships::<Test>::get(actor_id)
        .expect("Crossing Fire membership is restored")
        .key
        .threshold,
      100
    );

    let destination = Actors::control_latch_observation_crossing_fire(
      actor_id,
      crate::ObservationTransition {
        revision: 4,
        previous: Some(70),
        current: 150,
      },
      1,
    )
    .expect("funded Crossing Fire commits")
    .expect("funded Crossing Fire latches");
    assert_eq!(fee_collections(), vec![observation_crossing_trigger_fee()]);
    assert!(crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::ObservationCrossing,
        fee,
      } if *id == actor_id && *fee == observation_crossing_trigger_fee()
    )));
    assert_eq!(
      Actors::control_latch_observation_crossing_fire(
        actor_id,
        crate::ObservationTransition {
          revision: 5,
          previous: Some(150),
          current: 200,
        },
        1,
      )
      .expect("disabled Crossing detector coalesces redundant work"),
      None
    );
    assert_eq!(fee_collections(), vec![observation_crossing_trigger_fee()]);

    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 70,
        observed_at: 4,
      },
    );
    let C1Location::Waiting { page, .. } = destination else {
      panic!("Crossing Fire enters N+1 Waiting");
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, page, 2).expect("Crossing N+1 process promotes"),
      vec![(actor_id, 0)]
    );
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (1).min(head_before_report.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);
    let completed = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("Crossing one-Step cycle completes");
    assert!(matches!(
      completed.hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        installed_at_revision: 1,
      }
    ));
    assert_eq!(
      crate::CrossingMemberships::<Test>::get(actor_id)
        .expect("Opening-time Crossing rearm membership exists")
        .key
        .threshold,
      100
    );
    assert!(!crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id));
    assert!(!completed.hot.pending_signal);
    frame_assert_single_owner();
  });
}

#[test]
fn control_manual_trigger_boundary_matches_immutable_oracle_logical_fee_and_event_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("Manual differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();

      let hot = {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        Actors::actor_hot(actor_id).expect("reference Manual differential hot state exists")
      };
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::Manual,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      let manual_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::ManualTriggerSet { actor_id: id }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        trigger_events,
        manual_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("manual_success", &baseline);
}

#[test]
fn control_at_time_trigger_boundary_matches_immutable_oracle_logical_fee_and_event_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        at_time_schedule(1),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("AtTime differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      frame_system::Pallet::<Test>::set_block_number(2);

      let hot = {
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        Actors::drain_overdue_wakeups_cursor(2, &mut meter);
        Actors::actor_hot(actor_id).expect("reference AtTime differential hot state exists")
      };
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::AtTime,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("at_time_success", &baseline);
}

#[test]
fn control_address_event_boundary_matches_immutable_oracle_logical_fee_funding_and_event_authority()
{
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        percentage_trigger_schedule(),
        None,
        contract_steps_with_step(make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
        })),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("AddressEvent differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();

      let hot = {
        assert_ok!(Actors::notify_address_event(
          actor_id,
          TestAsset::Native,
          100,
          &ALICE,
        ));
        Actors::actor_hot(actor_id).expect("reference AddressEvent differential hot state exists")
      };
      let funding = ActorFunding::<Test>::get(actor_id)
        .expect("AddressEvent differential funding exists")
        .funding_accumulated
        .get(&TestAsset::Native)
        .copied();
      let funding_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::FundingAccumulated {
              actor_id: id,
              asset: TestAsset::Native,
              added: 100,
              accumulated: 100,
            }) if *id == actor_id
          )
        })
        .count();
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::AddressEvent,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        funding,
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        funding_events,
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("address_event_success", &baseline);
}

#[test]
fn control_observation_change_boundary_matches_immutable_oracle_logical_fee_and_detector_authority()
{
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let feed = 33;
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        observation_schedule(vec![feed]),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("ObservationChange differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let subscription = crate::ObservationSubscriptionSlot::<Test>::get(actor_id)
        .expect("ObservationChange differential subscription exists");
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();

      let hot = {
        assert_ok!(Actors::note_observation_changed(feed, 1));
        assert_eq!(Actors::do_fanout_dirty_observation_page(), Ok(false));
        Actors::actor_hot(actor_id)
          .expect("reference ObservationChange differential hot state exists")
      };
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::ObservationChange,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id),
        crate::ObservationSubscriptionSlot::<Test>::get(actor_id),
        subscription,
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("observation_change_success", &baseline);
}

#[test]
fn control_observation_crossing_boundary_matches_immutable_oracle_fee_phase_and_detector_authority()
{
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      set_observation(
        7,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("ObservationCrossing differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      let transition = crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      };

      let hot = {
        assert_ok!(Actors::note_observation_transition(7, transition));
        drain_crossing_work();
        Actors::actor_hot(actor_id)
          .expect("reference ObservationCrossing differential hot state exists")
      };
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::ObservationCrossing,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id),
        crate::CrossingMemberships::<Test>::get(actor_id)
          .map(|membership| membership.key.threshold),
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("observation_crossing_success", &baseline);
}

#[test]
fn control_cadenced_trigger_boundary_matches_immutable_oracle_logical_fee_and_event_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        timer_schedule(5),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("Cadenced differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      frame_system::Pallet::<Test>::set_block_number(6);

      let hot = {
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        Actors::drain_overdue_wakeups_cursor(6, &mut meter);
        Actors::actor_hot(actor_id).expect("reference Cadenced differential hot state exists")
      };
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::Cadenced,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        hot.trigger_wakeup_pointer.is_some(),
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("cadenced_success", &baseline);
}

#[test]
fn control_manual_collection_failure_matches_immutable_oracle_retained_logical_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("Manual failure differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      set_fail_fee_sink_transfer(true);
      {
        assert!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id).is_err());
      }
      set_fail_fee_sink_transfer(false);
      let hot =
        { Actors::actor_hot(actor_id).expect("reference Manual failure hot state remains") };
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
                actor_id: id,
                trigger_family: TriggerFamily::Manual,
                ..
              }) if *id == actor_id
            )
          })
          .count(),
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("manual_collection_failure", &baseline);
}

#[test]
fn control_at_time_collection_failure_matches_immutable_oracle_retained_due_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        at_time_schedule(1),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("AtTime failure differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      set_fail_fee_sink_transfer(true);
      frame_system::Pallet::<Test>::set_block_number(2);
      {
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        Actors::drain_overdue_wakeups_cursor(2, &mut meter);
      }
      set_fail_fee_sink_transfer(false);
      let hot =
        { Actors::actor_hot(actor_id).expect("reference AtTime failure hot state remains") };
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
        Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
                actor_id: id,
                trigger_family: TriggerFamily::AtTime,
                ..
              }) if *id == actor_id
            )
          })
          .count(),
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("at_time_collection_failure", &baseline);
}

#[test]
fn control_address_event_collection_failure_matches_immutable_oracle_independent_funding_authority()
{
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        percentage_trigger_schedule(),
        None,
        contract_steps_with_step(make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
        })),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("AddressEvent failure differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      set_fail_fee_sink_transfer(true);
      {
        assert_ok!(Actors::notify_address_event(
          actor_id,
          TestAsset::Native,
          100,
          &ALICE,
        ));
      }
      set_fail_fee_sink_transfer(false);
      let hot =
        { Actors::actor_hot(actor_id).expect("reference AddressEvent failure hot state remains") };
      let funding = ActorFunding::<Test>::get(actor_id)
        .expect("AddressEvent failure funding remains")
        .funding_accumulated
        .get(&TestAsset::Native)
        .copied();
      let funding_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::FundingAccumulated {
              actor_id: id,
              asset: TestAsset::Native,
              added: 100,
              accumulated: 100,
            }) if *id == actor_id
          )
        })
        .count();
      let trigger_events = System::events()
        .iter()
        .filter(|record| {
          matches!(
            &record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
              actor_id: id,
              trigger_family: TriggerFamily::AddressEvent,
              ..
            }) if *id == actor_id
          )
        })
        .count();
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        funding,
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        funding_events,
        trigger_events,
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("address_event_collection_failure", &baseline);
}

#[test]
fn control_observation_change_collection_failure_matches_immutable_oracle_detector_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let feed = 35;
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        observation_schedule(vec![feed]),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("ObservationChange failure differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let subscription = crate::ObservationSubscriptionSlot::<Test>::get(actor_id)
        .expect("ObservationChange failure differential subscription exists");
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      set_fail_fee_sink_transfer(true);
      {
        assert_ok!(Actors::note_observation_changed(feed, 1));
        assert_eq!(Actors::do_fanout_dirty_observation_page(), Ok(false));
      }
      set_fail_fee_sink_transfer(false);
      let hot = {
        Actors::actor_hot(actor_id).expect("reference ObservationChange failure hot state remains")
      };
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id),
        crate::ObservationSubscriptionSlot::<Test>::get(actor_id),
        subscription,
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
                actor_id: id,
                trigger_family: TriggerFamily::ObservationChange,
                ..
              }) if *id == actor_id
            )
          })
          .count(),
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("observation_change_collection_failure", &baseline);
}

#[test]
fn control_observation_crossing_collection_failure_matches_immutable_oracle_phase_authority() {
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      set_observation(
        7,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("ObservationCrossing failure differential Actor exists")
        .sovereign_account;
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        u128::from(u64::MAX / 4),
      );
      let custody_before = native_balance(&sovereign);
      clear_fee_collections();
      System::reset_events();
      set_fail_fee_sink_transfer(true);
      let transition = crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      };
      {
        assert_ok!(Actors::note_observation_transition(7, transition));
        drain_crossing_work();
      }
      set_fail_fee_sink_transfer(false);
      let hot = {
        Actors::actor_hot(actor_id)
          .expect("reference ObservationCrossing failure hot state remains")
      };
      (
        custody_before.saturating_sub(native_balance(&sovereign)),
        fee_collections(),
        hot.pending_signal,
        hot.cycle_state,
        hot.trigger_runtime_state,
        crate::IndexedTriggerDetectionDisabled::<Test>::contains_key(actor_id),
        crate::CrossingMemberships::<Test>::get(actor_id)
          .map(|membership| membership.key.threshold),
        System::events()
          .iter()
          .filter(|record| {
            matches!(
              &record.event,
              RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
                actor_id: id,
                trigger_family: TriggerFamily::ObservationCrossing,
                ..
              }) if *id == actor_id
            )
          })
          .count(),
      )
    })
  };

  let baseline = execute();
  emit_baseline_oracle("observation_crossing_collection_failure", &baseline);
}

#[test]
fn control_canonical_frame_backing_round_trips_owner_seam_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = frame_install_temporal_user_unsignaled();
    let (location, mut identity, hot, admission) = Actors::load_frame_control_authority(actor_id)
      .expect("canonical frame backing resolves sole authority");
    assert_eq!(location, C1Location::Unsignaled);
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    frame_assert_single_owner();

    identity.last_control_mutation_block = 2;
    assert!(Actors::store_frame_control_authority(
      actor_id,
      location,
      identity.clone(),
      hot.clone(),
      admission.clone(),
    ));
    let restored = Actors::load_frame_control_authority(actor_id)
      .expect("canonical frame backing reloads updated authority");
    assert_eq!(
      restored,
      (location, identity.clone(), hot.clone(), admission.clone())
    );

    let mut invalid_hot = hot;
    invalid_hot.queue_ticket = Some(9);
    assert!(!Actors::store_frame_control_authority(
      actor_id,
      location,
      identity.clone(),
      invalid_hot,
      admission.clone(),
    ));
    assert_eq!(
      Actors::load_frame_control_authority(actor_id),
      Some((location, identity, restored.2, admission))
    );
    frame_assert_single_owner();
  });
}

fn frame_install_temporal_user_unsignaled() -> ActorId {
  frame_system::Pallet::<Test>::set_block_number(1);
  let actor_id = create_user_with(
    ALICE,
    Mutability::Mutable,
    timer_schedule(5),
    None,
    contract_steps_with_step(make_step(Task::StopCycle)),
  );
  Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
    .expect("reference User Trigger wakeup invalidates before candidate projection");
  let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
    .expect("temporal User Actor remains coherent");
  let unsignaled = Actors::control_unsignaled_cell_from_scalar(
    actor_id,
    state.identity,
    state.hot,
    admission,
    &loaded_step,
  )
  .expect("temporal User authority projects Unsignaled");
  crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, unsignaled);
  crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);
  actor_id
}

#[test]
fn control_user_temporal_latch_charges_exact_trigger_fee_and_advances_failed_cadence() {
  new_test_ext().execute_with(|| {
    let actor_id = frame_install_temporal_user_unsignaled();
    let source_key = WakeupKey::Tick(9);
    let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("candidate temporal User cell exists");
    let cell = Actors::control_schedule_fresh_wakeup_reference(cell, source_key)
      .expect("candidate User Trigger reference schedules");
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    clear_fee_collections();
    System::reset_events();

    set_fail_fee_sink_transfer(true);
    assert_eq!(
      Actors::control_latch_due_temporal_reference(source_key, 1, 9)
        .expect("failed Cadenced collection advances detector deadline"),
      (actor_id, C1Location::Unsignaled)
    );
    set_fail_fee_sink_transfer(false);
    clear_fee_collections();
    let next_key = WakeupKey::Tick(11);
    let advanced = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("failed Cadenced collection retains sole Unsignaled primary");
    assert!(!advanced.hot.pending_signal);
    assert_eq!(
      advanced
        .hot
        .trigger_wakeup_pointer
        .map(|pointer| pointer.tick),
      Some(11)
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(next_key)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Block), None);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));

    let (_, destination) = Actors::control_latch_due_temporal_reference(next_key, 1, 11)
      .expect("funded User temporal occurrence latches");
    assert_eq!(fee_collections(), vec![cadenced_trigger_fee()]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        fee,
      } if *id == actor_id && *fee == cadenced_trigger_fee()
    )));
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(destination)
    );
    frame_assert_single_owner();
  });
}

fn frame_assert_single_owner() {
  Actors::frame_control_entries().expect("canonical frame traversal preserves sole ownership");
}

#[test]
fn control_ready_user_opening_projects_exact_scalar_authority_without_event_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("ready User Opening is coherent");
    let ticket = state.hot.queue_ticket.expect("Manual Opening is ready");
    let (_, queue_entry) = Actors::paged_head_entry().expect("canonical Ready ticket exists");
    let expected_identity = state.identity.clone();
    let expected_hot = state.hot.clone();
    let expected_admission = admission.clone();
    let events = System::events();
    let cell = Actors::control_opening_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &queue_entry,
      &loaded_step,
    )
    .expect("ready Opening projects to one frame cell");
    let restored = Actors::project_control_cell(&cell, C1Location::Ready { ticket })
      .expect("ready frame cell restores complete scalar authority");
    assert_eq!(restored.0, expected_identity);
    assert_eq!(restored.1, expected_hot);
    assert_eq!(restored.2, expected_admission);
    assert_eq!(System::events(), events, "pure projection emits no event");
  });
}

#[test]
fn control_complete_user_stop_cycle_projects_exact_scalar_authority_without_event_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("completed StopCycle re-arms one coherent Manual Opening");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(!state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert!(state.run_state.is_none());
    let expected_identity = state.identity.clone();
    let expected_hot = state.hot.clone();
    let expected_admission = admission.clone();
    let events = System::events();
    let cell = Actors::control_unsignaled_cell_from_scalar(
      actor_id,
      state.identity,
      state.hot,
      admission,
      &loaded_step,
    )
    .expect("complete StopCycle authority projects to one unsignaled frame cell");

    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
    crate::ActorControlLocators::<Test>::insert(actor_id, C1Location::Unsignaled);

    let restored = Actors::project_control_cell(&cell, C1Location::Unsignaled)
      .expect("unsignaled frame cell restores complete scalar authority");
    assert_eq!(restored.0, expected_identity);
    assert_eq!(restored.1, expected_hot);
    assert_eq!(restored.2, expected_admission);
    assert_eq!(
      System::events(),
      events,
      "projection emits no semantic event"
    );
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    frame_assert_single_owner();
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Unsignaled)
    );
    frame_assert_single_owner();
  });
}

#[test]
fn control_direct_ready_stop_cycle_matches_immutable_oracle() {
  let baseline = new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(0);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("reference User exists")
      .sovereign_account;
    let reserve = u128::from(u64::MAX / 4);
    let _ = <Test as crate::Config>::AssetOps::mint(
      &sovereign,
      <Test as crate::Config>::FeeNativeAssetId::get(),
      reserve,
    );
    let fee_sink = <Test as crate::Config>::FeeSink::get();
    let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
    System::reset_events();
    let sovereign_before = <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset);
    let sink_before = <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset);
    run_idle(Weight::MAX);
    let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
      .expect("reference StopCycle successor is coherent");
    let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
    let hold = crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
    (
      state.identity,
      state.hot,
      admission,
      loaded_step,
      funding,
      System::events(),
      sovereign_before.saturating_sub(<Test as crate::Config>::AssetOps::balance(
        &sovereign,
        native_asset,
      )),
      <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset)
        .saturating_sub(sink_before),
      hold.owner,
    )
  });

  emit_baseline_oracle("direct_stop_cycle", &baseline);
}

#[test]
fn control_direct_transfer_predicate_matrix_matches_immutable_oracle() {
  let execute = |predicate_count: u32, predicates_match: bool| {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(0);
      let predicate_assets = (0..predicate_count)
        .map(|index| TestAsset::Local(10_000 + index))
        .collect::<Vec<_>>();
      let precondition = (!predicate_assets.is_empty()).then(|| {
        timed_all_conditions(
          ObservationTiming::Opening,
          predicate_assets
            .iter()
            .copied()
            .map(|asset| Predicate::BalanceAbove {
              asset,
              threshold: if predicates_match { 50 } else { Balance::MAX },
            })
            .collect(),
        )
        .expect("direct Transfer predicates fit")
      });
      let step = StepOf::<Test> {
        precondition,
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        },
        on_error: StepErrorPolicy::AbortCycle,
      };
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps_with_step(step),
      );
      let sovereign = Actors::active_actor_view(actor_id)
        .expect("direct Transfer User exists")
        .sovereign_account;
      let reserve = u128::from(u64::MAX / 4);
      let _ = <Test as crate::Config>::AssetOps::mint(
        &sovereign,
        <Test as crate::Config>::FeeNativeAssetId::get(),
        reserve,
      );
      for asset in predicate_assets {
        set_asset_balance(&sovereign, asset, 100);
      }
      frame_system::Pallet::<Test>::set_block_number(1);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let fee_sink = <Test as crate::Config>::FeeSink::get();
      let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
      System::reset_events();
      let sovereign_before = <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset);
      let recipient_before = <Test as crate::Config>::AssetOps::balance(&BOB, native_asset);
      let sink_before = <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset);

      {
        run_idle(Weight::MAX);
        let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
          .expect("reference Transfer successor is coherent");
        let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
        let hold =
          crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
        (
          state.identity,
          state.hot,
          admission,
          loaded_step,
          funding,
          System::events(),
          sovereign_before.saturating_sub(<Test as crate::Config>::AssetOps::balance(
            &sovereign,
            native_asset,
          )),
          <Test as crate::Config>::AssetOps::balance(&BOB, native_asset)
            .saturating_sub(recipient_before),
          <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset)
            .saturating_sub(sink_before),
          hold.owner,
        )
      }
    })
  };

  for (predicate_count, predicates_match) in [(0u32, true), (2, true), (4, true), (2, false)] {
    let baseline = execute(predicate_count, predicates_match);
    emit_baseline_oracle(
      &format!("direct_transfer_p{predicate_count}_match_{predicates_match}"),
      &baseline,
    );
  }
}

#[test]
fn control_direct_funding_unavailable_requeues_exact_next_block_matching_immutable_oracle() {
  let step = StepOf::<Test> {
    precondition: None,
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(Balance::MAX),
    },
    on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
  };
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = {
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          contract_steps_with_step(step.clone()),
        );
        let sovereign = Actors::active_actor_view(actor_id)
          .expect("reference FundingUnavailable User exists")
          .sovereign_account;
        let reserve = u128::from(u64::MAX / 4);
        let _ = <Test as crate::Config>::AssetOps::mint(
          &sovereign,
          <Test as crate::Config>::FeeNativeAssetId::get(),
          reserve,
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        actor_id
      };
      let sovereign = Actors::sovereign_account_id(&ALICE, 0);
      let fee_sink = <Test as crate::Config>::FeeSink::get();
      let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
      System::reset_events();
      let sovereign_before = <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset);
      let sink_before = <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset);

      {
        run_idle(Weight::MAX);
        let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
          .expect("reference FundingUnavailable suspension is coherent");
        let run = state
          .run_state
          .expect("reference FundingUnavailable run persists");
        let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
        let hold =
          crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
        (
          state.identity,
          state.hot,
          admission,
          loaded_step,
          run.encode(),
          funding,
          System::events(),
          sovereign_before.saturating_sub(<Test as crate::Config>::AssetOps::balance(
            &sovereign,
            native_asset,
          )),
          <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset)
            .saturating_sub(sink_before),
          hold.owner,
        )
      }
    })
  };

  let baseline = execute();
  emit_baseline_oracle("direct_funding_unavailable", &baseline);
}

#[test]
fn control_direct_temporary_failure_requeues_with_atomic_effect_rollback_matching_immutable_oracle()
{
  let step = temporary_retry_swap_plan()[0].clone();
  let execute = || {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      setup_temporary_retry_pool();
      set_temporary_dex_failure(true);
      let actor_id = {
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          contract_steps_with_step(step.clone()),
        );
        let sovereign = Actors::active_actor_view(actor_id)
          .expect("reference Temporary User exists")
          .sovereign_account;
        let reserve = u128::from(u64::MAX / 4);
        let _ = <Test as crate::Config>::AssetOps::mint(
          &sovereign,
          <Test as crate::Config>::FeeNativeAssetId::get(),
          reserve,
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        actor_id
      };
      let sovereign = Actors::sovereign_account_id(&ALICE, 0);
      let fee_sink = <Test as crate::Config>::FeeSink::get();
      let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
      let output_asset = TestAsset::Local(77);
      System::reset_events();

      {
        run_idle(Weight::MAX);
        let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
          .expect("reference Temporary suspension is coherent");
        let run = state.run_state.expect("reference Temporary run persists");
        let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
        let hold =
          crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
        (
          state.identity,
          state.hot,
          admission,
          loaded_step,
          run.encode(),
          funding,
          System::events(),
          [
            <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset),
            <Test as crate::Config>::AssetOps::balance(&sovereign, output_asset),
            <Test as crate::Config>::AssetOps::balance(&u64::MAX, native_asset),
            <Test as crate::Config>::AssetOps::balance(&u64::MAX, output_asset),
            <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset),
          ],
          hold.owner,
        )
      }
    })
  };

  let baseline = execute();
  emit_baseline_oracle("direct_temporary_failure", &baseline);
}

#[test]
fn control_suspended_temporary_continuation_matches_immutable_oracle_success_and_backoff() {
  let step = temporary_retry_swap_plan()[0].clone();
  let execute = |retry_fails: bool| {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      setup_temporary_retry_pool();
      set_temporary_dex_failure(true);
      let actor_id = {
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          contract_steps_with_step(step.clone()),
        );
        let sovereign = Actors::active_actor_view(actor_id)
          .expect("reference retry User exists")
          .sovereign_account;
        let _ = <Test as crate::Config>::AssetOps::mint(
          &sovereign,
          <Test as crate::Config>::FeeNativeAssetId::get(),
          u128::from(u64::MAX / 4),
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        actor_id
      };
      {
        run_idle(Weight::MAX);
      }

      set_temporary_dex_failure(retry_fails);
      frame_system::Pallet::<Test>::set_block_number(2);
      System::reset_events();
      {
        run_idle(Weight::MAX);
      }

      let sovereign = Actors::sovereign_account_id(&ALICE, 0);
      let fee_sink = <Test as crate::Config>::FeeSink::get();
      let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
      let output_asset = TestAsset::Local(77);
      {
        let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
          .expect("reference retry successor is coherent");
        let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
        let hold =
          crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
        (
          state.identity,
          state.hot,
          admission,
          loaded_step,
          state.run_state.map(|run| run.encode()),
          funding,
          System::events(),
          [
            <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset),
            <Test as crate::Config>::AssetOps::balance(&sovereign, output_asset),
            <Test as crate::Config>::AssetOps::balance(&u64::MAX, native_asset),
            <Test as crate::Config>::AssetOps::balance(&u64::MAX, output_asset),
            <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset),
          ],
          hold.owner,
        )
      }
    })
  };

  for retry_fails in [false, true] {
    let baseline = execute(retry_fails);
    emit_baseline_oracle(&format!("temporary_retry_fails_{retry_fails}"), &baseline);
  }
}

#[test]
fn control_suspended_funding_continuation_matches_immutable_oracle_success_and_backoff() {
  let transfer_amount = u128::from(u64::MAX);
  let step = StepOf::<Test> {
    precondition: None,
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(transfer_amount),
    },
    on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
  };
  let execute = |retry_fails: bool| {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = {
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          contract_steps_with_step(step.clone()),
        );
        let sovereign = Actors::active_actor_view(actor_id)
          .expect("reference Funding retry User exists")
          .sovereign_account;
        let _ = <Test as crate::Config>::AssetOps::mint(
          &sovereign,
          <Test as crate::Config>::FeeNativeAssetId::get(),
          u128::from(u64::MAX / 4),
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        actor_id
      };
      {
        run_idle(Weight::MAX);
      }

      let sovereign = Actors::sovereign_account_id(&ALICE, 0);
      let native_asset = <Test as crate::Config>::FeeNativeAssetId::get();
      if !retry_fails {
        let _ = <Test as crate::Config>::AssetOps::mint(&sovereign, native_asset, transfer_amount);
      }
      frame_system::Pallet::<Test>::set_block_number(2);
      System::reset_events();
      {
        run_idle(Weight::MAX);
      }

      let fee_sink = <Test as crate::Config>::FeeSink::get();
      {
        let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
          .expect("reference Funding retry successor is coherent");
        let funding = ActorFunding::<Test>::get(actor_id).expect("reference funding survives");
        let hold =
          crate::ActorStateHolds::<Test>::get(actor_id).expect("reference User hold survives");
        (
          state.identity,
          state.hot,
          admission,
          loaded_step,
          state.run_state.map(|run| run.encode()),
          funding,
          System::events(),
          [
            <Test as crate::Config>::AssetOps::balance(&sovereign, native_asset),
            <Test as crate::Config>::AssetOps::balance(&BOB, native_asset),
            <Test as crate::Config>::AssetOps::balance(&fee_sink, native_asset),
          ],
          hold.owner,
        )
      }
    })
  };

  for retry_fails in [false, true] {
    let baseline = execute(retry_fails);
    emit_baseline_oracle(&format!("funding_retry_fails_{retry_fails}"), &baseline);
  }
}

#[test]
fn ordinary_fifo_cutoff_preserves_rejected_head_and_untouched_suffix() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_direct_stop_cycle_ready(3);
    System::reset_events();
    let head_before_first = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (3).min(head_before_first.saturating_add(1)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_first), 1);

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 1);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 2);
    assert!(crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_ids[0]
    ));
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_ids[1]),
      Some(C1Location::Ready { ticket: 1 })
    ));
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_ids[2]),
      Some(C1Location::Ready { ticket: 2 })
    ));
    frame_assert_single_owner();

    let events_before_rejection = System::events();
    let root_before_rejection =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let head_before_rejected = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(
      Weight::zero(),
      (3).min(head_before_rejected.saturating_add(32)),
    );
    assert_eq!(Actors::queue_head().saturating_sub(head_before_rejected), 0);

    assert_eq!(System::events(), events_before_rejection);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before_rejection,
    );
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 1);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 2);
    frame_assert_single_owner();

    let head_before_resumed = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (3).min(head_before_resumed.saturating_add(32)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_resumed), 2);

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
    for actor_id in actor_ids {
      let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
        .expect("ordinary FIFO successor exists");
      assert_eq!(cell.identity.cycle_nonce, 1);
      assert!(!cell.hot.pending_signal);
    }
    frame_assert_single_owner();
  });
}

#[test]
fn control_empty_partial_ready_chunk_retains_ticket_addressability() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_direct_stop_cycle_ready(3);
    Actors::execute_cycle_to_cutoff(Weight::MAX, 3);
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
    assert!(!crate::ActorReadyFrameChunks::<Test>::contains_key(0));
    for actor_id in actor_ids {
      let cell = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
        .expect("completed Actor retains its Unsignaled primary");
      assert_eq!(cell.identity.cycle_nonce, 1);
      assert_eq!(cell.hot.cycle_state, CycleState::Idle);
      assert!(!cell.hot.pending_signal);
      assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    }

    let actor_id = frame_install_direct_stop_cycle_ready(1)[0];
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 4);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 1);
    let appended = crate::ActorReadyFrameChunks::<Test>::get(0)
      .expect("new Ready cell appends after retained ticket slots");
    assert_eq!(appended.len(), 32);
    assert!(appended[..3].iter().all(Option::is_none));
    assert_eq!(
      appended[3].as_ref().map(|cell| cell.actor_id),
      Some(actor_id)
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(C1Location::Ready { ticket: 3 })
    );
    frame_assert_single_owner();
  });
}

#[test]
fn control_ready_removal_preserves_ticket_addresses_and_normalizes_head_boundedly() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_direct_stop_cycle_ready(35);
    for actor_id in actor_ids.iter().take(32).copied() {
      Actors::control_remove_ready_primary(actor_id)
        .expect("head-page Ready removal commits atomically");
    }
    Actors::control_remove_ready_primary(actor_ids[34])
      .expect("tail Ready removal commits atomically");

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 0);
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 35);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 2);
    assert!(crate::ActorReadyFrameChunks::<Test>::contains_key(0));
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_ids[32]),
      Some(C1Location::Ready { ticket: 32 })
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_ids[33]),
      Some(C1Location::Ready { ticket: 33 })
    );

    assert_eq!(Actors::control_normalize_ready_head(35, 16), Ok((16, None)));
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 16);
    assert!(crate::ActorReadyFrameChunks::<Test>::contains_key(0));
    assert_eq!(
      Actors::control_normalize_ready_head(35, 16),
      Ok((16, Some(32)))
    );
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 32);
    assert!(!crate::ActorReadyFrameChunks::<Test>::contains_key(0));
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 35);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 2);
    frame_assert_single_owner();
  });
}

#[test]
fn ordinary_fifo_corrupt_head_keeps_committed_prefix_and_untouched_suffix() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_direct_stop_cycle_ready(3);
    let failed_head = crate::ActorContractHeads::<Test>::take(actor_ids[1])
      .expect("failed-head Contract exists before injection");
    let untouched = [
      Actors::actor_control_cell(actor_ids[1]).expect("corrupt head retains physical primary"),
      Actors::actor_control_cell(actor_ids[2]).expect("unreached suffix primary"),
    ];
    System::reset_events();
    let head_before_report = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (3).min(head_before_report.saturating_add(32)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_report), 1);

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 1);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 2);
    assert!(crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_ids[0]
    ));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_ids[1]
    ));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_ids[2]
    ));
    let started = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect::<Vec<_>>();
    assert_eq!(started, vec![actor_ids[0]]);
    assert_eq!(
      Actors::actor_control_cell(actor_ids[1]),
      Some(untouched[0].clone()),
    );
    assert_eq!(
      Actors::actor_control_cell(actor_ids[2]),
      Some(untouched[1].clone()),
    );
    frame_assert_single_owner();

    crate::ActorContractHeads::<Test>::insert(actor_ids[1], failed_head);
    let head_before_resumed = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::execute_cycle_to_cutoff(Weight::MAX, (3).min(head_before_resumed.saturating_add(32)));
    assert_eq!(Actors::queue_head().saturating_sub(head_before_resumed), 2);

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
    frame_assert_single_owner();
  });
}

#[test]
fn control_waiting_round_trip_preserves_primary_and_coexisting_wakeup_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(5),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("temporal User state is coherent");
    let trigger_pointer = state
      .hot
      .trigger_wakeup_pointer
      .expect("Cadenced User has Trigger wakeup authority");
    assert!(state.hot.wakeup_pointer.is_none());
    assert!(!state.hot.pending_signal);
    let trigger_location = C1Location::Waiting {
      key: WakeupKey::Tick(trigger_pointer.tick),
      page: trigger_pointer.page_id,
      slot: u8::try_from(trigger_pointer.slot).expect("Trigger slot fits C32"),
    };
    let trigger_cell = Actors::control_cell_from_parts(
      actor_id,
      state.identity.clone(),
      state.hot.clone(),
      admission.clone(),
      &loaded_step,
      None,
    )
    .expect("trigger-only Waiting cell projects");
    Actors::remove_primary_control_cell_inner(actor_id)
      .expect("diagnostic projection removes the lifecycle-installed source primary");
    let mut trigger_chunk = BoundedVec::try_from(vec![None; trigger_pointer.slot as usize + 1])
      .expect("trigger Waiting slot fits C32");
    trigger_chunk[trigger_pointer.slot as usize] =
      Some(crate::ActorWaitingEntry::Primary(trigger_cell.clone()));
    crate::ActorWaitingFrameChunks::<Test>::insert(
      (
        WakeupKey::Tick(trigger_pointer.tick),
        trigger_pointer.page_id,
      ),
      crate::ActorWaitingPageOf::<Test> {
        entries: trigger_chunk,
        live_entries: 1,
        scan_slot: 0,
        previous_page: None,
        next_page: None,
      },
    );
    crate::ActorControlLocators::<Test>::insert(actor_id, trigger_location);
    let restored_trigger = Actors::project_control_cell(&trigger_cell, trigger_location)
      .expect("trigger-only Waiting authority restores");
    assert_eq!(restored_trigger.0, state.identity);
    assert_eq!(restored_trigger.1, state.hot);
    assert_eq!(restored_trigger.2, admission);
    assert!(
      Actors::project_control_cell(&trigger_cell, C1Location::Unsignaled).is_none(),
      "temporal membership cannot masquerade as Unsignaled"
    );
    frame_assert_single_owner();

    crate::ActorWaitingFrameChunks::<Test>::remove((
      WakeupKey::Tick(trigger_pointer.tick),
      trigger_pointer.page_id,
    ));
    let process_pointer = WakeupPointer {
      block: WakeupKey::Block(7),
      page_id: trigger_pointer.page_id.saturating_add(1),
      slot: 2,
    };
    let mut coexisting_cell = trigger_cell;
    coexisting_cell.hot.cycle_state = CycleState::Suspended;
    coexisting_cell.hot.wakeup_pointer = Some(process_pointer);
    coexisting_cell.eligible_at = Some(7);
    let process_location = C1Location::Waiting {
      key: process_pointer.block,
      page: process_pointer.page_id,
      slot: u8::try_from(process_pointer.slot).expect("process slot fits C32"),
    };
    let mut process_chunk = BoundedVec::try_from(vec![None; process_pointer.slot as usize + 1])
      .expect("process Waiting slot fits C32");
    process_chunk[process_pointer.slot as usize] =
      Some(crate::ActorWaitingEntry::Primary(coexisting_cell.clone()));
    crate::ActorWaitingFrameChunks::<Test>::insert(
      (process_pointer.block, process_pointer.page_id),
      crate::ActorWaitingPageOf::<Test> {
        entries: process_chunk,
        live_entries: 1,
        scan_slot: 0,
        previous_page: None,
        next_page: None,
      },
    );
    crate::ActorControlLocators::<Test>::insert(actor_id, process_location);
    let restored_coexisting = Actors::project_control_cell(&coexisting_cell, process_location)
      .expect("coexisting process/Trigger authority restores");
    let mut expected_hot = state.hot;
    expected_hot.cycle_state = CycleState::Suspended;
    expected_hot.wakeup_pointer = Some(process_pointer);
    assert_eq!(restored_coexisting.0, state.identity);
    assert_eq!(restored_coexisting.1, expected_hot);
    assert_eq!(restored_coexisting.2, admission);
    assert_eq!(
      restored_coexisting.1.trigger_wakeup_pointer,
      Some(trigger_pointer)
    );
    assert!(
      Actors::project_control_cell(&coexisting_cell, trigger_location).is_none(),
      "process wakeup is the primary physical placement while Trigger membership coexists"
    );
    frame_assert_single_owner();
  });
}

#[test]
fn control_temporal_transition_preserves_n_plus_one_cutoff_and_pointer_cleanup() {
  new_test_ext().execute_with(|| {
    let actor_id = frame_install_temporal_system_unsignaled(1)[0];

    let trigger_location = Actors::control_stage_unsignaled_temporal(actor_id, 10)
      .expect("Unsignaled temporal cell stages atomically");
    assert_eq!(
      trigger_location,
      C1Location::Waiting {
        key: WakeupKey::Tick(10),
        page: 0,
        slot: 0,
      }
    );
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      crate::ActorWaitingHeads::<Test>::get(WakeupKey::Tick(10)),
      0
    );
    assert_eq!(
      crate::ActorWaitingTails::<Test>::get(WakeupKey::Tick(10)),
      1
    );
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Tick(10)),
      1
    );
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Tick),
      Some(WakeupKey::Tick(10))
    );
    assert_eq!(
      crate::ActorWaitingCursorIndices::<Test>::get(WakeupKey::Tick(10)),
      Some(0)
    );
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(10, 0, 1, 9),
      Err(crate::scheduler::ActorControlTransitionError::Invariant),
      "future Trigger key cannot bypass the shared deadline heap"
    );

    let latched = Actors::control_latch_temporal_waiting_page(10, 0, 1, 10)
      .expect("temporal due page latches into N+1 service waiting");
    assert_eq!(latched, vec![actor_id]);
    assert!(!crate::ActorWaitingFrameChunks::<Test>::contains_key((
      WakeupKey::Tick(10),
      0
    )));
    assert!(!crate::ActorWaitingTails::<Test>::contains_key(
      WakeupKey::Tick(10)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Block), 1);
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Block),
      Some(WakeupKey::Block(2))
    );
    let service_location = C1Location::Waiting {
      key: WakeupKey::Block(2),
      page: 0,
      slot: 0,
    };
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(service_location)
    );
    let service_cell = crate::ActorWaitingFrameChunks::<Test>::get((WakeupKey::Block(2), 0))
      .and_then(|chunk| chunk.entries.first().cloned().flatten())
      .and_then(crate::ActorWaitingEntry::into_primary)
      .expect("N+1 service cell exists");
    assert!(service_cell.hot.pending_signal);
    assert!(service_cell.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(service_cell.eligible_at, Some(2));
    assert_eq!(
      service_cell.hot.wakeup_pointer,
      Some(WakeupPointer {
        block: WakeupKey::Block(2),
        page_id: 0,
        slot: 0,
      })
    );
    assert!(Actors::project_control_cell(&service_cell, service_location).is_some());
    assert_eq!(Actors::wakeup_cursor_pop_min(), None);
    assert!(!Actors::wakeup_cursor_remove(2));
    assert_eq!(
      Actors::wakeup_cursor_peek_key(WakeupClock::Block),
      Some(WakeupKey::Block(2)),
      "generic cursor controls cannot detach candidate-owned Waiting authority"
    );

    assert_eq!(
      Actors::control_promote_due_waiting_page(2, 0, 1),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    );
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(service_location),
      "premature promotion rolls back source authority"
    );
    let cutoff_before_due = crate::ActorReadyTail::<Test>::get();
    assert_eq!(cutoff_before_due, 0);
    let promoted = Actors::control_promote_due_waiting_page(2, 0, 2)
      .expect("N+1 waiting page promotes in physical order");
    assert_eq!(promoted, vec![(actor_id, 0)]);
    assert!(!crate::ActorWaitingTails::<Test>::contains_key(
      WakeupKey::Block(2)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Block), 0);
    assert_eq!(crate::ActorReadyHead::<Test>::get(), 0);
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 1);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 1);
    let ready = crate::ActorReadyFrameChunks::<Test>::get(0)
      .and_then(|chunk| chunk.first().cloned().flatten())
      .expect("promoted Ready cell exists");
    assert!(ready.hot.pending_signal);
    assert!(ready.hot.wakeup_pointer.is_none());
    assert!(ready.hot.trigger_wakeup_pointer.is_none());
    assert_eq!(ready.eligible_at, Some(2));
    assert!(Actors::project_control_cell(&ready, C1Location::Ready { ticket: 0 }).is_some());

    let head_before_isolated = Actors::queue_head();
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle_to_cutoff(
      Weight::MAX,
      (cutoff_before_due).min(head_before_isolated.saturating_add(1)),
    );
    assert_eq!(Actors::queue_head().saturating_sub(head_before_isolated), 0);

    assert_eq!(crate::ActorReadyHead::<Test>::get(), 0);
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 1);
    frame_assert_single_owner();
  });
}

#[test]
fn control_due_page_moves_are_atomic_and_preserve_causal_ticket_order() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_temporal_system_unsignaled(3);
    for actor_id in actor_ids.iter().copied() {
      Actors::control_stage_unsignaled_temporal(actor_id, 10)
        .expect("causal temporal cohort stages");
    }
    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Tick(10), 0), |maybe| {
      let chunk = maybe.as_mut().expect("Trigger cohort page exists");
      let cell = chunk.entries[1]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("second Trigger cell exists");
      cell
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("second Trigger pointer exists")
        .tick = 99;
    });
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(10, 0, 1, 10),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    );
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Tick(10)),
      3
    );
    assert!(!crate::ActorWaitingFrameChunks::<Test>::contains_key((
      WakeupKey::Block(2),
      0
    )));
    for (ticket, actor_id) in actor_ids.iter().copied().enumerate() {
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Waiting {
          key: WakeupKey::Tick(10),
          page: 0,
          slot: ticket as u8,
        })
      );
    }

    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Tick(10), 0), |maybe| {
      maybe
        .as_mut()
        .expect("Trigger cohort page survives rollback")
        .entries[1]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("second Trigger cell survives rollback")
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("second Trigger pointer survives rollback")
        .tick = 10;
    });
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(10, 0, 1, 10)
        .expect("repaired Trigger cohort latches"),
      actor_ids
    );
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(2)),
      3
    );

    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Block(2), 0), |maybe| {
      maybe.as_mut().expect("N+1 cohort page exists").entries[1]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("second N+1 cell exists")
        .eligible_at = Some(3);
    });
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, 0, 2),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    );
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 0);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(2)),
      3
    );
    for (slot, actor_id) in actor_ids.iter().copied().enumerate() {
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Waiting {
          key: WakeupKey::Block(2),
          page: 0,
          slot: slot as u8,
        })
      );
    }

    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Block(2), 0), |maybe| {
      maybe
        .as_mut()
        .expect("N+1 cohort page survives rollback")
        .entries[1]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("second N+1 cell survives rollback")
        .eligible_at = Some(2);
    });
    let promoted = Actors::control_promote_due_waiting_page(2, 0, 2)
      .expect("repaired N+1 cohort promotes atomically");
    assert_eq!(
      promoted,
      actor_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(ticket, actor_id)| (actor_id, ticket as u64))
        .collect::<Vec<_>>()
    );
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 3);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 3);
    assert!(!crate::ActorWaitingTails::<Test>::contains_key(
      WakeupKey::Block(2)
    ));
    frame_assert_single_owner();
  });
}

#[test]
fn control_crossing_chunk_transition_rolls_back_then_resumes_exactly() {
  new_test_ext().execute_with(|| {
    let actor_ids = frame_install_temporal_system_unsignaled(34);
    for actor_id in actor_ids[..31].iter().copied() {
      Actors::control_stage_unsignaled_temporal(actor_id, 10)
        .expect("destination prefix cohort stages");
    }
    for actor_id in actor_ids[31..].iter().copied() {
      Actors::control_stage_unsignaled_temporal(actor_id, 11)
        .expect("crossing source cohort stages");
    }
    Actors::control_latch_temporal_waiting_page(10, 0, 1, 10)
      .expect("31-cell destination prefix latches");
    assert_eq!(
      crate::ActorWaitingTails::<Test>::get(WakeupKey::Block(2)),
      31
    );

    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Tick(11), 0), |maybe| {
      maybe.as_mut().expect("crossing source page exists").entries[2]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("third crossing source exists")
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("third crossing pointer exists")
        .tick = 99;
    });
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(11, 0, 1, 11),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    );
    assert_eq!(
      crate::ActorWaitingTails::<Test>::get(WakeupKey::Block(2)),
      31,
      "failed crossing append restores the destination tail"
    );
    assert_eq!(
      crate::ActorWaitingFrameChunks::<Test>::get((WakeupKey::Block(2), 0))
        .expect("destination prefix page survives")
        .entries
        .len(),
      32
    );
    assert!(!crate::ActorWaitingFrameChunks::<Test>::contains_key((
      WakeupKey::Block(2),
      1
    )));
    for (slot, actor_id) in actor_ids[31..].iter().copied().enumerate() {
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Waiting {
          key: WakeupKey::Tick(11),
          page: 0,
          slot: slot as u8,
        })
      );
    }

    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Tick(11), 0), |maybe| {
      maybe
        .as_mut()
        .expect("crossing source survives rollback")
        .entries[2]
        .as_mut()
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("third crossing source survives rollback")
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("third crossing pointer survives rollback")
        .tick = 11;
    });
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(11, 0, 1, 11).expect("crossing source resumes"),
      actor_ids[31..]
    );
    for (actor_id, page, slot) in [
      (actor_ids[31], 0, 31),
      (actor_ids[32], 1, 0),
      (actor_ids[33], 1, 1),
    ] {
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Waiting {
          key: WakeupKey::Block(2),
          page,
          slot,
        })
      );
    }
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, 0, 2)
        .expect("full first service page promotes")
        .len(),
      32
    );
    assert_eq!(
      crate::ActorWaitingHeads::<Test>::get(WakeupKey::Block(2)),
      32
    );
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(2)),
      2
    );
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, 1, 2)
        .expect("partial crossing service page promotes"),
      vec![(actor_ids[32], 32), (actor_ids[33], 33)]
    );
    assert_eq!(crate::ActorReadyTail::<Test>::get(), 34);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 34);
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Block), 0);
    frame_assert_single_owner();
  });
}

#[test]
fn control_temporal_actors_share_one_deadline_heap_key_until_last_close() {
  for close_first_created in [true, false] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let first = create_system_with(
        BOB,
        timer_schedule(5),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let due_tick = Actors::actor_hot(first)
        .expect("first temporal owner exists")
        .trigger_wakeup_pointer
        .expect("first deadline exists")
        .tick;
      let key = WakeupKey::Tick(due_tick);
      let cursor_len = crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick);
      let index = crate::ActorWaitingCursorIndices::<Test>::get(key)
        .expect("canonical Waiting owns its deadline index");
      let second = create_system_with(
        CHARLIE,
        timer_schedule(5),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      assert_eq!(
        crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick),
        cursor_len
      );
      assert_eq!(
        crate::ActorWaitingCursorIndices::<Test>::get(key),
        Some(index)
      );
      assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 2);
      let (first_closed, last_closed) = if close_first_created {
        (first, second)
      } else {
        (second, first)
      };
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_closed));
      assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 1);
      assert_eq!(
        crate::ActorWaitingCursorIndices::<Test>::get(key),
        Some(index)
      );
      assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), Some(key));
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), last_closed));
      assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(key));
      assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(key));
      assert_eq!(
        crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick),
        cursor_len - 1
      );
      frame_assert_single_owner();
    });
  }
}

#[test]
fn control_partial_ready_tail_preserves_existing_cells_before_causal_cohort() {
  new_test_ext().execute_with(|| {
    let existing = frame_install_direct_stop_cycle_ready(31);
    let before = crate::ActorReadyFrameChunks::<Test>::get(0)
      .expect("admitted existing actors occupy a partial Ready page");
    let cohort = frame_install_temporal_system_unsignaled(2);
    for actor_id in cohort.iter().copied() {
      Actors::control_stage_unsignaled_temporal(actor_id, 10)
        .expect("admitted temporal actor stages in its causal cohort");
    }
    assert_eq!(
      Actors::control_latch_temporal_waiting_page(10, 0, 1, 10)
        .expect("temporal cohort latches into next-block Waiting"),
      cohort
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      Actors::control_promote_due_waiting_page(2, 0, 2)
        .expect("due cohort crosses the partial Ready tail"),
      vec![(cohort[0], 31), (cohort[1], 32)]
    );
    let first = crate::ActorReadyFrameChunks::<Test>::get(0).expect("first page remains");
    let second = crate::ActorReadyFrameChunks::<Test>::get(1).expect("cohort spills to next page");
    assert_eq!(first.len(), 32);
    assert_eq!(second.len(), 32);
    for (slot, actor_id) in existing.iter().copied().enumerate() {
      assert_eq!(first[slot], before[slot]);
      assert_eq!(
        crate::ActorControlLocators::<Test>::get(actor_id),
        Some(C1Location::Ready {
          ticket: slot as u64
        })
      );
    }
    assert_eq!(
      first[31].as_ref().map(|cell| cell.actor_id),
      Some(cohort[0])
    );
    assert_eq!(
      second[0].as_ref().map(|cell| cell.actor_id),
      Some(cohort[1])
    );
    assert!(second.iter().skip(1).all(Option::is_none));
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 33);
    assert_eq!(Actors::queue_occupancy(), 33);
    assert!(!crate::ActorWaitingTails::<Test>::contains_key(
      WakeupKey::Block(2)
    ));
    frame_assert_single_owner();
  });
}

#[test]
fn control_mixed_clock_arbitration_flips_only_after_success_and_faults_fail_closed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let temporal_actor = frame_install_temporal_system_unsignaled(1)[0];
    Actors::control_stage_unsignaled_temporal(temporal_actor, 5)
      .expect("Tick primary enters canonical Waiting");
    Actors::control_append_waiting(
      frame_cell(900_000, 1),
      WakeupKey::Block(1),
      crate::scheduler::ActorWaitingAuthority::Service,
    )
    .expect("Block primary enters canonical Waiting");
    crate::NextWakeupClock::<Test>::put(WakeupClock::Tick);

    assert_eq!(
      Actors::control_service_next_due_waiting_unit(1, 5),
      Ok(Some((WakeupClock::Tick, 1)))
    );
    assert_eq!(crate::NextWakeupClock::<Test>::get(), WakeupClock::Block);
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(1)),
      1
    );
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(2)),
      1
    );

    assert_eq!(
      Actors::control_service_next_due_waiting_unit(1, 5),
      Ok(Some((WakeupClock::Block, 1)))
    );
    assert_eq!(crate::NextWakeupClock::<Test>::get(), WakeupClock::Tick);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 1);
    assert_eq!(
      Actors::control_service_next_due_waiting_unit(1, 5),
      Ok(None)
    );
    assert_eq!(crate::NextWakeupClock::<Test>::get(), WakeupClock::Tick);
    frame_assert_single_owner();
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let temporal_actor = frame_install_temporal_system_unsignaled(1)[0];
    Actors::control_stage_unsignaled_temporal(temporal_actor, 5)
      .expect("fault fixture Tick primary enters Waiting");
    Actors::control_append_waiting(
      frame_cell(900_001, 1),
      WakeupKey::Block(1),
      crate::scheduler::ActorWaitingAuthority::Service,
    )
    .expect("fault fixture Block primary enters Waiting");
    crate::NextWakeupClock::<Test>::put(WakeupClock::Tick);
    crate::ActorWaitingFrameChunks::<Test>::remove((WakeupKey::Tick(5), 0));

    assert_eq!(
      Actors::control_service_next_due_waiting_unit(1, 5),
      Err(crate::scheduler::ActorControlTransitionError::Invariant)
    );
    assert_eq!(crate::NextWakeupClock::<Test>::get(), WakeupClock::Tick);
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Block(1)),
      1
    );
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
  });
}
