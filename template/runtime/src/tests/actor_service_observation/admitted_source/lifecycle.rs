//! Exact complete callers for paid retry exhaustion/destruction and public owner close.

use super::retry::{TASK_ROOTS, bound, task_notice};
use super::*;
use pallet_deos_actors::{ActorClass, CloseReason, CycleResult, Event, RetryClass, WakeupKey};
use polkadot_sdk::frame_support::traits::fungible::InspectHold;

std::thread_local! {
  pub(super) static BOUNDARIES: RefCell<Vec<(Phase, bool, usize)>> = const {RefCell::new(Vec::new())};
}

pub(super) fn notice(phase: Phase, entering: bool) {
  task_notice(phase, entering);
  let operations = state(|o| o.operations.len());
  BOUNDARIES.with_borrow_mut(|v| v.push((phase, entering, operations)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Wakeup,
  Exhaustion,
  OwnerClose,
}

struct Case {
  name: &'static str,
  block: u32,
  kind: Kind,
  parent: &'static str,
  post: &'static str,
  packet: &'static str,
}

#[test]
#[ignore = "three exact terminal-wakeup/destruction/public-Close packets; no source reconstruction"]
fn retained_exhaustion_and_close_packets_qualify_complete_caller() {
  let base = std::path::PathBuf::from(
    std::env::var("DEOS_RETRY_CLOSE_OBSERVATION_SOURCE")
      .expect("qualified lifecycle raw directory"),
  );
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_SERVICE_OBSERVATION_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  let cases = [
    Case {
      name: "terminal-wakeup",
      block: 187_574,
      kind: Kind::Wakeup,
      parent: "01068ebe28dcc03d880dd5640b987371061175c20bce20da51a225511deb91cc",
      post: "b488c32ba7192ce813951c2ae71031d29af16009f1ea2f68a6d97ded5cd91882",
      packet: "489ff997cc518dbadab6ecdd5b3f2539ad02211cfd8a154c37ce66e425002b26",
    },
    Case {
      name: "terminal-failure",
      block: 187_575,
      kind: Kind::Exhaustion,
      parent: "b488c32ba7192ce813951c2ae71031d29af16009f1ea2f68a6d97ded5cd91882",
      post: "48978e14e0bed484c04c37d90e481039304cf0f52c28caeae396162fe5a02143",
      packet: "a27923597b850d40474b4a2f8aa1bc9c77fe9c3028f6ffc10d9ab3e4df4fb7d8",
    },
    Case {
      name: "owner-close",
      block: 187_576,
      kind: Kind::OwnerClose,
      parent: "48978e14e0bed484c04c37d90e481039304cf0f52c28caeae396162fe5a02143",
      post: "fe468faca5524cf814293780495e61fbc6e3cbacb2d5e86fc1e45e4946f70b9d",
      packet: "29fc4b2b3436afac8cf0b3740741e31bbeabaa4899bd1662458ce37136727db1",
    },
  ];
  let mut reports = Vec::new();
  for case in cases {
    let actor = if case.kind == Kind::OwnerClose {
      90
    } else {
      89
    };
    let path = base.join(case.name);
    let parent = Storage {
      top: State::decode_all(&mut bound(&path.join("parent.scale"), case.parent).as_slice())
        .unwrap(),
      ..Default::default()
    };
    let expected =
      State::decode_all(&mut bound(&path.join("post.scale"), case.post).as_slice()).unwrap();
    let block = crate::Block::decode_all(
      &mut bound(
        &path.join("selected/development.sealed-block.scale"),
        case.packet,
      )
      .as_slice(),
    )
    .unwrap();
    let root = Layout::trie_root(parent.top.clone());
    assert_eq!(*block.header.number(), case.block);
    assert_eq!(
      Layout::trie_root(expected.clone()),
      *block.header.state_root()
    );
    assert_eq!(
      format!(
        "{:x}",
        H256::from(polkadot_sdk::sp_io::hashing::sha2_256(
          &parent.top[polkadot_sdk::sp_core::storage::well_known_keys::CODE]
        ))
      ),
      "2df308749408946a7b4ad6f90b3d5aabca8defe1fa03c7c11806fac409ccb8d9"
    );
    let reason: crate::RuntimeHoldReason = pallet_deos_actors::HoldReason::ActorState.into();
    let (fixture, before, balances, reserves, supply, hold, owner_held, bitmap, identities, active) =
      polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
        assert_eq!(System::block_number(), case.block - 1);
        let before = Actors::active_actor_state(actor).unwrap();
        assert_eq!(before.identity.cycle_nonce, 0);
        assert!(!before.hot.pending_signal);
        if case.kind == Kind::OwnerClose {
          assert!(before.run_state.is_none());
          assert_eq!(before.contract.steps.len(), 1);
          assert!(Actors::actor_identity(89).is_none());
        } else {
          let run = before.run_state.as_ref().unwrap();
          assert_eq!(
            (
              run.cycle_nonce,
              run.cursor,
              run.unsuccessful_attempts_at_cursor
            ),
            (1, 1, 2)
          );
          assert_eq!(run.last_committed_step_block, Some(187_568));
          assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
          assert_eq!(run.cumulative_outcomes.failed_steps, 2);
          assert_eq!(
            before.contract.steps[1].on_error,
            StepErrorPolicy::RetryLater { max_attempts: 3 }
          );
          assert_eq!(before.contract.cooldown_blocks, 2);
        }
        if case.kind == Kind::Wakeup {
          assert_eq!(
            Actors::deadline_handles(actor).unwrap().key,
            WakeupKey::Block(case.block)
          );
          assert!(Actors::service_nodes(actor).is_none());
        } else {
          assert!(Actors::deadline_handles(actor).is_none());
          if case.kind == Kind::Exhaustion {
            assert!(Actors::service_nodes(actor).unwrap().eligible_from <= case.block);
          }
        }
        let native = <Runtime as Config>::FeeNativeAssetId::get();
        let paired = Asset::Local(0);
        let lp_id = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((native, paired))
          .unwrap()
          .lp_token;
        let lp = Asset::Local(lp_id);
        let payer = before.identity.sovereign_account.clone();
        let sink = <Runtime as Config>::FeeSink::get();
        let fixture = Fixture {
          actor,
          payer: payer.clone(),
          native,
          a: native,
          b: paired,
          lp,
          amount: crate::UNIT,
          spend: crate::UNIT,
          sink_before: Assets::balance(&sink, native),
          sink,
          pipeline_fee: 0,
          inspection_predecessors: None,
          paid_prefix: None,
        };
        let held = <Balances as InspectHold<crate::AccountId>>::balance_on_hold(
          &reason,
          &before.identity.owner,
        );
        let bitmap = Actors::owner_slot_bitmap(&before.identity.owner);
        (
          fixture,
          before,
          [
            Assets::balance(&payer, native),
            Assets::balance(&payer, paired),
            Assets::balance(&payer, lp),
          ],
          crate::AssetConversion::get_reserves(native, paired).unwrap(),
          <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
            crate::AccountId,
          >>::total_issuance(lp_id),
          Actors::actor_state_hold(actor).unwrap().breakdown,
          held,
          bitmap,
          Actors::actor_identity_count(),
          Actors::active_actor_count(),
        )
      });
    TASK_ROOTS.with_borrow_mut(Vec::clear);
    BOUNDARIES.with_borrow_mut(Vec::clear);
    let observed = run_call(&parent, &fixture, true, false, notice, true, || {
      import(&block)
    });
    let roots = TASK_ROOTS.with_borrow(Clone::clone);
    let boundaries = BOUNDARIES.with_borrow(Clone::clone);
    TASK_ROOTS.with_borrow_mut(Vec::clear);
    BOUNDARIES.with_borrow_mut(Vec::clear);
    let plain = run_call(&parent, &fixture, false, false, notice, true, || {
      import(&block)
    });
    let plain_roots = TASK_ROOTS.with_borrow(Clone::clone);
    let compact = observed
      .proof
      .to_compact_proof::<Blake2Hasher>(root)
      .unwrap();
    export_case(&output, case.name, root, &observed, &compact);
    std::fs::write(
      output.join(case.name).join("plain-post.scale"),
      plain.top.encode(),
    )
    .unwrap();
    let encounters = boundaries.split_inclusive(|(p, entering, _)| (*p, *entering) == (Phase::Encounter, false))
      .filter(|e| e.iter().any(|(p, entering, _)| matches!((*p, *entering), (Phase::HeadAdmission {actor: id, ..}, true) if id == actor))).collect::<Vec<_>>();
    let cleanup_keys = [
      (
        "semantic",
        pallet_deos_actors::ActorSemanticStates::<Runtime>::hashed_key_for(actor),
      ),
      (
        "run",
        pallet_deos_actors::ActorRunStateStore::<Runtime>::hashed_key_for(actor),
      ),
      (
        "hold",
        pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(actor),
      ),
      (
        "sovereign",
        pallet_deos_actors::SovereignIndex::<Runtime>::hashed_key_for(&fixture.payer),
      ),
    ];
    let removals = cleanup_keys
      .iter()
      .map(|(name, key)| {
        (
          *name,
          observed
            .observation
            .operations
            .iter()
            .enumerate()
            .filter(|(_, op)| {
              op.method == "put" && &op.key == key && op.value.is_none() && !op.observer
            })
            .map(|(index, op)| (index, op.phase))
            .collect::<Vec<_>>(),
        )
      })
      .collect::<Vec<_>>();
    std::fs::write(output.join(case.name).join("before-assertions.json"), serde_json::to_vec_pretty(&serde_json::json!({
      "targetPhases": format!("{encounters:?}"), "taskRoots": format!("{roots:?}"), "cleanupRemovals": format!("{removals:?}"), "acceptance": false,
    })).unwrap()).unwrap();
    assert_eq!(observed.top, expected);
    assert_eq!(observed.top, plain.top);
    assert_eq!(observed.root, plain.root);
    assert_eq!(observed.root, *block.header.state_root());
    assert_eq!(observed.proof, plain.proof);
    assert_eq!(observed.db, plain.db);
    assert_eq!(observed.observation.markers, plain.observation.markers);
    assert!(!observed.observation.fault_applied);
    assert_eq!(roots.len(), plain_roots.len());
    for (a, b) in roots.iter().zip(&plain_roots) {
      assert_eq!((a.0, a.1), (b.0, b.1));
    }
    if case.kind == Kind::Exhaustion {
      assert_eq!(encounters.len(), 1);
      assert_eq!(roots.len(), 2);
      assert_eq!(roots[0].1, roots[1].1);
      assert!(roots[0].0 && !roots[1].0);
      let phases = [
        Phase::Preflight,
        Phase::Task,
        Phase::Retirement,
        Phase::Settlement,
        Phase::Collection,
        Phase::Authentication,
      ];
      let indices = phases.map(|p| {
        assert_eq!(
          encounters[0]
            .iter()
            .filter(|(q, e, _)| *q == p && *e)
            .count(),
          1
        );
        let start = encounters[0]
          .iter()
          .position(|(q, e, _)| *q == p && *e)
          .unwrap();
        let end = encounters[0]
          .iter()
          .position(|(q, e, _)| *q == p && !*e)
          .unwrap();
        assert!(start < end);
        start
      });
      assert!(indices.windows(2).all(|p| p[0] < p[1]));
      let transactions = &observed.observation.transactions[roots[0].2..roots[1].2];
      assert!(
        transactions
          .iter()
          .filter(|(kind, _, phase)| *kind == "rollback" && *phase == Some(Phase::Task))
          .count()
          >= 2
      );
      assert!(
        observed.observation.operations[roots[0].3..roots[1].3]
          .iter()
          .any(|op| op.method == "put" && op.phase == Some(Phase::Task) && !op.observer)
      );
      let collection = encounters[0]
        .iter()
        .find(|(p, e, _)| *p == Phase::Collection && *e)
        .unwrap()
        .2;
      for (_, positions) in &removals {
        assert!(
          positions
            .iter()
            .any(|(position, _)| *position >= roots[1].3 && *position < collection),
          "selected destruction must remove each retained authority after Task rollback and before fee collection"
        );
      }
    } else {
      assert!(
        encounters.is_empty() && roots.is_empty(),
        "wake/public Close is not a target Service turn"
      );
    }
    let (usage, released, tx_fee) =
      polkadot_sdk::sp_io::TestExternalities::new(Storage {
        top: observed.top.clone(),
        ..Default::default()
      })
      .execute_with(|| {
        let mut fees = Vec::new();
        let mut failed = 0;
        let mut continued = 0;
        let mut summaries = 0;
        let mut closed = 0;
        let mut effects = polkadot_sdk::frame_support::weights::Weight::zero();
        let mut distributions = 0;
        let mut tx_fee = 0;
        for record in System::events() {
          match record.event {
            crate::RuntimeEvent::TransactionPayment(
              polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid {
                who,
                actual_fee,
                ..
              },
            ) => {
              assert_eq!(who, before.identity.owner);
              tx_fee += actual_fee;
            }
            crate::RuntimeEvent::Actors(Event::ActionFeeCharged {
              actor_id,
              fee,
              actual_effect_weight,
              ..
            }) => {
              effects = effects.saturating_add(actual_effect_weight);
              if actor_id == actor {
                fees.push(fee);
              }
            }
            crate::RuntimeEvent::Actors(Event::StepFailed {
              actor_id,
              cycle_nonce,
              step_index,
              retry_class,
              ..
            }) if actor_id == actor => {
              assert_eq!((cycle_nonce, step_index), (1, 1));
              assert_eq!(retry_class, RetryClass::Temporary);
              failed += 1;
            }
            crate::RuntimeEvent::Actors(Event::CycleContinued {
              actor_id,
              cycle_nonce,
              ..
            }) if actor_id == actor => {
              assert_eq!(cycle_nonce, 1);
              continued += 1;
            }
            crate::RuntimeEvent::Actors(Event::CycleSummary {
              actor_id,
              cycle_nonce,
              result,
              outcomes,
            }) if actor_id == actor => {
              assert_eq!(cycle_nonce, 1);
              assert_eq!(result, CycleResult::Failed);
              assert_eq!(outcomes.failed_steps, 3);
              assert_eq!(outcomes.executed_steps, 1);
              assert_eq!(outcomes.committed_effectful_tasks, 1);
              summaries += 1;
            }
            crate::RuntimeEvent::Actors(Event::ActorClosed { actor_id, reason })
              if actor_id == actor =>
            {
              assert_eq!(
                reason,
                if case.kind == Kind::OwnerClose {
                  CloseReason::OwnerInitiated
                } else {
                  CloseReason::RetryAttemptsExhausted
                }
              );
              assert_eq!(
                record.phase,
                polkadot_sdk::frame_system::Phase::ApplyExtrinsic(
                  if case.kind == Kind::OwnerClose { 3 } else { 2 }
                )
              );
              closed += 1;
            }
            crate::RuntimeEvent::Actors(Event::TransferExecuted {
              actor_id,
              asset: Asset::Native,
              amount,
              ..
            }) if actor_id == primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID => {
              distributions += amount
            }
            crate::RuntimeEvent::Actors(
              Event::PipelineFeeCharged { actor_id, .. }
              | Event::CycleStarted { actor_id, .. }
              | Event::CycleSuspended { actor_id, .. }
              | Event::StepSkipped { actor_id, .. }
              | Event::LiquidityAdded { actor_id, .. }
              | Event::TransferExecuted { actor_id, .. },
            ) if actor_id == actor => {
              panic!("no second Opening, economic output, new suspension or compensation")
            }
            _ => {}
          }
        }
        let fee = if case.kind == Kind::Exhaustion {
          19_195_368
        } else {
          0
        };
        if case.kind == Kind::Exhaustion {
          assert_eq!(fees, [fee]);
          assert_eq!((failed, continued, summaries, closed), (1, 1, 1, 1));
        } else {
          assert!(fees.is_empty());
          assert_eq!(
            (failed, continued, summaries, closed),
            (0, 0, 0, u32::from(case.kind == Kind::OwnerClose))
          );
        }
        if case.kind == Kind::OwnerClose {
          assert!(tx_fee > 0);
          assert_eq!(block.extrinsics.len(), 4);
        } else {
          assert_eq!(tx_fee, 0);
        }
        assert_eq!(
          Assets::balance(&fixture.sink, fixture.native) + distributions - fixture.sink_before,
          fee + tx_fee
        );
        assert_eq!(
          balances[0] - Assets::balance(&fixture.payer, fixture.native),
          fee
        );
        assert_eq!(Assets::balance(&fixture.payer, fixture.b), balances[1]);
        assert_eq!(Assets::balance(&fixture.payer, fixture.lp), balances[2]);
        assert_eq!(
          crate::AssetConversion::get_reserves(fixture.native, fixture.b).unwrap(),
          reserves
        );
        let Asset::Local(lp_id) = fixture.lp else {
          unreachable!()
        };
        assert_eq!(
          <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
            crate::AccountId,
          >>::total_issuance(lp_id),
          supply
        );
        let held_after = <Balances as InspectHold<crate::AccountId>>::balance_on_hold(
          &reason,
          &before.identity.owner,
        );
        let released = if case.kind == Kind::Wakeup {
          assert_eq!(
            Actors::active_actor_state(actor).unwrap().run_state,
            before.run_state
          );
          assert!(Actors::deadline_handles(actor).is_none());
          assert_eq!(
            Actors::service_nodes(actor).unwrap().eligible_from,
            case.block + 1
          );
          assert_eq!(Actors::actor_state_hold(actor).unwrap().breakdown, hold);
          assert_eq!(held_after, owner_held);
          assert_eq!(Actors::actor_identity_count(), identities);
          0
        } else {
          assert!(Actors::actor_identity(actor).is_none());
          assert!(Actors::actor_semantic_states(actor).is_none());
          assert!(Actors::actor_run_state(actor).is_none());
          assert!(Actors::actor_state_hold(actor).is_none());
          assert!(Actors::service_nodes(actor).is_none());
          assert!(Actors::deadline_handles(actor).is_none());
          assert!(Actors::sovereign_index(&fixture.payer).is_none());
          assert_eq!(
            Actors::actor_cost_quote(actor).unwrap_err(),
            pallet_deos_actors::ActorCostQuoteError::ActorNotFound
          );
          assert_eq!(Actors::actor_identity_count(), identities - 1);
          assert_eq!(Actors::active_actor_count(), active - 1);
          let ActorClass::User { owner_slot } = before.identity.actor_class else {
            unreachable!()
          };
          let mut bitmap = bitmap;
          bitmap[usize::from(owner_slot / 8)] &= !(1 << (owner_slot % 8));
          assert_eq!(Actors::owner_slot_bitmap(&before.identity.owner), bitmap);
          let released =
            hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
          assert!(released > 0);
          assert_eq!(owner_held - held_after, released);
          let ledger = pallet_deos_actors::ActorStateHolds::<Runtime>::iter()
            .filter(|(_, h)| h.owner == before.identity.owner)
            .map(|(_, h)| {
              h.breakdown.identity
                + h.breakdown.contract_head
                + h.breakdown.contract_body
                + h.breakdown.detector
                + h.breakdown.run
            })
            .sum::<u128>();
          assert_eq!(held_after, ledger);
          for (_, positions) in &removals {
            assert!(
              !positions.is_empty(),
              "actual destruction has a tagged deletion"
            );
          }
          released
        };
        assert!(Actors::block_resource_state().is_none());
        let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
        assert!(!telemetry.optional_actor_work_halted());
        let usage = telemetry.usage();
        let limits = telemetry.budget().limits();
        assert_eq!(usage.actor_effect_used(), effects);
        assert!(usage.actor_control_used().all_lte(limits.actor_control()));
        assert!(
          usage
            .actor_effect_used()
            .saturating_add(usage.user_dispatch_used())
            .all_lte(limits.shared_economic())
        );
        let key = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key();
        let bytes = observed
          .observation
          .operations
          .iter()
          .rev()
          .find(|op| op.method == "put" && op.key == key && op.value.is_some())
          .unwrap()
          .value
          .as_ref()
          .unwrap();
        let state = pallet_deos_actors::BlockResourceState::<crate::BlockNumber>::decode_all(
          &mut bytes.as_slice(),
        )
        .unwrap();
        assert_eq!(state.outstanding_reservations(), 0);
        assert_eq!(state.usage(), usage);
        (usage, released, tx_fee)
      });
    verify_original_and_compact(&parent, root, &block, &observed, &compact);
    reports.push(serde_json::json!({"stage": case.name, "actor": actor, "block": case.block,
      "parentRoot": format!("{root:?}"), "postRoot": format!("{:?}", observed.root), "taskRoots": format!("{roots:?}"),
      "targetPhases": format!("{encounters:?}"), "cleanupRemovals": format!("{removals:?}"),
      "operations": observed.observation.operations.len(), "sdkDb": observed.db, "witnessBytes": observed.proof.encoded_size(), "compactBytes": compact.encoded_size(),
      "actorEffects": [usage.actor_effect_used().ref_time(), usage.actor_effect_used().proof_size()], "userDispatch": [usage.user_dispatch_used().ref_time(), usage.user_dispatch_used().proof_size()],
      "releasedHold": released.to_string(), "transactionFee": tx_fee.to_string(), "defaultNativeWasmTopEqual": true, "physicalPricingQualified": false}));
  }
  std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "stages": reports, "originalCompactReplayQualified": true, "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
  })).unwrap()).unwrap();
}
