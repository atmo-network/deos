use super::*;

type ControlWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

#[test]
fn control_weight_identity_binds_every_consumed_user_profile() {
  let maximum_tail_chunks = ActorMaxContractSteps::get()
    .saturating_sub(1)
    .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
  let profiles = (0..=maximum_tail_chunks)
    .map(|tails| {
      let expected = if tails == 0 {
        ControlWeights::scheduler_inner_opening_user_complete_header_max()
      } else {
        ControlWeights::scheduler_inner_opening_user_complete_header_max_tail(tails)
      };
      assert_eq!(
        RuntimeStepControlWeight::user_opening_control_weight(tails),
        expected
      );
      expected
    })
    .collect::<alloc::vec::Vec<_>>();
  let progress = (1..=maximum_tail_chunks)
    .map(ControlWeights::scheduler_inner_opening_user_progress_min)
    .collect::<alloc::vec::Vec<_>>();
  let identity = RuntimeStepControlWeight::production_weight_identity().unwrap();
  let retry = (1..=pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK)
    .flat_map(|steps| {
      (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
        ControlWeights::scheduler_inner_suspended_user_tail_retry(steps, predicates)
      })
    })
    .collect::<alloc::vec::Vec<_>>();
  let resumed = (2..=pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK)
    .flat_map(|steps| {
      (0..=ActorMaxPredicatesPerStep::get()).map(move |predicates| {
        ControlWeights::scheduler_inner_suspended_user_tail_progress(steps, predicates)
      })
    })
    .collect::<alloc::vec::Vec<_>>();
  let profiles = [profiles, progress, retry, resumed];
  let identity_from = |values: &[alloc::vec::Vec<Weight>; 4]| {
    RuntimeStepControlWeight::production_weight_identity_from_user_profiles(
      &values[0], &values[1], &values[2], &values[3],
    )
  };
  assert_eq!(identity_from(&profiles), Some(identity));
  for group in 0..profiles.len() {
    for index in 0..profiles[group].len() {
      for delta in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
        let mut changed = profiles.clone();
        changed[group][index] = changed[group][index].checked_add(&delta).unwrap();
        assert_ne!(
          identity_from(&changed),
          Some(identity),
          "User profile group={group} index={index} must bind both dimensions ({delta:?})"
        );
      }
    }
  }
}

#[test]
fn control_weight_identity_user_tail_covers_both_classes_and_action_suffixes() {
  use pallet_deos_actors::{
    AmountResolution, Step, StepControlWeightContext, StepErrorPolicy, TaskEffectExecution,
  };
  let step: pallet_deos_actors::StepOf<Runtime> = Step {
    precondition: None,
    task: Task::Transfer {
      to: AccountId::from([23u8; 32]),
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
  };
  for progress in [false, true] {
    for steps in (if progress { 2 } else { 1 })..=pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK {
      for predicates in 0..=ActorMaxPredicatesPerStep::get() {
        let (system, user, selected) = if progress {
          (
            ControlWeights::scheduler_inner_suspended_tail_progress(steps, predicates),
            ControlWeights::scheduler_inner_suspended_user_tail_progress(steps, predicates),
            RuntimeStepControlWeight::suspended_tail_progress_control_weight(steps, predicates),
          )
        } else {
          (
            ControlWeights::scheduler_inner_suspended_tail_retry(steps, predicates),
            ControlWeights::scheduler_inner_suspended_user_tail_retry(steps, predicates),
            RuntimeStepControlWeight::suspended_tail_retry_control_weight(steps, predicates),
          )
        };
        let expected = system.max(user);
        assert_eq!(selected, expected);
        assert!(selected.all_gte(system) && selected.all_gte(user));
        let context = StepControlWeightContext {
          cursor: 1,
          steps_in_fragment: steps,
          opening_tail_chunks: 0,
          predicate_evaluation_units: predicates,
        };
        let maximum = RuntimeStepControlWeight::maximum_control_weight(context, &step).unwrap();
        for (task_effect, action_fee_collected) in [
          (TaskEffectExecution::NotInvoked, false),
          (TaskEffectExecution::Invoked, false),
          (TaskEffectExecution::Invoked, true),
        ] {
          let mut complete = expected;
          if task_effect == TaskEffectExecution::Invoked {
            complete = complete
              .checked_add(&ControlWeights::action_invocation_receipt())
              .unwrap();
          }
          if action_fee_collected {
            complete = complete
              .checked_add(&ControlWeights::fee_collection())
              .unwrap();
          }
          assert!(complete.all_lte(maximum));
          assert_eq!(
            RuntimeStepControlWeight::actual_control_weight(
              context,
              &step,
              maximum,
              StepControlExecution {
                task_effect,
                action_fee_collected,
                phase: StepControlPhase::Suspended,
                outcome: if progress {
                  StepControlOutcome::Continued
                } else {
                  StepControlOutcome::Suspended
                },
                placement: if progress {
                  StepControlPlacement::Queue
                } else {
                  StepControlPlacement::Wakeup
                }
              }
            ),
            Some(complete)
          );
        }
      }
    }
  }
}

#[test]
fn control_weight_identity_user_progress_covers_both_classes_and_action_suffixes() {
  use pallet_deos_actors::{
    AmountResolution, Step, StepControlWeightContext, StepErrorPolicy, TaskEffectExecution,
  };
  let step: pallet_deos_actors::StepOf<Runtime> = Step {
    precondition: None,
    task: Task::Transfer {
      to: AccountId::from([23u8; 32]),
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  let maximum_tails = ActorMaxContractSteps::get()
    .saturating_sub(1)
    .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
  for tails in 1..=maximum_tails {
    let system = ControlWeights::scheduler_inner_opening_progress_min(tails);
    let user = ControlWeights::scheduler_inner_opening_user_progress_min(tails);
    let expected = system.max(user);
    assert_eq!(
      RuntimeStepControlWeight::opening_progress_control_weight(tails),
      expected
    );
    assert!(expected.all_gte(system) && expected.all_gte(user));
    let context = StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: tails,
      predicate_evaluation_units: 0,
    };
    let maximum = RuntimeStepControlWeight::maximum_control_weight(context, &step).unwrap();
    for (task_effect, action_fee_collected) in [
      (TaskEffectExecution::NotInvoked, false),
      (TaskEffectExecution::Invoked, false),
      (TaskEffectExecution::Invoked, true),
    ] {
      let mut complete = expected;
      if task_effect == TaskEffectExecution::Invoked {
        complete = complete
          .checked_add(&ControlWeights::action_invocation_receipt())
          .unwrap();
      }
      if action_fee_collected {
        complete = complete
          .checked_add(&ControlWeights::fee_collection())
          .unwrap();
      }
      assert!(complete.all_lte(maximum));
      assert_eq!(
        RuntimeStepControlWeight::actual_control_weight(
          context,
          &step,
          maximum,
          StepControlExecution {
            task_effect,
            action_fee_collected,
            phase: StepControlPhase::Opening,
            outcome: StepControlOutcome::Continued,
            placement: StepControlPlacement::Queue,
          },
        ),
        Some(complete),
        "Pipeline/hold belong to User progress; Action collection/receipt are independent additions",
      );
    }
  }
}
