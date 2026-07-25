use super::pallet::*;
use super::types::Task as AaaTask;
use super::{
  AssetOps, DexOps, FeeCollector, LiquidityDonationOps, RetryClass, StakingOps, TaskFailure,
};
use frame::prelude::*;
use polkadot_sdk::sp_runtime::{
  Perbill,
  traits::{SaturatedConversion, Zero},
};
use polkadot_sdk::sp_weights::WeightToFee as _;

// Any extrinsic or runtime entrypoint that can fail after mutating multiple storage
// locations SHOULD either pre-validate all fallible conditions first or use
// transactional semantics so capacity / late-guard failures cannot strand partial state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AmountResolutionPolicy {
  PreserveSpend,
  ExpendableSpend,
  Mint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AmountResolutionOutcome<Balance> {
  Resolved(Balance),
  Skipped,
  FundingUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskResolutionOutcome {
  Skipped,
  FundingUnavailable,
}

enum PreparedTask<T: Config> {
  Transfer {
    to: T::AccountId,
    asset: T::AssetId,
    amount: T::Balance,
  },
  SplitTransfer {
    asset: T::AssetId,
    total: T::Balance,
    legs: SplitTransferLegsOf<T>,
  },
  Burn {
    asset: T::AssetId,
    amount: T::Balance,
  },
  Mint {
    asset: T::AssetId,
    amount: T::Balance,
  },
  SwapExactIn {
    asset_in: T::AssetId,
    asset_out: T::AssetId,
    amount_in: T::Balance,
    slippage_tolerance: Perbill,
  },
  SwapExactOut {
    asset_in: T::AssetId,
    asset_out: T::AssetId,
    amount_out: T::Balance,
    max_amount_in: T::Balance,
    slippage_tolerance: Perbill,
  },
  AddLiquidity {
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount_a: T::Balance,
    amount_b: T::Balance,
  },
  RemoveLiquidity {
    lp_asset: T::AssetId,
    amount: T::Balance,
  },
  Stake {
    asset: T::AssetId,
    amount: T::Balance,
  },
  DonateLiquidity {
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount: T::Balance,
    max_ratio_error: Perbill,
  },
  Unstake {
    asset: T::AssetId,
    shares: T::Balance,
  },
  StopCycle,
}

enum PreparedTaskOutcome<T: Config> {
  Executable(PreparedTask<T>),
  Skipped,
  FundingUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepResult {
  Completed,
  Stopped,
  FundingUnavailable,
  Failed(RetryClass),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepControl {
  Advance,
  CompleteCycle,
  Terminate,
  SuspendCurrent,
}

fn evaluate_condition_set_with<C, MaxConditions, E, Evaluate>(
  condition_set: &ConditionSet<C, MaxConditions>,
  mut evaluate: Evaluate,
) -> Result<bool, E>
where
  MaxConditions: Get<u32>,
  Evaluate: FnMut(&C) -> Result<bool, E>,
{
  let (conditions, require_all) = match condition_set {
    ConditionSet::Always => return Ok(true),
    ConditionSet::All(conditions) => (conditions, true),
    ConditionSet::Any(conditions) => (conditions, false),
  };
  let mut all_pass = true;
  let mut any_pass = false;
  let mut first_error = None;
  for condition in conditions {
    match evaluate(condition) {
      Ok(pass) => {
        all_pass &= pass;
        any_pass |= pass;
      }
      Err(error) => {
        if first_error.is_none() {
          first_error = Some(error);
        }
      }
    }
  }
  if let Some(error) = first_error {
    return Err(error);
  }
  Ok(if require_all { all_pass } else { any_pass })
}

fn resolve_step_control(
  mutability: Mutability,
  result: StepResult,
  error_policy: StepErrorPolicy,
) -> StepControl {
  match (result, error_policy, mutability) {
    (StepResult::Completed, _, _) => StepControl::Advance,
    (StepResult::Stopped, _, _) => StepControl::CompleteCycle,
    (StepResult::FundingUnavailable, StepErrorPolicy::RetryLater, Mutability::Mutable) => {
      StepControl::SuspendCurrent
    }
    (StepResult::FundingUnavailable, StepErrorPolicy::RetryLater, Mutability::Immutable) => {
      StepControl::Terminate
    }
    (StepResult::FundingUnavailable, _, _) => StepControl::Advance,
    (StepResult::Failed(_), StepErrorPolicy::ContinueNextStep, _) => StepControl::Advance,
    (
      StepResult::Failed(RetryClass::Temporary),
      StepErrorPolicy::RetryLater,
      Mutability::Mutable,
    ) => StepControl::SuspendCurrent,
    (StepResult::Failed(_), StepErrorPolicy::AbortCycle | StepErrorPolicy::RetryLater, _) => {
      StepControl::Terminate
    }
  }
}

impl<T: Config> Pallet<T> {
  pub(crate) fn promote_pending_funding(aaa_id: AaaId) {
    let mut promotions = alloc::vec::Vec::new();
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
      let Some(funding) = maybe.as_mut() else {
        return;
      };
      let assets: alloc::vec::Vec<_> = funding.funding_snapshots.keys().copied().collect();
      for asset in assets {
        let Some(batch) = funding.funding_snapshots.get_mut(&asset) else {
          continue;
        };
        if batch.pending_amount.is_zero() {
          continue;
        }
        batch.amount = batch.pending_amount;
        batch.pending_amount = Zero::zero();
        promotions.push((asset, batch.amount));
      }
    });
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      if let Some(hot) = maybe_hot {
        hot.pending_funding_count = 0;
      }
    });
    for (asset, amount) in promotions {
      Self::deposit_event(Event::FundingBatchPromoted {
        aaa_id,
        asset,
        amount,
      });
    }
  }

  pub(crate) fn cancel_continuation_internal(
    aaa_id: AaaId,
    reason: CancellationReason,
    outcomes: Option<OutcomeTotals>,
  ) -> Result<bool, DispatchError> {
    let Some(continuation) = ContinuationStateStore::<T>::get(aaa_id) else {
      return Ok(false);
    };
    let hot = ActorHot::<T>::get(aaa_id).ok_or(Error::<T>::ContinuationInvariant)?;
    ensure!(
      hot.run_state == RunState::Suspended && hot.cycle_nonce > 0,
      Error::<T>::ContinuationInvariant
    );
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Continuation prevalidation requires active hot state");
      hot.run_state = RunState::Idle;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
    });
    ContinuationStateStore::<T>::remove(aaa_id);
    let totals = outcomes.unwrap_or(continuation.cumulative_outcomes);
    Self::deposit_event(Event::CycleCancelled {
      aaa_id,
      cycle_nonce: hot.cycle_nonce,
      reason,
    });
    Self::deposit_event(Event::CycleSummary {
      aaa_id,
      cycle_nonce: hot.cycle_nonce,
      executed_steps: totals.executed_steps,
      skipped_conditions: totals.skipped_conditions,
      skipped_resolution: totals.skipped_resolution,
      skipped_funding_unavailable: totals.skipped_funding_unavailable,
      failed_steps: totals.failed_steps,
    });
    Ok(true)
  }

  pub(crate) fn write_continuation_state(
    aaa_id: AaaId,
    state: Option<ContinuationStateOf<T>>,
  ) -> DispatchResult {
    let hot = ActorHot::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if let Some(continuation) = state.as_ref() {
      let program = ActorProgram::<T>::get(aaa_id).ok_or(Error::<T>::ContinuationInvariant)?;
      ensure!(
        hot.mutability == Mutability::Mutable
          && continuation.cursor < program.execution_plan.len() as u32,
        Error::<T>::ContinuationInvariant
      );
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor existence was prevalidated")
          .run_state = if state.is_some() {
          RunState::Suspended
        } else {
          RunState::Idle
        };
      });
      if let Some(continuation) = state {
        ContinuationStateStore::<T>::insert(aaa_id, continuation);
      } else {
        ContinuationStateStore::<T>::remove(aaa_id);
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  pub(crate) fn persist_continuation_suspension(
    aaa_id: AaaId,
    cycle_nonce: u64,
    state: ContinuationStateOf<T>,
    reason: SuspensionReason,
  ) -> DispatchResult {
    let attempt = state.attempt;
    let cursor = state.cursor;
    let cumulative_outcomes = state.cumulative_outcomes;
    Self::write_continuation_state(aaa_id, Some(state))?;
    Self::deposit_event(Event::CycleSuspended {
      aaa_id,
      cycle_nonce,
      attempt,
      cursor,
      reason,
      cumulative_outcomes,
    });
    Ok(())
  }

  pub(crate) fn begin_continuation_attempt(
    aaa_id: AaaId,
    cycle_nonce: u64,
    now: BlockNumberFor<T>,
  ) -> ContinuationStateOf<T> {
    ContinuationStateStore::<T>::mutate(aaa_id, |maybe| {
      let continuation = maybe
        .as_mut()
        .expect("Suspended run_state requires ContinuationState");
      continuation.attempt = continuation.attempt.saturating_add(1);
      continuation.last_attempt_block = now;
    });
    let continuation = ContinuationStateStore::<T>::get(aaa_id)
      .expect("Suspended run_state requires ContinuationState");
    Self::deposit_event(Event::CycleContinued {
      aaa_id,
      cycle_nonce,
      attempt: continuation.attempt,
      cursor: continuation.cursor,
    });
    continuation
  }

  pub(crate) fn record_stop_cycle_event(aaa_id: AaaId, cycle_nonce: u64, step_index: u32) {
    Self::deposit_event(Event::CycleStopped {
      aaa_id,
      cycle_nonce,
      step_index,
    });
  }

  fn record_simulation_step(
    trace: &mut Option<&mut alloc::vec::Vec<SimulationStepRecord>>,
    step_index: u32,
    outcome: SimulationStepOutcome,
  ) {
    if let Some(records) = trace.as_deref_mut() {
      records.push(SimulationStepRecord {
        step_index,
        outcome,
      });
    }
  }

  pub(crate) fn execute_single_cycle(
    aaa_id: AaaId,
    instance: AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
  ) -> Weight {
    Self::execute_single_cycle_traced(aaa_id, instance, now, None).0
  }

  pub(crate) fn execute_single_cycle_traced(
    aaa_id: AaaId,
    instance: AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    mut trace: Option<&mut alloc::vec::Vec<SimulationStepRecord>>,
  ) -> (Weight, bool) {
    let base_weight = T::DbWeight::get().writes(1);
    let is_continuation = instance.run_state == RunState::Suspended;
    let mut persisted_trigger_snapshot = None;
    let (cycle_nonce, start_cursor, attempt, cumulative_outcomes) = if is_continuation {
      let continuation = Self::begin_continuation_attempt(aaa_id, instance.cycle_nonce, now);
      persisted_trigger_snapshot = Some(continuation.trigger_snapshot);
      (
        instance.cycle_nonce,
        continuation.cursor,
        continuation.attempt,
        continuation.cumulative_outcomes,
      )
    } else {
      if instance.cycle_nonce == u64::MAX {
        if instance.actor_class.aaa_type() == AaaType::User {
          Self::close_actor(aaa_id, &instance, CloseReason::CycleNonceExhausted)
            .expect("fresh execution snapshot satisfies terminal preconditions");
        } else {
          ActorHot::<T>::mutate(aaa_id, |maybe| {
            if let Some(inst) = maybe.as_mut() {
              inst.lifecycle = ActiveLifecycle::Paused(PauseReason::CycleNonceExhausted);
            }
          });
          Self::deposit_event(Event::AaaPaused {
            aaa_id,
            reason: PauseReason::CycleNonceExhausted,
          });
        }
        return (base_weight, true);
      }
      let Some(cycle_nonce) = ActorHot::<T>::mutate(aaa_id, |maybe| {
        let inst = maybe.as_mut()?;
        inst.cycle_nonce = inst.cycle_nonce.saturating_add(1);
        inst.pending_signal = false;
        inst.last_cycle_block = now;
        Some(inst.cycle_nonce)
      }) else {
        return (base_weight, true);
      };
      Self::deposit_event(Event::CycleStarted {
        aaa_id,
        cycle_nonce,
      });
      (cycle_nonce, 0, 0, OutcomeTotals::default())
    };
    let funding = if instance.funding_tracked_count == 0 {
      None
    } else {
      Some(ActorFunding::<T>::get(aaa_id).expect("active actor funding existence was prevalidated"))
    };
    let empty_funding_snapshots = FundingSnapshotsOf::<T>::default();
    let actor = instance.sovereign_account.clone();
    let is_user = instance.actor_class.aaa_type() == AaaType::User;
    let execution_plan = &instance.execution_plan;
    let funding_snapshots = funding
      .as_ref()
      .map_or(&empty_funding_snapshots, |state| &state.funding_snapshots);
    let mut executed_steps = cumulative_outcomes.executed_steps;
    let mut skipped_conditions = cumulative_outcomes.skipped_conditions;
    let mut skipped_resolution = cumulative_outcomes.skipped_resolution;
    let mut skipped_funding_unavailable = cumulative_outcomes.skipped_funding_unavailable;
    let mut failed_steps = cumulative_outcomes.failed_steps;
    let mut attempt_executed_steps: u32 = 0;
    let mut execution_plan_failed = false;
    let mut suspended_at: Option<(u32, SuspensionReason)> = None;
    let mut reserved_fee_remaining = if is_user {
      Self::attempt_fee_upper_bound(&instance, start_cursor as usize)
    } else {
      T::Balance::zero()
    };
    let trigger_snapshot = persisted_trigger_snapshot.unwrap_or_else(|| {
      Self::capture_trigger_snapshot(&actor, execution_plan, reserved_fee_remaining)
    });
    for step_idx in start_cursor as usize..execution_plan.len() {
      let step = &execution_plan[step_idx];
      let step_num = step_idx as u32;
      let eval_fee = if is_user {
        Self::compute_eval_fee(step.conditions.len())
      } else {
        T::Balance::zero()
      };
      let exec_fee = if is_user {
        T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task))
      } else {
        T::Balance::zero()
      };
      let reserved_step_fee = eval_fee.saturating_add(exec_fee);
      match Self::evaluate_condition_set(&step.conditions, &actor, reserved_fee_remaining) {
        Ok(true) => {}
        Ok(false) => {
          if is_user {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
            if let Err(error) = Self::collect_user_step_fee(&actor, eval_fee) {
              failed_steps = failed_steps.saturating_add(1);
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                aaa_id,
                cycle_nonce,
                step_index: step_num,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          skipped_conditions = skipped_conditions.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::ConditionsNotMet),
          );
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ConditionsNotMet,
          });
          continue;
        }
        Err(error) => {
          let charged_error = if is_user {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
            Self::collect_user_step_fee(&actor, eval_fee)
              .err()
              .unwrap_or(error)
          } else {
            error
          };
          failed_steps = failed_steps.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: charged_error,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, charged_error);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      }
      let prepared_task = match Self::prepare_task(
        &step.task,
        &actor,
        instance.actor_class.aaa_type(),
        reserved_fee_remaining,
        &trigger_snapshot,
        funding_snapshots,
      ) {
        Ok(PreparedTaskOutcome::Executable(task)) => task,
        Ok(PreparedTaskOutcome::Skipped) => {
          if is_user {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
            if let Err(error) = Self::collect_user_step_fee(&actor, eval_fee) {
              failed_steps = failed_steps.saturating_add(1);
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                aaa_id,
                cycle_nonce,
                step_index: step_num,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          skipped_resolution = skipped_resolution.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::ResolutionSkipped),
          );
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          continue;
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          if is_user {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
            if let Err(error) = Self::collect_user_step_fee(&actor, eval_fee) {
              failed_steps = failed_steps.saturating_add(1);
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                aaa_id,
                cycle_nonce,
                step_index: step_num,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          match resolve_step_control(
            instance.mutability,
            StepResult::FundingUnavailable,
            step.on_error,
          ) {
            StepControl::Advance => {}
            StepControl::CompleteCycle => {
              unreachable!("FundingUnavailable cannot complete a cycle")
            }
            StepControl::Terminate => {
              execution_plan_failed = true;
              break;
            }
            StepControl::SuspendCurrent => {
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Suspended(SuspensionReason::FundingUnavailable),
              );
              suspended_at = Some((step_num, SuspensionReason::FundingUnavailable));
              break;
            }
          }
          skipped_funding_unavailable = skipped_funding_unavailable.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::FundingUnavailable),
          );
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::FundingUnavailable,
          });
          continue;
        }
        Err(error) => {
          let charged_error = if is_user {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
            Self::collect_user_step_fee(&actor, eval_fee)
              .err()
              .unwrap_or(error)
          } else {
            error
          };
          failed_steps = failed_steps.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: charged_error,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, charged_error);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      };
      if is_user {
        reserved_fee_remaining = reserved_fee_remaining.saturating_sub(reserved_step_fee);
        if let Err(error) = Self::collect_user_step_fee(&actor, reserved_step_fee) {
          failed_steps = failed_steps.saturating_add(1);
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, error);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      }
      if let Err(failure) = Self::execute_prepared_task(prepared_task, aaa_id, &actor) {
        failed_steps = failed_steps.saturating_add(1);
        let retry = failure.retry;
        let control = resolve_step_control(
          instance.mutability,
          StepResult::Failed(retry),
          step.on_error,
        );
        Self::record_simulation_step(&mut trace, step_num, SimulationStepOutcome::Failed(retry));
        Self::deposit_event(Event::StepFailed {
          aaa_id,
          cycle_nonce,
          step_index: step_num,
          error: failure.error,
        });
        match control {
          StepControl::Advance => continue,
          StepControl::CompleteCycle => {
            unreachable!("task failure cannot complete a cycle")
          }
          StepControl::Terminate => {
            execution_plan_failed = true;
            break;
          }
          StepControl::SuspendCurrent => {
            suspended_at = Some((step_num, SuspensionReason::Temporary));
            break;
          }
        }
      }
      let successful_result = if matches!(&step.task, AaaTask::StopCycle) {
        StepResult::Stopped
      } else {
        StepResult::Completed
      };
      let successful_control =
        resolve_step_control(instance.mutability, successful_result, step.on_error);
      let simulation_outcome = if successful_control == StepControl::CompleteCycle {
        Self::record_stop_cycle_event(aaa_id, cycle_nonce, step_num);
        SimulationStepOutcome::Stopped
      } else {
        debug_assert_eq!(successful_control, StepControl::Advance);
        SimulationStepOutcome::Executed
      };
      Self::record_simulation_step(&mut trace, step_num, simulation_outcome);
      executed_steps = executed_steps.saturating_add(1);
      attempt_executed_steps = attempt_executed_steps.saturating_add(1);
      if successful_control == StepControl::CompleteCycle {
        break;
      }
    }
    let attempt_weight = base_weight.saturating_add(Weight::from_parts(
      5_000_000u64.saturating_mul(attempt_executed_steps as u64 + 1),
      1000u64.saturating_mul(attempt_executed_steps as u64 + 1),
    ));
    let mut failure_already_recorded = false;
    if let Some((cursor, suspension_reason)) = suspended_at {
      let consecutive_failures = ActorHot::<T>::mutate(aaa_id, |maybe| {
        let Some(hot) = maybe.as_mut() else {
          return 0;
        };
        hot.consecutive_failures = hot.consecutive_failures.saturating_add(1);
        hot.consecutive_failures
      });
      failure_already_recorded = true;
      if !Self::failure_limit_reached(consecutive_failures) {
        let cumulative_outcomes = OutcomeTotals {
          executed_steps,
          skipped_conditions,
          skipped_resolution,
          skipped_funding_unavailable,
          failed_steps,
        };
        Self::persist_continuation_suspension(
          aaa_id,
          cycle_nonce,
          ContinuationState {
            cursor,
            attempt,
            last_attempt_block: now,
            trigger_snapshot: Self::trim_trigger_snapshot(
              execution_plan,
              cursor as usize,
              &trigger_snapshot,
            ),
            cumulative_outcomes,
          },
          suspension_reason,
        )
        .expect("admitted mutable RetryLater plan has a valid unresolved cursor");
        return (attempt_weight, false);
      }
      execution_plan_failed = true;
    }
    let terminal_outcomes = OutcomeTotals {
      executed_steps,
      skipped_conditions,
      skipped_resolution,
      skipped_funding_unavailable,
      failed_steps,
    };
    let continuation_cancelled = if is_continuation && execution_plan_failed {
      Self::cancel_continuation_internal(
        aaa_id,
        CancellationReason::Terminal,
        Some(terminal_outcomes),
      )
      .expect("terminal Continuation cancellation satisfies stored invariants")
    } else {
      if is_continuation {
        Self::write_continuation_state(aaa_id, None)
          .expect("successful Continuation can be cleared atomically");
      }
      false
    };
    let should_promote_funding = ActorHot::<T>::mutate(aaa_id, |maybe| {
      let Some(hot) = maybe.as_mut() else {
        return false;
      };
      if execution_plan_failed {
        if !failure_already_recorded {
          hot.consecutive_failures = hot.consecutive_failures.saturating_add(1);
        }
        false
      } else {
        hot.consecutive_failures = 0;
        hot.pending_funding_count > 0
      }
    });
    if !continuation_cancelled {
      Self::deposit_event(Event::CycleSummary {
        aaa_id,
        cycle_nonce,
        executed_steps,
        skipped_conditions,
        skipped_resolution,
        skipped_funding_unavailable,
        failed_steps,
      });
    }
    if should_promote_funding {
      Self::promote_pending_funding(aaa_id);
    }
    if execution_plan_failed {
      if let Some(inst) = Self::active_actor_snapshot(aaa_id) {
        if !inst.lifecycle.is_paused() && Self::failure_limit_reached(inst.consecutive_failures) {
          Self::close_actor(aaa_id, &inst, CloseReason::ConsecutiveFailures)
            .expect("fresh execution snapshot satisfies terminal preconditions");
        }
      }
    } else if let Some(inst) = Self::active_actor_snapshot(aaa_id) {
      if let Some(target_nonce) = inst.auto_close_at_cycle_nonce {
        if cycle_nonce >= target_nonce {
          Self::close_actor(aaa_id, &inst, CloseReason::AutoCloseNonceReached)
            .expect("fresh execution snapshot satisfies terminal preconditions");
        }
      }
    }
    (attempt_weight, execution_plan_failed)
  }

  pub fn simulate_current_program(
    aaa_id: AaaId,
    expected_type: AaaType,
    expected_mutability: Mutability,
    expected_program: ProgramInputOf<T>,
    mode: SimulationMode,
  ) -> Result<SimulationResult, SimulationError> {
    let instance = Self::active_actor_snapshot(aaa_id).ok_or(SimulationError::ActorNotFound)?;
    if instance.actor_class.aaa_type() != expected_type {
      return Err(SimulationError::TypeMismatch);
    }
    if instance.mutability != expected_mutability {
      return Err(SimulationError::MutabilityMismatch);
    }
    let ProgramInput::Active {
      schedule,
      schedule_window,
      execution_plan,
      funding_source_policy,
    } = expected_program
    else {
      return Err(SimulationError::ProgramMismatch);
    };
    let stored_program = ActorProgram::<T>::get(aaa_id).ok_or(SimulationError::ActorNotFound)?;
    let stored_funding = ActorFunding::<T>::get(aaa_id).ok_or(SimulationError::ActorNotFound)?;
    if stored_program.schedule != schedule
      || stored_program.schedule_window != schedule_window
      || stored_program.execution_plan != execution_plan
      || stored_funding.funding_source_policy != funding_source_policy
    {
      return Err(SimulationError::ProgramMismatch);
    }
    Self::ensure_simulation_ready(aaa_id, &instance, mode)?;

    let (cycle_nonce, attempt, start_cursor, initial_outcomes) = match mode {
      SimulationMode::FreshCurrentPlan => (
        instance.cycle_nonce.saturating_add(1),
        0,
        0,
        OutcomeTotals::default(),
      ),
      SimulationMode::CurrentContinuation => {
        let continuation =
          ContinuationStateStore::<T>::get(aaa_id).ok_or(SimulationError::ContinuationInvariant)?;
        (
          instance.cycle_nonce,
          continuation.attempt.saturating_add(1),
          continuation.cursor,
          continuation.cumulative_outcomes,
        )
      }
    };
    let now = frame_system::Pallet::<T>::block_number();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let mut trace = alloc::vec::Vec::new();
      let (_, failed) = Self::execute_single_cycle_traced(aaa_id, instance, now, Some(&mut trace));
      let continuation = ContinuationStateStore::<T>::get(aaa_id);
      let status = if continuation.is_some() {
        SimulationStatus::Suspended
      } else if failed {
        SimulationStatus::Aborted
      } else {
        SimulationStatus::Completed
      };
      let continuation_cursor = continuation.as_ref().map(|state| state.cursor);
      let finalized_through = continuation_cursor
        .map(|cursor| cursor.checked_sub(1))
        .unwrap_or_else(|| trace.last().map(|record| record.step_index))
        .or_else(|| start_cursor.checked_sub(1));
      let mut cumulative_outcomes = initial_outcomes;
      for record in &trace {
        match record.outcome {
          SimulationStepOutcome::Executed | SimulationStepOutcome::Stopped => {
            cumulative_outcomes.executed_steps =
              cumulative_outcomes.executed_steps.saturating_add(1);
          }
          SimulationStepOutcome::Skipped(StepSkippedReason::ConditionsNotMet) => {
            cumulative_outcomes.skipped_conditions =
              cumulative_outcomes.skipped_conditions.saturating_add(1);
          }
          SimulationStepOutcome::Skipped(StepSkippedReason::ResolutionSkipped) => {
            cumulative_outcomes.skipped_resolution =
              cumulative_outcomes.skipped_resolution.saturating_add(1);
          }
          SimulationStepOutcome::Skipped(StepSkippedReason::FundingUnavailable) => {
            cumulative_outcomes.skipped_funding_unavailable = cumulative_outcomes
              .skipped_funding_unavailable
              .saturating_add(1);
          }
          SimulationStepOutcome::Failed(_) => {
            cumulative_outcomes.failed_steps = cumulative_outcomes.failed_steps.saturating_add(1);
          }
          SimulationStepOutcome::Suspended(_) => {}
        }
      }
      if let Some(state) = continuation.as_ref() {
        debug_assert_eq!(state.cumulative_outcomes, cumulative_outcomes);
      }
      debug_assert!(trace.len() <= T::MaxExecutionPlanSteps::get() as usize);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(SimulationResult {
        status,
        cycle_nonce,
        attempt,
        start_cursor,
        continuation_cursor,
        finalized_through,
        cumulative_outcomes,
        steps: trace,
      }))
    })
  }

  pub(crate) fn failure_limit_reached(consecutive_failures: u32) -> bool {
    let max_failures = T::MaxConsecutiveFailures::get();
    max_failures > 0 && consecutive_failures >= max_failures
  }

  pub(crate) fn compute_eval_fee(num_conditions: u32) -> BalanceOf<T> {
    let base = T::StepBaseFee::get();
    let per_cond = T::ConditionReadFee::get();
    base.saturating_add(per_cond.saturating_mul(num_conditions.into()))
  }

  fn collect_user_step_fee(actor: &T::AccountId, fee: T::Balance) -> DispatchResult {
    if fee.is_zero() {
      return Ok(());
    }
    let native = T::NativeAssetId::get();
    ensure!(
      T::AssetOps::balance(actor, native) >= fee,
      Error::<T>::InsufficientFee
    );
    T::FeeCollector::collect_fee(actor, &T::FeeSink::get(), native, fee)
      .map_err(|_| DispatchError::Other("StepFeeTransferFailed"))
  }

  fn apply_error_policy(
    _aaa_id: AaaId,
    _cycle_nonce: u64,
    _step: u32,
    policy: StepErrorPolicy,
    _failure: DispatchError,
  ) -> bool {
    resolve_step_control(
      Mutability::Immutable,
      StepResult::Failed(RetryClass::Permanent),
      policy,
    ) == StepControl::Terminate
  }

  fn push_trigger_surface(
    amount: &AmountResolution<T::Balance>,
    surface: ResolutionSurface<T::AssetId>,
    surfaces: &mut alloc::vec::Vec<ResolutionSurface<T::AssetId>>,
  ) {
    if matches!(amount, AmountResolution::PercentageOfTrigger(_)) && !surfaces.contains(&surface) {
      surfaces.push(surface);
    }
  }

  fn collect_percentage_trigger_surfaces(
    task: &TaskOf<T>,
    surfaces: &mut alloc::vec::Vec<ResolutionSurface<T::AssetId>>,
  ) {
    match task {
      AaaTask::Transfer { asset, amount, .. }
      | AaaTask::SplitTransfer { asset, amount, .. }
      | AaaTask::Burn { asset, amount }
      | AaaTask::Mint { asset, amount }
      | AaaTask::RemoveLiquidity {
        lp_asset: asset,
        amount,
      } => Self::push_trigger_surface(amount, ResolutionSurface::Asset(*asset), surfaces),
      AaaTask::SwapExactIn {
        asset_in,
        amount_in,
        ..
      } => Self::push_trigger_surface(amount_in, ResolutionSurface::Asset(*asset_in), surfaces),
      AaaTask::SwapExactOut {
        asset_out,
        amount_out,
        ..
      } => Self::push_trigger_surface(amount_out, ResolutionSurface::Asset(*asset_out), surfaces),
      AaaTask::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
      } => {
        Self::push_trigger_surface(amount_a, ResolutionSurface::Asset(*asset_a), surfaces);
        Self::push_trigger_surface(amount_b, ResolutionSurface::Asset(*asset_b), surfaces);
      }
      AaaTask::Stake { asset, amount } => {
        Self::push_trigger_surface(amount, ResolutionSurface::Asset(*asset), surfaces);
      }
      AaaTask::DonateLiquidity {
        asset_a, amount, ..
      } => {
        Self::push_trigger_surface(amount, ResolutionSurface::Asset(*asset_a), surfaces);
      }
      AaaTask::Unstake { asset, shares } => {
        Self::push_trigger_surface(shares, ResolutionSurface::StakingShares(*asset), surfaces)
      }
      AaaTask::StopCycle => {}
    }
  }

  pub(crate) fn trigger_surfaces(
    execution_plan: &ExecutionPlanOf<T>,
    start_cursor: usize,
  ) -> alloc::vec::Vec<ResolutionSurface<T::AssetId>> {
    let mut surfaces = alloc::vec::Vec::new();
    for step_index in start_cursor..execution_plan.len() {
      Self::collect_percentage_trigger_surfaces(&execution_plan[step_index].task, &mut surfaces);
    }
    surfaces
  }

  fn capture_trigger_snapshot(
    actor: &T::AccountId,
    execution_plan: &ExecutionPlanOf<T>,
    reserved: T::Balance,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::trigger_surfaces(execution_plan, 0) {
      let balance = match surface {
        ResolutionSurface::Asset(asset) => Self::spendable_balance(actor, asset, reserved),
        ResolutionSurface::StakingShares(asset) => T::StakingOps::share_balance(actor, asset),
      };
      snapshot
        .try_insert(surface, balance)
        .unwrap_or_else(|_| panic!("trigger surfaces fit MaxContinuationSnapshotEntries"));
    }
    snapshot
  }

  fn trim_trigger_snapshot(
    execution_plan: &ExecutionPlanOf<T>,
    start_cursor: usize,
    source: &ContinuationSnapshotOf<T>,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::trigger_surfaces(execution_plan, start_cursor) {
      if let Some(balance) = source.get(&surface) {
        snapshot
          .try_insert(surface, *balance)
          .unwrap_or_else(|_| panic!("suffix surfaces fit MaxContinuationSnapshotEntries"));
      }
    }
    snapshot
  }

  fn trigger_balance(
    trigger_snapshot: &ContinuationSnapshotOf<T>,
    surface: ResolutionSurface<T::AssetId>,
  ) -> Result<T::Balance, DispatchError> {
    trigger_snapshot
      .get(&surface)
      .copied()
      .ok_or(Error::<T>::SnapshotUnavailable.into())
  }

  fn prepare_task(
    task: &TaskOf<T>,
    actor: &T::AccountId,
    aaa_type: AaaType,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingBatch<T::Balance>,
      T::MaxFundingTrackedAssets,
    >,
  ) -> Result<PreparedTaskOutcome<T>, DispatchError> {
    match task {
      AaaTask::Transfer { to, asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Transfer {
          to: to.clone(),
          asset: *asset,
          amount: resolved,
        }))
      }
      AaaTask::SplitTransfer {
        asset,
        amount,
        legs,
      } => {
        Self::validate_split_transfer_legs(legs)?;
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::SplitTransfer {
            asset: *asset,
            total: resolved,
            legs: legs.clone(),
          },
        ))
      }
      AaaTask::Burn { asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::ExpendableSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Burn {
          asset: *asset,
          amount: resolved,
        }))
      }
      AaaTask::Mint { asset, amount } => {
        ensure!(
          aaa_type == AaaType::System,
          Error::<T>::MintNotAllowedForUserAaa
        );
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::Mint,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Mint {
          asset: *asset,
          amount: resolved,
        }))
      }
      AaaTask::SwapExactIn {
        asset_in,
        asset_out,
        amount_in,
        slippage_tolerance,
      } => {
        let resolved = match Self::resolve_for_task(
          amount_in,
          *asset_in,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::SwapExactIn {
          asset_in: *asset_in,
          asset_out: *asset_out,
          amount_in: resolved,
          slippage_tolerance: *slippage_tolerance,
        }))
      }
      AaaTask::SwapExactOut {
        asset_in,
        asset_out,
        amount_out,
        slippage_tolerance,
      } => {
        let resolved = match Self::resolve_for_task(
          amount_out,
          *asset_out,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::Mint,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        let max_amount_in = Self::spendable_balance(actor, *asset_in, reserved)
          .saturating_sub(T::AssetOps::minimum_balance(*asset_in));
        if max_amount_in.is_zero() {
          return Ok(PreparedTaskOutcome::FundingUnavailable);
        }
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::SwapExactOut {
            asset_in: *asset_in,
            asset_out: *asset_out,
            amount_out: resolved,
            max_amount_in,
            slippage_tolerance: *slippage_tolerance,
          },
        ))
      }
      AaaTask::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
      } => {
        let outcome_a = Self::resolve_for_task(
          amount_a,
          *asset_a,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )?;
        let outcome_b = Self::resolve_for_task(
          amount_b,
          *asset_b,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )?;
        match (outcome_a, outcome_b) {
          (Err(TaskResolutionOutcome::FundingUnavailable), _)
          | (_, Err(TaskResolutionOutcome::FundingUnavailable)) => {
            Ok(PreparedTaskOutcome::FundingUnavailable)
          }
          (Err(TaskResolutionOutcome::Skipped), _) | (_, Err(TaskResolutionOutcome::Skipped)) => {
            Ok(PreparedTaskOutcome::Skipped)
          }
          (Ok(resolved_a), Ok(resolved_b)) => Ok(PreparedTaskOutcome::Executable(
            PreparedTask::AddLiquidity {
              asset_a: *asset_a,
              asset_b: *asset_b,
              amount_a: resolved_a,
              amount_b: resolved_b,
            },
          )),
        }
      }
      AaaTask::RemoveLiquidity { lp_asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *lp_asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::RemoveLiquidity {
            lp_asset: *lp_asset,
            amount: resolved,
          },
        ))
      }
      AaaTask::Stake { asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Stake {
          asset: *asset,
          amount: resolved,
        }))
      }
      AaaTask::DonateLiquidity {
        asset_a,
        asset_b,
        amount,
        max_ratio_error,
      } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset_a,
          actor,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )? {
          Ok(value) => value,
          Err(TaskResolutionOutcome::Skipped) => return Ok(PreparedTaskOutcome::Skipped),
          Err(TaskResolutionOutcome::FundingUnavailable) => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::DonateLiquidity {
            asset_a: *asset_a,
            asset_b: *asset_b,
            amount: resolved,
            max_ratio_error: *max_ratio_error,
          },
        ))
      }
      AaaTask::Unstake { asset, shares } => {
        let resolved = match Self::resolve_unstake_shares(
          shares,
          *asset,
          actor,
          trigger_balances,
          funding_snapshots,
        )? {
          AmountResolutionOutcome::Resolved(value) => value,
          AmountResolutionOutcome::Skipped => return Ok(PreparedTaskOutcome::Skipped),
          AmountResolutionOutcome::FundingUnavailable => {
            return Ok(PreparedTaskOutcome::FundingUnavailable);
          }
        };
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Unstake {
          asset: *asset,
          shares: resolved,
        }))
      }
      AaaTask::StopCycle => Ok(PreparedTaskOutcome::Executable(PreparedTask::StopCycle)),
    }
  }

  fn execute_prepared_task(
    task: PreparedTask<T>,
    aaa_id: AaaId,
    actor: &T::AccountId,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> Result<(), TaskFailure> {
        match task {
          PreparedTask::Transfer { to, asset, amount } => {
            T::AssetOps::transfer(actor, &to, asset, amount)?;
            Self::deposit_event(Event::TransferExecuted {
              aaa_id,
              to,
              asset,
              amount,
            });
          }
          PreparedTask::SplitTransfer { asset, total, legs } => {
            let actor_balance = T::AssetOps::balance(actor, asset);
            if actor_balance < total {
              return Err(TaskFailure::permanent(Error::<T>::InsufficientBalance));
            }
            let mut effective_distributed = T::Balance::zero();
            let mut normalized_transfers: alloc::vec::Vec<(T::AccountId, T::Balance)> =
              alloc::vec::Vec::with_capacity(legs.len());
            for leg in legs.iter() {
              let leg_amount = leg.share.mul_floor(total);
              if leg_amount.is_zero() {
                continue;
              }
              if !T::AssetOps::can_deposit(&leg.to, asset, leg_amount) {
                continue;
              }
              effective_distributed = effective_distributed.saturating_add(leg_amount);
              normalized_transfers.push((leg.to.clone(), leg_amount));
            }
            let retained = total.saturating_sub(effective_distributed);
            for (to, leg_amount) in normalized_transfers.iter() {
              T::AssetOps::transfer(actor, to, asset, *leg_amount)?;
            }
            Self::deposit_event(Event::SplitTransferExecuted {
              aaa_id,
              asset,
              total,
              distributed: effective_distributed,
              retained,
              legs: legs.len() as u32,
              effective_legs: normalized_transfers.len() as u32,
            });
          }
          PreparedTask::Burn { asset, amount } => {
            T::AssetOps::burn(actor, asset, amount)?;
            Self::deposit_event(Event::BurnExecuted {
              aaa_id,
              asset,
              amount,
            });
          }
          PreparedTask::Mint { asset, amount } => {
            T::AssetOps::mint(actor, asset, amount)?;
            Self::deposit_event(Event::MintExecuted {
              aaa_id,
              asset,
              amount,
            });
          }
          PreparedTask::SwapExactIn {
            asset_in,
            asset_out,
            amount_in,
            slippage_tolerance,
          } => {
            let amount_out =
              T::DexOps::swap_exact_in(actor, asset_in, asset_out, amount_in, slippage_tolerance)?;
            Self::deposit_event(Event::SwapExecuted {
              aaa_id,
              asset_in,
              asset_out,
              amount_in,
              amount_out,
            });
          }
          PreparedTask::SwapExactOut {
            asset_in,
            asset_out,
            amount_out,
            max_amount_in,
            slippage_tolerance,
          } => {
            let amount_in = T::DexOps::swap_exact_out(
              actor,
              asset_in,
              asset_out,
              amount_out,
              max_amount_in,
              slippage_tolerance,
            )?;
            Self::deposit_event(Event::SwapExecuted {
              aaa_id,
              asset_in,
              asset_out,
              amount_in,
              amount_out,
            });
          }
          PreparedTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a,
            amount_b,
          } => {
            let (used_a, used_b, lp_minted) =
              T::DexOps::add_liquidity(actor, asset_a, asset_b, amount_a, amount_b)?;
            let _ = (used_a, used_b);
            Self::deposit_event(Event::LiquidityAdded {
              aaa_id,
              asset_a,
              asset_b,
              lp_minted,
            });
          }
          PreparedTask::RemoveLiquidity { lp_asset, amount } => {
            let (out_a, out_b) = T::DexOps::remove_liquidity(actor, lp_asset, amount)?;
            Self::deposit_event(Event::LiquidityRemoved {
              aaa_id,
              lp_asset,
              amount_a: out_a,
              amount_b: out_b,
            });
          }
          PreparedTask::Stake { asset, amount } => {
            T::StakingOps::stake(actor, asset, amount)?;
            Self::deposit_event(Event::StakeExecuted {
              aaa_id,
              asset,
              amount,
            });
          }
          PreparedTask::DonateLiquidity {
            asset_a,
            asset_b,
            amount,
            max_ratio_error,
          } => {
            let (amount_a, amount_b) = T::LiquidityDonationOps::donate_liquidity(
              actor,
              asset_a,
              asset_b,
              amount,
              max_ratio_error,
            )?;
            Self::deposit_event(Event::LiquidityDonated {
              aaa_id,
              asset_a,
              asset_b,
              amount,
              amount_a,
              amount_b,
            });
          }
          PreparedTask::Unstake { asset, shares } => {
            T::StakingOps::unstake(actor, asset, shares)?;
            Self::deposit_event(Event::UnstakeExecuted {
              aaa_id,
              asset,
              shares,
            });
          }
          PreparedTask::StopCycle => {}
        }
        Ok(())
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(err) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(err)),
      }
    })
  }

  fn resolve_for_task(
    spec: &AmountResolution<T::Balance>,
    asset: T::AssetId,
    actor: &T::AccountId,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingBatch<T::Balance>,
      T::MaxFundingTrackedAssets,
    >,
    policy: AmountResolutionPolicy,
  ) -> Result<Result<T::Balance, TaskResolutionOutcome>, DispatchError> {
    Ok(
      match Self::resolve_amount_with_policy(
        spec,
        asset,
        actor,
        reserved,
        trigger_balances,
        funding_snapshots,
        policy,
      )? {
        AmountResolutionOutcome::Resolved(value) => Ok(value),
        AmountResolutionOutcome::Skipped => Err(TaskResolutionOutcome::Skipped),
        AmountResolutionOutcome::FundingUnavailable => {
          Err(TaskResolutionOutcome::FundingUnavailable)
        }
      },
    )
  }

  pub(crate) fn evaluate_condition_set(
    condition_set: &ConditionSetOf<T>,
    who: &T::AccountId,
    reserved: T::Balance,
  ) -> Result<bool, DispatchError> {
    evaluate_condition_set_with(condition_set, |condition| {
      Self::evaluate_atomic_condition(condition, who, reserved)
    })
  }

  fn evaluate_atomic_condition(
    condition: &Condition<T::AssetId, T::Balance>,
    who: &T::AccountId,
    reserved: T::Balance,
  ) -> Result<bool, DispatchError> {
    Ok(match condition {
      Condition::BalanceAbove { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) > *threshold
      }
      Condition::BalanceBelow { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) < *threshold
      }
      Condition::BalanceEquals { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) == *threshold
      }
      Condition::BalanceNotEquals { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) != *threshold
      }
      Condition::BlockNumberAbove { threshold } => {
        let now: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
        now > *threshold
      }
      Condition::BlockNumberBelow { threshold } => {
        let now: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
        now < *threshold
      }
    })
  }

  /// Balance visible to AAA resolution — adapter-visible balance minus AAA-local reserved fees
  fn spendable_balance(who: &T::AccountId, asset: T::AssetId, reserved: T::Balance) -> T::Balance {
    let raw = T::AssetOps::balance(who, asset);
    if asset == T::NativeAssetId::get() {
      raw.saturating_sub(reserved)
    } else {
      raw
    }
  }

  fn resolve_unstake_shares(
    spec: &AmountResolution<T::Balance>,
    position_asset: T::AssetId,
    who: &T::AccountId,
    trigger_share_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingBatch<T::Balance>,
      T::MaxFundingTrackedAssets,
    >,
  ) -> Result<AmountResolutionOutcome<T::Balance>, DispatchError> {
    let current_shares = T::StakingOps::share_balance(who, position_asset);
    let resolved = match spec {
      AmountResolution::Fixed(shares) => *shares,
      AmountResolution::AllBalance => current_shares,
      AmountResolution::PercentageOfCurrent(pct) => pct.mul_floor(current_shares),
      AmountResolution::PercentageOfTrigger(pct) => pct.mul_floor(Self::trigger_balance(
        trigger_share_balances,
        ResolutionSurface::StakingShares(position_asset),
      )?),
      AmountResolution::PercentageOfLastFunding(pct) => {
        let share_asset =
          T::StakingOps::share_asset(position_asset).ok_or(Error::<T>::InvalidAmountResolution)?;
        let Some(snapshot) = funding_snapshots.get(&share_asset) else {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        };
        if snapshot.amount.is_zero() {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        }
        pct.mul_floor(snapshot.amount)
      }
    };
    if resolved.is_zero() {
      return Ok(AmountResolutionOutcome::Skipped);
    }
    if resolved > current_shares {
      return Ok(AmountResolutionOutcome::FundingUnavailable);
    }
    Ok(AmountResolutionOutcome::Resolved(resolved))
  }

  fn resolve_amount_with_policy(
    spec: &AmountResolution<T::Balance>,
    asset: T::AssetId,
    who: &T::AccountId,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingBatch<T::Balance>,
      T::MaxFundingTrackedAssets,
    >,
    policy: AmountResolutionPolicy,
  ) -> Result<AmountResolutionOutcome<T::Balance>, DispatchError> {
    let spendable_current = Self::spendable_balance(who, asset, reserved);
    let policy_spend_limit = if policy == AmountResolutionPolicy::PreserveSpend {
      spendable_current.saturating_sub(T::AssetOps::minimum_balance(asset))
    } else {
      spendable_current
    };
    let resolved = match spec {
      AmountResolution::Fixed(amount) => *amount,
      AmountResolution::AllBalance => policy_spend_limit,
      AmountResolution::PercentageOfCurrent(pct) => {
        let value = pct.mul_floor(policy_spend_limit);
        if !pct.is_zero() && !policy_spend_limit.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
      AmountResolution::PercentageOfTrigger(pct) => {
        let trigger_balance =
          Self::trigger_balance(trigger_balances, ResolutionSurface::Asset(asset))?;
        let value = pct.mul_floor(trigger_balance);
        if !pct.is_zero() && !trigger_balance.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
      AmountResolution::PercentageOfLastFunding(pct) => {
        let Some(snapshot) = funding_snapshots.get(&asset) else {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        };
        if snapshot.amount.is_zero() {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        }
        let value = pct.mul_floor(snapshot.amount);
        if !pct.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
    };
    if resolved.is_zero() {
      return Ok(AmountResolutionOutcome::Skipped);
    }
    if policy != AmountResolutionPolicy::Mint && resolved > policy_spend_limit {
      return Ok(AmountResolutionOutcome::FundingUnavailable);
    }
    Ok(AmountResolutionOutcome::Resolved(resolved))
  }
}

#[cfg(test)]
mod step_control_tests {
  use super::*;

  #[test]
  fn step_control_matrix_is_exhaustive() {
    let fixed_cases = [
      (
        StepResult::Completed,
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        StepResult::Completed,
        StepErrorPolicy::AbortCycle,
        StepControl::Advance,
      ),
      (
        StepResult::Completed,
        StepErrorPolicy::RetryLater,
        StepControl::Advance,
      ),
      (
        StepResult::Stopped,
        StepErrorPolicy::ContinueNextStep,
        StepControl::CompleteCycle,
      ),
      (
        StepResult::Stopped,
        StepErrorPolicy::AbortCycle,
        StepControl::CompleteCycle,
      ),
      (
        StepResult::Stopped,
        StepErrorPolicy::RetryLater,
        StepControl::CompleteCycle,
      ),
      (
        StepResult::Failed(RetryClass::Permanent),
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        StepResult::Failed(RetryClass::Permanent),
        StepErrorPolicy::AbortCycle,
        StepControl::Terminate,
      ),
      (
        StepResult::Failed(RetryClass::Permanent),
        StepErrorPolicy::RetryLater,
        StepControl::Terminate,
      ),
      (
        StepResult::Failed(RetryClass::Temporary),
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        StepResult::Failed(RetryClass::Temporary),
        StepErrorPolicy::AbortCycle,
        StepControl::Terminate,
      ),
    ];
    for mutability in [Mutability::Mutable, Mutability::Immutable] {
      for (result, policy, expected) in fixed_cases {
        assert_eq!(resolve_step_control(mutability, result, policy), expected);
      }
      assert_eq!(
        resolve_step_control(
          mutability,
          StepResult::Failed(RetryClass::Temporary),
          StepErrorPolicy::RetryLater,
        ),
        if mutability == Mutability::Mutable {
          StepControl::SuspendCurrent
        } else {
          StepControl::Terminate
        },
      );
    }
  }

  #[test]
  fn funding_unavailable_uses_the_same_control_owner() {
    for mutability in [Mutability::Mutable, Mutability::Immutable] {
      for policy in [
        StepErrorPolicy::ContinueNextStep,
        StepErrorPolicy::AbortCycle,
      ] {
        assert_eq!(
          resolve_step_control(mutability, StepResult::FundingUnavailable, policy),
          StepControl::Advance,
        );
      }
    }
    assert_eq!(
      resolve_step_control(
        Mutability::Mutable,
        StepResult::FundingUnavailable,
        StepErrorPolicy::RetryLater,
      ),
      StepControl::SuspendCurrent,
    );
    assert_eq!(
      resolve_step_control(
        Mutability::Immutable,
        StepResult::FundingUnavailable,
        StepErrorPolicy::RetryLater,
      ),
      StepControl::Terminate,
    );
  }

  #[test]
  fn condition_set_aggregation_never_short_circuits_truth_or_error() {
    use polkadot_sdk::frame_support::traits::ConstU32;

    let all = ConditionSet::<u8, ConstU32<4>>::All(
      BoundedVec::try_from(alloc::vec![1, 2]).expect("two atoms fit"),
    );
    let mut all_visited = alloc::vec::Vec::new();
    let all_result = evaluate_condition_set_with(&all, |atom| {
      all_visited.push(*atom);
      Ok::<_, &'static str>(*atom == 2)
    });
    assert_eq!(all_result, Ok(false));
    assert_eq!(all_visited, alloc::vec![1, 2]);

    let any = ConditionSet::<u8, ConstU32<4>>::Any(
      BoundedVec::try_from(alloc::vec![1, 2, 3]).expect("three atoms fit"),
    );
    let mut any_visited = alloc::vec::Vec::new();
    let any_result = evaluate_condition_set_with(&any, |atom| {
      any_visited.push(*atom);
      match atom {
        1 => Ok(true),
        2 => Err("condition failed"),
        _ => Ok(false),
      }
    });
    assert_eq!(any_result, Err("condition failed"));
    assert_eq!(any_visited, alloc::vec![1, 2, 3]);
  }

  #[test]
  fn conditions_and_amount_resolution_do_not_write_storage() {
    use crate::mock::{ALICE, TEST_INITIAL_BALANCE, Test, TestAsset, new_test_ext};
    use polkadot_sdk::sp_runtime::StateVersion;

    new_test_ext().execute_with(|| {
      let conditions = ConditionSet::All(
        BoundedVec::try_from(alloc::vec![Condition::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 1,
        }])
        .expect("one condition fits"),
      );
      let before_conditions = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Pallet::<Test>::evaluate_condition_set(&conditions, &ALICE, 0),
        Ok(true),
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        before_conditions,
      );

      let trigger_snapshot = ContinuationSnapshotOf::<Test>::default();
      let funding_snapshots = FundingSnapshotsOf::<Test>::default();
      let before_resolution = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Pallet::<Test>::resolve_amount_with_policy(
          &AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
          TestAsset::Native,
          &ALICE,
          0,
          &trigger_snapshot,
          &funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        ),
        Ok(AmountResolutionOutcome::Resolved(
          (TEST_INITIAL_BALANCE - 1) / 2,
        )),
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        before_resolution,
      );
    });
  }
}
