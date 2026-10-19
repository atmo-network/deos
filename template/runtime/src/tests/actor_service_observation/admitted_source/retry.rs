//! Complete native replay of the genuine Temporary/Deadline/retry source.

use super::*;
use pallet_deos_actors::{Event, RetryClass, SuspensionReason, WakeupKey};

std::thread_local! {
  pub(super) static TASK_ROOTS: RefCell<Vec<(bool, H256, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn task_notice(phase: Phase, entering: bool) {
  block_notice(phase, entering);
  let target_task = phase == Phase::Task
    && state(|o| {
      o.markers
        .iter()
        .rev()
        .find_map(|(p, start)| match (*p, *start) {
          (Phase::Booking { actor, .. }, true) => Some(actor),
          _ => None,
        })
        == Some(o.fixture.actor)
    });
  if target_task {
    state(|o| o.diagnostic = true);
    let root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1));
    state(|o| o.diagnostic = false);
    let (transaction_boundary, operation_boundary) =
      state(|o| (o.transactions.len(), o.operations.len()));
    TASK_ROOTS.with_borrow_mut(|roots| {
      roots.push((entering, root, transaction_boundary, operation_boundary))
    });
  }
}

struct Case {
  name: &'static str,
  block: u32,
  before_failures: u32,
  attempt: bool,
  parent: &'static str,
  post: &'static str,
  packet: &'static str,
}

pub(super) fn bound(path: &std::path::Path, expected: &str) -> Vec<u8> {
  let bytes = std::fs::read(path).unwrap();
  assert_eq!(
    format!(
      "{:x}",
      H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
    ),
    expected
  );
  bytes
}

#[test]
#[ignore = "three exact Temporary/Deadline/retry source packets; no source reconstruction"]
fn retained_temporary_retry_packets_qualify_complete_caller() {
  let base = std::path::PathBuf::from(
    std::env::var("DEOS_RETRY_OBSERVATION_SOURCE")
      .expect("explicit qualified raw source directory"),
  );
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_SERVICE_OBSERVATION_OUTPUT").expect("new observation directory"),
  );
  std::fs::create_dir(&output).unwrap();
  let cases = [
    Case {
      name: "first-failure",
      block: 187_569,
      before_failures: 0,
      attempt: true,
      parent: "9597c3af76e85e1d27b183b4ebb7eec002a81cfdd1ad08ab3356f313b7013e08",
      post: "69be315c0599cec3283aca9d8558739533d8434b2b326e0aad535592c3f57d15",
      packet: "4e1f53803cc1d80e755b24906a8ddaa7e25872ca3be794c0781376fcd9c41935",
    },
    Case {
      name: "wakeup",
      block: 187_571,
      before_failures: 1,
      attempt: false,
      parent: "ea4ee5e78cd7fb92e2439675b82b717cd14c3eb34b76c428b9c93a9efdfe2cbb",
      post: "1194aa7c3e221a01115d52ac74a5a049e0b83dcb6bbc4c0c0640b46cc850dd5a",
      packet: "d604ab53bd72dd0a85820a573d0a9570bf2bd7b4b5e42b1ccd0163a34ed5615e",
    },
    Case {
      name: "retry-failure",
      block: 187_572,
      before_failures: 1,
      attempt: true,
      parent: "1194aa7c3e221a01115d52ac74a5a049e0b83dcb6bbc4c0c0640b46cc850dd5a",
      post: "e21988dc00b1dc52b3f8cb68d6fda8462ea78df357a413039f080e9ac394bda3",
      packet: "2762b5a289f939bcd735b0a56263ac6b294dac9ee204cc11bca5f24801adbe8d",
    },
  ];
  let mut reports = Vec::new();
  for case in cases {
    let source = base.join(case.name);
    let parent = Storage {
      top: State::decode_all(&mut bound(&source.join("parent.scale"), case.parent).as_slice())
        .unwrap(),
      ..Default::default()
    };
    let expected =
      State::decode_all(&mut bound(&source.join("post.scale"), case.post).as_slice()).unwrap();
    let block = crate::Block::decode_all(
      &mut bound(
        &source.join("selected/development.sealed-block.scale"),
        case.packet,
      )
      .as_slice(),
    )
    .unwrap();
    let parent_root = Layout::trie_root(parent.top.clone());
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
    let (fixture, before, balances, reserves, supply) =
      polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
        assert_eq!(System::block_number(), case.block - 1);
        let actor = Actors::active_actor_state(89).unwrap();
        assert_eq!(actor.identity.cycle_nonce, 0);
        assert!(!actor.hot.pending_signal);
        let run = actor.run_state.as_ref().unwrap();
        assert_eq!(
          (
            run.cycle_nonce,
            run.cursor,
            run.unsuccessful_attempts_at_cursor
          ),
          (1, 1, case.before_failures)
        );
        assert_eq!(run.last_committed_step_block, Some(187_568));
        if case.attempt {
          assert!(Actors::deadline_handles(89).is_none());
          assert!(Actors::service_nodes(89).unwrap().eligible_from <= case.block);
        } else {
          assert_eq!(
            Actors::deadline_handles(89).unwrap().key,
            WakeupKey::Block(case.block)
          );
          assert!(Actors::service_nodes(89).is_none());
        }
        assert_eq!(actor.contract.cooldown_blocks, 2);
        assert_eq!(
          actor.contract.steps[1].on_error,
          pallet_deos_actors::StepErrorPolicy::RetryLater { max_attempts: 3 }
        );
        let native = <Runtime as Config>::FeeNativeAssetId::get();
        let paired = Asset::Local(0);
        let lp_id = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((native, paired))
          .unwrap()
          .lp_token;
        let lp = Asset::Local(lp_id);
        let payer = actor.identity.sovereign_account.clone();
        let sink = <Runtime as Config>::FeeSink::get();
        let fixture = Fixture {
          actor: 89,
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
        (
          fixture,
          actor,
          [
            Assets::balance(&payer, native),
            Assets::balance(&payer, paired),
            Assets::balance(&payer, lp),
          ],
          crate::AssetConversion::get_reserves(native, paired).unwrap(),
          <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
            crate::AccountId,
          >>::total_issuance(lp_id),
        )
      });
    TASK_ROOTS.with_borrow_mut(Vec::clear);
    let observed = run_call(&parent, &fixture, true, false, task_notice, true, || {
      import(&block)
    });
    let roots = TASK_ROOTS.with_borrow(Clone::clone);
    TASK_ROOTS.with_borrow_mut(Vec::clear);
    let plain = run_call(&parent, &fixture, false, false, task_notice, true, || {
      import(&block)
    });
    let plain_roots = TASK_ROOTS.with_borrow(Clone::clone);
    let compact = observed
      .proof
      .to_compact_proof::<Blake2Hasher>(parent_root)
      .unwrap();
    export_case(&output, case.name, parent_root, &observed, &compact);
    std::fs::write(
      output.join(case.name).join("plain-post.scale"),
      plain.top.encode(),
    )
    .unwrap();
    let encounters = observed
      .observation
      .markers
      .split_inclusive(|m| *m == (Phase::Encounter, false))
      .filter(|markers| {
        markers.iter().any(|(p, entering)| {
          matches!(
            (*p, *entering),
            (Phase::HeadAdmission { actor: 89, .. }, true)
          )
        })
      })
      .collect::<Vec<_>>();
    let phases = encounters
      .iter()
      .map(|e| {
        e.iter()
          .filter(|(_, entering)| *entering)
          .map(|(p, _)| *p)
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    std::fs::write(output.join(case.name).join("before-assertions.json"), serde_json::to_vec_pretty(&serde_json::json!({
      "targetPhases": format!("{phases:?}"), "taskRoots": format!("{roots:?}"), "acceptance": false,
    })).unwrap()).unwrap();
    assert_eq!(observed.top, expected);
    assert_eq!(observed.top, plain.top);
    assert_eq!(observed.root, plain.root);
    assert_eq!(observed.root, *block.header.state_root());
    assert_eq!(observed.proof, plain.proof);
    assert_eq!(observed.db, plain.db);
    assert_eq!(observed.observation.markers, plain.observation.markers);
    assert_eq!(roots.len(), plain_roots.len());
    for (wrapped, unwrapped) in roots.iter().zip(&plain_roots) {
      assert_eq!((wrapped.0, wrapped.1), (unwrapped.0, unwrapped.1));
    }
    assert!(!observed.observation.fault_applied);
    if case.attempt {
      assert_eq!(encounters.len(), 1);
      let phases = [
        Phase::Preflight,
        Phase::Task,
        Phase::Retirement,
        Phase::Settlement,
        Phase::Collection,
        Phase::Authentication,
      ];
      let positions = phases.map(|p| {
        assert_eq!(
          encounters[0].iter().filter(|m| **m == (p, true)).count(),
          1,
          "{p:?}"
        );
        assert!(encounters[0].contains(&(p, false)));
        encounters[0].iter().position(|m| *m == (p, true)).unwrap()
      });
      assert!(positions.windows(2).all(|p| p[0] < p[1]));
      assert_eq!(roots.len(), 2);
      assert!(roots[0].0 && !roots[1].0);
      assert_eq!(roots[0].1, roots[1].1);
      let task_transactions = &observed.observation.transactions[roots[0].2..roots[1].2];
      assert!(
        task_transactions
          .iter()
          .filter(|(kind, _, p)| *kind == "rollback" && *p == Some(Phase::Task))
          .count()
          >= 2,
        "nested rollback evidence belongs to the selected target Task, not another Actor"
      );
      let task_operations = &observed.observation.operations[roots[0].3..roots[1].3];
      assert!(
        task_operations
          .iter()
          .any(|op| op.method == "put" && op.phase == Some(Phase::Task) && !op.observer),
        "the selected target Task performs real tentative writes"
      );
    } else {
      assert!(
        encounters.is_empty() && roots.is_empty(),
        "Deadline return is Control-only, not an Actor turn"
      );
    }
    let usage = polkadot_sdk::sp_io::TestExternalities::new(Storage {
      top: observed.top.clone(),
      ..Default::default()
    })
    .execute_with(|| {
      let after = Actors::active_actor_state(89).unwrap();
      let old = before.run_state.as_ref().unwrap();
      let run = after.run_state.as_ref().unwrap();
      assert_eq!(after.identity.cycle_nonce, 0);
      assert!(!after.hot.pending_signal);
      assert_eq!(run.contract_authority, old.contract_authority);
      assert_eq!(run.last_committed_step_block, old.last_committed_step_block);
      assert_eq!((run.cycle_nonce, run.cursor), (1, 1));
      assert_eq!(after.contract, before.contract);
      let mut fees = Vec::new();
      let mut failed = 0;
      let mut suspended = 0;
      let mut continued = 0;
      let mut effect_receipts = polkadot_sdk::frame_support::weights::Weight::zero();
      let mut distributed = 0;
      for record in System::events() {
        match record.event {
          crate::RuntimeEvent::Actors(Event::ActionFeeCharged {
            actor_id,
            fee,
            actual_effect_weight,
            ..
          }) => {
            effect_receipts = effect_receipts.saturating_add(actual_effect_weight);
            if actor_id == 89 {
              assert!(fee > 0);
              fees.push(fee);
            }
          }
          crate::RuntimeEvent::Actors(Event::StepFailed {
            actor_id: 89,
            retry_class,
            ..
          }) => {
            assert_eq!(retry_class, RetryClass::Temporary);
            failed += 1;
          }
          crate::RuntimeEvent::Actors(Event::CycleSuspended {
            actor_id: 89,
            reason,
            ..
          }) => {
            assert_eq!(reason, SuspensionReason::Temporary);
            suspended += 1;
          }
          crate::RuntimeEvent::Actors(Event::CycleContinued { actor_id: 89, .. }) => continued += 1,
          crate::RuntimeEvent::Actors(Event::TransferExecuted {
            actor_id,
            asset: Asset::Native,
            amount,
            ..
          }) if actor_id == primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID => {
            distributed += amount
          }
          crate::RuntimeEvent::Actors(
            Event::PipelineFeeCharged { actor_id: 89, .. }
            | Event::CycleStarted { actor_id: 89, .. }
            | Event::LiquidityAdded { actor_id: 89, .. }
            | Event::StepSkipped { actor_id: 89, .. }
            | Event::CycleSummary { actor_id: 89, .. }
            | Event::ActorClosed { actor_id: 89, .. },
          ) => panic!("failed continuation must preserve prefix and paid rights"),
          _ => {}
        }
      }
      let expected_fee = if case.attempt { 19_195_368 } else { 0 };
      if case.attempt {
        assert_eq!(fees, [expected_fee]);
        assert_eq!(
          (failed, suspended, continued),
          (1, 1, u32::from(case.before_failures > 0))
        );
        assert_eq!(
          run.unsuccessful_attempts_at_cursor,
          case.before_failures + 1
        );
        assert_eq!(after.hot.cycle_state, CycleState::Suspended);
        assert_eq!(run.suspension, Some(SuspensionReason::Temporary));
        let handle = Actors::deadline_handles(89).unwrap();
        assert_eq!(handle.key, WakeupKey::Block(run.eligible_at));
        assert!(Actors::service_nodes(89).is_none());
      } else {
        assert!(fees.is_empty());
        assert_eq!((failed, suspended, continued), (0, 0, 0));
        assert_eq!(run.encode(), old.encode());
        assert!(Actors::deadline_handles(89).is_none());
        assert_eq!(
          Actors::service_nodes(89).unwrap().eligible_from,
          case.block + 1
        );
      }
      assert_eq!(
        balances[0] - Assets::balance(&fixture.payer, fixture.native),
        expected_fee
      );
      assert_eq!(Assets::balance(&fixture.payer, fixture.b), balances[1]);
      assert_eq!(Assets::balance(&fixture.payer, fixture.lp), balances[2]);
      assert_eq!(
        Assets::balance(&fixture.sink, fixture.native) + distributed - fixture.sink_before,
        expected_fee
      );
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
      assert_eq!(
        Actors::actor_state_hold(89).unwrap().breakdown,
        Actors::actor_cost_quote(89)
          .unwrap()
          .actor_state_hold
          .breakdown
      );
      assert!(Actors::block_resource_state().is_none());
      let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
      assert!(!telemetry.optional_actor_work_halted());
      let usage = telemetry.usage();
      let limits = telemetry.budget().limits();
      assert_eq!(usage.actor_effect_used(), effect_receipts);
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
      usage
    });
    verify_original_and_compact(&parent, parent_root, &block, &observed, &compact);
    reports.push(serde_json::json!({"stage": case.name, "block": case.block, "attempt": case.attempt,
      "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root), "taskRoots": format!("{roots:?}"),
      "targetPhases": format!("{phases:?}"), "operations": observed.observation.operations.len(), "sdkDb": observed.db,
      "witnessBytes": observed.proof.encoded_size(), "compactBytes": compact.encoded_size(),
      "actorEffects": [usage.actor_effect_used().ref_time(), usage.actor_effect_used().proof_size()],
      "defaultNativeWasmTopEqual": true, "economicRollback": case.attempt, "physicalPricingQualified": false,
    }));
  }
  std::fs::write(
    output.join("report.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "stages": reports, "paidPrefixPreserved": true, "originalCompactReplayQualified": true,
      "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
    }))
    .unwrap(),
  )
  .unwrap();
}
