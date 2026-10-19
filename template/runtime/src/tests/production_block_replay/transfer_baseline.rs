//! Practical economic adapters over the existing W1 block/replay loop; no production code.

use super::*;
use pallet_deos_actors::{
  StepControlExecution, StepControlOutcome, StepControlPhase, StepControlPlacement,
  StepControlWeightContext, StepControlWeightProvider, TaskEffectExecution,
  TaskEffectWeightProvider,
};
use polkadot_sdk::frame_support::traits::{Currency, Get, fungible::InspectHold};
use std::collections::VecDeque;

#[path = "resource_frontier.rs"]
mod resource_frontier;
#[path = "router_workload.rs"]
mod router_workload;
#[path = "running_source_control.rs"]
mod running_source_control;
use router_workload::{FOREIGN, WITNESSES, Workload};

type Quote = pallet_deos_actors::ActorCostQuote<crate::Balance>;

#[derive(Clone, Copy)]
struct Baseline {
  class: ActorType,
  workload: Workload,
  recurring: bool,
  actors: u32,
  warmup: u32,
  measured: u32,
}

impl Baseline {
  fn full(class: ActorType, recurring: bool) -> Self {
    Self {
      class,
      workload: Workload::Transfer,
      recurring,
      actors: if recurring { 1_024 } else { 128 },
      warmup: if recurring { 64 } else { 0 },
      measured: if recurring { 100 } else { 1 },
    }
  }

  fn blocks(self) -> u32 {
    assert!(self.actors > 0 && self.measured > 0);
    assert!(self.recurring || (self.warmup == 0 && self.measured == 1));
    self.warmup.checked_add(self.measured).unwrap()
  }

  fn schedule(self) -> WorkloadSchedule {
    if self.recurring {
      WorkloadSchedule::CadencedOnly
    } else {
      WorkloadSchedule::ManualOnly
    }
  }
}

struct Member {
  owner: crate::AccountId,
  account: crate::AccountId,
  generation: u64,
  quote: Quote,
}

struct BaselineFixture {
  actors: PreparedActorFixture,
  members: BTreeMap<ActorId, Member>,
  setup: serde_json::Value,
}

fn owner_for(index: u32) -> crate::AccountId {
  let mut seed = [89u8; 32];
  seed[..4].copy_from_slice(&(index / 255).to_le_bytes());
  crate::AccountId::from(sr25519::Pair::from_seed(&seed).public())
}

fn generation(id: ActorId) -> u64 {
  let Some(pallet_deos_actors::ActorSemanticState::Active(record)) =
    pallet_deos_actors::ActorSemanticStates::<Runtime>::get(id)
  else {
    panic!("baseline Actor must remain Active");
  };
  record.generation
}

fn total(account: &crate::AccountId) -> u128 {
  <Balances as Currency<crate::AccountId>>::total_balance(account)
}

fn add_balance(account: &crate::AccountId, amount: u128) {
  Balances::total_issuance()
    .checked_add(amount)
    .expect("finite baseline issuance");
  drop(<Balances as Currency<crate::AccountId>>::deposit_creating(
    account, amount,
  ));
}

fn prepare_baseline(wasm: &[u8], profile: Baseline) -> BaselineFixture {
  prepare_baseline_with_steps(wasm, profile, profile.workload.steps())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FundingHorizon {
  BlockBuffer,
  CompleteManualCycle,
}

fn prepare_baseline_with_steps(
  wasm: &[u8],
  profile: Baseline,
  steps: ContractSteps<Runtime>,
) -> BaselineFixture {
  prepare_baseline_with_funding(wasm, profile, steps, FundingHorizon::BlockBuffer)
}

fn prepare_baseline_with_funding(
  wasm: &[u8],
  profile: Baseline,
  steps: ContractSteps<Runtime>,
  funding: FundingHorizon,
) -> BaselineFixture {
  let blocks = profile.blocks();
  if funding == FundingHorizon::CompleteManualCycle {
    assert!(!profile.recurring, "one finite Manual Cycle only");
    assert!(
      steps
        .iter()
        .all(|step| step == &profile.workload.steps()[0]),
      "one Action quote applies only to identical authored Steps"
    );
  }
  let step_count = u32::try_from(steps.len()).unwrap();
  assert!(
    profile.actors
      <= <Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get()
        .checked_sub(REFERENCE_SYSTEM_ACTOR_IDENTITIES)
        .unwrap()
  );
  assert_eq!(
    <Runtime as pallet_deos_actors::Config>::MaxOwnerSlots::get(),
    255
  );
  let effect =
    crate::configs::actor_config::TmctolTaskEffectWeight::maximum_effect_weight(&steps[0].task)
      .expect("real Transfer effect");
  let control = Actors::contract_steps_admission_weight_upper(profile.class, &steps)
    .checked_sub(&effect)
    .unwrap();
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    reference_genesis_storage(wasm),
    crate::VERSION.state_version(),
  );
  let (ids, members, setup) = ext.execute_with(|| {
    assert_eq!(Actors::actor_identity_count(), REFERENCE_SYSTEM_ACTOR_IDENTITIES);
    assert_eq!(Actors::active_actor_count(), REFERENCE_ACTIVE_SYSTEM_ACTORS);
    System::set_block_number(1);
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(crate::SLOT_DURATION);
    assert!(!Actors::user_creation_enabled());
    if profile.workload == Workload::RouterFourPredicates { router_workload::prepare_market(); }
    if profile.class == ActorType::User {
      assert_ok!(Actors::set_user_creation_enabled(RuntimeOrigin::root(), true));
    }
    let owners = (0..profile.actors).map(owner_for).collect::<BTreeSet<_>>();
    for owner in &owners {
      add_balance(owner, crate::EXISTENTIAL_DEPOSIT.checked_mul(1_000_000).unwrap());
    }
    let mut ids = Vec::new();
    let mut members = BTreeMap::new();
    let mut full_cycle_funding = BTreeMap::new();
    for index in 0..profile.actors {
      let owner = owner_for(index);
      let id = Actors::next_actor_id();
      let contract = pallet_deos_actors::ActorContract {
        trigger: if profile.recurring { Trigger::cadenced(1) } else { Trigger::manual() },
        cooldown_blocks: 0, window: None, steps: steps.clone(),
        on_unfunded: pallet_deos_actors::OnUnfunded::Close, close_when: None,
        parked_balance_activation: None,
        funding: if profile.class == ActorType::User { pallet_deos_actors::FundingSourcePolicy::OwnerOnly }
          else { pallet_deos_actors::FundingSourcePolicy::RuntimePolicy },
        auto_close_at_cycle_nonce: None,
      };
      if profile.class == ActorType::User {
        let sovereign = Actors::sovereign_account_id(&owner, (index % 255) as u8);
        add_balance(&sovereign, Actors::user_pipeline_machine_capacity_requirement(&steps).unwrap()
          .checked_add(crate::EXISTENTIAL_DEPOSIT).unwrap());
        assert_ok!(Actors::create_user_actor(RuntimeOrigin::signed(owner.clone()), Mutability::Mutable, Some(contract)));
      } else {
        assert_ok!(Actors::create_system_actor(RuntimeOrigin::root(), owner.clone(), Mutability::Mutable, Some(contract)));
      }
      let identity = Actors::actor_identity(id).unwrap();
      assert_eq!(identity.last_control_mutation_block, 1, "no fixture control-clock aging");
      let quote = Actors::actor_cost_quote(id).unwrap();
      let cycle = crate::EXISTENTIAL_DEPOSIT
        .checked_add(quote.prospective_trigger_fee.unwrap().fee).unwrap()
        .checked_add(quote.prospective_pipeline_fee.unwrap().total_fee).unwrap()
        .checked_add(quote.maximum_next_action_fee.maximum_effect_fee).unwrap();
      let required = match funding {
        FundingHorizon::BlockBuffer => cycle.checked_mul(u128::from(blocks.checked_add(1).unwrap())).unwrap()
          .checked_add(<Runtime as pallet_deos_actors::Config>::MinUserBalance::get()).unwrap(),
        FundingHorizon::CompleteManualCycle => {
          let principal = if profile.workload == Workload::Transfer {
            crate::EXISTENTIAL_DEPOSIT.checked_mul(u128::from(step_count)).unwrap()
          } else { 0 };
          let actions = quote.maximum_next_action_fee.maximum_effect_fee
            .checked_mul(u128::from(step_count)).unwrap();
          let floor = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
            .checked_add(crate::EXISTENTIAL_DEPOSIT).unwrap()
            .checked_add(quote.actor_state_hold.total).unwrap();
          let required = floor.checked_add(principal).unwrap()
            .checked_add(quote.prospective_trigger_fee.unwrap().fee).unwrap()
            .checked_add(quote.prospective_pipeline_fee.unwrap().total_fee).unwrap()
            .checked_add(actions).unwrap();
          full_cycle_funding.insert(id, serde_json::json!({
            "steps": step_count, "nativePrincipal": principal.to_string(),
            "actionUpperTotal": actions.to_string(), "retainedNativeFloorAndHold": floor.to_string(),
            "nativeTotalBeforeTriggerMinimum": required.to_string(),
            "foreignInputCapTotal": if profile.workload == Workload::RouterFourPredicates {
              router_workload::input_cap().checked_mul(u128::from(step_count)).unwrap()
            } else { 0 }.to_string(),
          }));
          required
        }
      };
      let account = identity.sovereign_account;
      if let Some(extra) = required.checked_sub(total(&account)) { add_balance(&account, extra); }
      if profile.workload == Workload::RouterFourPredicates {
        match funding {
          FundingHorizon::BlockBuffer => router_workload::fund(&account, blocks),
          FundingHorizon::CompleteManualCycle => router_workload::fund_actions(&account, step_count),
        }
      }
      let generation = generation(id);
      members.insert(id, Member { owner, account, generation, quote });
      ids.push(id);
    }
    System::set_block_number(2);
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(2 * crate::SLOT_DURATION);
    if !profile.recurring {
      for id in &ids {
        let origin = if profile.class == ActorType::User {
          RuntimeOrigin::signed(members[id].owner.clone())
        } else { RuntimeOrigin::root() };
        assert_ok!(Actors::manual_trigger(origin, *id));
        assert_eq!(Actors::service_nodes(*id).unwrap().eligible_from, 3);
      }
    }
    assert_eq!(Actors::actor_identity_count(), REFERENCE_SYSTEM_ACTOR_IDENTITIES + profile.actors);
    assert_eq!(Actors::active_actor_count(), REFERENCE_ACTIVE_SYSTEM_ACTORS + profile.actors);
    let setup = serde_json::json!({
      "syntheticPublicDispatchSetup": true, "birthBlock": 1, "manualSignalBlock": (!profile.recurring).then_some(2),
      "firstAuthoredBlock": 3, "ownerCount": owners.len(), "maxOwnerSlots": 255,
      "userCreationEnabled": Actors::user_creation_enabled(),
      "recipient": if profile.workload == Workload::Transfer { "existing-external-BOB-shared" } else { "each-Actor-sovereign" },
      "workload": format!("{:?}", profile.workload), "predicatesPerStep": profile.workload.predicates(),
      "contractSteps": step_count,
      "fundingMode": format!("{funding:?}"),
      "fundedHorizonBlocks": (funding == FundingHorizon::BlockBuffer).then_some(blocks),
      "fundedManualCycles": (funding == FundingHorizon::CompleteManualCycle).then_some(1),
      "fullCycleFunding": full_cycle_funding,
      "totalIssuance": Balances::total_issuance().to_string(),
      "creationFees": members.values().map(|m| m.quote.creation_fee).sum::<u128>().to_string(),
      "sources": members.iter().map(|(id,m)| serde_json::json!({"actor": id,
        "balance": total(&m.account).to_string(), "trigger": m.quote.prospective_trigger_fee.unwrap().fee.to_string(),
        "pipeline": m.quote.prospective_pipeline_fee.unwrap().total_fee.to_string(),
        "action": m.quote.maximum_next_action_fee.maximum_effect_fee.to_string(),
        "hold": m.quote.actor_state_hold.total.to_string()})).collect::<Vec<_>>(),
    });
    (ids, members, setup)
  });
  ext.commit_all().unwrap();
  let storage = ext.execute_with(current_top_storage);
  BaselineFixture {
    actors: PreparedActorFixture {
      storage,
      actor_profiles: ids
        .iter()
        .map(|id| {
          (
            *id,
            WorkloadActorProfile {
              step_count,
              opening_predicates_per_step: profile.workload.predicates(),
              task: Some(if profile.workload == Workload::Transfer {
                WorkloadTask::Transfer
              } else {
                WorkloadTask::RouterSwapOut
              }),
            },
          )
        })
        .collect(),
      actor_ids: ids,
      signer: sr25519::Pair::from_seed(&[66u8; 32]),
      next_control_maximum: control,
      next_effect_maximum: effect,
    },
    members,
    setup,
  }
}

/// Selected successful-path prices, not admission authority or measured physical work.
fn successful_transfer_control(class: ActorType) -> (Weight, serde_json::Value) {
  successful_action_control(class, Workload::Transfer)
}

fn successful_action_control(class: ActorType, workload: Workload) -> (Weight, serde_json::Value) {
  type W = <Runtime as pallet_deos_actors::Config>::WeightInfo;
  type P = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
  let steps = workload.steps();
  let context = StepControlWeightContext {
    cursor: 0,
    steps_in_fragment: 1,
    opening_tail_chunks: 0,
    predicate_evaluation_units: workload.predicates(),
  };
  let maximum = P::maximum_control_weight(context, &steps[0]).unwrap();
  let execution = StepControlExecution {
    phase: StepControlPhase::Opening,
    outcome: StepControlOutcome::Completed,
    placement: StepControlPlacement::None,
    task_effect: TaskEffectExecution::Invoked,
    action_fee_collected: false,
  };
  let shared_inner = P::actual_control_weight(context, &steps[0], maximum, execution).unwrap();
  let actual_inner = P::actual_control_weight(
    context,
    &steps[0],
    maximum,
    StepControlExecution {
      action_fee_collected: class == ActorType::User,
      ..execution
    },
  )
  .unwrap();
  let collection = if class == ActorType::User {
    W::fee_collection()
  } else {
    Weight::zero()
  };
  assert_eq!(actual_inner, shared_inner.checked_add(&collection).unwrap());
  let receipt = W::action_invocation_receipt();
  let inspection = W::service_round_begin_populated()
    .checked_add(&W::service_round_probe_eligible())
    .unwrap()
    .checked_add(&W::scheduler_actor_state_probe())
    .unwrap();
  let suffix = W::service_round_admit_eligible()
    .max(W::service_member_retire_interior())
    .max(W::service_member_retire_pair_cursor())
    .max(W::service_member_retire_singleton());
  let selected = inspection
    .checked_add(&actual_inner)
    .unwrap()
    .checked_add(&suffix)
    .unwrap();
  let pair = |w: Weight| [w.ref_time(), w.proof_size()];
  (
    selected,
    serde_json::json!({
      "scope": "selected successful one-Step Opening/completion; not admission or physical cost",
      "workload": format!("{:?}", workload), "predicateUnits": workload.predicates(),
      "inspection": pair(inspection), "sharedInnerWithoutReceipt": pair(shared_inner.checked_sub(&receipt).unwrap()),
      "invocationReceipt": pair(receipt), "outerSuffixMaximum": pair(suffix),
      "actionCollection": pair(collection), "successfulAction": pair(selected),
      "successfulTransfer": if workload == Workload::Transfer { Some(pair(selected)) } else { None },
      "configuredInnerMaximum": pair(maximum)
    }),
  )
}

fn balances(
  wasm: &[u8],
  storage: &Storage,
  members: &BTreeMap<ActorId, Member>,
) -> (BTreeMap<ActorId, u128>, u128, u128) {
  TestExternalities::new_with_code_and_state(wasm, storage.clone(), crate::VERSION.state_version())
    .execute_with(|| {
      (
        members
          .iter()
          .map(|(id, m)| (*id, total(&m.account)))
          .collect(),
        total(&super::super::common::BOB),
        total(&<Runtime as pallet_deos_actors::Config>::FeeSink::get()),
      )
    })
}

#[derive(Clone, Copy)]
struct StepPosition {
  workload: Workload,
  index: u32,
  completes_cycle: bool,
}

/// Exact per-block money and relative FIFO oracle; all reads are outside execution/proof recording.
fn inspect_block(
  wasm: &[u8],
  authored: &AuthoredBlock,
  members: &BTreeMap<ActorId, Member>,
  fifo: &mut VecDeque<ActorId>,
  next: (Weight, Weight),
  terminal: bool,
  step: StepPosition,
) -> serde_json::Value {
  let workload = step.workload;
  let foreign_before = TestExternalities::new_with_code_and_state(
    wasm,
    authored.pre_state.clone(),
    crate::VERSION.state_version(),
  )
  .execute_with(|| {
    members
      .iter()
      .map(|(id, m)| (*id, Assets::balance(FOREIGN, &m.account)))
      .collect::<BTreeMap<_, _>>()
  });
  let (before, recipient_before, sink_before) = balances(wasm, &authored.pre_state, members);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    authored.post_state.clone(),
    crate::VERSION.state_version(),
  );
  ext.execute_with(|| {
    let block = System::block_number();
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let mut fees = BTreeMap::<ActorId, [u128; 3]>::new();
    let mut counts = BTreeMap::<ActorId, [u32; 3]>::new();
    let mut transfers = BTreeMap::<ActorId, u128>::new();
    let mut swaps = BTreeMap::<ActorId, (u128, u128)>::new();
    let mut routes = BTreeMap::<ActorId, serde_json::Value>::new();
    let mut reference = BTreeMap::<String, u32>::new();
    let (mut sink_in, mut sink_out, mut recipient_in, mut recipient_out) = (0u128, 0u128, 0u128, 0u128);
    let mut runtime_steps = 0u32;
    for record in System::events() {
      if let RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted { who, from, to, outcome }) = &record.event {
        if let Some((id, _)) = members.iter().find(|(_,m)| &m.account == who) {
          assert_eq!((*from, *to), (primitives::AssetKind::Foreign(FOREIGN), primitives::AssetKind::Native));
          assert!(routes.insert(*id, serde_json::json!({"family": format!("{:?}", outcome.family),
            "legs": format!("{:?}", outcome.legs), "weightClass": format!("{:?}", outcome.weight_class),
            "amountIn": outcome.total_amount_in.to_string(), "amountOut": outcome.recipient_amount_out.to_string(),
            "routerFee": outcome.router_fee.to_string()})).is_none());
        }
      }
      if let RuntimeEvent::Balances(polkd_event) = &record.event {
        if let polkadot_sdk::pallet_balances::Event::Transfer { from, to, amount } = polkd_event {
          if to == &sink { sink_in = sink_in.checked_add(*amount).unwrap(); }
          if from == &sink { sink_out = sink_out.checked_add(*amount).unwrap(); }
          if to == &super::super::common::BOB { recipient_in = recipient_in.checked_add(*amount).unwrap(); }
          if from == &super::super::common::BOB { recipient_out = recipient_out.checked_add(*amount).unwrap(); }
        }
      }
      let RuntimeEvent::Actors(event) = record.event else { continue; };
      let charge = match &event {
        Event::TriggerOccurrenceProcessed { actor_id, fee, .. } => Some((*actor_id, 0, *fee)),
        Event::PipelineFeeCharged { actor_id, fee } => Some((*actor_id, 1, *fee)),
        Event::ActionFeeCharged { actor_id, fee, .. } => Some((*actor_id, 2, *fee)),
        _ => None,
      };
      if let Some((id, kind, fee)) = charge.filter(|(id,_,_)| members.contains_key(id)) {
        assert_eq!(counts.entry(id).or_default()[kind], 0, "one collection per owning boundary");
        counts.entry(id).or_default()[kind] += 1;
        let q = members[&id].quote;
        let expected = [q.prospective_trigger_fee.unwrap().fee, q.prospective_pipeline_fee.unwrap().total_fee,
          q.maximum_next_action_fee.maximum_effect_fee][kind];
        assert_eq!(fee, expected);
        fees.entry(id).or_default()[kind] = fee;
      }
      match &event {
        Event::TriggerOccurrenceProcessed { actor_id, .. } if members.contains_key(actor_id) => {
          assert!(!fifo.contains(actor_id), "no duplicate readiness");
          fifo.push_back(*actor_id);
        }
        Event::TransferExecuted { actor_id, amount, asset, to, step_index, .. } => {
          runtime_steps += 1;
          if members.contains_key(actor_id) {
            assert_eq!(fifo.pop_front(), Some(*actor_id), "relative FIFO, including partial rounds");
            assert_eq!((*asset, *amount, to, *step_index), (primitives::AssetKind::Native, crate::EXISTENTIAL_DEPOSIT, &super::super::common::BOB, step.index));
            assert!(transfers.insert(*actor_id, *amount).is_none(), "Q1");
            if !step.completes_cycle { fifo.push_back(*actor_id); }
          }
        }
        Event::SwapExecuted { actor_id, asset_in, asset_out, amount_in, amount_out, step_index, .. } => {
          runtime_steps += 1;
          if members.contains_key(actor_id) {
            assert_eq!(workload, Workload::RouterFourPredicates);
            assert_eq!(fifo.pop_front(), Some(*actor_id));
            assert_eq!((*asset_in, *asset_out, *amount_out, *step_index),
              (primitives::AssetKind::Foreign(FOREIGN), primitives::AssetKind::Native, crate::EXISTENTIAL_DEPOSIT, step.index));
            assert!(*amount_in > 0 && *amount_in <= 10 * crate::EXISTENTIAL_DEPOSIT);
            assert!(swaps.insert(*actor_id, (*amount_in, *amount_out)).is_none(), "Q1");
            if !step.completes_cycle { fifo.push_back(*actor_id); }
          }
        }
        Event::StepSkipped { actor_id, .. } | Event::StepFailed { actor_id, .. } => {
          runtime_steps += 1;
          if members.contains_key(actor_id) { assert_eq!(fifo.pop_front(), Some(*actor_id)); }
        }
        Event::CycleStopped { .. } => runtime_steps += 1,
        _ => {}
      }
      if let Some((id, label)) = actor_lifecycle_event_label(&event) {
        if !members.contains_key(&id) { *reference.entry(label.to_owned()).or_default() += 1; }
      }
    }
    let mut owner_holds = BTreeMap::<crate::AccountId, u128>::new();
    let mut residences = BTreeMap::<String, u32>::new();
    let mut pending = 0;
    let mut trigger_deadlines = 0;
    let mut terminal_members = Vec::new();
    let mut total_fees = [0u128; 3];
    for (id, member) in members {
      assert_eq!(generation(*id), member.generation);
      let identity = Actors::actor_identity(*id).expect("funded baseline cannot close");
      assert_eq!(identity.owner, member.owner);
      assert_eq!(identity.actor_class.actor_type(), member.quote.actor_type);
      assert_eq!(identity.sovereign_account, member.account);
      let charged = fees.get(id).copied().unwrap_or_default();
      let spent = charged.iter().try_fold(transfers.get(id).copied().unwrap_or(0), |s,f| s.checked_add(*f)).unwrap();
      let received = swaps.get(id).map_or(0, |swap| swap.1);
      assert_eq!(total(&member.account), before[id].checked_add(received).unwrap().checked_sub(spent).unwrap(), "Native principal/output plus independent fees");
      if workload == Workload::RouterFourPredicates {
        let input = swaps.get(id).map_or(0, |swap| swap.0);
        assert_eq!(Assets::balance(FOREIGN, &member.account), foreign_before[id].checked_sub(input).unwrap());
        for asset in WITNESSES { assert_eq!(Assets::balance(asset, &member.account), 10); }
        if let Some((input, output)) = swaps.get(id) {
          assert_eq!(routes[id]["amountIn"], input.to_string());
          assert_eq!(routes[id]["amountOut"], output.to_string());
        }
      }
      assert!(Balances::free_balance(&member.account) >= <Runtime as pallet_deos_actors::Config>::MinUserBalance::get());
      if identity.actor_class.actor_type() == ActorType::User {
        let quote = Actors::actor_cost_quote(*id).unwrap();
        let hold = Actors::actor_state_hold(*id).expect("User hold persists");
        assert_eq!(hold.owner, member.owner);
        let amount = [hold.breakdown.identity, hold.breakdown.contract_head, hold.breakdown.contract_body, hold.breakdown.detector, hold.breakdown.run]
          .into_iter().try_fold(0u128, u128::checked_add).unwrap();
        assert_eq!(amount, quote.actor_state_hold.total);
        let sum = owner_holds.entry(member.owner.clone()).or_default();
        *sum = sum.checked_add(amount).unwrap();
        if transfers.contains_key(id) || swaps.contains_key(id) { assert_eq!(counts[id][1..], [u32::from(step.index == 0), 1]); }
      } else { assert_eq!(Actors::actor_state_hold(*id), None); }
      for i in 0..3 { total_fees[i] = total_fees[i].checked_add(charged[i]).unwrap(); }
      let process = Actors::actor_processes(*id).expect("persistent process");
      assert_eq!(process.generation, member.generation);
      let residence = match process.residence {
        Some(ProcessResidence::Service(_)) => "Service",
        Some(ProcessResidence::Deadline { .. }) => "Deadline",
        Some(ProcessResidence::Parked(_)) => "Parked",
        None => "Disabled",
      };
      *residences.entry(residence.to_owned()).or_default() += 1;
      let hot = Actors::actor_hot(*id).unwrap();
      pending += u32::from(hot.pending_signal);
      trigger_deadlines += u32::from(pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::contains_key(id));
      if terminal { terminal_members.push(serde_json::json!({"actor": id, "generation": member.generation,
        "residence": format!("{:?}", process.residence), "status": format!("{:?}", process.status),
        "pendingSignal": hot.pending_signal, "cycleNonce": identity.cycle_nonce,
        "balance": total(&member.account).to_string()})); }
    }
    let held_total = owner_holds.values().copied().try_fold(0u128, u128::checked_add).unwrap();
    for (owner, held) in owner_holds {
      assert_eq!(<Balances as InspectHold<crate::AccountId>>::balance_on_hold(
        &crate::RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState), &owner), held);
    }
    assert_eq!(total(&sink), sink_before.checked_add(sink_in).unwrap().checked_sub(sink_out).unwrap(), "Fee Sink allocations reconciled independently of fee receipts");
    assert_eq!(total(&super::super::common::BOB), recipient_before.checked_add(recipient_in).unwrap().checked_sub(recipient_out).unwrap());
    assert!(recipient_in >= transfers.values().sum::<u128>());
    let header = Actors::service_header();
    let mut current = header.cursor;
    let mut work_ring = Vec::new();
    for _ in 0..header.count {
      let actor = current.expect("nonempty ring");
      let node = Actors::service_nodes(actor.actor_id).unwrap();
      assert_eq!(node.generation, actor.generation);
      assert_eq!(Actors::service_nodes(node.next.actor_id).unwrap().previous, actor);
      assert_eq!(Actors::service_nodes(node.previous.actor_id).unwrap().next, actor);
      if members.contains_key(&actor.actor_id) { work_ring.push(actor.actor_id); }
      current = Some(node.next);
    }
    assert_eq!(current, header.cursor);
    assert_eq!(work_ring, fifo.iter().copied().collect::<Vec<_>>());
    let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
    let limits = telemetry.budget().limits();
    let usage = telemetry.usage();
    assert!(usage.actor_control_used().all_lte(limits.actor_control()));
    assert!(usage.shared_used().unwrap().all_lte(limits.shared_economic()));
    let control_left = limits.actor_control().checked_sub(&usage.actor_control_used()).unwrap();
    let effect_left = limits.shared_economic().checked_sub(&usage.shared_used().unwrap()).unwrap();
    let head = header.cursor.and_then(|a| Actors::service_nodes(a.actor_id));
    let deferred = head.is_some_and(|n| n.last_considered == block || n.eligible_from > block);
    let frontier = if fifo.is_empty() || deferred { "readiness-source-limited" }
      else if runtime_steps >= <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() { "execution-count" }
      else { "inconclusive" };
    // A conservative next-contract maximum is a hint, not an observed refusal cause.
    assert_eq!(routes.len(), swaps.len());
    serde_json::json!({"successfulActions": transfers.len() + swaps.len(), "routerOutcomes": routes,
      "timestampMillis": crate::Timestamp::get(),
      "tickMillis": <Runtime as pallet_deos_actors::Config>::CadenceTickMillis::get(),
      "pending": pending, "residences": residences, "triggerDeadlines": trigger_deadlines,
      "actorStateHoldTotal": held_total.to_string(), "workloadRing": work_ring, "terminalMembers": terminal_members,
      "referenceActivity": reference, "runtimeSteps": runtime_steps,
      "collectedFees": total_fees.map(|v| v.to_string()),
      "sinkIn": sink_in.to_string(), "sinkOut": sink_out.to_string(), "sinkBalance": total(&sink).to_string(),
      "frontierClassification": frontier, "nextMaximumIsOnlyHint": true,
      "nextMaximumScope": "generic contract upper, not observed rejecting Service envelope",
      "nextControlMaximum": [next.0.ref_time(), next.0.proof_size()],
      "nextEffectMaximum": [next.1.ref_time(), next.1.proof_size()],
      "controlLimit": [limits.actor_control().ref_time(), limits.actor_control().proof_size()],
      "sharedEconomicLimit": [limits.shared_economic().ref_time(), limits.shared_economic().proof_size()],
      "controlRemaining": [control_left.ref_time(), control_left.proof_size()],
      "effectRemaining": [effect_left.ref_time(), effect_left.proof_size()],
      "nextControlMaximumWouldFit": next.0.all_lte(control_left), "nextEffectMaximumWouldFit": next.1.all_lte(effect_left)})
  })
}

fn run_baseline(wasm: &[u8], profile: Baseline, replay_wasm: bool) -> serde_json::Value {
  run_prepared_baseline(wasm, profile, replay_wasm, prepare_baseline(wasm, profile))
}

fn run_prepared_baseline(
  wasm: &[u8],
  profile: Baseline,
  replay_wasm: bool,
  fixture: BaselineFixture,
) -> serde_json::Value {
  let BaselineFixture {
    actors,
    members,
    setup,
  } = fixture;
  let maxima = (actors.next_control_maximum, actors.next_effect_maximum);
  let (successful_control, control_owners) =
    successful_action_control(profile.class, profile.workload);
  let mut fifo = if profile.recurring {
    VecDeque::new()
  } else {
    actors.actor_ids.iter().copied().collect()
  };
  let mut rows = Vec::new();
  let mut warm_progress = BTreeSet::new();
  let mut gaps_seen = 0;
  let mut phase_delays = [Vec::new(), Vec::new()];
  let mut phase_gaps = [Vec::new(), Vec::new()];
  let service = visit_schedule_campaign(
    wasm,
    &actors,
    UserDemand::ActorOnly,
    profile.schedule(),
    TransferCampaignWindow {
      first_block: 3,
      blocks: profile.blocks(),
      replay_wasm,
    },
    |authored, proof, service| {
      let block = *authored.block.header.number();
      let warm = block - 3 < profile.warmup;
      assert_eq!(authored.metrics.user_calls, 0);
      assert_eq!(authored.metrics.user_dispatch, Weight::zero());
      assert!(authored.metrics.next_user_weight.is_none());
      if warm {
        warm_progress.extend(authored.metrics.progressed_steps.iter().map(|(id, _)| *id));
      }
      let phase = usize::from(!warm);
      phase_delays[phase].extend(
        service
          .signal_to_step_blocks
          .iter()
          .rev()
          .take(authored.metrics.actor_steps as usize)
          .map(|(_, d)| *d),
      );
      phase_gaps[phase].extend(service.inter_step_gaps[gaps_seen..].iter().map(|(_, d)| *d));
      gaps_seen = service.inter_step_gaps.len();
      let accounting = inspect_block(
        wasm,
        authored,
        &members,
        &mut fifo,
        maxima,
        block == 2 + profile.blocks(),
        StepPosition {
          workload: profile.workload,
          index: 0,
          completes_cycle: true,
        },
      );
      let successful_subtotal = successful_control
        .checked_mul(accounting["successfulActions"].as_u64().unwrap())
        .unwrap();
      let other_control = authored
        .metrics
        .actor_control
        .checked_sub(&successful_subtotal)
        .expect("successful workload prices stay within complete booked Control");
      rows.push(serde_json::json!({"block": block, "phase": if warm { "warmup" } else { "measurement" },
        "successfulTransferControlSubtotal": if profile.workload == Workload::Transfer { Some([successful_subtotal.ref_time(), successful_subtotal.proof_size()]) } else { None },
        "successfulActionControlSubtotal": [successful_subtotal.ref_time(), successful_subtotal.proof_size()],
        "successfulActions": accounting["successfulActions"],
        "otherControlResidual": [other_control.ref_time(), other_control.proof_size()],
        "steps": authored.metrics.actor_steps, "actors": authored.metrics.distinct_actors,
        "cycles": authored.metrics.completed_cycles, "attempted": authored.metrics.actor_steps,
        "failedOrSkipped": authored.metrics.non_successful_steps, "triggerMaterializations": authored.metrics.trigger_occurrences.len(),
        "control": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
        "effect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
        "executionProofBytes": proof.map(|p| p.execution_storage_proof_bytes),
        "compactProofBytes": proof.map(|p| p.execution_compact_proof_bytes),
        "verificationProofBytes": proof.map(|p| p.verification_storage_proof_bytes), "accounting": accounting}));
    },
  );
  let distribution = |phase: &str| {
    rows
      .iter()
      .filter(|r| r["phase"] == phase)
      .map(|r| r["steps"].as_u64().unwrap())
      .collect::<Vec<_>>()
  };
  let measured = distribution("measurement");
  assert_eq!(measured.len(), profile.measured as usize);
  let cycles = measured.iter().sum::<u64>();
  let target_met = cycles >= u64::from(W1_TARGET_CYCLES) && measured.iter().all(|s| *s >= 100);
  let summary = serde_json::json!({"schema": 1, "class": format!("{:?}",profile.class),
    "recurring": profile.recurring, "population": profile.actors, "setup": setup,
    "controlOwners": control_owners,
    "nativeAuthorAndImport": true, "exactProductionWasmReplay": replay_wasm,
    "warmupBlocks": profile.warmup, "measurementBlocks": profile.measured,
    "warmupCoverage": warm_progress.len(), "warmed": profile.recurring && warm_progress.len() == profile.actors as usize,
    "warmupSteps": observed_distribution(&distribution("warmup")), "measuredSteps": observed_distribution(&measured),
    "measuredActors": observed_distribution(&rows.iter().filter(|r| r["phase"] == "measurement").map(|r| r["actors"].as_u64().unwrap()).collect::<Vec<_>>()),
    "measuredCycles": cycles, "historicalTargetCycles": W1_TARGET_CYCLES, "historicalMinimumSteps": 100,
    "historicalTargetWindowApplicable": profile.measured == W1_TARGET_BLOCKS, "targetMet": target_met,
    "warmupDelays": observed_distribution(&phase_delays[0]), "measuredDelays": observed_distribution(&phase_delays[1]),
    "warmupGaps": observed_distribution(&phase_gaps[0]), "measuredGaps": observed_distribution(&phase_gaps[1]),
    "terminalCensoring": service.summary(1, 2 + profile.blocks()),
    "terminalHistory": service.actors.iter().map(|(id,h)| serde_json::json!({"actor": id,
      "firstSignalBlock": h.first_signal_block, "firstStepBlock": h.first_step_block,
      "lastStepBlock": h.last_step_block, "pendingSignalBlock": h.pending_signal_block,
      "pendingAgeBlocks": h.pending_signal_block.map(|b| 2 + profile.blocks() - b)})).collect::<Vec<_>>(), "blocks": rows,
    "cpu": null, "timing": null, "networkPov": null});
  if profile.workload == Workload::Transfer {
    println!("ACTOR_TRANSFER_BASELINE_V1 {summary}");
  } else {
    println!("ACTOR_ROUTER_PREDICATE_BASELINE_V1 {summary}");
  }
  summary
}

fn production_wasm() -> Vec<u8> {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM").expect("explicit accepted production Wasm");
  let wasm = std::fs::read(path).unwrap();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  wasm
}

fn short_profiles(wasm: &[u8], replay: bool) {
  for class in [ActorType::System, ActorType::User] {
    let first = run_baseline(
      wasm,
      Baseline {
        actors: 2,
        ..Baseline::full(class, false)
      },
      replay,
    );
    assert_eq!(first["measuredCycles"], 2);
    assert_eq!(first["targetMet"], false);
    let profile = Baseline {
      class,
      workload: Workload::Transfer,
      recurring: true,
      actors: 2,
      warmup: 2,
      measured: 8,
    };
    let a = run_baseline(wasm, profile, replay);
    let b = run_baseline(wasm, profile, replay);
    assert_eq!(
      a, b,
      "deterministic phase/count/fee/frontier reproducibility"
    );
    assert!(a["measuredCycles"].as_u64().unwrap() > 0);
    assert_eq!(a["warmupSteps"]["samples"], 2);
    assert_eq!(a["measuredSteps"]["samples"], 8);
    assert_eq!(
      a["warmed"], false,
      "insufficient fixed warmup is not promoted"
    );
    assert!(
      a["blocks"].as_array().unwrap().last().unwrap()["accounting"]["terminalMembers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["cycleNonce"].as_u64().unwrap() >= 2),
      "both Actors must recur, not merely complete an initial wave"
    );
    assert_eq!(a["blocks"][0]["steps"], 0, "cold causal block retained");
  }
}

#[test]
fn router_predicate_baseline_short_profiles() {
  for class in [ActorType::System, ActorType::User] {
    let profile = Baseline {
      workload: Workload::RouterFourPredicates,
      actors: 2,
      ..Baseline::full(class, false)
    };
    let result = run_baseline(&[], profile, false);
    assert_eq!(result["blocks"][0]["successfulActions"], 2);
    assert_eq!(result["blocks"][0]["failedOrSkipped"], 0);
    assert_eq!(result["measuredActors"]["min"], 2);
  }
}

#[test]
#[ignore = "explicit native first-block diagnostic"]
fn router_predicate_baseline_first_profiles() {
  for class in [ActorType::System, ActorType::User] {
    let result = run_baseline(
      &[],
      Baseline {
        workload: Workload::RouterFourPredicates,
        ..Baseline::full(class, false)
      },
      false,
    );
    assert_eq!(result["population"], 128);
    assert_eq!(result["exactProductionWasmReplay"], false);
    let row = &result["blocks"][0];
    let successful = row["successfulActions"].as_u64().unwrap();
    assert!(
      (1..=128).contains(&successful),
      "first-block evidence must include real swaps"
    );
    assert_eq!(row["failedOrSkipped"], 0);
    assert_eq!(row["cycles"].as_u64().unwrap(), successful);
    assert_eq!(
      row["accounting"]["routerOutcomes"]
        .as_object()
        .unwrap()
        .len() as u64,
      successful
    );
    assert_eq!(
      row["accounting"]["workloadRing"].as_array().unwrap().len() as u64,
      128 - successful
    );
  }
}

#[test]
fn router_predicate_baseline_each_false_condition_skips_real_swap() {
  for index in 0..4 {
    let profile = Baseline {
      workload: Workload::RouterFourPredicates,
      actors: 2,
      ..Baseline::full(ActorType::User, false)
    };
    let mut steps = profile.workload.steps();
    let pallet_deos_actors::Predicate::BalanceAbove { threshold, .. } =
      &mut steps[0].precondition.as_mut().unwrap().clauses[0][index]
    else {
      unreachable!()
    };
    *threshold = u128::MAX;
    let fixture = prepare_baseline_with_steps(&[], profile, steps).actors;
    let parent = parent_header_for(fixture.storage.clone(), &[], 2);
    let authored = author_complete_block_after(
      fixture.storage,
      &[],
      &parent,
      3,
      UserDemand::ActorOnly,
      &fixture.signer,
      0,
      &fixture.actor_profiles,
    );
    assert_complete_block_replays_natively(&authored, &[]);
    assert_eq!(authored.metrics.swap_out_steps, 0);
    assert_eq!(authored.metrics.non_successful_steps, 2);
    TestExternalities::new(authored.post_state).execute_with(|| {
      let events = System::events();
      let skipped = events.iter().filter(|record| matches!(record.event,
        RuntimeEvent::Actors(Event::StepSkipped { actor_id, .. })
          if fixture.actor_profiles.contains_key(&actor_id))).count();
      assert_eq!(skipped, 2, "each false condition skips before economic execution");
      assert!(!events.iter().any(|record| matches!(record.event,
        RuntimeEvent::Actors(Event::StepFailed { actor_id, .. } | Event::ActionFeeCharged { actor_id, .. })
          if fixture.actor_profiles.contains_key(&actor_id))), "no failed Task or Action fee may stand in for a predicate skip");
    });
  }
}

#[test]
#[ignore = "explicit accepted production-Wasm short check"]
fn router_predicate_baseline_short_replays_exact_production_wasm() {
  let wasm = production_wasm();
  for class in [ActorType::System, ActorType::User] {
    let profile = Baseline {
      workload: Workload::RouterFourPredicates,
      actors: 2,
      ..Baseline::full(class, false)
    };
    let result = run_baseline(&wasm, profile, true);
    assert_eq!(result["blocks"][0]["successfulActions"], 2);
  }
}

#[test]
#[ignore = "explicit accepted production-Wasm first block"]
fn router_predicate_baseline_system_first() {
  run_baseline(
    &production_wasm(),
    Baseline {
      workload: Workload::RouterFourPredicates,
      ..Baseline::full(ActorType::System, false)
    },
    true,
  );
}

#[test]
#[ignore = "explicit accepted production-Wasm first block"]
fn router_predicate_baseline_user_first() {
  run_baseline(
    &production_wasm(),
    Baseline {
      workload: Workload::RouterFourPredicates,
      ..Baseline::full(ActorType::User, false)
    },
    true,
  );
}

#[test]
fn transfer_baseline_short_profiles() {
  short_profiles(&[], false);
}

#[test]
fn transfer_baseline_legacy_runner_preserves_demand_modes() {
  for demand in [UserDemand::ActorOnly, UserDemand::ContinuousValid] {
    let fixture = prepare_actor_fixture(&[], 3, WorkloadSchedule::MixedManualCadenced);
    let mut blocks = 0;
    visit_schedule_campaign(
      &[],
      &fixture,
      demand,
      WorkloadSchedule::MixedManualCadenced,
      TransferCampaignWindow {
        first_block: 2,
        blocks: 4,
        replay_wasm: false,
      },
      |authored, proof, _| {
        blocks += 1;
        assert!(proof.is_none());
        match demand {
          UserDemand::ActorOnly => assert_eq!(authored.metrics.user_calls, 0),
          UserDemand::ContinuousValid => assert!(authored.metrics.user_calls > 0),
          _ => unreachable!(),
        }
      },
    );
    assert_eq!(blocks, 4);
  }
}

#[test]
fn transfer_baseline_first_block_preserves_partial_fifo() {
  for class in [ActorType::System, ActorType::User] {
    let result = run_baseline(&[], Baseline::full(class, false), false);
    let steps = result["measuredCycles"].as_u64().unwrap();
    assert!(
      steps > 0 && steps < 128,
      "current resources leave an untouched FIFO suffix"
    );
    assert_eq!(
      result["blocks"][0]["accounting"]["workloadRing"]
        .as_array()
        .unwrap()
        .len(),
      128 - steps as usize
    );
  }
}

#[test]
fn transfer_baseline_fee_sink_allocations_reconcile() {
  let profile = Baseline {
    class: ActorType::User,
    workload: Workload::Transfer,
    recurring: true,
    actors: 2,
    warmup: 2,
    measured: 16,
  };
  let mut fixture = prepare_baseline(&[], profile);
  // Exercise allocation without waiting for a large cadence cohort to fund the reference Actor.
  // This seed is regression setup only; all full baseline entrypoints use the unmodified preset.
  let mut ext = TestExternalities::new_with_code_and_state(
    &[],
    fixture.actors.storage,
    crate::VERSION.state_version(),
  );
  ext.execute_with(|| {
    add_balance(
      &<Runtime as pallet_deos_actors::Config>::FeeSink::get(),
      100 * crate::EXISTENTIAL_DEPOSIT,
    )
  });
  ext.commit_all().unwrap();
  fixture.actors.storage = ext.execute_with(current_top_storage);
  fixture.setup["extraFeeSinkSeedForLedgerRegression"] = serde_json::json!(true);
  let result = run_prepared_baseline(&[], profile, false, fixture);
  assert!(
    result["blocks"]
      .as_array()
      .unwrap()
      .iter()
      .any(|r| r["accounting"]["sinkOut"]
        .as_str()
        .unwrap()
        .parse::<u128>()
        .unwrap()
        > 0),
    "real reference cadence must allocate collected User fees"
  );
}

#[test]
fn transfer_baseline_owner_partition_and_phase_contract() {
  type W = <Runtime as pallet_deos_actors::Config>::WeightInfo;
  let (system, _) = successful_transfer_control(ActorType::System);
  let (user, _) = successful_transfer_control(ActorType::User);
  assert_eq!(user, system.checked_add(&W::fee_collection()).unwrap());
  for (population, owners) in [(128, 1), (255, 1), (256, 2), (1024, 5), (9985, 40)] {
    assert_eq!(
      (0..population)
        .map(owner_for)
        .collect::<BTreeSet<_>>()
        .len(),
      owners
    );
  }
  for class in [ActorType::System, ActorType::User] {
    let fixture = prepare_baseline(
      &[],
      Baseline {
        actors: 256,
        ..Baseline::full(class, true)
      },
    );
    assert_eq!(fixture.setup["ownerCount"], 2);
    assert_eq!(fixture.members.len(), 256);
    assert_eq!(Baseline::full(class, true).blocks(), 164);
  }
}

#[test]
#[ignore = "requires exact accepted production Wasm; short validation only"]
fn transfer_baseline_short_replays_exact_production_wasm() {
  short_profiles(&production_wasm(), true);
}

#[test]
#[ignore = "explicit first-block baseline, no long campaign"]
fn transfer_baseline_system_first() {
  run_baseline(
    &production_wasm(),
    Baseline::full(ActorType::System, false),
    true,
  );
}

#[test]
#[ignore = "explicit first-block baseline, no long campaign"]
fn transfer_baseline_user_first() {
  run_baseline(
    &production_wasm(),
    Baseline::full(ActorType::User, false),
    true,
  );
}

fn recurring(class: ActorType) {
  let wasm = production_wasm();
  let a = run_baseline(&wasm, Baseline::full(class, true), true);
  let b = run_baseline(&wasm, Baseline::full(class, true), true);
  assert_eq!(
    a, b,
    "full repeated baseline retains identical deterministic evidence"
  );
}

#[test]
#[ignore = "explicit 2 x (64 warmup + 100 measurement) campaign"]
fn transfer_baseline_system_recurring() {
  recurring(ActorType::System);
}

#[test]
#[ignore = "explicit 2 x (64 warmup + 100 measurement) campaign"]
fn transfer_baseline_user_recurring() {
  recurring(ActorType::User);
}
