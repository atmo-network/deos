//! Tiny public-history controls for Running continuation, not throughput collection.
use super::*;

#[test]
fn two_step_public_history_preserves_paid_pipeline() {
  run_uniform_public_history(&[2], FundingHorizon::BlockBuffer);
}

#[test]
fn repeated_step_lengths_have_complete_manual_cycle_funding() {
  run_uniform_public_history(&[4, 8, 12], FundingHorizon::CompleteManualCycle);
}

fn run_uniform_public_history(lengths: &[u32], funding: FundingHorizon) {
  let mut cases = Vec::new();
  for &length in lengths {
    for workload in [Workload::Transfer, Workload::RouterFourPredicates] {
      for class in [ActorType::System, ActorType::User] {
        let profile = Baseline {
          workload,
          actors: 2,
          ..Baseline::full(class, false)
        };
        let one = workload.steps();
        let steps = vec![one[0].clone(); length as usize].try_into().unwrap();
        let fixture = prepare_baseline_with_funding(&[], profile, steps, funding);
        assert_eq!(fixture.setup["contractSteps"], length);
        let setup = fixture.setup;
        if funding == FundingHorizon::CompleteManualCycle {
          TestExternalities::new(fixture.actors.storage.clone()).execute_with(|| {
            for (id, member) in &fixture.members {
              let principal = if workload == Workload::Transfer {
                u128::from(length) * crate::EXISTENTIAL_DEPOSIT
              } else {
                0
              };
              let actions =
                u128::from(length) * member.quote.maximum_next_action_fee.maximum_effect_fee;
              let row = &setup["fullCycleFunding"][id.to_string()];
              assert_eq!(row["nativePrincipal"], principal.to_string());
              assert_eq!(row["actionUpperTotal"], actions.to_string());
              let minimum = principal
                + actions
                + member.quote.prospective_pipeline_fee.unwrap().total_fee
                + member.quote.actor_state_hold.total
                + <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
                + crate::EXISTENTIAL_DEPOSIT;
              assert!(
                total(&member.account) >= minimum,
                "complete post-Trigger funding"
              );
              if workload == Workload::RouterFourPredicates {
                assert_eq!(
                  Assets::balance(FOREIGN, &member.account),
                  u128::from(length) * 10 * crate::EXISTENTIAL_DEPOSIT + 10
                );
              }
            }
          });
        }
        assert!(
          fixture
            .actors
            .actor_profiles
            .values()
            .all(|p| p.step_count == length)
        );
        let members = fixture.members;
        let actors = fixture.actors;
        let mut fifo = actors.actor_ids.iter().copied().collect::<VecDeque<_>>();
        let mut state = actors.storage;
        let mut parent = parent_header_for(state.clone(), &[], 2);
        let mut paid = BTreeMap::new();
        let mut rows = Vec::new();
        for index in 0..length {
          let final_step = index + 1 == length;
          let block = 3 + index;
          if index > 0 {
            TestExternalities::new(state.clone()).execute_with(|| {
              for id in &actors.actor_ids {
                let run = Actors::actor_run_state(*id).unwrap();
                assert_eq!(run.contract_authority, paid[id]);
                assert_eq!((run.cursor, run.eligible_at), (index, block));
              }
            });
          }
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
          assert_eq!(authored.metrics.actor_steps, 2);
          assert_eq!(authored.metrics.distinct_actors, 2);
          assert_eq!(authored.metrics.non_successful_steps, 0);
          assert_eq!(authored.metrics.user_dispatch, Weight::zero());
          assert_eq!(
            authored.metrics.opening_steps,
            if index == 0 { 2 } else { 0 }
          );
          assert_eq!(authored.metrics.final_steps, if final_step { 2 } else { 0 });
          assert_eq!(
            authored.metrics.completed_cycles,
            if final_step { 2 } else { 0 }
          );
          let accounting = inspect_block(
            &[],
            &authored,
            &members,
            &mut fifo,
            (actors.next_control_maximum, actors.next_effect_maximum),
            final_step,
            StepPosition {
              workload,
              index,
              completes_cycle: final_step,
            },
          );
          assert_eq!(accounting["successfulActions"], 2);
          if index > 0 {
            assert_eq!(
              accounting["collectedFees"][1], "0",
              "Running never recharges Pipeline"
            );
          }
          TestExternalities::new(authored.post_state.clone()).execute_with(|| {
            for id in &actors.actor_ids {
              let hot = Actors::actor_hot(*id).unwrap();
              let identity = Actors::actor_identity(*id).unwrap();
              assert!(!hot.pending_signal);
              assert_eq!(identity.cycle_nonce, u64::from(final_step));
              if !final_step {
                assert_eq!(hot.cycle_state, CycleState::Running);
                let run = Actors::actor_run_state(*id).unwrap();
                assert_eq!(
                  (run.cursor, run.eligible_at, run.last_committed_step_block),
                  (index + 1, block + 1, Some(block))
                );
                let node = Actors::service_nodes(*id).unwrap();
                // Service ingress floor is not the paid Run's next-Step clock.
                assert_eq!(node.eligible_from, 3);
                assert_eq!(node.last_considered, block);
                if index == 0 {
                  paid.insert(*id, run.contract_authority);
                } else {
                  assert_eq!(run.contract_authority, paid[id]);
                }
              } else {
                assert_eq!(hot.cycle_state, CycleState::Idle);
                assert!(Actors::actor_run_state(*id).is_none());
                assert!(Actors::service_nodes(*id).is_none());
                if funding == FundingHorizon::CompleteManualCycle {
                  assert!(
                    total(&members[id].account)
                      >= members[id].quote.actor_state_hold.total
                        + <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
                        + crate::EXISTENTIAL_DEPOSIT
                  );
                  if workload == Workload::RouterFourPredicates {
                    assert!(Assets::balance(FOREIGN, &members[id].account) >= 10);
                  }
                }
              }
            }
          });
          rows.push(serde_json::json!({"block": block, "index": index,
          "phase": if index == 0 { "Opening-to-Running" } else if final_step { "Running-to-completion" } else { "Running-progress" },
          "steps": authored.metrics.actor_steps, "actors": authored.metrics.distinct_actors,
          "cycles": authored.metrics.completed_cycles,
          "control": [authored.metrics.actor_control.ref_time(), authored.metrics.actor_control.proof_size()],
          "effect": [authored.metrics.actor_effect.ref_time(), authored.metrics.actor_effect.proof_size()],
          "accounting": accounting}));
          parent = authored.block.header;
          state = authored.post_state;
        }
        assert!(fifo.is_empty());
        cases.push(
          serde_json::json!({"workload": format!("{workload:?}"), "class": format!("{class:?}"),
        "actors": 2, "contractSteps": length, "setup": setup, "blocks": rows}),
        );
      }
    }
  }
  println!(
    "ACTOR_RUNNING_SOURCE_V1 {}",
    serde_json::json!({"nativeOnly": true,
    "notCapacity": true, "fullBlocks": 4 * lengths.iter().sum::<u32>(),
    "actions": 8 * lengths.iter().sum::<u32>(), "cases": cases})
  );
}
