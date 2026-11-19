//! Six exact larger-geometry callers; neither declarations nor witness samples are maxima.

use super::lifecycle::{BOUNDARIES, notice};
use super::retry::{TASK_ROOTS, bound};
use super::*;
use pallet_deos_actors::{
  ActorClass, ActorContractHeads, ActorContractTailChunks, CloseReason, CycleResult, Event,
  RetryClass, SuspensionReason, WakeupKey,
};
use polkadot_sdk::frame_support::traits::fungible::InspectHold;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Opening,
  Failure,
  Wakeup,
  Exhaustion,
}

struct Case {
  name: &'static str,
  block: u32,
  kind: Kind,
  failures: u32,
  parent: &'static str,
  post: &'static str,
  packet: &'static str,
}

#[test]
#[ignore = "six exact larger-geometry Opening/failure/wakeup/destruction packets"]
fn retained_geometry_packets_qualify_complete_callers() {
  let source = std::path::PathBuf::from(
    std::env::var("DEOS_GEOMETRY_OBSERVATION_SOURCE")
      .expect("qualified geometry source raw directory"),
  );
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_SERVICE_OBSERVATION_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  let cases = [
    Case {
      name: "opening-prefix",
      block: 187579,
      kind: Kind::Opening,
      failures: 0,
      parent: "f923c0b619814fbfd06ed4dc10760d3640abad31f629aec963c3684d72fbbab9",
      post: "2e17e9bc8af844fb4954b8a9a0ce018a2181d25e977af8504db1550dddc6dc5b",
      packet: "9e5cf5ca93581b4675a206671775259ae6cb1bae130cc812d1b2531944759414",
    },
    Case {
      name: "first-failure",
      block: 187580,
      kind: Kind::Failure,
      failures: 0,
      parent: "2e17e9bc8af844fb4954b8a9a0ce018a2181d25e977af8504db1550dddc6dc5b",
      post: "62d9129b79908110d229e944aca6f8c6514a91e5defe78f1d56ba3d13a3158b3",
      packet: "fb63086f05e694980fa32cbf0ddb3733befb0b7e3194eb435eba838d6f3b7cea",
    },
    Case {
      name: "retry-wakeup",
      block: 187582,
      kind: Kind::Wakeup,
      failures: 1,
      parent: "215c6d371fc39daf295020c90be4cf670a220ae4b68f1757593ff4b6a6b6fcc1",
      post: "ea6e364fe5808f7a97d3d60178c240ce15d3a4866746af1554a2430552e64f1c",
      packet: "3a76d9f5f6b198227f7a91780883c4fd296b75cb755020dbbd7632502bb9eb9c",
    },
    Case {
      name: "retry-failure",
      block: 187583,
      kind: Kind::Failure,
      failures: 1,
      parent: "ea6e364fe5808f7a97d3d60178c240ce15d3a4866746af1554a2430552e64f1c",
      post: "7977cff17a3cae18a5787b842b19a0778b2f656ff2d518c386982a06faf2086e",
      packet: "e30a58f1b1a83e7188463e3af109181378faaed5137ade3363c20273dabd1c6b",
    },
    Case {
      name: "terminal-wakeup",
      block: 187585,
      kind: Kind::Wakeup,
      failures: 2,
      parent: "2cc1614498669213f7ea1643ef320cd89c44289a7c8d8e0d7e08e33dbe224e0b",
      post: "b3e353c9d5404414391b38ec58d75c947fd6e6c9720d61ab5bda0299fbafb5c2",
      packet: "10b743e1aa1f2e3d30ad149420a8726e34377934661b17657e1580fb8e26b4ea",
    },
    Case {
      name: "terminal-exhaustion",
      block: 187586,
      kind: Kind::Exhaustion,
      failures: 2,
      parent: "b3e353c9d5404414391b38ec58d75c947fd6e6c9720d61ab5bda0299fbafb5c2",
      post: "635ce70b87b6b5bf058e7d270b6838470ed773ab33b98628ed446ab4895b8796",
      packet: "c37e11f70e6d769ead239bc8c25ea04a9acd73ee3d62bec5143076b531210f98",
    },
  ];
  let total = |h: pallet_deos_actors::ActorStateHoldBreakdown<u128>| {
    h.identity + h.contract_head + h.contract_body + h.detector + h.run
  };
  let reason: crate::RuntimeHoldReason = pallet_deos_actors::HoldReason::ActorState.into();
  let mut reports = Vec::new();
  for case in cases {
    let path = source.join(case.name);
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
    let (fixture, before, balances, reserves, supply, held_before, hold, bitmap, counts) =
      polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
        assert_eq!(System::block_number(), case.block - 1);
        let before = Actors::active_actor_state(91).unwrap();
        assert_eq!(before.identity.cycle_nonce, 0);
        assert_eq!(before.contract.steps.len(), 12);
        assert_eq!(before.contract.cooldown_blocks, 2);
        for step in &before.contract.steps {
          assert_eq!(step.precondition.as_ref().unwrap().evaluation_units(), 4);
        }
        assert_eq!(
          ActorContractHeads::<Runtime>::get(91)
            .unwrap()
            .encoded_size(),
          772
        );
        let chunks =
          ActorContractTailChunks::<Runtime>::iter_prefix(91).collect::<BTreeMap<_, _>>();
        assert_eq!(
          chunks.values().map(|c| c.steps.len()).collect::<Vec<_>>(),
          [4, 4, 3]
        );
        if case.kind == Kind::Opening {
          assert!(before.run_state.is_none() && before.hot.pending_signal);
        } else {
          assert!(!before.hot.pending_signal);
          let run = before.run_state.as_ref().unwrap();
          assert_eq!(
            (
              run.cycle_nonce,
              run.cursor,
              run.unsuccessful_attempts_at_cursor
            ),
            (1, 1, case.failures)
          );
          assert_eq!(run.last_committed_step_block, Some(187579));
          assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
        }
        if case.kind == Kind::Wakeup {
          assert_eq!(
            Actors::deadline_handles(91).unwrap().key,
            WakeupKey::Block(case.block)
          );
          assert!(Actors::service_nodes(91).is_none());
        } else {
          assert!(Actors::deadline_handles(91).is_none());
          assert!(Actors::service_nodes(91).unwrap().eligible_from <= case.block);
        }
        let native = <Runtime as Config>::FeeNativeAssetId::get();
        let paired = Asset::Local(0);
        let lp_id = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((native, paired))
          .unwrap()
          .lp_token;
        let lp = Asset::Local(lp_id);
        let payer = before.identity.sovereign_account.clone();
        let sink = <Runtime as Config>::FeeSink::get();
        let pipeline_fee = if case.kind == Kind::Opening {
          Actors::actor_cost_quote(91)
            .unwrap()
            .prospective_pipeline_fee
            .unwrap()
            .total_fee
        } else {
          0
        };
        let fixture = Fixture {
          actor: 91,
          payer: payer.clone(),
          native,
          a: native,
          b: paired,
          lp,
          amount: crate::UNIT,
          spend: crate::UNIT,
          sink_before: Assets::balance(&sink, native),
          sink,
          pipeline_fee,
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
          held,
          Actors::actor_state_hold(91).unwrap().breakdown,
          bitmap,
          (Actors::actor_identity_count(), Actors::active_actor_count()),
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
    let encounters = boundaries
      .split_inclusive(|(p, e, _)| (*p, *e) == (Phase::Encounter, false))
      .filter(|e| {
        e.iter()
          .any(|(p, e, _)| matches!((*p, *e), (Phase::HeadAdmission { actor: 91, .. }, true)))
      })
      .collect::<Vec<_>>();
    let mut keys = vec![
      (
        "semantic".to_owned(),
        pallet_deos_actors::ActorSemanticStates::<Runtime>::hashed_key_for(91),
      ),
      (
        "run".to_owned(),
        pallet_deos_actors::ActorRunStateStore::<Runtime>::hashed_key_for(91),
      ),
      (
        "head".to_owned(),
        ActorContractHeads::<Runtime>::hashed_key_for(91),
      ),
      (
        "hold".to_owned(),
        pallet_deos_actors::ActorStateHolds::<Runtime>::hashed_key_for(91),
      ),
      (
        "sovereign".to_owned(),
        pallet_deos_actors::SovereignIndex::<Runtime>::hashed_key_for(&fixture.payer),
      ),
    ];
    for index in 0u32..3 {
      keys.push((
        format!("tail-{index}"),
        ActorContractTailChunks::<Runtime>::hashed_key_for(91, index),
      ));
    }
    let removals = keys
      .iter()
      .map(|(name, key)| {
        (
          name,
          observed
            .observation
            .operations
            .iter()
            .enumerate()
            .filter(|(_, op)| {
              op.method == "put" && &op.key == key && op.value.is_none() && !op.observer
            })
            .map(|(i, op)| (i, op.phase))
            .collect::<Vec<_>>(),
        )
      })
      .collect::<Vec<_>>();
    std::fs::write(output.join(case.name).join("before-assertions.json"),serde_json::to_vec_pretty(&serde_json::json!({
      "targetPhases":format!("{encounters:?}"),"taskRoots":format!("{roots:?}"),"cleanupRemovals":format!("{removals:?}"),"acceptance":false,
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
    if case.kind == Kind::Wakeup {
      assert!(encounters.is_empty() && roots.is_empty());
    } else {
      assert_eq!(encounters.len(), 1);
      assert_eq!(roots.len(), 2);
      assert!(roots[0].0 && !roots[1].0);
      let mut phases = vec![Phase::Preflight, Phase::Task];
      if case.kind == Kind::Opening {
        assert!(
          !encounters[0]
            .iter()
            .any(|(p, _, _)| *p == Phase::Retirement),
          "continuing Opening retains Service membership"
        );
      } else {
        phases.push(Phase::Retirement);
      }
      phases.extend([Phase::Settlement, Phase::Collection, Phase::Authentication]);
      let spans = phases
        .iter()
        .copied()
        .map(|p| {
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
          (start, end)
        })
        .collect::<Vec<_>>();
      assert!(spans.windows(2).all(|p| p[0].0 < p[1].0));
      let span = |phase| spans[phases.iter().position(|p| *p == phase).unwrap()];
      assert!(span(Phase::Preflight).1 < span(Phase::Task).0);
      assert!(span(Phase::Task).1 < span(Phase::Settlement).0);
      if case.kind != Kind::Opening {
        assert!(span(Phase::Task).1 < span(Phase::Retirement).0);
        assert!(span(Phase::Retirement).1 < span(Phase::Settlement).0);
      }
      assert!(span(Phase::Collection).1 < span(Phase::Authentication).0);
      assert!(
        span(Phase::Authentication).1 < span(Phase::Settlement).1,
        "Settlement encloses Collection and final Authentication, so their windows are not additive"
      );
      let transactions = &observed.observation.transactions[roots[0].2..roots[1].2];
      if case.kind == Kind::Opening {
        assert_ne!(roots[0].1, roots[1].1);
        assert!(
          transactions
            .iter()
            .any(|(kind, _, p)| *kind == "commit" && *p == Some(Phase::Task))
        );
        assert!(!transactions.iter().any(|(kind, _, _)| *kind == "rollback"));
      } else {
        assert_eq!(roots[0].1, roots[1].1);
        assert!(
          transactions
            .iter()
            .filter(|(kind, _, p)| *kind == "rollback" && *p == Some(Phase::Task))
            .count()
            >= 2
        );
      }
      assert!(
        observed.observation.operations[roots[0].3..roots[1].3]
          .iter()
          .any(|op| op.method == "put" && op.phase == Some(Phase::Task) && !op.observer)
      );
      if case.kind == Kind::Exhaustion {
        let collection = encounters[0]
          .iter()
          .find(|(p, e, _)| *p == Phase::Collection && *e)
          .unwrap()
          .2;
        for (_, deletions) in &removals {
          assert!(
            deletions
              .iter()
              .any(|(i, _)| *i >= roots[1].3 && *i < collection),
            "all selected authority/head/tails must retire after rollback and before collection"
          );
        }
      }
    }
    let (usage, released) = polkadot_sdk::sp_io::TestExternalities::new(Storage {
      top: observed.top.clone(),
      ..Default::default()
    })
    .execute_with(|| {
      let mut actions = Vec::new();
      let mut pipelines = Vec::new();
      let mut starts = 0;
      let mut transfers = 0;
      let mut failures = 0;
      let mut continued = 0;
      let mut suspended = 0;
      let mut closed = 0;
      let mut summaries = 0;
      let mut effects = polkadot_sdk::frame_support::weights::Weight::zero();
      let mut distributed = 0;
      for record in System::events() {
        match record.event {
          crate::RuntimeEvent::Actors(Event::ActionFeeCharged {
            actor_id,
            fee,
            actual_effect_weight,
            ..
          }) => {
            effects = effects.saturating_add(actual_effect_weight);
            if actor_id == 91 {
              actions.push(fee);
            }
          }
          crate::RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: 91, fee }) => {
            pipelines.push(fee)
          }
          crate::RuntimeEvent::Actors(Event::CycleStarted { actor_id: 91, .. }) => starts += 1,
          crate::RuntimeEvent::Actors(Event::TransferExecuted {
            actor_id: 91,
            cycle_nonce,
            step_index,
            to,
            asset,
            amount,
          }) => {
            assert_eq!((cycle_nonce, step_index, amount), (1, 0, 1));
            assert_eq!(to, before.identity.owner);
            assert_eq!(asset, fixture.native);
            transfers += 1;
          }
          crate::RuntimeEvent::Actors(Event::StepFailed {
            actor_id: 91,
            cycle_nonce,
            step_index,
            retry_class,
            ..
          }) => {
            assert_eq!((cycle_nonce, step_index), (1, 1));
            assert_eq!(retry_class, RetryClass::Temporary);
            failures += 1;
          }
          crate::RuntimeEvent::Actors(Event::CycleContinued {
            actor_id: 91,
            cycle_nonce,
            ..
          }) => {
            assert_eq!(cycle_nonce, 1);
            continued += 1;
          }
          crate::RuntimeEvent::Actors(Event::CycleSuspended {
            actor_id: 91,
            reason,
            ..
          }) => {
            assert_eq!(reason, SuspensionReason::Temporary);
            suspended += 1;
          }
          crate::RuntimeEvent::Actors(Event::CycleSummary {
            actor_id: 91,
            cycle_nonce,
            result,
            outcomes,
          }) => {
            assert_eq!(cycle_nonce, 1);
            assert_eq!(result, CycleResult::Failed);
            assert_eq!(outcomes.executed_steps, 1);
            assert_eq!(outcomes.committed_effectful_tasks, 1);
            assert_eq!(outcomes.failed_steps, 3);
            summaries += 1;
          }
          crate::RuntimeEvent::Actors(Event::ActorClosed {
            actor_id: 91,
            reason,
          }) => {
            assert_eq!(reason, CloseReason::RetryAttemptsExhausted);
            closed += 1;
          }
          crate::RuntimeEvent::Actors(
            Event::StepSkipped { actor_id: 91, .. } | Event::LiquidityAdded { actor_id: 91, .. },
          ) => panic!("all predicates pass; failed LP never publishes output"),
          crate::RuntimeEvent::Actors(Event::TransferExecuted {
            actor_id,
            asset: Asset::Native,
            amount,
            ..
          }) if actor_id == primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID => {
            distributed += amount
          }
          _ => {}
        }
      }
      let (action, pipeline, spend) = match case.kind {
        Kind::Opening => {
          assert_eq!(
            (
              starts, transfers, failures, continued, suspended, closed, summaries
            ),
            (1, 1, 0, 0, 0, 0, 0)
          );
          assert_eq!(fixture.pipeline_fee, 1_246_093_689);
          (11_708_264, fixture.pipeline_fee, 1)
        }
        Kind::Failure => {
          assert_eq!(
            (
              starts, transfers, failures, continued, suspended, closed, summaries
            ),
            (0, 0, 1, u32::from(case.failures > 0), 1, 0, 0)
          );
          (19_195_368, 0, 0)
        }
        Kind::Wakeup => {
          assert_eq!(
            (
              starts, transfers, failures, continued, suspended, closed, summaries
            ),
            (0, 0, 0, 0, 0, 0, 0)
          );
          (0, 0, 0)
        }
        Kind::Exhaustion => {
          assert_eq!(
            (
              starts, transfers, failures, continued, suspended, closed, summaries
            ),
            (0, 0, 1, 1, 0, 1, 1)
          );
          (19_195_368, 0, 0)
        }
      };
      if action == 0 {
        assert!(actions.is_empty());
      } else {
        assert_eq!(actions, [action]);
      }
      if pipeline == 0 {
        assert!(pipelines.is_empty());
      } else {
        assert_eq!(pipelines, [pipeline]);
      }
      assert_eq!(
        balances[0] - Assets::balance(&fixture.payer, fixture.native),
        action + pipeline + spend
      );
      assert_eq!(
        Assets::balance(&fixture.sink, fixture.native) + distributed - fixture.sink_before,
        action + pipeline
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
      let released = if case.kind == Kind::Exhaustion {
        assert!(
          Actors::actor_identity(91).is_none()
            && Actors::actor_semantic_states(91).is_none()
            && Actors::actor_run_state(91).is_none()
        );
        assert!(ActorContractHeads::<Runtime>::get(91).is_none());
        assert_eq!(
          ActorContractTailChunks::<Runtime>::iter_prefix(91).count(),
          0
        );
        assert!(
          Actors::actor_state_hold(91).is_none()
            && Actors::deadline_handles(91).is_none()
            && Actors::service_nodes(91).is_none()
        );
        assert!(Actors::sovereign_index(&fixture.payer).is_none());
        assert_eq!(
          (Actors::actor_identity_count(), Actors::active_actor_count()),
          (counts.0 - 1, counts.1 - 1)
        );
        let ActorClass::User { owner_slot } = before.identity.actor_class else {
          unreachable!()
        };
        let mut bitmap = bitmap;
        bitmap[usize::from(owner_slot / 8)] &= !(1 << (owner_slot % 8));
        assert_eq!(Actors::owner_slot_bitmap(&before.identity.owner), bitmap);
        assert_eq!(total(hold), 7_239_000_000);
        assert_eq!(held_before - held_after, total(hold));
        total(hold)
      } else {
        let after = Actors::active_actor_state(91).unwrap();
        assert_eq!(after.contract, before.contract);
        assert_eq!(after.identity.cycle_nonce, 0);
        assert!(!after.hot.pending_signal);
        let run = after.run_state.as_ref().unwrap();
        assert_eq!((run.cycle_nonce, run.cursor), (1, 1));
        assert_eq!(run.last_committed_step_block, Some(187579));
        assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
        if case.kind == Kind::Opening {
          assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
          // This is a retained member, not B+1 reentry: the immutable round already consumed its
          // sole turn. Exactly one target encounter/Transfer is checked above; the next captured
          // actual attempt is in the following block, without rewriting this eligibility scalar.
          assert_eq!(Actors::service_nodes(91).unwrap().eligible_from, case.block);
        } else {
          assert_eq!(
            run.contract_authority,
            before.run_state.as_ref().unwrap().contract_authority
          );
          if case.kind == Kind::Wakeup {
            assert_eq!(run.encode(), before.run_state.as_ref().unwrap().encode());
            assert!(Actors::deadline_handles(91).is_none());
            assert_eq!(
              Actors::service_nodes(91).unwrap().eligible_from,
              case.block + 1
            );
          } else {
            assert_eq!(run.unsuccessful_attempts_at_cursor, case.failures + 1);
            assert_eq!(
              Actors::deadline_handles(91).unwrap().key,
              WakeupKey::Block(case.block + 2)
            );
            assert!(Actors::service_nodes(91).is_none());
          }
        }
        let retained = Actors::actor_state_hold(91).unwrap().breakdown;
        assert_eq!(
          retained,
          Actors::actor_cost_quote(91)
            .unwrap()
            .actor_state_hold
            .breakdown
        );
        assert_eq!(held_after + total(hold), held_before + total(retained));
        assert_eq!(
          (Actors::actor_identity_count(), Actors::active_actor_count()),
          counts
        );
        assert_eq!(
          ActorContractTailChunks::<Runtime>::iter_prefix(91).count(),
          3
        );
        0
      };
      let ledger = pallet_deos_actors::ActorStateHolds::<Runtime>::iter()
        .filter(|(_, h)| h.owner == before.identity.owner)
        .map(|(_, h)| total(h.breakdown))
        .sum::<u128>();
      assert_eq!(held_after, ledger);
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
      (usage, released)
    });
    verify_original_and_compact(&parent, root, &block, &observed, &compact);
    reports.push(serde_json::json!({"stage":case.name,"block":case.block,"parentRoot":format!("{root:?}"),"postRoot":format!("{:?}",observed.root),"taskRoots":format!("{roots:?}"),
      "targetPhases":format!("{encounters:?}"),"cleanupRemovals":format!("{removals:?}"),"operations":observed.observation.operations.len(),"sdkDb":observed.db,
      "witnessBytes":observed.proof.encoded_size(),"compactBytes":compact.encoded_size(),"actorEffects":[usage.actor_effect_used().ref_time(),usage.actor_effect_used().proof_size()],
      "releasedHold":released.to_string(),"defaultNativeWasmTopEqual":true,"physicalPricingQualified":false}));
  }
  std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({
    "stages":reports,"originalCompactReplayQualified":true,"physicalPricingQualified":false,"completeDomainMaximaQualified":false,
  })).unwrap()).unwrap();
}
