//! Native isolated Actors preparation, not a production or throughput certificate.

use super::{actors_integration_tests as fixtures, common};
use crate::{Actors, Balances, BlockNumber, Runtime, RuntimeEvent, RuntimeOrigin, System};
use pallet_deos_actors::{
  ActorType, BlockResourceBudget, BlockResourceError, BlockResourcePhase, CompletionPolicy, Event,
  FundingSourcePolicy, Mutability, StepControlWeightContext, StepControlWeightProvider, Trigger,
  WeightInfo,
};
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{Get, Hooks},
  weights::Weight,
};
use std::cell::Cell;

const REFERENCE_BLOCK: Weight = Weight::from_parts(2_000_000_000_000, 10_485_760);

thread_local! {
  static ISOLATED_BLOCK: Cell<Option<BlockNumber>> = const { Cell::new(None) };
}

struct IsolatedBlock;

impl Drop for IsolatedBlock {
  fn drop(&mut self) {
    ISOLATED_BLOCK.set(None);
  }
}

fn with_isolated_block<R>(block: BlockNumber, run: impl FnOnce() -> R) -> R {
  assert!(
    ISOLATED_BLOCK.get().is_none(),
    "isolated scopes cannot nest"
  );
  ISOLATED_BLOCK.set(Some(block));
  let _guard = IsolatedBlock;
  run()
}

/// The production host has no override. Tests opt in per thread and per block, without storage.
pub(crate) fn prepass_budget(
  now: BlockNumber,
  configured: BlockResourceBudget,
) -> Result<BlockResourceBudget, BlockResourceError> {
  let Some(block) = ISOLATED_BLOCK.get() else {
    return Ok(configured);
  };
  if now != block || configured.maximum_block() != REFERENCE_BLOCK {
    return Err(BlockResourceError::InvalidLimits);
  }
  BlockResourceBudget::from_settled_prefix(
    REFERENCE_BLOCK,
    configured.fixed_envelope(),
    Weight::zero(),
    Weight::zero(),
    1,
    3,
  )
}

fn transfers() -> Vec<pallet_deos_actors::ActorId> {
  System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(Event::TransferExecuted { actor_id, .. }) => Some(actor_id),
      RuntimeEvent::Actors(Event::StepFailed { .. } | Event::StepSkipped { .. }) => {
        panic!("isolated Transfer preparation requires useful successful effects")
      }
      _ => None,
    })
    .collect()
}

#[test]
fn isolated_budget_scope_restores_production_and_rejects_wrong_block() {
  use pallet_deos_actors::BlockResourceBudgetProvider;
  type Host = crate::configs::BlockResourceBudgetValue;
  common::synthetic_actor_test_ext().execute_with(|| {
    let configured = Host::get();
    assert_eq!(Host::for_prepass(2, configured), Ok(configured));
    with_isolated_block(2, || {
      assert_eq!(
        Host::for_prepass(3, configured),
        Err(BlockResourceError::InvalidLimits)
      );
      assert_eq!(
        Host::for_prepass(2, configured).unwrap().fixed_envelope(),
        Weight::zero()
      );
    });
    assert_eq!(Host::for_prepass(2, configured), Ok(configured));
    assert!(
      std::panic::catch_unwind(|| with_isolated_block(2, || panic!("scope unwind witness")))
        .is_err()
    );
    assert_eq!(Host::for_prepass(2, configured), Ok(configured));
  });
}

#[test]
fn isolated_liquidity_borrowing_and_control_refusal_preserve_order() {
  fixtures::assert_paired_base_refusal(
    &[
      fixtures::BaseRefusalFrontier::EffectProofSize,
      fixtures::BaseRefusalFrontier::ControlProofSize,
    ],
    || {
      with_isolated_block(2, || {
        prepass_budget(2, crate::configs::BlockResourceBudgetValue::get()).unwrap()
      })
    },
    |block| {
      with_isolated_block(block, || {
        Actors::actor_prepass(RuntimeOrigin::none()).unwrap()
      })
    },
  );
}

#[test]
fn isolated_drain_refusal_preserves_fees_custody_and_next_block_recovery() {
  fixtures::assert_paired_drain_refusal(
    || {
      with_isolated_block(2, || {
        prepass_budget(2, crate::configs::BlockResourceBudgetValue::get()).unwrap()
      })
    },
    |block| {
      with_isolated_block(block, || {
        Actors::actor_prepass(RuntimeOrigin::none()).unwrap()
      })
    },
  );
}

#[test]
fn isolated_business_failure_is_charged_and_does_not_block_successor() {
  fixtures::assert_paired_business_failure(
    || {
      with_isolated_block(2, || {
        prepass_budget(2, crate::configs::BlockResourceBudgetValue::get()).unwrap()
      })
    },
    |block| {
      with_isolated_block(block, || {
        Actors::actor_prepass(RuntimeOrigin::none()).unwrap()
      })
    },
  );
}

#[test]
fn isolated_transfer_preparation_preserves_envelope_phases_and_recovery() {
  type Control = crate::configs::actor_config::RuntimeStepControlWeight;
  type Weights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  common::synthetic_actor_test_ext().execute_with(|| {
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(Actors::service_header().count, 0);
    System::set_block_number(1);
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let steps =
      fixtures::transfer_contract_steps(common::BOB, primitives::AssetKind::Native, amount);
    assert_eq!(steps.len(), 1);
    assert_eq!(
      steps[0].task,
      pallet_deos_actors::Task::Transfer {
        to: common::BOB,
        asset: primitives::AssetKind::Native,
        amount: pallet_deos_actors::AmountResolution::Fixed(amount),
      }
    );
    assert_eq!(
      steps[0].on_error,
      pallet_deos_actors::StepErrorPolicy::AbortCycle
    );
    let context = StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 0,
      predicate_evaluation_units: 0,
    };
    let control_maximum = Control::maximum_control_weight(context, &steps[0]).unwrap();
    let effect = Weights::task_transfer();
    assert!(Balances::free_balance(common::BOB) >= amount);
    assert!(!pallet_deos_actors::SovereignIndex::<Runtime>::contains_key(common::BOB));
    let ids = (0..128)
      .map(|_| {
        let id = fixtures::create_system(
          common::ALICE,
          fixtures::manual_schedule(),
          None,
          steps.clone(),
        );
        let identity = Actors::actor_identity(id).unwrap();
        assert_eq!(identity.actor_class.actor_type(), ActorType::System);
        assert_eq!(identity.mutability, Mutability::Mutable);
        assert_ne!(identity.sovereign_account, common::BOB);
        let head = pallet_deos_actors::ActorContractHeads::<Runtime>::get(id).unwrap();
        assert_eq!(head.header.trigger, Trigger::manual());
        assert_eq!(head.header.cooldown_blocks, 0);
        assert_eq!(head.header.window, None);
        assert_eq!(head.header.funding, FundingSourcePolicy::RuntimePolicy);
        assert_eq!(head.header.completion, CompletionPolicy::Persistent);
        assert_eq!(head.header.parked_balance_activation, None);
        assert_eq!(head.header.auto_close_at_cycle_nonce, None);
        assert_eq!(head.header.step_count, 1);
        assert_eq!(head.first_step, Some(steps[0].clone()));
        assert!(steps[0].precondition.is_none());
        assert_eq!(head.first_step_resources.unwrap().control, control_maximum);
        assert_eq!(head.first_step_resources.unwrap().effect, effect);
        assert!(!pallet_deos_actors::ActorContractTailChunks::<Runtime>::contains_key(id, 0));
        fixtures::fund_native(id, 1_000 * crate::UNIT);
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), id));
        assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 2);
        id
      })
      .collect::<Vec<_>>();
    assert_eq!(Actors::actor_identity_count(), ids.len() as u32);
    let recipient_before = Balances::free_balance(common::BOB);
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let sink_before = Balances::free_balance(&sink);
    let source_before = ids
      .iter()
      .map(|id| Balances::free_balance(Actors::actor_identity(*id).unwrap().sovereign_account))
      .collect::<Vec<_>>();
    let mut completed = 0usize;
    for block in 2..=3 {
      with_isolated_block(block, || {
        // Synthetic consensus context is outside Actor execution; no other runtime hooks run.
        System::set_block_number(block);
        System::reset_events();
        System::set_block_consumed_resources(Weight::zero(), 0);
        common::set_consensus_timestamp(
          u64::from(block) * primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS,
        );
        polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
          polkadot_sdk::cumulus_primitives_core::PersistedValidationData {
            max_pov_size: REFERENCE_BLOCK.proof_size() as u32,
            ..Default::default()
          },
        );
        assert_eq!(Actors::on_initialize(block), Weight::zero());
        let post = Actors::actor_prepass(RuntimeOrigin::none()).unwrap();
        let prepass = Actors::block_resource_state().unwrap();
        let budget = prepass.budget().unwrap();
        let limits = budget.limits();
        assert_eq!(budget.maximum_block(), REFERENCE_BLOCK);
        assert_eq!(budget.fixed_envelope(), Weight::zero());
        assert_eq!(
          limits.actor_control(),
          Weight::from_parts(666_666_666_666, 3_495_253)
        );
        assert_eq!(
          limits.shared_economic(),
          Weight::from_parts(1_333_333_333_334, 6_990_507)
        );
        assert_eq!(
          limits.actor_base_turn(),
          Weight::from_parts(666_666_666_667, 3_495_253)
        );
        assert_eq!(prepass.phase(), BlockResourcePhase::ExternalPhase);
        assert_eq!(prepass.outstanding_reservations(), 0);
        assert_eq!(prepass.usage().user_dispatch_used(), Weight::zero());
        assert!(
          prepass
            .usage()
            .actor_effect_used()
            .all_lte(limits.actor_base_turn())
        );
        let prepass_weight = prepass
          .usage()
          .actor_control_used()
          .saturating_add(prepass.usage().actor_effect_used());
        assert_eq!(post.actual_weight, Some(prepass_weight));
        let before_drain = transfers();
        let remaining = REFERENCE_BLOCK.checked_sub(&prepass_weight).unwrap();
        Actors::on_idle(block, remaining);
        let final_state = Actors::block_resource_state().unwrap();
        assert_eq!(final_state.budget(), Ok(budget));
        assert_eq!(final_state.phase(), BlockResourcePhase::Finalizable);
        assert_eq!(final_state.outstanding_reservations(), 0);
        assert!(!final_state.optional_actor_work_halted());
        assert_eq!(final_state.usage().user_dispatch_used(), Weight::zero());
        assert!(
          final_state
            .usage()
            .actor_control_used()
            .all_lte(limits.actor_control())
        );
        assert!(
          final_state
            .usage()
            .actor_effect_used()
            .all_lte(limits.shared_economic())
        );
        let executed = transfers();
        assert_eq!(
          final_state.usage().actor_effect_used(),
          effect.saturating_mul(executed.len() as u64)
        );
        let receipts = System::events()
          .into_iter()
          .filter_map(|record| match record.event {
            RuntimeEvent::Actors(Event::ActionFeeCharged {
              actor_id,
              cycle_nonce,
              step_index,
              actual_effect_weight,
              fee,
            }) => {
              assert_eq!(
                (cycle_nonce, step_index, actual_effect_weight, fee),
                (1, 0, effect, 0)
              );
              Some(actor_id)
            }
            RuntimeEvent::Actors(Event::PipelineFeeCharged { .. }) => {
              panic!("the selected System domain must not collect a Pipeline fee")
            }
            _ => None,
          })
          .collect::<Vec<_>>();
        assert_eq!(receipts, executed);
        assert_eq!(Balances::free_balance(&sink), sink_before);
        assert!(executed.starts_with(&before_drain));
        assert!(!executed.is_empty());
        assert!(completed + executed.len() <= ids.len());
        if block == 2 {
          assert!(
            executed.len() < ids.len(),
            "first block must retain a refused suffix"
          );
        }
        assert_eq!(executed, ids[completed..completed + executed.len()]);
        completed += executed.len();
        if completed < ids.len() {
          assert_eq!(
            Actors::service_header().cursor.unwrap().actor_id,
            ids[completed]
          );
        }
        for id in &ids[completed..] {
          assert_eq!(
            Actors::active_actor_state(*id)
              .unwrap()
              .identity
              .cycle_nonce,
            0
          );
          assert!(Actors::actor_run_state(*id).is_none());
          assert_eq!(Actors::service_nodes(*id).unwrap().eligible_from, 2);
        }
        assert_eq!(
          Balances::free_balance(common::BOB) - recipient_before,
          amount * completed as u128
        );
        for (index, id) in ids.iter().enumerate() {
          assert_eq!(
            Balances::free_balance(Actors::actor_identity(*id).unwrap().sovereign_account),
            source_before[index] - if index < completed { amount } else { 0 },
            "source custody pays exactly its Task amount, not System fees"
          );
        }
        let snapshot = final_state.finalized_snapshot().unwrap();
        assert_eq!(Actors::finalized_block_resource_telemetry(), Some(snapshot));
        Actors::on_finalize(block);
        assert!(Actors::block_resource_state().is_none());
      });
    }
    for id in &ids[..completed] {
      assert_eq!(
        Actors::active_actor_state(*id)
          .unwrap()
          .identity
          .cycle_nonce,
        1
      );
      assert!(Actors::actor_run_state(*id).is_none());
    }
  });
}
