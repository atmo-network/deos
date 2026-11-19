//! Genuine fresh/paid-Running User successors: one LP Task and membership retirement.

use super::*;

struct Case {
  actor: u64,
  block: u32,
  running: bool,
  parent_sha: &'static str,
  post_sha: &'static str,
  block_sha: &'static str,
  parent_root: &'static str,
}
#[test]
#[ignore = "exact inherited-genesis successor inputs, not reconstructed Actor state"]
fn retained_fresh_opening_reaches_retirement() {
  qualify(Case {
    actor: 87,
    block: 187_466,
    running: false,
    parent_sha: "216baf777b9edaf3e9f10a1864daa8912815573d4dd5a183443d70cee907974e",
    post_sha: "811ab7822799a0eb8d6eeeb5f349d232b92f65f562eb10b3e852b06830b15187",
    block_sha: "bdcc8d587704126705e13626971a6338f1e5d9a6a150247e90af3d3cfaddd666",
    parent_root: "895d730499cccb26f84fdaa434a60a3ec4ed48fdec0e21c62a32618f2819dfa2",
  });
}
#[test]
#[ignore = "exact paid Running successor, no new Opening or reconstructed authority"]
fn retained_paid_running_k_reaches_retirement() {
  qualify(Case {
    actor: 88,
    block: 187_565,
    running: true,
    parent_sha: "b4702064d9831c8dd0c613fcdc0caf6142ab353d02ff69bf364a3de99e61399a",
    post_sha: "d20528728b140fca4dd16ba0ae44959d53bd5fc9d23870fba1f35a9c7620f615",
    block_sha: "bbe1019879951635342d5a3a7cd0ed0fb4bccabeec72acc26d19d0c1e01a1bdc",
    parent_root: "3e2c1641390af2f102c54ea63729e89f6b1d77b7d9fef8682291de52ab7c9cc9",
  });
}
fn qualify(case: Case) {
  let parent = Storage {
    top: State::decode_all(&mut input("DEOS_ADMITTED_SOURCE_PARENT", case.parent_sha).as_slice())
      .unwrap(),
    ..Default::default()
  };
  let expected =
    State::decode_all(&mut input("DEOS_ADMITTED_SOURCE_POST", case.post_sha).as_slice()).unwrap();
  let block =
    crate::Block::decode_all(&mut input("DEOS_ADMITTED_SOURCE_BLOCK", case.block_sha).as_slice())
      .unwrap();
  let parent_root = Layout::trie_root(parent.top.clone());
  assert_eq!(format!("{parent_root:x}"), case.parent_root);
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
  let (fixture, before, native_before, paired_before, lp_before) =
    polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
      assert_eq!(System::block_number(), case.block - 1);
      let actor = Actors::active_actor_state(case.actor).unwrap();
      assert_eq!(actor.identity.cycle_nonce, 0);
      if case.running {
        assert_eq!(actor.hot.cycle_state, CycleState::Running);
        assert!(!actor.hot.pending_signal);
        let run = actor.run_state.as_ref().unwrap();
        assert_eq!(
          (
            run.cycle_nonce,
            run.cursor,
            run.unsuccessful_attempts_at_cursor
          ),
          (1, 1, 0)
        );
        assert!(run.suspension.is_none() && run.eligible_at <= case.block);
        assert_eq!(actor.contract.steps.len(), 2);
        assert_eq!(
          actor.contract.steps[1].on_error,
          pallet_deos_actors::StepErrorPolicy::RetryLater { max_attempts: 2 }
        );
      } else {
        assert!(actor.hot.pending_signal && actor.run_state.is_none());
        assert_eq!(actor.contract.steps.len(), 1);
      }
      let native = <Runtime as Config>::FeeNativeAssetId::get();
      let paired = Asset::Local(0);
      let lp = Asset::Local(
        polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((native, paired))
          .unwrap()
          .lp_token,
      );
      let sink = <Runtime as Config>::FeeSink::get();
      let payer = actor.identity.sovereign_account.clone();
      let fixture = Fixture {
        actor: case.actor,
        payer: payer.clone(),
        native,
        a: native,
        b: paired,
        lp,
        amount: crate::UNIT,
        spend: crate::UNIT,
        sink_before: Assets::balance(&sink, native),
        sink,
        // Expected new Opening charge, never a replacement for paid Run authority.
        pipeline_fee: if case.running {
          0
        } else {
          Actors::actor_cost_quote(case.actor)
            .unwrap()
            .prospective_pipeline_fee
            .unwrap()
            .total_fee
        },
        inspection_predecessors: None,
        paid_prefix: None,
      };
      (
        fixture,
        actor,
        Assets::balance(&payer, native),
        Assets::balance(&payer, paired),
        Assets::balance(&payer, lp),
      )
    });
  let output = std::path::PathBuf::from(std::env::var("DEOS_SERVICE_OBSERVATION_OUTPUT").unwrap());
  std::fs::create_dir(&output).unwrap();
  let observed = run_call(&parent, &fixture, true, false, block_notice, true, || {
    import(&block)
  });
  let plain = run_call(&parent, &fixture, false, false, block_notice, true, || {
    import(&block)
  });
  let compact = observed
    .proof
    .to_compact_proof::<Blake2Hasher>(parent_root)
    .unwrap();
  export_case(&output, "block", parent_root, &observed, &compact);
  std::fs::write(output.join("plain-post.scale"), plain.top.encode()).unwrap();
  assert_eq!(
    observed.top, expected,
    "notices preserve the qualified default-native post-state"
  );
  assert_eq!(observed.top, plain.top);
  assert_eq!(observed.root, plain.root);
  assert_eq!(observed.root, *block.header.state_root());
  assert_eq!(observed.proof, plain.proof);
  assert_eq!(observed.db, plain.db);
  assert_eq!(observed.observation.markers, plain.observation.markers);
  assert!(!observed.observation.fault_applied);
  let encounters = observed
    .observation
    .markers
    .split_inclusive(|m| *m == (Phase::Encounter, false))
    .filter(|markers| {
      markers.iter().any(|(phase, entering)| {
        matches!(
          (*phase, *entering),
          (Phase::Booking { actor, .. }, true) if actor == case.actor
        )
      })
    })
    .collect::<Vec<_>>();
  assert_eq!(
    encounters.len(),
    1,
    "one actual successful target encounter"
  );
  let positions = [
    Phase::Preflight,
    Phase::Task,
    Phase::Retirement,
    Phase::Settlement,
    Phase::Collection,
    Phase::Authentication,
  ]
  .map(|phase| {
    assert_eq!(
      encounters[0]
        .iter()
        .filter(|marker| **marker == (phase, true))
        .count(),
      1
    );
    assert!(encounters[0].contains(&(phase, false)));
    encounters[0]
      .iter()
      .position(|marker| *marker == (phase, true))
      .unwrap()
  });
  assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
  let (pipeline_fee, action_fee, lp_minted, usage) =
    polkadot_sdk::sp_io::TestExternalities::new(Storage {
      top: observed.top.clone(),
      ..Default::default()
    })
    .execute_with(|| {
      let mut pipelines = Vec::new();
      let mut actions = Vec::new();
      let mut outputs = Vec::new();
      let mut summaries = Vec::new();
      for record in System::events() {
        match record.event {
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::PipelineFeeCharged {
            actor_id,
            fee,
          }) if actor_id == case.actor => pipelines.push(fee),
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::ActionFeeCharged {
            actor_id,
            fee,
            actual_effect_weight,
            ..
          }) if actor_id == case.actor => actions.push((fee, actual_effect_weight)),
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::LiquidityAdded {
            actor_id,
            lp_minted,
            ..
          }) if actor_id == case.actor => outputs.push(lp_minted),
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::CycleSummary {
            actor_id,
            cycle_nonce,
            ..
          }) if actor_id == case.actor => summaries.push(cycle_nonce),
          crate::RuntimeEvent::Actors(pallet_deos_actors::Event::CycleStarted {
            actor_id, ..
          }) if actor_id == case.actor && case.running => {
            panic!("paid Running K cannot open another Pipeline")
          }
          crate::RuntimeEvent::Actors(
            pallet_deos_actors::Event::StepSkipped { actor_id, .. }
            | pallet_deos_actors::Event::StepFailed { actor_id, .. }
            | pallet_deos_actors::Event::ActorClosed { actor_id, .. },
          ) if actor_id == case.actor => panic!("unexpected terminal result: {:?}", record.event),
          _ => {}
        }
      }
      let pipeline_fee = if case.running {
        assert!(
          pipelines.is_empty(),
          "Running reuses already paid authority"
        );
        0
      } else {
        assert_eq!(pipelines, [fixture.pipeline_fee]);
        pipelines[0]
      };
      assert_eq!(actions.len(), 1);
      assert!(actions[0].0 > 0);
      assert_eq!(outputs.len(), 1);
      assert!(outputs[0] > 0);
      assert_eq!(summaries, [1]);
      let after = Actors::active_actor_state(case.actor).unwrap();
      assert_eq!(after.identity.cycle_nonce, 1);
      assert_eq!(after.hot.cycle_state, CycleState::Idle);
      assert!(after.run_state.is_none() && !after.hot.pending_signal);
      assert_eq!(before.contract.encode(), after.contract.encode());
      assert_eq!(
        native_before - Assets::balance(&fixture.payer, fixture.native),
        crate::UNIT + pipeline_fee + actions[0].0
      );
      assert_eq!(
        paired_before - Assets::balance(&fixture.payer, fixture.b),
        crate::UNIT
      );
      assert_eq!(
        Assets::balance(&fixture.payer, fixture.lp) - lp_before,
        outputs[0]
      );
      assert_eq!(
        Assets::balance(&fixture.sink, fixture.native) - fixture.sink_before,
        pipeline_fee + actions[0].0
      );
      assert!(Actors::block_resource_state().is_none());
      let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
      assert!(!telemetry.optional_actor_work_halted());
      let usage = telemetry.usage();
      assert_eq!(usage.actor_effect_used(), actions[0].1);
      assert!(
        usage
          .actor_control_used()
          .all_lte(telemetry.budget().limits().actor_control())
      );
      assert!(
        usage
          .actor_effect_used()
          .saturating_add(usage.user_dispatch_used())
          .all_lte(telemetry.budget().limits().shared_economic())
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
      let last = pallet_deos_actors::BlockResourceState::<crate::BlockNumber>::decode_all(
        &mut bytes.as_slice(),
      )
      .unwrap();
      assert_eq!(last.outstanding_reservations(), 0);
      assert_eq!(last.usage(), usage);
      (pipeline_fee, actions[0].0, outputs[0], usage)
    });
  verify_original_and_compact(&parent, parent_root, &block, &observed, &compact);
  let report = serde_json::json!({
    "parentRoot": format!("{parent_root:?}"), "postRoot": format!("{:?}", observed.root),
    "actor": case.actor, "freshOpening": !case.running, "paidRunning": case.running, "membershipRetirement": true,
    "pipelineFee": pipeline_fee.to_string(), "actionFee": action_fee.to_string(), "lpMinted": lp_minted.to_string(),
    "actorEffectUsed": [usage.actor_effect_used().ref_time(), usage.actor_effect_used().proof_size()],
    "rawWitnessBytes": observed.proof.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
    "defaultNativeWasmRootTransfer": true, "actualRetryTransitionQualified": false, "completeKMaximumQualified": false, "physicalPricingQualified": false,
  });
  std::fs::write(
    output.join("report.json"),
    serde_json::to_vec_pretty(&report).unwrap(),
  )
  .unwrap();
  println!("TERMINAL_SUCCESSOR_K {report}");
}
