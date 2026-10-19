//! Bounded native ledger/frontier witness; no capacity or physical-cost claim.
use super::*;

#[test]
fn router_first_frontier_ledger_and_next_block() {
  let mut cases = Vec::new();
  for class in [ActorType::System, ActorType::User] {
    let profile = Baseline {
      workload: Workload::RouterFourPredicates,
      ..Baseline::full(class, false)
    };
    let fixture = prepare_baseline(&[], profile);
    let members = fixture.members;
    let actors = fixture.actors;
    let mut fifo = actors.actor_ids.iter().copied().collect::<VecDeque<_>>();
    let mut state = actors.storage;
    let mut parent = parent_header_for(state.clone(), &[], 2);
    let mut first_pending = None;
    let mut rows = Vec::new();
    for block in 3..=4 {
      let authored = author_complete_block_after(
        state,
        &[],
        &parent,
        block,
        UserDemand::ActorOnly,
        &actors.signer,
        0,
        &actors.actor_profiles,
      );
      assert_complete_block_replays_natively(&authored, &[]);
      let accounting = inspect_block(
        &[],
        &authored,
        &members,
        &mut fifo,
        (actors.next_control_maximum, actors.next_effect_maximum),
        false,
        StepPosition {
          workload: profile.workload,
          index: 0,
          completes_cycle: true,
        },
      );
      let ledger = TestExternalities::new(authored.post_state.clone()).execute_with(|| {
        let configured = crate::configs::BlockResourceBudgetValue::get();
        let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
        let budget = telemetry.budget();
        assert_eq!(budget, configured, "no settled-prefix credits in reference provider");
        let maximum = budget.maximum_block();
        let fixed = budget.fixed_envelope();
        let schedulable = maximum.checked_sub(&fixed).unwrap();
        let control = budget.limits().actor_control();
        assert_eq!(control, Weight::from_parts(schedulable.ref_time() / 3, schedulable.proof_size() / 3));
        assert_eq!(control.proof_size(), 2_499_209);
        assert_eq!(maximum.proof_size() / 3 - control.proof_size(), 996_044);
        assert_eq!(control.checked_sub(&telemetry.usage().actor_control_used()).unwrap().proof_size(),
          accounting["controlRemaining"][1].as_u64().unwrap());
        let header = Actors::service_header();
        let head = header.cursor.unwrap();
        let node = Actors::service_nodes(head.actor_id).unwrap();
        assert_eq!(Some(&head.actor_id), fifo.front());
        if block == 3 {
          assert_eq!(authored.metrics.actor_steps, if class == ActorType::System { 50 } else { 44 });
          assert!(node.eligible_from <= block && node.last_considered < block);
          let hot = Actors::actor_hot(head.actor_id).unwrap();
          assert!(hot.pending_signal);
          assert_eq!(hot.cycle_state, CycleState::Idle);
          assert!(Actors::actor_run_state(head.actor_id).is_none());
          first_pending = Some(head.actor_id);
        } else {
          let id = first_pending.unwrap();
          assert!(!fifo.contains(&id), "previous head progresses without retrigger or topup");
          assert_eq!(Actors::actor_identity(id).unwrap().cycle_nonce, 1);
          assert!(!Actors::actor_hot(id).unwrap().pending_signal);
        }
        serde_json::json!({
          "maximum": [maximum.ref_time(), maximum.proof_size()],
          "fixed": [fixed.ref_time(), fixed.proof_size()],
          "fixedComponents": format!("{:?}", crate::configs::FixedBlockWeightComponentsValue::get()),
          "configuredAndFrozen": format!("{budget:?}"),
          "finalizedSnapshot": format!("{telemetry:?}"),
          "liveLedgerAfterFinalization": format!("{:?}", Actors::block_resource_state()),
          "head": head.actor_id, "headNode": format!("{node:?}"),
          "firstPending": first_pending,
          "countCeiling": <Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get(),
          "scanCeiling": <Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get(),
        })
      });
      rows.push(serde_json::json!({"block": block, "steps": authored.metrics.actor_steps,
        "actors": authored.metrics.distinct_actors, "nonSuccessful": authored.metrics.non_successful_steps,
        "prepassSteps": authored.metrics.prepass_steps,
        "prepassControl": [authored.metrics.prepass_actor_control.ref_time(), authored.metrics.prepass_actor_control.proof_size()],
        "prepassEffect": [authored.metrics.prepass_actor_effect.ref_time(), authored.metrics.prepass_actor_effect.proof_size()],
        "control": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
        "effect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
        "root": format!("{:?}", authored.block.header.state_root), "ledger": ledger, "accounting": accounting}));
      parent = authored.block.header;
      state = authored.post_state;
    }
    cases.push(serde_json::json!({"class": format!("{class:?}"), "blocks": rows}));
  }
  println!(
    "ACTOR_FRONTIER_LEDGER_V1 {}",
    serde_json::json!({"nativeOnly": true,
    "notCapacity": true, "cases": cases})
  );
}

#[test]
fn configured_fixed_envelope_has_exact_owner_ledger() {
  use crate::configs::*;
  use polkadot_sdk::frame_support::dispatch::DispatchClass;
  let frame = RuntimeBlockWeights::get();
  let base = frame.get(DispatchClass::Mandatory).base_extrinsic;
  let timestamp = <polkadot_sdk::pallet_timestamp::weights::SubstrateWeight<Runtime> as
    polkadot_sdk::pallet_timestamp::WeightInfo>::set()
    .checked_add(&<polkadot_sdk::pallet_timestamp::weights::SubstrateWeight<Runtime> as
      polkadot_sdk::pallet_timestamp::WeightInfo>::on_finalize()).unwrap();
  let owners = [
    ("frame_block_base", "prefix", frame.base_block),
    (
      "three_mandatory_bases",
      "context/bookkeeping",
      base.checked_mul(3).unwrap(),
    ),
    (
      "message_queue_service",
      "initialization maximum",
      MessageQueueServiceWeight::get(),
    ),
    (
      "xcmp_idle",
      "outstanding idle maximum",
      crate::XcmpQueue::on_idle_weight(),
    ),
    (
      "governance",
      "initialization maximum",
      governance_config::GovernanceFixedWeight::get(),
    ),
    (
      "session_rotation",
      "conditional initialization maximum",
      SessionRotationFixedWeight::get(),
    ),
    ("authorship", "initialization", AuthorshipFixedWeight::get()),
    ("aura", "initialization", AuraFixedWeight::get()),
    (
      "aura_ext",
      "initialization/finalization ownership to qualify",
      AuraExtFixedWeight::get(),
    ),
    (
      "xcm_version_discovery",
      "outstanding idle maximum",
      XcmVersionDiscoveryFixedWeight::get(),
    ),
    ("timestamp", "context plus prepaid finalization", timestamp),
    (
      "parachain_validation",
      "zero separate owner; composition to qualify",
      Weight::zero(),
    ),
    (
      "downward_messages",
      "context maximum",
      DmpFixedWeight::get(),
    ),
    (
      "horizontal_messages",
      "context maximum plus bookkeeping",
      XcmpRegisteredFixedWeight::get(),
    ),
  ];
  let sum = owners
    .iter()
    .fold(Weight::zero(), |a, (_, _, w)| a.checked_add(w).unwrap());
  assert_eq!(sum, FixedBlockWeight::get());
  assert_eq!(sum, Weight::from_parts(1_090_309_152_836, 2_988_132));
  let quarter = Weight::from_parts(
    crate::MAXIMUM_BLOCK_WEIGHT.ref_time() / 4,
    crate::MAXIMUM_BLOCK_WEIGHT.proof_size() / 4,
  );
  assert!(
    sum.any_gt(quarter),
    "known policy mismatch must not become a hidden denominator"
  );
  let without_mq = sum.checked_sub(&MessageQueueServiceWeight::get()).unwrap();
  use governance_config::*;
  type G = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
  let governance_phases = [
    (
      "maturing",
      <G as pallet_governance::WeightInfo>::service_maturing_proposals(
        MaxGovernanceMaturingProposalsPerBlock::get(),
      ),
    ),
    (
      "finalized_outcomes",
      <G as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(
        MaxGovernanceFinalizedOutcomesPerBlock::get(),
      ),
    ),
    (
      "reward_expiry",
      <G as pallet_governance::WeightInfo>::service_expiring_accounts(
        MaxGovernanceExpiringAccountsPerBlock::get(),
      ),
    ),
  ];
  let phase_max = governance_phases
    .iter()
    .fold(Weight::zero(), |a, (_, w)| a.max(*w));
  let governance_prefix = <G as pallet_governance::WeightInfo>::service_pending_enactments(
    MaxGovernancePendingEnactmentsPerBlock::get(),
  )
  .checked_add(&<G as pallet_governance::WeightInfo>::service_epoch_catch_up())
  .unwrap();
  assert_eq!(
    governance_prefix.checked_add(&phase_max).unwrap(),
    GovernanceFixedWeight::get()
  );
  println!(
    "ACTOR_FIXED_OWNER_LEDGER_V1 {}",
    serde_json::json!({
      "declaredOnly": true, "notActualPrefixOrCapacity": true,
      "total": [sum.ref_time(), sum.proof_size()],
      "quarter": [quarter.ref_time(), quarter.proof_size()],
      "withoutMessageQueue": [without_mq.ref_time(), without_mq.proof_size()],
      "governancePrefix": [governance_prefix.ref_time(), governance_prefix.proof_size()],
      "governancePhaseBoundsNotSum": governance_phases.iter().map(|(name, w)| serde_json::json!({
        "phase": name, "weight": [w.ref_time(), w.proof_size()]
      })).collect::<Vec<_>>(),
      "owners": owners.iter().map(|(name, phase, weight)| serde_json::json!({
        "owner": name, "phase": phase, "weight": [weight.ref_time(), weight.proof_size()]
      })).collect::<Vec<_>>()
    })
  );
}
