use super::*;
use crate::{
  ActorAdmissionCertificates, ActorContractHeads, ActorContractTailChunks, ActorCostQuoteError,
  PipelineMachineFeeStrategy,
};
use frame::traits::ConstU32;

fn test_pipeline_machine_envelope() -> crate::PipelineMachineEnvelope<Balance> {
  crate::PipelineMachineEnvelope {
    pipeline_machine_fee_upper: 11,
    cleanup_fee_upper: 22,
  }
}

fn geometry_certificate(
  contract: &RuntimeActorContract,
) -> crate::ActorAdmissionCertificateOf<Test> {
  crate::ActorAdmissionCertificate::new(
    contract.semantic_contract_id(),
    contract.body_commitment().expect("body commitment"),
    1,
    [4u8; 32],
    1,
    [6u8; 32],
    Weight::from_parts(77, 88),
  )
}

#[test]
fn public_api_error_signatures_use_shared_typed_cores() {
  let _: fn(ActorId) -> Result<ActorEligibility<u32, u64>, ActorClassificationError> =
    Actors::actor_eligibility;
  let _: fn(ActorId) -> Result<crate::ActorCostQuote<Balance>, ActorCostQuoteError> =
    Actors::actor_cost_quote;
  let _: fn(
    ActorId,
    ActorType,
    Mutability,
    RuntimeActorContract,
    SimulationMode,
  ) -> Result<crate::SimulationResultOf<Test>, SimulationError> = Actors::simulate_current_contract;

  let classification_cases = [
    (
      ActorClassificationError::ActorInvariant,
      Error::<Test>::ActorInvariant,
    ),
    (
      ActorClassificationError::RunInvariant,
      Error::<Test>::ActorRunInvariant,
    ),
    (
      ActorClassificationError::ComputationOverflow,
      Error::<Test>::ComputationOverflow,
    ),
  ];
  for (core, dispatch) in classification_cases {
    assert_eq!(
      Actors::classification_dispatch_error(core).encode(),
      dispatch.encode()
    );
  }
}

#[test]
fn actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let quote = Actors::actor_cost_quote(actor_id).expect("active User cost quote exists");
    assert_eq!(quote.actor_type, ActorType::User);
    assert_eq!(quote.creation_fee, 10);
    let trigger = quote
      .prospective_trigger_fee
      .expect("active Trigger quote exists");
    assert_eq!(trigger.trigger_family, TriggerFamily::Manual);
    assert_eq!(
      trigger.maximum_weight,
      <TestWeightInfo as crate::WeightInfo>::manual_trigger()
    );
    assert_eq!(trigger.fee, manual_trigger_fee());
    assert_ne!(trigger.production_weight_identity, [0; 32]);
    let pipeline = quote
      .prospective_pipeline_fee
      .expect("active Pipeline quote exists");
    assert_eq!(
      pipeline.strategy,
      PipelineMachineFeeStrategy::UpfrontBounded
    );
    assert_eq!(
      pipeline.total_fee,
      pipeline.pipeline_machine_fee + pipeline.cleanup_fee
    );
    assert_eq!(
      pipeline.production_weight_identity,
      crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
    );
    let loaded =
      Actors::load_current_step_from_storage(actor_id, 0).expect("current Step resources exist");
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_weight,
      loaded.resources.effect
    );
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_fee,
      TestWeightToFee::weight_to_fee(&loaded.resources.effect)
    );
    let hold = crate::ActorStateHolds::<Test>::get(actor_id)
      .expect("User hold record exists")
      .breakdown;
    let expected_hold_total = [
      hold.identity,
      hold.contract_head,
      hold.contract_body,
      hold.detector,
      hold.funding,
      hold.run,
    ]
    .into_iter()
    .sum::<Balance>();
    assert_eq!(quote.actor_state_hold.total, expected_hold_total);
    assert!(!quote.actor_state_hold.exempt);
    assert_eq!(quote.actor_state_hold.base_per_component, 1);
    assert_eq!(quote.actor_state_hold.per_encoded_byte, 1);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(BOB),
      Mutability::Mutable,
      None,
    ));
    let dormant_id = Actors::next_actor_id() - 1;
    let dormant = Actors::actor_cost_quote(dormant_id).expect("Dormant quote exists");
    assert!(dormant.prospective_trigger_fee.is_none());
    assert!(dormant.prospective_pipeline_fee.is_none());
    assert_eq!(dormant.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(dormant.actor_state_hold.total > 0);

    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let system = Actors::actor_cost_quote(system_id).expect("System quote exists");
    assert_eq!(system.actor_type, ActorType::System);
    assert_eq!(system.creation_fee, 0);
    assert_eq!(system.prospective_trigger_fee.expect("Trigger").fee, 0);
    assert_eq!(
      system.prospective_pipeline_fee.expect("Pipeline").total_fee,
      0
    );
    assert_eq!(system.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(system.actor_state_hold.exempt);
    assert_eq!(system.actor_state_hold.total, 0);

    assert_eq!(
      Actors::actor_cost_quote(u64::MAX),
      Err(ActorCostQuoteError::ActorNotFound)
    );
  });
}

#[test]
fn contract_header_extracts_only_hot_metadata_and_runtime_bindings() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  let semantic_contract_id = [1u8; 32];
  let body_commitment = [2u8; 32];
  let admission_identity = [3u8; 32];

  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded Contract produces a header");

  assert_eq!(header.trigger, contract.trigger);
  assert_eq!(header.cooldown_blocks, contract.cooldown_blocks);
  assert_eq!(header.window, contract.window);
  assert_eq!(header.funding, contract.funding);
  assert_eq!(header.completion, contract.completion);
  assert_eq!(
    header.auto_close_at_cycle_nonce,
    contract.auto_close_at_cycle_nonce
  );
  assert_eq!(
    header.step_count,
    u32::try_from(contract.steps.len()).expect("bounded Step count fits u32")
  );
  assert_eq!(header.semantic_contract_id, semantic_contract_id);
  assert_eq!(header.body_commitment, body_commitment);
  assert_eq!(header.admission_identity, admission_identity);
  assert_eq!(
    header.pipeline_machine_envelope,
    test_pipeline_machine_envelope()
  );
}

#[test]
fn semantic_contract_id_uses_fixed_domain_and_authored_field_order() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  assert_eq!(crate::ACTOR_CONTRACT_HASH_DOMAIN, *b"DEOS_ACTOR_CONTRACT");
  let expected = (
    crate::ACTOR_CONTRACT_HASH_DOMAIN,
    (
      &contract.trigger,
      contract.cooldown_blocks,
      &contract.window,
      &contract.funding,
      contract.completion,
      contract.auto_close_at_cycle_nonce,
    ),
    &contract.steps,
  )
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.semantic_contract_id(), expected);

  let mut changed = contract.clone();
  changed.cooldown_blocks = changed.cooldown_blocks.saturating_add(1);
  assert_ne!(
    contract.semantic_contract_id(),
    changed.semantic_contract_id()
  );
}

#[test]
fn body_commitment_uses_fixed_domain_and_exact_ordered_indexes() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  assert_eq!(crate::ACTOR_BODY_HASH_DOMAIN, *b"DEOS_ACTOR_BODY");
  let indexed_steps = vec![(0u32, &contract.steps[0]), (1u32, &contract.steps[1])];
  let expected = (crate::ACTOR_BODY_HASH_DOMAIN, indexed_steps.as_slice())
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.body_commitment(), Some(expected));

  let mut reordered = contract.clone();
  reordered.steps =
    BoundedVec::try_from(vec![contract.steps[1].clone(), contract.steps[0].clone()])
      .expect("reordered Steps fit");
  assert_ne!(contract.body_commitment(), reordered.body_commitment());
}

#[test]
fn current_step_service_state_does_not_load_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    ActorContractTailChunks::<Test>::remove(actor_id, 0);

    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("Step 0 needs no tail chunk");
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert_eq!(state.contract.steps.len(), 8);
    assert_eq!(loaded_step.cursor, 0);
    assert_eq!(
      loaded_step.resources,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources"),
      "the loaded Step keeps exact fragment authority"
    );
    assert!(admission.has_valid_identity());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    System::reset_events();
    Actors::on_idle(1, Weight::MAX);
    assert!(ActorHot::<Test>::get(actor_id).is_some_and(|hot| hot.pending_signal));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn run_head_and_immutable_payload_remain_coherent_across_progress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = inert_contract_steps()[0].clone();
    let steps =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    let first_head = crate::ActorRunHeads::<Test>::get(actor_id).expect("run head exists");
    let first_payload = crate::ActorRunPayloads::<Test>::get(actor_id).expect("run payload exists");

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    Actors::execute_cycle(Weight::MAX);
    let second_head = crate::ActorRunHeads::<Test>::get(actor_id).expect("run head persists");
    let second_payload =
      crate::ActorRunPayloads::<Test>::get(actor_id).expect("run payload persists");
    assert_eq!(second_head.cursor, 2);
    assert_eq!(
      second_head.payload_commitment,
      first_head.payload_commitment
    );
    assert_eq!(second_payload.encode(), first_payload.encode());

    crate::ActorRunHeads::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("run head exists").payload_commitment[0] ^= 1;
    });
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn current_fragment_resources_are_authoritative_without_certificate_duplication() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      let head = maybe.as_mut().expect("canonical head exists");
      let resources = head
        .first_step_resources
        .as_mut()
        .expect("nonempty Contract has inline Step resources");
      resources.control = resources.control.saturating_add(Weight::from_parts(1, 0));
    });
    let (_, _, loaded) = Actors::load_current_step_service_state(actor_id)
      .expect("current fragment remains authoritative");
    assert_eq!(
      loaded.resources.control,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources")
        .control
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
  });
}

#[test]
fn running_execution_and_post_placement_ignore_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let inert = inert_contract_steps()[0].clone();
    let steps = BoundedVec::try_from((0..8).map(|_| inert.clone()).collect::<Vec<_>>())
      .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|run| run.cursor),
      Some(1)
    );

    ActorContractTailChunks::<Test>::remove(actor_id, 1);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    assert!(ActorHot::<Test>::get(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
    let (_, queued) = Actors::paged_head_entry().expect("successor is queued");
    assert_eq!(queued.actor_id, actor_id);
    assert_eq!(queued.eligible_at, 2);
    assert!(Actors::load_current_step_service_state(actor_id).is_some());
    Actors::execute_cycle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped { actor_id: id, step_index: 1, .. } if *id == actor_id
    )));
    let run = Actors::actor_run_state(actor_id).expect("Running suffix remains live");
    assert_eq!(run.cursor, 2);
    assert_eq!(run.last_committed_step_block, Some(2));
    assert!(ActorHot::<Test>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Running
        && (hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some())
    }));
  });
}

#[test]
fn current_step_plan_builds_only_from_coherent_opening_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let identity = ActorIdentities::<Test>::get(actor_id).expect("identity exists");
    let mut hot = ActorHot::<Test>::get(actor_id).expect("hot state exists");
    hot.queue_ticket = Some(9);
    ActorHot::<Test>::insert(actor_id, hot.clone());
    let funding = ActorFunding::<Test>::get(actor_id).expect("funding exists");
    let admission = ActorAdmissionCertificates::<Test>::get(actor_id)
      .expect("canonical admission certificate exists");
    let head = ActorContractHeads::<Test>::get(actor_id).expect("canonical head exists");
    let loaded_step = Actors::load_current_step_from_geometry(actor_id, &head, &admission, 0, None)
      .expect("Step 0 loads");
    let ticket = crate::ActorStepTicket {
      actor_id,
      cycle_nonce: identity.cycle_nonce + 1,
      cursor: 0,
      ticket: 9,
      eligible_at: 1,
      contract_commitment: crate::ActorContractCommitment {
        semantic_contract_id: admission.semantic_contract_id,
        body_commitment: admission.body_commitment,
      },
    };
    assert_eq!(
      Actors::build_actor_step_ticket(actor_id, 9, 1, &identity, &hot, None, &admission,),
      Some(ticket)
    );
    let mut stale_hot = hot.clone();
    stale_hot.queue_ticket = Some(10);
    assert!(
      Actors::build_actor_step_ticket(actor_id, 9, 1, &identity, &stale_hot, None, &admission,)
        .is_none()
    );
    let user_fee = Actors::maximum_current_step_fee(ActorType::User, loaded_step.resources)
      .expect("User current-Step fee is representable");
    assert_eq!(
      user_fee.total_fee,
      user_fee.control_fee + user_fee.effect_fee
    );
    let maximum_fee = Actors::maximum_current_step_fee(ActorType::System, loaded_step.resources)
      .expect("System current-Step fee is representable");
    assert_eq!(maximum_fee.total_fee, 0);
    let storage_plan = Actors::load_current_step_plan_from_storage(ticket)
      .expect("storage-backed Opening plan builds");
    assert_eq!(storage_plan.ticket, ticket);
    assert_eq!(storage_plan.loaded_step, loaded_step);
    assert_eq!(storage_plan.maximum_fee, maximum_fee);
    let mut future_ticket = ticket;
    future_ticket.eligible_at = 2;
    assert!(Actors::load_current_step_plan_from_storage(future_ticket).is_none());
    let plan = Actors::build_current_step_plan(
      actor_id,
      identity.clone(),
      hot.clone(),
      None,
      funding.clone(),
      admission.clone(),
      ticket,
      loaded_step.clone(),
      maximum_fee.clone(),
    )
    .expect("coherent Opening plan builds");
    assert_eq!(plan.loaded_step, loaded_step);
    assert_eq!(plan.maximum_fee, maximum_fee);

    let mut running_hot = plan.hot.clone();
    running_hot.cycle_state = CycleState::Running;
    let running = RuntimeActorRunState {
      contract_authority: run_contract_authority(actor_id),
      cycle_nonce: plan.ticket.cycle_nonce,
      cursor: plan.ticket.cursor,
      opening_predicate_cursor: 0,
      unsuccessful_attempts_at_cursor: 0,
      last_attempt_block: 0,
      last_committed_step_block: Some(0),
      eligible_at: plan.ticket.eligible_at,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
      last_step_outcome: None,
      suspension: None,
    };
    assert_eq!(
      Actors::build_actor_step_ticket(
        actor_id,
        9,
        running.eligible_at,
        &plan.identity,
        &running_hot,
        Some(&running),
        &plan.admission,
      ),
      Some(plan.ticket)
    );
    assert!(
      Actors::build_actor_step_ticket(
        actor_id,
        9,
        running.eligible_at + 1,
        &plan.identity,
        &running_hot,
        Some(&running),
        &plan.admission,
      )
      .is_none()
    );
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot.clone(),
        Some(running.clone()),
        plan.funding.clone(),
        plan.admission.clone(),
        plan.ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_some()
    );
    let mut stale_run = running.clone();
    stale_run.contract_authority.body_commitment[0] ^= 1;
    assert!(
      Actors::build_actor_step_ticket(
        actor_id,
        9,
        stale_run.eligible_at,
        &plan.identity,
        &running_hot,
        Some(&stale_run),
        &plan.admission,
      )
      .is_none()
    );
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot.clone(),
        Some(stale_run),
        plan.funding.clone(),
        plan.admission.clone(),
        plan.ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_none()
    );
    let mut suspended_hot = running_hot.clone();
    suspended_hot.cycle_state = CycleState::Suspended;
    let mut suspended = running.clone();
    suspended.last_step_outcome = Some(StepOutcome::FundingUnavailable);
    suspended.suspension = Some(SuspensionReason::FundingUnavailable);
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        suspended_hot,
        Some(suspended),
        plan.funding.clone(),
        plan.admission.clone(),
        plan.ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_some()
    );
    let mut incoherent_suspension = running;
    incoherent_suspension.suspension = Some(SuspensionReason::Temporary);
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot,
        Some(incoherent_suspension),
        plan.funding.clone(),
        plan.admission.clone(),
        plan.ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_none()
    );

    let mut stale_ticket = ticket;
    stale_ticket.cycle_nonce += 1;
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        identity,
        hot,
        None,
        funding,
        admission,
        stale_ticket,
        plan.loaded_step,
        plan.maximum_fee,
      )
      .is_none()
    );
  });
}

#[test]
fn step_ticket_binds_run_cursor_fifo_eligibility_and_contract_commitment() {
  let ticket = crate::ActorStepTicket {
    actor_id: 7,
    cycle_nonce: 8,
    cursor: 2,
    ticket: 9,
    eligible_at: 10u64,
    contract_commitment: crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  };
  let decoded =
    crate::ActorStepTicket::decode(&mut ticket.encode().as_slice()).expect("Step ticket decodes");
  assert_eq!(decoded, ticket);
  assert_eq!(decoded.ticket, 9);
  assert_eq!(decoded.cursor, 2);
  assert_eq!(decoded.eligible_at, 10);
  assert!(decoded.matches(
    7,
    8,
    2,
    9,
    &10,
    &crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  ));
  assert!(!decoded.matches(7, 8, 3, 9, &10, &decoded.contract_commitment,));
}

#[test]
fn admission_identity_binds_every_runtime_owned_domain_field() {
  let certificate: crate::ActorAdmissionCertificateOf<Test> = crate::ActorAdmissionCertificate::new(
    [1u8; 32],
    [2u8; 32],
    3,
    [4u8; 32],
    5,
    [6u8; 32],
    Weight::from_parts(77, 88),
  );
  assert!(certificate.has_valid_identity());
  let mut stale = certificate.clone();
  stale.body_geometry_version = 6;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.production_weight_identity[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate;
  stale.maximum_lifecycle_weight = Weight::from_parts(78, 88);
  assert!(!stale.has_valid_identity());
}

#[test]
fn admission_certificate_encoding_is_independent_of_resource_ceiling() {
  type SmallResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<1>>;
  type LargeResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<32>>;
  let small: crate::ActorAdmissionCertificate<SmallResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  let large: crate::ActorAdmissionCertificate<LargeResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  assert_eq!(small.encode(), large.encode());
  assert_eq!(small.admission_identity, large.admission_identity);
}

#[test]
fn admission_certificate_builder_composes_compact_host_authority() {
  let contract = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(vec![make_step(Task::StopCycle), make_step(Task::StopCycle)])
      .expect("two Steps fit"),
  )
  .expect("active Contract");
  let certificate = Actors::build_admission_certificate(&contract).expect("host authority exists");
  assert!(certificate.has_valid_identity());
  assert_eq!(
    certificate.semantic_contract_id,
    contract.semantic_contract_id()
  );
  assert_eq!(
    certificate.body_commitment,
    contract.body_commitment().expect("body commitment")
  );
  assert_eq!(certificate.runtime_actor_semantics_version, 1);
  assert_eq!(
    certificate.production_weight_identity,
    crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
  );
  assert_eq!(certificate.body_geometry_version, 1);
  assert_eq!(certificate.configured_bounds_commitment, [6; 32]);
  assert_eq!(
    certificate.maximum_lifecycle_weight,
    Weight::from_parts(77, 88)
  );
  assert_eq!(
    certificate.marker,
    std::marker::PhantomData::<crate::ActorAdmissionResourcesOf<Test>>
  );
}

#[test]
fn step_resource_derivation_binds_authored_opening_and_predicate_geometry() {
  let steps = BoundedVec::try_from(vec![StepOf::<Test> {
    precondition: timed_all_conditions(
      ObservationTiming::Opening,
      vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1,
      }],
    ),
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
    },
    on_error: StepErrorPolicy::AbortCycle,
  }])
  .expect("one Step fits");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let resources = Actors::derive_step_resource_envelopes(&contract)
    .expect("configured resource providers admit Contract");
  assert_eq!(resources.len(), 1);
  assert_eq!(
    resources[0].control,
    Weight::from_parts(100_000_025, 100_023),
    "control base + 2 evaluation units + 1 Opening surface + 1 Opening result + 10 funding entries",
  );
}

#[test]
fn opening_control_envelope_charges_only_authored_tail_chunks() {
  let one_step = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
  )
  .expect("one-Step Contract exists");
  let eight_steps = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit"),
  )
  .expect("eight-Step Contract exists");

  let one =
    Actors::derive_step_resource_envelopes(&one_step).expect("one-Step resources exist")[0].control;
  let eight = Actors::derive_step_resource_envelopes(&eight_steps)
    .expect("eight-Step resources exist")[0]
    .control;

  assert_eq!(eight.ref_time().saturating_sub(one.ref_time()), 2);
  assert_eq!(eight.proof_size(), one.proof_size());
}

#[test]
fn unconfigured_resource_weight_ports_fail_closed() {
  assert_eq!(
    <() as crate::AdmissionCertificateAuthorityProvider>::current(),
    None
  );
  let step = make_step(Task::StopCycle);
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<_>>::maximum_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
        opening_snapshot_entries: 0,
        opening_predicate_results: 0,
        funding_snapshot_entries: 0,
      },
      &step,
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::actual_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
        opening_snapshot_entries: 0,
        opening_predicate_results: 0,
        funding_snapshot_entries: 0,
      },
      &step,
      Weight::from_parts(1, 1),
      crate::StepControlExecution {
        phase: crate::StepControlPhase::Opening,
        outcome: crate::StepControlOutcome::Completed,
        placement: crate::StepControlPlacement::None,
      },
    ),
    None
  );
  let task =
    Task::<TestAsset, Balance, AccountId, <Test as crate::Config>::MaxSplitTransferLegs>::StopCycle;
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<RuntimeTask>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::maximum_effect_weight(&task),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::actual_effect_weight(
      &task,
      crate::TaskEffectExecution::NotInvoked,
    ),
    None
  );
}

#[test]
fn step_control_weight_context_matches_c6_head_and_tail_geometry() {
  assert_eq!(
    Actors::step_control_weight_context(1, 0, 7, 8, 9, 10),
    Some(crate::StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 0,
      predicate_evaluation_units: 7,
      opening_snapshot_entries: 8,
      opening_predicate_results: 9,
      funding_snapshot_entries: 10,
    })
  );
  assert_eq!(
    Actors::step_control_weight_context(8, 0, 7, 8, 9, 10)
      .expect("eight-Step head context exists")
      .opening_tail_chunks,
    2,
  );
  for (cursor, steps_in_fragment) in [(1, 4), (4, 4), (5, 3), (7, 3)] {
    assert_eq!(
      Actors::step_control_weight_context(8, cursor, 7, 8, 9, 10),
      Some(crate::StepControlWeightContext {
        cursor,
        steps_in_fragment,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 7,
        opening_snapshot_entries: 0,
        opening_predicate_results: 0,
        funding_snapshot_entries: 0,
      })
    );
  }
  assert_eq!(Actors::step_control_weight_context(0, 0, 0, 0, 0, 0), None);
  assert_eq!(Actors::step_control_weight_context(8, 8, 0, 0, 0, 0), None);
  assert_eq!(Actors::step_control_weight_context(9, 0, 0, 0, 0, 0), None);
}

#[test]
fn current_step_resource_admission_requires_both_weight_components() {
  let resources = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let control = WeightMeter::with_limit(resources.control);
  let effect = WeightMeter::with_limit(resources.effect);
  assert!(Actors::current_step_resources_fit(
    &control, &effect, resources
  ));
  let short_control = WeightMeter::with_limit(Weight::from_parts(10, 22));
  assert!(!Actors::current_step_resources_fit(
    &short_control,
    &effect,
    resources,
  ));
  let short_effect_proof = WeightMeter::with_limit(Weight::from_parts(33, 43));
  assert!(!Actors::current_step_resources_fit(
    &control,
    &short_effect_proof,
    resources,
  ));
}

#[test]
fn step_resource_envelope_keeps_control_and_effect_weight_separate() {
  let envelope = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  assert_eq!(envelope.control, Weight::from_parts(11, 22));
  assert_eq!(envelope.effect, Weight::from_parts(33, 44));
  assert_eq!(
    crate::ActorStepResourceEnvelope::decode(&mut envelope.encode().as_slice()),
    Ok(envelope)
  );
}

#[test]
fn user_pipeline_machine_envelope_prices_control_retries_and_cleanup_only() {
  new_test_ext().execute_with(|| {
    let mut steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two Steps fit");
    steps[1].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let first_fee = Actors::maximum_current_step_fee(ActorType::User, resources[0])
      .expect("first Action fee fits");
    let second_fee = Actors::maximum_current_step_fee(ActorType::User, resources[1])
      .expect("second Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("Pipeline Machine envelope fits");

    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&resources[0].control)
        + TestWeightToFee::weight_to_fee(&resources[1].control) * 3
    );
    assert_eq!(first_fee.control_fee, 0);
    assert_eq!(second_fee.control_fee, 0);
    assert!(first_fee.effect_fee > 0);
    assert!(second_fee.effect_fee > 0);
    assert_eq!(
      envelope.cleanup_fee_upper,
      TestWeightToFee::weight_to_fee(&<TestWeightInfo as crate::WeightInfo>::close_actor())
    );
    assert!(
      envelope
        .pipeline_machine_fee_upper
        .checked_add(envelope.cleanup_fee_upper)
        .is_some()
    );
  });
}

#[test]
fn zero_step_pipeline_machine_envelope_prices_generated_control_and_cleanup() {
  new_test_ext().execute_with(|| {
    let steps = crate::ContractSteps::<Test>::default();
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("zero-Step Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("empty resources derive");
    let user = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("zero-Step User Pipeline Machine envelope fits");
    assert_eq!(
      user.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(
        &<TestWeightInfo as crate::WeightInfo>::scheduler_inner_zero_step_complete(),
      )
    );
    assert_eq!(
      user.cleanup_fee_upper,
      TestWeightToFee::weight_to_fee(&<TestWeightInfo as crate::WeightInfo>::close_actor())
    );
    assert!(user.pipeline_machine_fee_upper > 0);
    let system = Actors::derive_pipeline_machine_envelope(ActorType::System, &steps, &resources)
      .expect("zero-Step System envelope fits");
    assert_eq!(system.pipeline_machine_fee_upper, 0);
    assert_eq!(system.cleanup_fee_upper, 0);
  });
}

#[test]
fn pipeline_machine_envelope_absorbs_stop_cycle_control_effect() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits");
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let action_fee = Actors::maximum_current_action_fee(ActorType::User, &steps[0], resources[0])
      .expect("StopCycle Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("Pipeline Machine envelope fits");

    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&resources[0].control.saturating_add(resources[0].effect))
    );
    assert_eq!(action_fee.total_fee, 0);
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_pipeline_machine_envelope_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("C6 head exists")
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper = 1;
    });

    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_actor_state_hold_geometry_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ActorStateHolds::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("User Actor hold exists")
        .breakdown
        .contract_head = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn step_chunk_binds_authority_and_contiguous_first_index() {
  let chunk = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
      admission_identity: [3u8; 32],
    },
    first_step_index: 1,
    steps: vec![11u8, 12u8, 13u8, 14u8],
    step_resources: vec![
      crate::ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: Weight::zero(),
      };
      4
    ],
  };
  assert!(chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&8, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 2));
  assert_eq!(chunk.steps.len(), 4);
}

#[test]
fn contract_geometry_decomposition_is_gap_free_and_head_only_for_one_step() {
  new_test_ext().execute_with(|| {
    for step_count in [1u32, 8] {
      let steps = BoundedVec::try_from(
        (0..step_count)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("bounded Steps fit");
      let contract =
        system_active_contract(manual_schedule(), None, steps).expect("active Contract");
      let certificate = crate::ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment().expect("body commitment"),
        1,
        [4u8; 32],
        1,
        [6u8; 32],
        Weight::from_parts(77, 88),
      );
      let (head, chunks) =
        Actors::decompose_admitted_contract_geometry(7, ActorType::System, &contract, &certificate)
          .expect("admitted Contract geometry decomposes");
      assert_eq!(
        head.header.admission_identity,
        certificate.admission_identity
      );
      assert_eq!(
        head.header.pipeline_machine_envelope,
        crate::PipelineMachineEnvelope {
          pipeline_machine_fee_upper: 0,
          cleanup_fee_upper: 0,
        }
      );
      let loaded_first = Actors::load_current_step_from_geometry(7, &head, &certificate, 0, None)
        .expect("Step 0 loads from the head only");
      assert_eq!(loaded_first.step, contract.steps[0]);
      let ticket = crate::ActorStepTicket {
        actor_id: 7,
        cycle_nonce: 1,
        cursor: 0,
        ticket: 9,
        eligible_at: 1u64,
        contract_commitment: crate::ActorContractCommitment {
          semantic_contract_id: certificate.semantic_contract_id,
          body_commitment: certificate.body_commitment,
        },
      };
      assert!(Actors::validate_loaded_step_authority(
        7,
        9,
        &certificate,
        &ticket,
        &loaded_first,
      ));
      let mut stale_ticket = ticket;
      stale_ticket.cursor = 1;
      assert!(!Actors::validate_loaded_step_authority(
        7,
        9,
        &certificate,
        &stale_ticket,
        &loaded_first,
      ));
      if step_count > 1 {
        let cursor = step_count - 1;
        let chunk_index = cursor.saturating_sub(1) / 4;
        let chunk = chunks
          .iter(/* deos-bypass: bounded-iter */)
          .find(|(index, _)| *index == chunk_index)
          .expect("current tail chunk exists");
        let loaded = Actors::load_current_step_from_geometry(
          7,
          &head,
          &certificate,
          cursor,
          Some((chunk.0, &chunk.1)),
        )
        .expect("tail Step loads from exactly one chunk");
        assert_eq!(loaded.step, contract.steps[cursor as usize]);
        assert!(
          Actors::load_current_step_from_geometry(
            8,
            &head,
            &certificate,
            cursor,
            Some((chunk.0, &chunk.1)),
          )
          .is_none()
        );
      }
      let mut stale_certificate = certificate.clone();
      stale_certificate.body_geometry_version += 1;
      assert!(
        Actors::decompose_admitted_contract_geometry(
          7,
          ActorType::User,
          &contract,
          &stale_certificate,
        )
        .is_none()
      );
      assert_eq!(head.first_step, Some(contract.steps[0].clone()));
      assert_eq!(
        Actors::reconstruct_contract_geometry(7, head.clone(), &chunks),
        Some(contract.clone())
      );
      if !chunks.is_empty() {
        assert_eq!(
          Actors::reconstruct_contract_geometry(8, head.clone(), &chunks),
          None
        );
        let mut stale_range = chunks.clone();
        stale_range[0].1.first_step_index = 2;
        assert_eq!(
          Actors::reconstruct_contract_geometry(7, head.clone(), &stale_range),
          None
        );
      }
      assert_eq!(
        chunks.len(),
        usize::try_from(step_count.saturating_sub(1).div_ceil(4)).unwrap()
      );
      let reconstructed = head
        .first_step
        .into_iter()
        .chain(
          chunks
            .iter()
            .flat_map(|(_, chunk)| chunk.steps.iter().cloned()),
        )
        .collect::<Vec<_>>();
      assert_eq!(reconstructed.as_slice(), contract.steps.as_slice());
      for (chunk_index, chunk) in chunks {
        assert_eq!(chunk.first_step_index, 1 + chunk_index * 4);
        assert!(!chunk.steps.is_empty());
        assert!(chunk.steps.len() <= 4);
        assert!(chunk.matches(
          &7,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          chunk.first_step_index,
        ));
      }
    }
  });
}

#[test]
fn admitted_contract_storage_loads_one_current_fragment_and_replaces_exact_tail() {
  new_test_ext().execute_with(|| {
    let actor_id = 77;
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(
        (0..8)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("eight Steps fit"),
    )
    .expect("active Contract");
    let certificate = geometry_certificate(&contract);
    assert!(Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert_eq!(
      Actors::load_admitted_contract_geometry(actor_id),
      Some((contract.clone(), certificate.clone()))
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 0)
        .expect("head Step loads")
        .step,
      contract.steps[0]
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 7)
        .expect("one tail chunk loads current Step")
        .step,
      contract.steps[7]
    );
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 1));

    let replacement = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("replacement Contract");
    let replacement_certificate = geometry_certificate(&replacement);
    assert!(Actors::replace_admitted_contract_geometry(
      actor_id,
      &replacement,
      &replacement_certificate,
    ));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 1));
    assert!(Actors::load_current_step_from_storage(actor_id, 1).is_none());
    assert_eq!(
      Actors::remove_admitted_contract_geometry(actor_id),
      Some(replacement)
    );
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!ActorAdmissionCertificates::<Test>::contains_key(actor_id));
    assert!(Actors::remove_admitted_contract_geometry(actor_id).is_none());
  });
}

#[test]
fn admitted_contract_storage_fails_closed_before_mutation_on_stale_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = 78;
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("active Contract");
    let mut stale_certificate = geometry_certificate(&contract);
    stale_certificate.body_geometry_version += 1;
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &stale_certificate,
    ));
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!ActorAdmissionCertificates::<Test>::contains_key(actor_id));
  });
}

#[test]
fn inline_first_head_and_lazy_tail_have_unique_ordered_ownership() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let semantic_contract_id = contract.semantic_contract_id();
  let body_commitment = contract.body_commitment().expect("bounded body commitment");
  let admission_identity = [3u8; 32];
  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded header");
  let resource = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let head = crate::ActorContractHead {
    header,
    first_step: Some(contract.steps[0].clone()),
    first_step_resources: Some(resource),
  };
  let tail = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    },
    first_step_index: 1,
    steps: BoundedVec::<_, ConstU32<4>>::try_from(vec![contract.steps[1].clone()])
      .expect("one tail Step fits"),
    step_resources: BoundedVec::<_, ConstU32<4>>::try_from(vec![resource])
      .expect("one tail resource fits"),
  };

  assert_eq!(head.header.step_count, 2);
  assert_eq!(head.first_step, Some(contract.steps[0].clone()));
  assert_eq!(tail.steps[0], contract.steps[1]);
  assert!(tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &8,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    0,
  ));
  let reconstructed = BoundedVec::<_, <Test as crate::Config>::MaxContractSteps>::try_from(vec![
    head.first_step.expect("nonempty Contract has inline Step"),
    tail.steps[0].clone(),
  ])
  .expect("reconstructed Steps fit");
  assert_eq!(reconstructed, contract.steps);
}

#[test]
#[ignore = "retained-state Contract geometry profile"]
fn profile_contract_geometry_state_footprint() {
  for step_count in [1u32, 8] {
    let steps = BoundedVec::try_from(
      (0..step_count)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("profile Steps fit");
    let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
    let semantic_contract_id = contract.semantic_contract_id();
    let body_commitment = contract.body_commitment().expect("bounded body commitment");
    let admission_identity = [3u8; 32];
    let header = contract
      .try_header(
        semantic_contract_id,
        body_commitment,
        admission_identity,
        test_pipeline_machine_envelope(),
      )
      .expect("bounded header");
    let authority = crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    };
    let resource = crate::ActorStepResourceEnvelope {
      control: Weight::zero(),
      effect: Weight::zero(),
    };
    let head = crate::ActorContractHead {
      header,
      first_step: Some(contract.steps[0].clone()),
      first_step_resources: Some(resource),
    };
    let chunks = contract.steps.as_slice()[1..]
      .chunks(4)
      .enumerate()
      .map(|(chunk_index, steps)| crate::ActorStepChunk {
        authority: authority.clone(),
        first_step_index: 1u32.saturating_add(
          u32::try_from(chunk_index)
            .expect("bounded chunk index fits u32")
            .saturating_mul(4),
        ),
        steps: steps.to_vec(),
        step_resources: vec![resource; steps.len()],
      })
      .collect::<Vec<_>>();
    let b0_bytes = contract.encode().len();
    let head_bytes = head.encode().len();
    let chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .sum::<usize>();
    let max_chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .max()
      .unwrap_or_default();
    println!(
      "CONTRACT_GEOMETRY_FOOTPRINT step_count={step_count} monolithic_bytes={b0_bytes} chunked_total_bytes={} head_bytes={head_bytes} max_chunk_bytes={max_chunk_bytes}",
      head_bytes.saturating_add(chunk_bytes),
    );
  }
}

#[test]
fn public_reachability_inventory_is_closed_and_canonical() {
  assert_variant_names::<RuntimeTask>(&[
    "Transfer",
    "SplitTransfer",
    "SwapIn",
    "SwapOut",
    "AddLiquidity",
    "RemoveLiquidity",
    "Burn",
    "Mint",
    "Stake",
    "DonateLiquidity",
    "Unstake",
    "StopCycle",
  ]);
  assert_variant_names::<AmountResolution<u128>>(&[
    "Fixed",
    "PercentageOfCurrent",
    "PercentageAtOpening",
    "PercentageOfLastFunding",
    "AllAvailable",
  ]);
  assert_variant_names::<InputLimit<u128>>(&["LiveQuote", "Absolute"]);
  assert_variant_names::<Predicate<TestAsset, u128, u32, u32>>(&[
    "BalanceAbove",
    "BalanceBelow",
    "BalanceEquals",
    "BalanceNotEquals",
    "BlockNumberAbove",
    "BlockNumberBelow",
    "ObservationAbove",
    "ObservationBelow",
    "ObservationEquals",
    "ObservationNotEquals",
  ]);
  assert_variant_names::<ObservationTiming>(&["Opening", "Current"]);
  assert_variant_names::<crate::PredicateError>(&["InvalidObservation"]);
  assert_variant_names::<RuntimeSourceFilter>(&["Any", "OwnerOnly", "Whitelist"]);
  assert_variant_names::<RuntimeAssetFilter>(&["Any", "Whitelist"]);
  assert_variant_names::<RuntimeTrigger>(&[
    "Manual",
    "AddressEvent",
    "ObservationChange",
    "ObservationCrossing",
    "AtTime",
    "Cadenced",
  ]);
  assert_variant_names::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "Manual",
      "AddressEvent",
      "ObservationChange",
      "ObservationCrossing",
      "AtTime",
      "Cadenced",
    ],
  );
  assert_variant_names::<ActorType>(&["User", "System"]);
  assert_variant_names::<ActorClass>(&["User", "System"]);
  assert_variant_names::<Mutability>(&["Mutable", "Immutable"]);
  assert_variant_names::<crate::CompletionPolicy>(&["Persistent", "CloseAfterProductiveCycle"]);
  assert_variant_names::<ActiveLifecycle>(&["Active", "Paused"]);
  assert_variant_names::<CycleState>(&["Idle", "Running", "Suspended"]);
  assert_variant_names::<AttemptDisposition>(&[
    "Completed",
    "Continued",
    "Failed",
    "Suspended",
    "Closed",
  ]);
  assert_variant_names::<StepOutcome>(&[
    "Executed",
    "Stopped",
    "Skipped",
    "FundingUnavailable",
    "Failed",
  ]);
  assert_variant_names::<OpeningSurface<TestAsset>>(&[
    "PreservableAsset",
    "TargetAsset",
    "StakingShares",
  ]);
  assert_variant_names::<CloseReason>(&[
    "OwnerInitiated",
    "CycleAdmissionInsufficient",
    "TriggerAdmissionInsufficient",
    "ConsecutiveFailures",
    "WindowExpired",
    "CycleNonceExhausted",
    "AutoCloseNonceReached",
    "RetryAttemptsExhausted",
    "ProductiveCycleCompleted",
    "SchedulerIndexExhausted",
  ]);
  assert_variant_names::<StepErrorPolicy>(&["AbortCycle", "ContinueNextStep", "RetryLater"]);
  assert_variant_names::<SuspensionReason>(&["FundingUnavailable", "Temporary"]);
  assert_variant_names::<CancellationReason>(&[
    "Explicit",
    "ContractReplaced",
    "Deactivated",
    "Closing",
  ]);
  assert_variant_names::<StepSkippedReason>(&[
    "PreconditionFalse",
    "ResolutionSkipped",
    "FundingUnavailable",
  ]);
  assert_variant_names::<FundingSourcePolicy<AccountId, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "OwnerOnly",
      "SignedAllowlist",
      "RuntimePolicy",
      "AnyVerifiedIngress",
    ],
  );
  assert_variant_names::<crate::FundingProvenance>(&["Signed", "InternalProtocol", "Xcm"]);
  assert_variant_names::<RetryClass>(&["Permanent", "Temporary"]);
  assert_variant_names::<crate::ScalarObservationState<u64>>(&[
    "Unavailable",
    "Uninitialized",
    "Fresh",
    "Stale",
  ]);
  assert_variant_names::<ActorEligibility<u32, u64>>(&["NotRegistered", "Dormant", "Active"]);
  assert_variant_names::<SimulationMode>(&["FreshCurrentPlan", "CurrentRun"]);
  assert_variant_names::<SimulationError>(&[
    "TransactionDepthExceeded",
    "Classification",
    "ActorNotFound",
    "TypeMismatch",
    "MutabilityMismatch",
    "InvalidContract",
    "ContractMismatch",
    "ModeCycleStateMismatch",
    "GlobalCircuitBreaker",
    "Paused",
    "NotReady",
    "FeeCollectionFailed",
  ]);
}

#[test]
fn actor_storage_schema_is_explicit() {
  let storage_info = Actors::storage_info();
  assert!(
    storage_info
      .iter()
      .all(|entry| entry.pallet_name == b"Actors")
  );
  let actual: alloc::vec::Vec<_> = storage_info
    .iter()
    .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
    .collect();
  assert_eq!(
    actual,
    [
      "NextActorId",
      "ActorHot",
      "ActorContractHead",
      "ActorActivationAuthority",
      "ActorAdmissionCertificate",
      "ActorContractTailChunk",
      "ActorFunding",
      "ActorRunHead",
      "ActorRunPayload",
      "ActorIdentities",
      "ActorIdentityCount",
      "ActorStateHolds",
      "ActiveActorCount",
      "SystemSovereigns",
      "SystemSovereignCount",
      "NextQueueTicket",
      "PrepassExecutionCutoff",
      "CurrentBlockResourceState",
      "FinalizedBlockResourceTelemetry",
      "QueueHead",
      "QueueTail",
      "QueueOccupancy",
      "QueuePages",
      "WakeupPages",
      "WakeupBuckets",
      "WakeupCursorPages",
      "WakeupCursorLen",
      "NextWakeupClock",
      "WakeupWorkerFaultState",
      "OwnerSlotBitmaps",
      "SovereignIndex",
      "ActiveActorLimit",
      "IndexedTriggerDetectionDisabled",
      "ActorObservationFeeds",
      "ObservationSubscriptionSlot",
      "ObservationSubscriptionSlotOwner",
      "NextObservationSubscriptionSlot",
      "ObservationFreeSlotLen",
      "ObservationFreeSlotPages",
      "ObservationSubscriberPages",
      "ObservationSubscriberPageLists",
      "ObservationSubscriberCount",
      "ObservationSubscriptionCount",
      "ObservationIngressRevisions",
      "DirtyObservationFeeds",
      "DirtyObservationListState",
      "ObservationFanoutWorkerFaultState",
      "CrossingMemberships",
      "CrossingMemberPages",
      "CrossingLeafStates",
      "CrossingRadixNodes",
      "CrossingFeedMembershipCount",
      "CrossingUserFeedMembershipCount",
      "CrossingTransitionQueues",
      "CrossingPendingFeeds",
      "CrossingPendingFeedListState",
      "CrossingRangeCursors",
      "CrossingWorkerFaultState",
      "MaterializationFamilyCursor",
      "GlobalCircuitBreaker",
      "IdleStarvationState",
    ]
  );

  let metadata = Actors::storage_metadata();
  assert_eq!(metadata.prefix, "Actors");
  let actual_shapes: alloc::vec::Vec<_> = metadata
    .entries
    .iter()
    .map(|entry| {
      let optional = matches!(entry.modifier, StorageEntryModifierIR::Optional);
      let is_blake_map = match &entry.ty {
        StorageEntryTypeIR::Plain(_) => false,
        StorageEntryTypeIR::Map { hashers, .. } => {
          assert!(
            hashers
              .iter()
              .all(|hasher| *hasher == StorageHasherIR::Blake2_128Concat)
          );
          true
        }
      };
      (entry.name, optional, is_blake_map)
    })
    .collect();
  assert_eq!(
    actual_shapes,
    [
      ("NextActorId", false, false),
      ("ActorHot", true, true),
      ("ActorContractHead", true, true),
      ("ActorActivationAuthority", true, true),
      ("ActorAdmissionCertificate", true, true),
      ("ActorContractTailChunk", true, true),
      ("ActorFunding", true, true),
      ("ActorRunHead", true, true),
      ("ActorRunPayload", true, true),
      ("ActorIdentities", true, true),
      ("ActorIdentityCount", false, false),
      ("ActorStateHolds", true, true),
      ("ActiveActorCount", false, false),
      ("SystemSovereigns", true, true),
      ("SystemSovereignCount", false, false),
      ("NextQueueTicket", false, false),
      ("PrepassExecutionCutoff", true, false),
      ("CurrentBlockResourceState", true, false),
      ("FinalizedBlockResourceTelemetry", true, false),
      ("QueueHead", false, false),
      ("QueueTail", false, false),
      ("QueueOccupancy", false, false),
      ("QueuePages", true, true),
      ("WakeupPages", true, true),
      ("WakeupBuckets", true, true),
      ("WakeupCursorPages", true, true),
      ("WakeupCursorLen", false, true),
      ("NextWakeupClock", false, false),
      ("WakeupWorkerFaultState", true, false),
      ("OwnerSlotBitmaps", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("IndexedTriggerDetectionDisabled", true, true),
      ("ActorObservationFeeds", true, true),
      ("ObservationSubscriptionSlot", true, true),
      ("ObservationSubscriptionSlotOwner", true, true),
      ("NextObservationSubscriptionSlot", false, false),
      ("ObservationFreeSlotLen", false, false),
      ("ObservationFreeSlotPages", true, true),
      ("ObservationSubscriberPages", true, true),
      ("ObservationSubscriberPageLists", true, true),
      ("ObservationSubscriberCount", false, true),
      ("ObservationSubscriptionCount", false, false),
      ("ObservationIngressRevisions", true, true),
      ("DirtyObservationFeeds", true, true),
      ("DirtyObservationListState", false, false),
      ("ObservationFanoutWorkerFaultState", true, false),
      ("CrossingMemberships", true, true),
      ("CrossingMemberPages", true, true),
      ("CrossingLeafStates", true, true),
      ("CrossingRadixNodes", true, true),
      ("CrossingFeedMembershipCount", false, true),
      ("CrossingUserFeedMembershipCount", false, true),
      ("CrossingTransitionQueues", true, true),
      ("CrossingPendingFeeds", true, true),
      ("CrossingPendingFeedListState", false, false),
      ("CrossingRangeCursors", true, true),
      ("CrossingWorkerFaultState", true, false),
      ("MaterializationFamilyCursor", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("IdleStarvationState", false, false),
    ]
  );

  let entry = |name: &str| {
    metadata
      .entries
      .iter()
      .find(|entry| entry.name == name)
      .expect("declared storage entry exists")
  };
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entry("ActorContractHead"));
  // The candidate projection's concrete generic identity is covered by the storage declaration,
  // lifecycle round-trip tests, and generated metadata; its name/shape remain guarded above.
  assert_map_storage_types::<u64, crate::ActorAdmissionCertificateOf<Test>>(entry(
    "ActorAdmissionCertificate",
  ));
  assert_map_storage_types::<(u64, u32), crate::ActorStepChunkOf<Test>>(entry(
    "ActorContractTailChunk",
  ));
  assert_map_storage_types::<u64, crate::ActorStateHoldRecordOf<Test>>(entry("ActorStateHolds"));
  assert_map_storage_types::<u64, ()>(entry("IndexedTriggerDetectionDisabled"));
  let entries = metadata
    .entries
    .iter()
    .filter(|entry| {
      !matches!(
        entry.name,
        "ActorActivationAuthority"
          | "ActorAdmissionCertificate"
          | "ActorContractTailChunk"
          | "ActorRunPayload"
          | "ActorStateHolds"
          | "IndexedTriggerDetectionDisabled"
          | "PrepassExecutionCutoff"
          | "CurrentBlockResourceState"
          | "FinalizedBlockResourceTelemetry"
      )
    })
    .collect::<Vec<_>>();
  assert_plain_storage_type::<u64>(entries[0]);
  assert_map_storage_types::<u64, crate::ActorHotStateOf<Test>>(entries[1]);
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entries[2]);
  assert_map_storage_types::<u64, crate::ActorFundingStateOf<Test>>(entries[3]);

  let mut registry = scale_info::Registry::new();
  let contract_type =
    registry.register_type(&scale_info::meta_type::<crate::ActorContractOf<Test>>());
  let (_, contract) = registry
    .types()
    .find(|(symbol, _)| symbol.id == contract_type.id)
    .expect("Actor Contract type is registered");
  let scale_info::TypeDef::Composite(contract_fields) = &contract.type_def else {
    panic!("Actor Contract metadata must be composite");
  };
  assert_eq!(
    contract_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor Contract field"))
      .collect::<Vec<_>>(),
    [
      "trigger",
      "cooldown_blocks",
      "window",
      "steps",
      "funding",
      "completion",
      "auto_close_at_cycle_nonce"
    ]
  );
  assert_map_storage_types::<u64, crate::ActorRunHeadOf<Test>>(&entries[4]);
  assert_map_storage_types::<u64, crate::ActorRunPayloadOf<Test>>(entry("ActorRunPayload"));
  let run_type = registry.register_type(&scale_info::meta_type::<RuntimeActorRunState>());
  let (_, run_state) = registry
    .types()
    .find(|(symbol, _)| symbol.id == run_type.id)
    .expect("Actor run type is registered");
  let scale_info::TypeDef::Composite(run_fields) = &run_state.type_def else {
    panic!("Actor run metadata must be composite");
  };
  assert_eq!(
    run_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor run field"))
      .collect::<Vec<_>>(),
    [
      "contract_authority",
      "cycle_nonce",
      "cursor",
      "opening_predicate_cursor",
      "unsuccessful_attempts_at_cursor",
      "last_attempt_block",
      "last_committed_step_block",
      "eligible_at",
      "opening_snapshot",
      "opening_predicate_results",
      "funding_snapshot",
      "cumulative_outcomes",
      "last_step_outcome",
      "suspension"
    ]
  );
  assert_map_storage_types::<u64, crate::ActorIdentityOf<Test>>(&entries[5]);
  assert_plain_storage_type::<u32>(&entries[6]);
  assert_plain_storage_type::<u32>(&entries[7]);
  assert_map_storage_types::<u64, SystemSovereignState>(&entries[8]);
  assert_plain_storage_type::<u32>(&entries[9]);
  assert_plain_storage_type::<u64>(&entries[10]);
  assert_plain_storage_type::<u64>(&entries[11]);
  assert_plain_storage_type::<u64>(&entries[12]);
  assert_plain_storage_type::<u32>(&entries[13]);
  assert_map_storage_types::<u64, crate::QueuePageOf<Test>>(&entries[14]);
  assert_map_storage_types::<(WakeupKey<MockBlockNumber>, u64), crate::WakeupPageOf<Test>>(
    &entries[15],
  );
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, WakeupBucketState>(&entries[16]);
  assert_map_storage_types::<(WakeupClock, u64), crate::WakeupCursorPageOf<Test>>(&entries[17]);
  assert_map_storage_types::<WakeupClock, u32>(&entries[18]);
  assert_plain_storage_type::<WakeupClock>(&entries[19]);
  assert_plain_storage_type::<crate::WakeupWorkerFault<MockBlockNumber>>(&entries[20]);
  assert_map_storage_types::<AccountId, [u8; 32]>(&entries[21]);
  assert_map_storage_types::<AccountId, u64>(&entries[22]);
  assert_plain_storage_type::<u32>(&entries[23]);
  assert_map_storage_types::<u64, crate::ActorObservationFeedsOf<Test>>(&entries[24]);
  assert_map_storage_types::<u64, u32>(&entries[25]);
  assert_map_storage_types::<u32, u64>(&entries[26]);
  assert_plain_storage_type::<u32>(&entries[27]);
  assert_plain_storage_type::<u32>(&entries[28]);
  assert_map_storage_types::<u32, crate::ObservationFreeSlotPageOf<Test>>(&entries[29]);
  assert_map_storage_types::<(u32, u32), crate::ObservationSubscriberPageOf<Test>>(&entries[30]);
  assert_map_storage_types::<u32, ObservationSubscriberPageList>(&entries[31]);
  assert_map_storage_types::<u32, u32>(&entries[32]);
  assert_plain_storage_type::<u32>(&entries[33]);
  assert_map_storage_types::<u32, u64>(&entries[34]);
  assert_map_storage_types::<
    u32,
    crate::types::DirtyObservationState<
      u32,
      polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>,
    >,
  >(&entries[35]);
  assert_plain_storage_type::<crate::types::DirtyObservationList<u32>>(&entries[36]);
  assert_plain_storage_type::<crate::ObservationFanoutWorkerFault<u32>>(&entries[37]);
  assert_map_storage_types::<u64, crate::CrossingMembershipLocatorOf<Test>>(&entries[38]);
  assert_map_storage_types::<
    (crate::CrossingLeafKeyOf<Test>, u32),
    crate::CrossingMemberPageOf<Test>,
  >(&entries[39]);
  assert_map_storage_types::<crate::CrossingLeafKeyOf<Test>, crate::CrossingLeafState>(
    &entries[40],
  );
  assert_map_storage_types::<crate::CrossingRadixNodeKeyOf<Test>, u16>(&entries[41]);
  assert_map_storage_types::<u32, u32>(&entries[42]);
  assert_map_storage_types::<u32, u32>(&entries[43]);
  assert_map_storage_types::<u32, crate::CrossingTransitionQueueOf<Test>>(&entries[44]);
  assert_map_storage_types::<u32, crate::CrossingPendingFeedState<u32>>(&entries[45]);
  assert_plain_storage_type::<crate::CrossingPendingFeedList<u32>>(&entries[46]);
  assert_map_storage_types::<u32, crate::CrossingRangeCursor>(&entries[47]);
  assert_plain_storage_type::<crate::CrossingWorkerFault<u32>>(&entries[48]);
  assert_plain_storage_type::<u8>(&entries[49]);
  assert_plain_storage_type::<bool>(&entries[50]);
  assert_plain_storage_type::<IdleStarvationPhase>(&entries[51]);
}

#[test]
fn fresh_genesis_baseline_carries_no_migration_ceremony() {
  new_test_ext().execute_with(|| {
    // Genesis writes the current storage version directly; no legacy reader,
    // dual write, queue-merge bridge, or migration cursor exists (COMPAT-STORAGE).
    use polkadot_sdk::frame_support::traits::GetStorageVersion;
    let on_chain =
      polkadot_sdk::frame_support::traits::StorageVersion::get::<crate::Pallet<Test>>();
    let in_code = crate::Pallet::<Test>::in_code_storage_version();
    assert_eq!(
      on_chain, in_code,
      "genesis baseline must equal the current storage version"
    );
    // No queue-merge or migration-cursor storage exists on the schema surface.
    let storage_info = Actors::storage_info();
    let names: alloc::vec::Vec<_> = storage_info
      .iter()
      .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("UTF-8"))
      .collect();
    assert!(
      names.iter().all(|name| {
        !name.starts_with("Legacy")
          && !name.starts_with("Migration")
          && !name.contains("Merge")
          && !name.contains("OnRuntimeUpgrade")
      }),
      "fresh baseline must not carry migration-ceremony storage: {names:?}"
    );
    // The embedding fixture independently starts from the same fresh schema.
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn owner_slot_bitmap_try_state_rejects_invalid_and_orphaned_bits() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] |= 0b1000_0000);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] &= 0b0111_1111);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::insert(CHARLIE, [1; 32]);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn live_head_consume_rolls_back_on_occupancy_or_span_corruption() {
  for (tail, occupancy) in [(1u64, 0u32), (2u64, 1u32)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      QueueTail::<Test>::put(tail);
      QueueOccupancy::<Test>::put(occupancy);
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::paged_consume_head_at(0),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(
        Actors::actor_hot(actor_id).expect("hot").queue_ticket,
        Some(0)
      );
    });
  }
}

#[test]
fn reverse_index_corruption_matrix_fails_closed_for_system_close() {
  for dormant in [false, true] {
    for corruption in 0u8..5 {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
        if dormant {
          assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
          frame_system::Pallet::<Test>::set_block_number(2);
        }
        let identity = Actors::actor_identities(actor_id).expect("system identity");
        let sovereign_id = match identity.actor_class {
          ActorClass::System { sovereign_id } => sovereign_id,
          _ => unreachable!(),
        };
        match corruption {
          0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
          1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
          2 => crate::SystemSovereigns::<Test>::remove(sovereign_id),
          3 => crate::SystemSovereigns::<Test>::insert(sovereign_id, SystemSovereignState::Vacant),
          4 => crate::SystemSovereigns::<Test>::insert(
            sovereign_id,
            SystemSovereignState::Occupied(other),
          ),
          _ => unreachable!(),
        }
        let events_before = System::events();
        let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

        assert!(Actors::close_actor(RuntimeOrigin::root(), actor_id).is_err());
        assert_eq!(System::events(), events_before);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(StateVersion::V1),
          root_before,
          "dormant={dormant}, corruption={corruption}"
        );
        #[cfg(feature = "try-runtime")]
        assert!(
          crate::Pallet::<Test>::do_try_state().is_err(),
          "dormant={dormant}, corruption={corruption}"
        );
      });
    }
  }
}

#[test]
fn reverse_index_corruption_matrix_fails_closed_for_user_close() {
  for corruption in 0u8..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let other = create_user_with(
        BOB,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let identity = Actors::actor_identities(actor_id).expect("user identity");
      match corruption {
        0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
        1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
        2 => OwnerSlotBitmaps::<Test>::remove(ALICE),
        _ => unreachable!(),
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id).is_err());
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before,
        "corruption={corruption}"
      );
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}

#[test]
fn system_locator_corruption_surfaces_one_invariant_error_on_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    // Corrupt the locator truth: the live actor's entry no longer points at it.
    crate::SystemSovereigns::<Test>::insert(actor_id, SystemSovereignState::Vacant);
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::SystemSovereignInvariant
    );
  });
}

#[test]
fn temporal_membership_try_state_rejects_terminal_at_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102)
    );
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Terminal membership is derived from the schedule window: any `terminal_at` that is not
      // the exact window terminal (or absent without a window) must fail try_state.
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(999);
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorHot terminal_at disagrees with schedule window terminal membership\")"
            .into()
        )
      );
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = None;
      });
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(102);
      });
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_rejects_unconsumed_at_time_without_pointer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, at_time_schedule(10), None, inert_contract_steps());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("AtTime pointer is coherent")
      .expect("AtTime pointer exists");
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn temporal_membership_try_state_rejects_page_slot_pointing_at_different_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    assert!(Actors::wakeup_substrate_schedule(other, 10));
    // A physical slot whose entry addresses an actor that owns a different pointer in the
    // same clock domain is corruption.
    WakeupPages::<Test>::mutate((WakeupKey::Block(10), 0), |maybe| {
      let page = maybe.as_mut().expect("wakeup page");
      page.entries[0] = Some(crate::WakeupEntry { actor_id: other });
    });
    #[cfg(feature = "try-runtime")]
    assert_eq!(
      crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
      Err(
        "Other(\"WakeupPage slot addresses an actor with a different clock-domain pointer\")"
          .into()
      )
    );
  });
}

#[test]
fn missing_frozen_snapshot_is_a_permanent_invariant_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(8);
    let asset_out = TestAsset::Local(9);
    setup_pool(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      percentage_trigger_schedule(),
      None,
      contract_steps_with_step(step),
    );
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 100);
    set_temporary_dex_failure(true);
    signal_percentage_trigger(actor_id, asset_in);
    run_idle(Weight::MAX);
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("suspended continuation")
        .opening_snapshot
        .clear();
    });

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    let actor_state =
      Actors::active_actor_view(actor_id).expect("actor remains after permanent failure");
    assert_eq!(actor_state.cycle_state, CycleState::Idle);
    assert_eq!(actor_state.unsuccessful_attempt_streak, 2);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        step_index: 0,
        error,
        ..
      } if *id == actor_id && *error == Error::<Test>::SnapshotUnavailable.into()
    )));
  });
}

#[test]
fn canonical_loader_distinguishes_absence_dormancy_active_and_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let missing_id = Actors::next_actor_id();
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::NotRegistered
    ));

    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::Dormant(_)
    ));

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("active actor").pending_signal = true;
    });
    assert!(Actors::pending_signal(actor_id));
    ActorFunding::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_noop!(
      Actors::write_run_state(actor_id, None),
      Error::<Test>::ActorInvariant
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );

    ActorIdentities::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn canonical_loader_classifies_every_four_partition_presence_mask() {
  for mask in 0u8..16 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let identity = ActorIdentities::<Test>::take(actor_id).expect("identity fixture");
      let hot = ActorHot::<Test>::take(actor_id).expect("hot fixture");
      let contract = Actors::load_actor_contract(actor_id).expect("contract fixture");
      assert!(Actors::remove_admitted_contract_geometry(actor_id).is_some());
      let funding = ActorFunding::<Test>::take(actor_id).expect("funding fixture");
      ActorRunStateStore::<Test>::remove(actor_id);
      if mask & 0b0001 != 0 {
        ActorIdentities::<Test>::insert(actor_id, identity);
      }
      if mask & 0b0010 != 0 {
        ActorHot::<Test>::insert(actor_id, hot);
      }
      if mask & 0b0100 != 0 {
        assert_ok!(Actors::store_actor_contract(actor_id, contract));
      }
      if mask & 0b1000 != 0 {
        ActorFunding::<Test>::insert(actor_id, funding);
      }

      match mask {
        0 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        )),
        1 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::Dormant(_)
        )),
        15 => assert!(matches!(
          Actors::load_actor_state(actor_id),
          LoadedActorStateOf::Active(_)
        )),
        _ => {
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          assert_eq!(
            Actors::actor_eligibility(actor_id),
            Err(ActorClassificationError::ActorInvariant)
          );
          assert_noop!(
            Actors::pause_actor(RuntimeOrigin::root(), actor_id),
            Error::<Test>::ActorInvariant
          );
          assert_noop!(
            Actors::preflight_funding_event(actor_id, TestAsset::Native, 1, None, None),
            Error::<Test>::ActorInvariant
          );
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
        }
      }
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_every_corrupt_four_partition_presence_mask() {
  for mask in 2u8..15 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let identity = ActorIdentities::<Test>::take(actor_id).expect("identity fixture");
      let hot = ActorHot::<Test>::take(actor_id).expect("hot fixture");
      let contract = Actors::load_actor_contract(actor_id).expect("contract fixture");
      assert!(Actors::remove_admitted_contract_geometry(actor_id).is_some());
      let funding = ActorFunding::<Test>::take(actor_id).expect("funding fixture");
      if mask & 0b0001 != 0 {
        ActorIdentities::<Test>::insert(actor_id, identity);
      }
      if mask & 0b0010 != 0 {
        ActorHot::<Test>::insert(actor_id, hot);
      }
      if mask & 0b0100 != 0 {
        assert_ok!(Actors::store_actor_contract(actor_id, contract));
      }
      if mask & 0b1000 != 0 {
        ActorFunding::<Test>::insert(actor_id, funding);
      }
      crate::ActorIdentityCount::<Test>::put(u32::from(mask & 0b0001 != 0));
      crate::ActiveActorCount::<Test>::put(u32::from(mask & 0b0010 != 0));
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "mask {mask:04b}"
      );
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_system_identity_with_noncanonical_derived_account() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let original = ActorIdentities::<Test>::get(actor_id).expect("System identity fixture");
    let replacement_account = 999_998;
    crate::SovereignIndex::<Test>::remove(original.sovereign_account);
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("System identity fixture")
        .sovereign_account = replacement_account;
    });
    crate::SovereignIndex::<Test>::insert(replacement_account, actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn eligibility_projection_rejects_partial_active_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ActorFunding::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
  });
}
