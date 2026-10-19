//! One prepared-native heterogeneous economic workflow, not capacity or physical pricing.
use super::*;

#[test]
fn foreign_swap_then_native_delivery_completes_once() {
  let mut steps = Workload::RouterFourPredicates.steps().to_vec();
  steps.push(Workload::Transfer.steps()[0].clone());
  let steps: ContractSteps<Runtime> = steps.try_into().unwrap();
  let profile = Baseline {
    workload: Workload::RouterFourPredicates,
    actors: 1,
    ..Baseline::full(ActorType::User, false)
  };
  let mut fixture = prepare_baseline_with_steps(&[], profile, steps);
  let id = fixture.actors.actor_ids[0];
  // The profile has different Task families and predicate geometry per Step.
  fixture.actors.actor_profiles.get_mut(&id).unwrap().task = None;
  let member = &fixture.members[&id];
  let extra_native = crate::EXISTENTIAL_DEPOSIT.checked_mul(1_000).unwrap();
  let mut prepared = TestExternalities::new(fixture.actors.storage);
  let (initial, trigger_fee, contract) = prepared.execute_with(|| {
    add_balance(&member.account, extra_native);
    let recipient = super::super::super::common::BOB;
    if let Some(shortfall) = crate::EXISTENTIAL_DEPOSIT.checked_sub(total(&recipient)) {
      if shortfall > 0 {
        add_balance(&recipient, shortfall);
      }
    }
    let fees = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, fee, .. })
          if actor_id == id =>
        {
          Some(fee)
        }
        _ => None,
      })
      .collect::<Vec<_>>();
    assert_eq!(fees, [member.quote.prospective_trigger_fee.unwrap().fee]);
    let state = Actors::active_actor_state(id).unwrap();
    assert!(state.hot.pending_signal);
    assert_eq!(state.identity.last_control_mutation_block, 1);
    assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 3);
    assert!(state.run_state.is_none());
    assert_eq!(
      state.contract.steps[0]
        .precondition
        .as_ref()
        .unwrap()
        .evaluation_units(),
      4
    );
    assert!(state.contract.steps[1].precondition.is_none());
    (
      [
        total(&member.account),
        Assets::balance(FOREIGN, &member.account),
        total(&recipient),
      ],
      fees[0],
      state.contract,
    )
  });
  prepared.commit_all().unwrap();
  let mut storage = prepared.execute_with(current_top_storage);
  let [native_before, foreign_before, recipient_before] = initial;
  let mut parent = parent_header_for(storage.clone(), &[], 2);
  let mut fifo = VecDeque::from([id]);
  let mut paid_authority = None;
  let mut foreign_input = None;
  let mut fees_in_window = 0u128;
  let mut rows = Vec::new();
  for number in 3..=5 {
    if number == 4 {
      TestExternalities::new(storage.clone()).execute_with(|| {
        assert_eq!(
          Some(Actors::actor_run_state(id).unwrap().contract_authority),
          paid_authority
        );
      });
    }
    let authored = author_complete_block_after(
      storage,
      &[],
      &parent,
      number,
      UserDemand::ActorOnly,
      &fixture.actors.signer,
      0,
      &fixture.actors.actor_profiles,
    );
    assert_complete_block_replays_natively(&authored, &[]);
    let active = number != 5;
    assert_eq!(authored.metrics.actor_steps, u32::from(active));
    assert_eq!(authored.metrics.distinct_actors, u32::from(active));
    assert_eq!(authored.metrics.non_successful_steps, 0);
    assert_eq!(authored.metrics.user_calls, 0);
    assert_eq!(authored.metrics.user_dispatch, Weight::zero());
    assert_eq!(authored.metrics.swap_out_steps, u32::from(number == 3));
    assert_eq!(authored.metrics.transfer_steps, u32::from(number == 4));
    assert_eq!(authored.metrics.completed_cycles, u32::from(number == 4));
    let accounting = inspect_block(
      &[],
      &authored,
      &fixture.members,
      &mut fifo,
      (
        fixture.actors.next_control_maximum,
        fixture.actors.next_effect_maximum,
      ),
      number >= 4,
      StepPosition {
        workload: if number == 3 {
          Workload::RouterFourPredicates
        } else {
          Workload::Transfer
        },
        index: u32::from(number != 3),
        completes_cycle: number != 3,
      },
    );
    assert_eq!(accounting["successfulActions"], u32::from(active));
    let charged = accounting["collectedFees"]
      .as_array()
      .unwrap()
      .iter()
      .map(|value| value.as_str().unwrap().parse::<u128>().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(charged.len(), 3);
    assert_eq!(
      charged[0], 0,
      "Manual occurrence was paid before the window"
    );
    assert_eq!(charged[1] > 0, number == 3, "one Pipeline payment only");
    assert_eq!(
      charged[2] > 0,
      active,
      "one actual Action fee per invoked Task"
    );
    fees_in_window = charged
      .iter()
      .try_fold(fees_in_window, |sum, fee| sum.checked_add(*fee))
      .unwrap();
    if number == 3 {
      let route = &accounting["routerOutcomes"][id.to_string()];
      let input = route["amountIn"].as_str().unwrap().parse::<u128>().unwrap();
      assert!(input > 0 && input <= router_workload::input_cap());
      assert_eq!(route["amountOut"], crate::EXISTENTIAL_DEPOSIT.to_string());
      foreign_input = Some(input);
    } else {
      assert!(accounting["routerOutcomes"].as_object().unwrap().is_empty());
    }
    let input = foreign_input.unwrap();
    TestExternalities::new(authored.post_state.clone()).execute_with(|| {
      let state = Actors::active_actor_state(id).unwrap();
      assert_eq!(state.contract, contract);
      assert_eq!(generation(id), member.generation);
      assert!(!state.hot.pending_signal);
      assert_eq!(
        Assets::balance(FOREIGN, &member.account),
        foreign_before.checked_sub(input).unwrap()
      );
      for asset in WITNESSES {
        assert_eq!(Assets::balance(asset, &member.account), 10);
      }
      let delivered = if number >= 4 {
        crate::EXISTENTIAL_DEPOSIT
      } else {
        0
      };
      assert_eq!(
        total(&super::super::super::common::BOB),
        recipient_before.checked_add(delivered).unwrap()
      );
      assert_eq!(
        total(&member.account),
        native_before
          .checked_add(crate::EXISTENTIAL_DEPOSIT)
          .unwrap()
          .checked_sub(delivered)
          .unwrap()
          .checked_sub(fees_in_window)
          .unwrap()
      );
      assert_eq!(state.identity.cycle_nonce, u64::from(number >= 4));
      if number == 3 {
        assert_eq!(state.hot.cycle_state, CycleState::Running);
        let run = state.run_state.unwrap();
        assert_eq!(
          (run.cursor, run.eligible_at, run.last_committed_step_block),
          (1, 4, Some(3))
        );
        paid_authority = Some(run.contract_authority);
        let node = Actors::service_nodes(id).unwrap();
        assert_eq!((node.eligible_from, node.last_considered), (3, 3));
      } else {
        assert_eq!(state.hot.cycle_state, CycleState::Idle);
        assert!(state.run_state.is_none());
        assert!(Actors::service_nodes(id).is_none());
        assert!(Actors::actor_processes(id).unwrap().residence.is_none());
      }
    });
    rows.push(serde_json::json!({
      "block": number, "targetSteps": authored.metrics.actor_steps, "targetActors": authored.metrics.distinct_actors,
      "targetCompletedCycles": authored.metrics.completed_cycles,
      "globalBookedControl": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
      "globalBookedEffect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
      "globalBookedUserDispatch": [authored.metrics.user_dispatch.ref_time(), authored.metrics.user_dispatch.proof_size()],
      "accounting": accounting,
    }));
    parent = authored.block.header;
    storage = authored.post_state;
  }
  assert!(fifo.is_empty());
  assert_eq!(rows.len(), 3);
  println!(
    "ACTORS_H3_ROUTE_DELIVERY_V1 {}",
    serde_json::json!({
      "schema": 1, "scope": "prepared ordinary-composition native full Executive; explicit model identity capacity, Root-enabled permission, synthetic relay; not default authority/public genesis/Wasm/PVF",
      "identityCapacityModel": cfg!(feature = "benchmark-authority-scaffolding"),
      "runtimeBenchmarks": cfg!(feature = "runtime-benchmarks"),
      "optionalAuthoritySupplements": cfg!(all(feature = "benchmark-authority-scaffolding", feature = "runtime-benchmarks")),
      "productionCapacityQualified": false, "productionArtifactCompatible": false,
      "population": 1, "actor": id, "birthBlock": 1, "signalBlock": 2, "firstBlock": 3, "lastBlock": 5,
      "tasks": ["SwapOut", "Transfer"], "predicateUnits": [4, 0],
      "usefulWorkflowSucceeded": true, "foreignAsset": FOREIGN, "foreignInput": foreign_input.unwrap().to_string(),
      "foreignInputCap": router_workload::input_cap().to_string(), "nativeDelivered": crate::EXISTENTIAL_DEPOSIT.to_string(),
      "creationFeeObserved": fixture.setup["observedCreationFees"], "triggerFeeObservedBeforeWindow": trigger_fee.to_string(),
      "pipelineAndActionFeesObserved": fees_in_window.to_string(),
      "routerFeeScope": "Foreign trading fee is in the actual first-block Router outcome, separate from native Actor fees",
      "extraNativePrefunding": extra_native.to_string(), "fundingScope": "existing BlockBuffer preparation plus finite top-up, not a complete fee/funding upper",
      "recipientPreparedBeforeWindow": true, "ordinaryTransactionFees": "excluded from direct setup; no signed user demand in observation",
      "signalToCompletionBlocks": 2, "interStepGapBlocks": 1, "paidRunningAuthorityPreserved": true,
      "terminalNoRepeatBlock": 5, "blocks": rows, "physicalMetrics": null,
    })
  );
}
