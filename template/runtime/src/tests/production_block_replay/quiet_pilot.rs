//! Post-execution quiet-fixture reporting; no observation is resource or pricing authority.

use super::{Actors, AuthoredBlock, CycleState, Event, Runtime, RuntimeEvent, System};
use pallet_deos_actors::{ActorId, ActorProcesses, ActorType, ServiceHeader};
use polkadot_sdk::{frame_support::weights::Weight, sp_runtime::traits::Header as _};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

type CycleKey = (String, ActorId, u64);

#[derive(Default)]
pub(super) struct Report {
  rows: Vec<Value>,
  signals: BTreeMap<(String, ActorId), u32>,
  opened: BTreeMap<CycleKey, (u32, u32)>,
  cycles: Vec<Value>,
}

fn weight(value: Weight) -> [u64; 2] {
  [value.ref_time(), value.proof_size()]
}

fn add_fee(fees: &mut [u128; 5], index: usize, value: u128) {
  fees[index] = fees[index]
    .checked_add(value)
    .expect("bounded quiet fee sum");
}

impl Report {
  pub(super) fn record(&mut self, preset: &str, authored: &AuthoredBlock) {
    let number = *authored.block.header.number();
    assert_eq!(System::block_number(), number);
    let mut fees = [0u128; 5];
    let mut events = BTreeMap::<ActorId, BTreeMap<&str, u32>>::new();
    let mut step_actors = BTreeSet::new();
    let mut economic_effects = 0u32;
    for record in System::events() {
      let (actor, kind, step, economic) = match record.event {
        RuntimeEvent::TransactionPayment(
          polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
            actual_fee,
            tip,
            ..
          },
        ) => {
          add_fee(&mut fees, 3, actual_fee);
          add_fee(&mut fees, 4, tip);
          continue;
        }
        RuntimeEvent::Actors(event) => match event {
          Event::TriggerOccurrenceProcessed { actor_id, fee, .. } => {
            add_fee(&mut fees, 0, fee);
            assert!(
              self
                .signals
                .insert((preset.to_owned(), actor_id), number)
                .is_none()
            );
            (actor_id, "trigger_occurrence", false, false)
          }
          Event::PipelineFeeCharged { actor_id, fee } => {
            add_fee(&mut fees, 1, fee);
            (actor_id, "pipeline_fee", false, false)
          }
          Event::ActionFeeCharged { actor_id, fee, .. } => {
            add_fee(&mut fees, 2, fee);
            (actor_id, "action_fee", false, false)
          }
          Event::CycleStarted {
            actor_id,
            cycle_nonce,
          } => {
            let triggered = self
              .signals
              .remove(&(preset.to_owned(), actor_id))
              .expect("quiet Opening has a recorded useful-readiness occurrence");
            assert!(
              self
                .opened
                .insert(
                  (preset.to_owned(), actor_id, cycle_nonce),
                  (triggered, number)
                )
                .is_none()
            );
            (actor_id, "cycle_started", false, false)
          }
          Event::CycleSummary {
            actor_id,
            cycle_nonce,
            result,
            ..
          } => {
            let (triggered, opened) = self
              .opened
              .remove(&(preset.to_owned(), actor_id, cycle_nonce))
              .expect("quiet completion has a recorded Opening");
            self.cycles.push(json!({
              "preset": preset, "actor": actor_id, "nonce": cycle_nonce.to_string(),
              "triggerBlock": triggered, "openingBlock": opened, "completionBlock": number,
              "triggerToOpeningBlocks": opened.checked_sub(triggered).unwrap(),
              "openingToCompletionBlocks": number.checked_sub(opened).unwrap(),
              "result": format!("{result:?}"),
            }));
            (actor_id, "cycle_summary", false, false)
          }
          Event::StepSkipped { actor_id, .. } => (actor_id, "step_skipped", true, false),
          Event::StepFailed { actor_id, .. } => (actor_id, "step_failed", true, false),
          Event::CycleStopped { actor_id, .. } => (actor_id, "stop_cycle", true, false),
          Event::TransferExecuted { actor_id, .. }
          | Event::SwapExecuted { actor_id, .. }
          | Event::BurnExecuted { actor_id, .. }
          | Event::MintExecuted { actor_id, .. }
          | Event::StakeExecuted { actor_id, .. }
          | Event::UnstakeExecuted { actor_id, .. }
          | Event::LiquidityDonated { actor_id, .. }
          | Event::LiquidityAdded { actor_id, .. }
          | Event::LiquidityRemoved { actor_id, .. } => (actor_id, "economic_effect", true, true),
          unexpected => panic!("unqualified quiet Actor event: {unexpected:?}"),
        },
        _ => continue,
      };
      assert_eq!(
        Actors::active_actor_state(actor)
          .unwrap()
          .identity
          .actor_class
          .actor_type(),
        ActorType::System
      );
      if step {
        assert!(
          step_actors.insert(actor),
          "global quiet Step outcomes preserve Q1"
        );
      }
      economic_effects += u32::from(economic);
      *events.entry(actor).or_default().entry(kind).or_default() += 1;
    }
    let metrics = &authored.metrics;
    let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
    assert_eq!(telemetry.block_number(), number);
    let usage = telemetry.usage();
    assert_eq!(metrics.actor_control, usage.actor_control_used());
    assert_eq!(metrics.actor_effect, usage.actor_effect_used());
    assert_eq!(metrics.user_dispatch, usage.user_dispatch_used());
    assert_eq!(metrics.user_calls, 0);
    assert_eq!(
      metrics.actor_steps, 0,
      "empty workload counts are not global counts"
    );
    let sink = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let state = Actors::active_actor_state(sink).unwrap();
    let budget = telemetry.budget();
    let service = ServiceHeader::<Runtime>::get();
    self.rows.push(json!({
      "preset": preset, "block": number, "nativeStateRoot": format!("{:?}", authored.block.header.state_root()),
      "workload": {"profiles": 0, "stepOutcomeEvents": metrics.actor_steps, "distinctActors": metrics.distinct_actors, "userCalls": metrics.user_calls},
      "global": {"systemActorEvents": events, "stepOutcomeEvents": step_actors.len(), "successfulEconomicEffectEvents": economic_effects},
      "feeSink": {"actor": sink, "nonce": state.identity.cycle_nonce.to_string(), "idle": state.hot.cycle_state == CycleState::Idle,
        "pendingSignal": state.hot.pending_signal, "runPresent": state.run_state.is_some(),
        "resident": ActorProcesses::<Runtime>::get(sink).unwrap().residence.is_some(),
        "nextTriggerTick": state.hot.trigger_wakeup_pointer.map(|pointer| pointer.tick)},
      "service": {"members": service.count, "cursor": service.cursor.map(|actor| actor.actor_id)},
      "feesNativeUnits": {"creation": "0", "trigger": fees[0].to_string(), "pipeline": fees[1].to_string(),
        "action": fees[2].to_string(), "transactionActualFee": fees[3].to_string(), "transactionReportedTip": fees[4].to_string()},
      "bookedWeight": {"globalActorControl": weight(metrics.actor_control), "globalActorEffect": weight(metrics.actor_effect),
        "globalUserDispatch": weight(metrics.user_dispatch), "prepassActorControl": weight(metrics.prepass_actor_control),
        "prepassActorEffect": weight(metrics.prepass_actor_effect),
        "afterPrepassActorControl": weight(metrics.actor_control.checked_sub(&metrics.prepass_actor_control).unwrap()),
        "afterPrepassActorEffect": weight(metrics.actor_effect.checked_sub(&metrics.prepass_actor_effect).unwrap())},
      "budget": {"maximumBlock": weight(budget.maximum_block()), "fixedEnvelope": weight(budget.fixed_envelope()),
        "actorControlLimit": weight(budget.limits().actor_control()), "sharedEconomicLimit": weight(budget.limits().shared_economic()),
        "actorBaseTurn": weight(budget.limits().actor_base_turn()), "userBaseTurn": weight(budget.limits().user_base_turn())},
      "preIdleFrameRemaining": weight(metrics.pre_idle_remaining),
      "resourceStateCleared": Actors::block_resource_state().is_none(), "physicalMetrics": null,
    }));
  }

  pub(super) fn emit(self, presets: &[String]) {
    validate_row_keys(presets, &self.rows);
    assert!(
      self.signals.is_empty(),
      "quiet report ends without pending signals"
    );
    assert!(
      self.opened.is_empty(),
      "quiet report ends without unreported cycles"
    );
    let encoded = serde_json::to_vec(&self.rows).unwrap();
    for row in &self.rows {
      println!("ACTORS_QUIET_PILOT_ROW_V1 {row}");
    }
    println!(
      "ACTORS_QUIET_PILOT_SUMMARY_V1 {}",
      json!({
        "schema": 1, "scope": "default-native full Executive; synthetic relay context; no Wasm/PVF or physical price",
        "presets": presets, "firstBlock": 5, "lastBlock": 34, "rowCount": self.rows.len(), "cycles": self.cycles,
        "weights": "booked declarations [RefTime ps, ProofSize bytes], not measured cost",
        "fees": "native integer strings; creation absent because ActorCreated is refused; transaction fee and tip are not added",
        "stepOutcomes": "event observations, not an economic workflow-success metric",
        "physicalMetricsCollected": false, "rowsJsonSha256": format!("{:x}", polkadot_sdk::sp_core::H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&encoded))),
      })
    );
  }
}

fn validate_row_keys(presets: &[String], rows: &[Value]) {
  assert!(!presets.is_empty());
  let expected = presets
    .iter()
    .flat_map(|preset| (5..=34).map(move |block| (preset.as_str(), block)))
    .collect::<BTreeSet<_>>();
  assert_eq!(
    expected.len(),
    presets.len() * 30,
    "preset names are unique"
  );
  let actual = rows
    .iter()
    .map(|row| {
      (
        row["preset"].as_str().expect("quiet row preset"),
        u32::try_from(row["block"].as_u64().expect("quiet row block")).unwrap(),
      )
    })
    .collect::<BTreeSet<_>>();
  assert_eq!(actual.len(), rows.len(), "quiet row keys are unique");
  assert_eq!(
    actual, expected,
    "quiet rows exactly cover every preset/block"
  );
}

#[test]
fn quiet_row_coverage_rejects_missing_duplicate_and_out_of_domain_rows() {
  let presets = vec!["first".to_owned(), "second".to_owned()];
  let rows = presets
    .iter()
    .flat_map(|preset| (5..=34).map(move |block| json!({"preset": preset, "block": block})))
    .collect::<Vec<_>>();
  validate_row_keys(&presets, &rows);
  let mut missing = rows.clone();
  missing.pop();
  let mut duplicate = rows.clone();
  duplicate[0] = duplicate[1].clone();
  let mut wrong_block = rows.clone();
  wrong_block[0]["block"] = json!(4);
  let mut wrong_preset = rows.clone();
  wrong_preset[0]["preset"] = json!("third");
  for invalid in [missing, duplicate, wrong_block, wrong_preset] {
    assert!(std::panic::catch_unwind(|| validate_row_keys(&presets, &invalid)).is_err());
  }
}
