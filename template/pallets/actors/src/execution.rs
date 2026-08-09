use super::pallet::*;
use super::types::{InputLimit, Task as ActorTask};
use super::{
  AssetOps, DexOps, FeeChargeKind, FeeCollector, LiquidityOps, ObservationProvider as _,
  RetryClass, ScalarObservationState, StakingOps, TaskFailure, WeightInfo as _,
  fee_native_protected_minimum, settle_attempt_fee_step,
};
use frame::prelude::*;
use polkadot_sdk::{
  sp_runtime::{
    Perbill,
    traits::{SaturatedConversion, Zero},
  },
  sp_weights::WeightToFee as _,
};

/// Checked increment for protocol-semantic counters. The admitted bound
/// (`MaxExecutionPlanSteps * (MaxRetryAttempts + 1)` for outcome totals) precludes
/// overflow; a violation fails closed before mutation with an invariant error rather
/// than silently saturating (spec 4.4).
fn checked_semantic_increment(counter: u32) -> Result<u32, DispatchError> {
  counter
    .checked_add(1)
    .ok_or(DispatchError::Other("SemanticCounterOverflow"))
}

// Any extrinsic or runtime entrypoint that can fail after mutating multiple storage
// locations SHOULD either pre-validate all fallible conditions first or use
// transactional semantics so capacity / late-guard failures cannot strand partial state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AmountResolutionPolicy {
  PreserveSpend,
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
  SwapIn {
    asset_in: T::AssetId,
    amount_in: T::Balance,
    asset_out: T::AssetId,
    slippage_tolerance: Perbill,
  },
  SwapOut {
    asset_out: T::AssetId,
    amount_out: T::Balance,
    asset_in: T::AssetId,
    max_amount_in: T::Balance,
    slippage_tolerance: Perbill,
  },
  AddLiquidity {
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount_a: T::Balance,
    amount_b: T::Balance,
    min_lp_out: T::Balance,
  },
  RemoveLiquidity {
    lp_asset: T::AssetId,
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    lp_amount: T::Balance,
    min_amount_a: T::Balance,
    min_amount_b: T::Balance,
  },
  Stake {
    asset: T::AssetId,
    amount: T::Balance,
  },
  DonateLiquidity {
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount: T::Balance,
    max_amount_b: T::Balance,
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

fn resolve_step_control(result: StepResult, error_policy: StepErrorPolicy) -> StepControl {
  match (result, error_policy) {
    (StepResult::Completed, _) => StepControl::Advance,
    (StepResult::Stopped, _) => StepControl::CompleteCycle,
    (StepResult::FundingUnavailable, StepErrorPolicy::RetryLater { .. }) => {
      StepControl::SuspendCurrent
    }
    (StepResult::FundingUnavailable, _) => StepControl::Advance,
    (StepResult::Failed(_), StepErrorPolicy::ContinueNextStep) => StepControl::Advance,
    (StepResult::Failed(RetryClass::Temporary), StepErrorPolicy::RetryLater { .. }) => {
      StepControl::SuspendCurrent
    }
    (StepResult::Failed(_), StepErrorPolicy::AbortCycle | StepErrorPolicy::RetryLater { .. }) => {
      StepControl::Terminate
    }
  }
}

impl<T: Config> Pallet<T> {
  pub(crate) fn cancel_continuation_internal(
    actor_id: ActorId,
    reason: CancellationReason,
    outcomes: Option<OutcomeTotals>,
  ) -> Result<bool, DispatchError> {
    let Some(continuation) = ContinuationStateStore::<T>::get(actor_id) else {
      return Ok(false);
    };
    let hot = ActorHot::<T>::get(actor_id).ok_or(Error::<T>::ContinuationInvariant)?;
    let identity = ActorIdentities::<T>::get(actor_id).ok_or(Error::<T>::ContinuationInvariant)?;
    ensure!(
      hot.cycle_state == CycleState::Suspended && identity.cycle_nonce > 0,
      Error::<T>::ContinuationInvariant
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Continuation prevalidation requires active hot state");
      hot.cycle_state = CycleState::Idle;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
    });
    ContinuationStateStore::<T>::remove(actor_id);
    let totals = outcomes.unwrap_or(continuation.cumulative_outcomes);
    Self::deposit_event(Event::CycleCancelled {
      actor_id,
      cycle_nonce: identity.cycle_nonce,
      reason,
    });
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce: identity.cycle_nonce,
      result: CycleResult::Cancelled,
      outcomes: totals,
    });
    Ok(true)
  }

  pub(crate) fn write_continuation_state(
    actor_id: ActorId,
    state: Option<ContinuationStateOf<T>>,
  ) -> DispatchResult {
    ensure!(
      ActorHot::<T>::contains_key(actor_id),
      Error::<T>::ActorNotFound
    );
    let identity = ActorIdentities::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
    if let Some(continuation) = state.as_ref() {
      let program = ActorProgram::<T>::get(actor_id).ok_or(Error::<T>::ContinuationInvariant)?;
      ensure!(
        identity.mutability == Mutability::Mutable
          && continuation.cursor < program.execution_plan.len() as u32,
        Error::<T>::ContinuationInvariant
      );
      let max_attempts = program.execution_plan[continuation.cursor as usize]
        .on_error
        .retry_max_attempts()
        .ok_or(Error::<T>::ContinuationInvariant)?;
      ensure!(
        continuation.unsuccessful_attempts_at_cursor > 0
          && continuation.unsuccessful_attempts_at_cursor < max_attempts,
        Error::<T>::ContinuationInvariant
      );
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      ActorHot::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor existence was prevalidated")
          .cycle_state = if state.is_some() {
          CycleState::Suspended
        } else {
          CycleState::Idle
        };
      });
      if let Some(continuation) = state {
        ContinuationStateStore::<T>::insert(actor_id, continuation);
      } else {
        ContinuationStateStore::<T>::remove(actor_id);
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  pub(crate) fn persist_continuation_suspension(
    actor_id: ActorId,
    cycle_nonce: u64,
    state: ContinuationStateOf<T>,
    reason: SuspensionReason,
  ) -> DispatchResult {
    let attempt = state.attempt;
    let cursor = state.cursor;
    let cumulative_outcomes = state.cumulative_outcomes;
    Self::write_continuation_state(actor_id, Some(state))?;
    Self::deposit_event(Event::CycleSuspended {
      actor_id,
      cycle_nonce,
      attempt,
      cursor,
      reason,
      cumulative_outcomes,
    });
    Ok(())
  }

  pub(crate) fn begin_continuation_attempt(
    actor_id: ActorId,
    cycle_nonce: u64,
    now: BlockNumberFor<T>,
  ) -> ContinuationStateOf<T> {
    ContinuationStateStore::<T>::mutate(actor_id, |maybe| {
      let continuation = maybe
        .as_mut()
        .expect("Suspended cycle_state requires ContinuationState");
      continuation.attempt = checked_semantic_increment(continuation.attempt)
        .expect("attempt bound (MaxRetryAttempts) is precluded by admission");
      continuation.last_attempt_block = now;
    });
    let continuation = ContinuationStateStore::<T>::get(actor_id)
      .expect("Suspended cycle_state requires ContinuationState");
    Self::deposit_event(Event::CycleContinued {
      actor_id,
      cycle_nonce,
      attempt: continuation.attempt,
      cursor: continuation.cursor,
    });
    continuation
  }

  pub(crate) fn record_stop_cycle_event(actor_id: ActorId, cycle_nonce: u64, step_index: u32) {
    Self::deposit_event(Event::CycleStopped {
      actor_id,
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
    actor_id: ActorId,
    instance: ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
  ) -> (Weight, bool) {
    let result = Self::execute_single_cycle_traced(actor_id, instance, now, None);
    (result.0, result.3)
  }

  pub(crate) fn execute_single_cycle_traced(
    actor_id: ActorId,
    instance: ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
    mut trace: Option<&mut alloc::vec::Vec<SimulationStepRecord>>,
  ) -> (Weight, bool, Option<CloseReason>, bool) {
    let base_weight = T::WeightInfo::cycle_orchestration();
    let is_continuation = instance.cycle_state == CycleState::Suspended;
    let actor = instance.sovereign_account.clone();
    let execution_plan = &instance.execution_plan;
    let fee_envelope = Self::attempt_fee_envelope(
      instance.actor_class.actor_type(),
      execution_plan,
      if is_continuation {
        Self::continuation_state(actor_id).map_or(0, |state| state.cursor as usize)
      } else {
        0
      },
    )
    .expect("admitted execution plans have a checked fee envelope");
    let mut reserved_fee_remaining = fee_envelope.total;
    let mut fee_collection_failed = false;
    macro_rules! collect_step_fee {
      ($fee:expr) => {{
        let result = Self::collect_user_step_fee(&actor, $fee);
        if result.is_err() {
          fee_collection_failed = true;
        }
        result
      }};
    }
    let (
      cycle_nonce,
      start_cursor,
      attempt,
      prior_unsuccessful_attempts_at_cursor,
      cumulative_outcomes,
      funding_snapshot,
      opening_snapshot,
    ) = if is_continuation {
      let continuation = Self::begin_continuation_attempt(actor_id, instance.cycle_nonce, now);
      (
        instance.cycle_nonce,
        continuation.cursor,
        continuation.attempt,
        continuation.unsuccessful_attempts_at_cursor,
        continuation.cumulative_outcomes,
        continuation.funding_snapshot,
        continuation.opening_snapshot,
      )
    } else {
      if instance.cycle_nonce == u64::MAX {
        Self::finalize_actor(actor_id, &instance, CloseReason::CycleNonceExhausted)
          .expect("fresh execution snapshot satisfies terminal preconditions");
        return (
          base_weight,
          true,
          Some(CloseReason::CycleNonceExhausted),
          false,
        );
      }
      let funding_snapshot = ActorFunding::<T>::get(actor_id)
        .map(|funding| funding.funding_accumulated)
        .unwrap_or_default();
      let opening_snapshot = Self::capture_opening_snapshot(
        instance.actor_class.actor_type(),
        &actor,
        execution_plan,
        reserved_fee_remaining,
      );
      let Some(cycle_nonce) = ActorIdentities::<T>::mutate(actor_id, |maybe| {
        let identity = maybe.as_mut()?;
        identity.cycle_nonce = identity
          .cycle_nonce
          .checked_add(1)
          .expect("nonce-exhausted actors close before run opening");
        Some(identity.cycle_nonce)
      }) else {
        return (base_weight, true, None, false);
      };
      ActorHot::<T>::mutate(actor_id, |maybe| {
        if let Some(hot) = maybe.as_mut() {
          hot.pending_signal = false;
          hot.last_cycle_block = Some(now);
        }
      });
      ActorFunding::<T>::mutate(actor_id, |maybe| {
        if let Some(funding) = maybe.as_mut() {
          funding.funding_accumulated.clear();
        }
      });
      (
        cycle_nonce,
        0,
        0,
        0,
        OutcomeTotals::default(),
        funding_snapshot,
        opening_snapshot,
      )
    };
    let is_user = instance.actor_class.actor_type() == ActorType::User;
    let funding_snapshots = &funding_snapshot;
    let mut executed_steps = cumulative_outcomes.executed_steps;
    let mut committed_effectful_tasks = cumulative_outcomes.committed_effectful_tasks;
    let mut skipped_conditions = cumulative_outcomes.skipped_conditions;
    let mut skipped_resolution = cumulative_outcomes.skipped_resolution;
    let mut skipped_funding_unavailable = cumulative_outcomes.skipped_funding_unavailable;
    let mut failed_steps = cumulative_outcomes.failed_steps;
    let mut attempt_executed_steps: u32 = 0;
    let mut execution_plan_failed = false;
    let mut failure_close_reason = None;
    let mut suspended_at: Option<(u32, SuspensionReason)> = None;
    if !is_continuation {
      Self::deposit_event(Event::CycleStarted {
        actor_id,
        cycle_nonce,
      });
    }
    for step_idx in start_cursor as usize..execution_plan.len() {
      let step = &execution_plan[step_idx];
      let step_num = step_idx as u32;
      let step_fee = &fee_envelope.steps[step_idx - start_cursor as usize];
      match Self::evaluate_condition_set(&step.conditions, &actor, reserved_fee_remaining) {
        Ok(true) => {}
        Ok(false) => {
          if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            if let Err(error) = collect_step_fee!(charged_fee) {
              failed_steps = checked_semantic_increment(failed_steps)
                .expect("semantic counter bound is precluded by admission");
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                actor_id,
                cycle_nonce,
                step_index: step_num,
                retry_class: RetryClass::Permanent,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(actor_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          skipped_conditions = checked_semantic_increment(skipped_conditions)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::ConditionsNotMet),
          );
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ConditionsNotMet,
          });
          continue;
        }
        Err(error) => {
          let charged_error = if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            collect_step_fee!(charged_fee).err().unwrap_or(error)
          } else {
            error
          };
          failed_steps = checked_semantic_increment(failed_steps)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            retry_class: RetryClass::Permanent,
            error: charged_error,
          });
          execution_plan_failed = Self::apply_error_policy(
            actor_id,
            cycle_nonce,
            step_num,
            step.on_error,
            charged_error,
          );
          if execution_plan_failed {
            break;
          }
          continue;
        }
      }
      let prepared_task = match Self::prepare_task(
        &step.task,
        &actor,
        instance.actor_class.actor_type(),
        reserved_fee_remaining,
        &opening_snapshot,
        funding_snapshots,
      ) {
        Ok(PreparedTaskOutcome::Executable(task)) => task,
        Ok(PreparedTaskOutcome::Skipped) => {
          if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            if let Err(error) = collect_step_fee!(charged_fee) {
              failed_steps = checked_semantic_increment(failed_steps)
                .expect("semantic counter bound is precluded by admission");
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                actor_id,
                cycle_nonce,
                step_index: step_num,
                retry_class: RetryClass::Permanent,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(actor_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          skipped_resolution = checked_semantic_increment(skipped_resolution)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::ResolutionSkipped),
          );
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          continue;
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            if let Err(error) = collect_step_fee!(charged_fee) {
              failed_steps = checked_semantic_increment(failed_steps)
                .expect("semantic counter bound is precluded by admission");
              Self::record_simulation_step(
                &mut trace,
                step_num,
                SimulationStepOutcome::Failed(RetryClass::Permanent),
              );
              Self::deposit_event(Event::StepFailed {
                actor_id,
                cycle_nonce,
                step_index: step_num,
                retry_class: RetryClass::Permanent,
                error,
              });
              execution_plan_failed =
                Self::apply_error_policy(actor_id, cycle_nonce, step_num, step.on_error, error);
              if execution_plan_failed {
                break;
              }
              continue;
            }
          }
          match resolve_step_control(StepResult::FundingUnavailable, step.on_error) {
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
          skipped_funding_unavailable = checked_semantic_increment(skipped_funding_unavailable)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Skipped(StepSkippedReason::FundingUnavailable),
          );
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::FundingUnavailable,
          });
          continue;
        }
        Err(error) => {
          let charged_error = if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            collect_step_fee!(charged_fee).err().unwrap_or(error)
          } else {
            error
          };
          failed_steps = checked_semantic_increment(failed_steps)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            retry_class: RetryClass::Permanent,
            error: charged_error,
          });
          execution_plan_failed = Self::apply_error_policy(
            actor_id,
            cycle_nonce,
            step_num,
            step.on_error,
            charged_error,
          );
          if execution_plan_failed {
            break;
          }
          continue;
        }
      };
      if is_user {
        let charged_fee = Self::settle_user_step_fee(
          &mut reserved_fee_remaining,
          step_fee,
          FeeChargeKind::Attempted,
        );
        if let Err(error) = collect_step_fee!(charged_fee) {
          failed_steps = checked_semantic_increment(failed_steps)
            .expect("semantic counter bound is precluded by admission");
          Self::record_simulation_step(
            &mut trace,
            step_num,
            SimulationStepOutcome::Failed(RetryClass::Permanent),
          );
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            retry_class: RetryClass::Permanent,
            error,
          });
          execution_plan_failed =
            Self::apply_error_policy(actor_id, cycle_nonce, step_num, step.on_error, error);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      }
      if let Err(failure) = Self::execute_prepared_task(
        prepared_task,
        actor_id,
        cycle_nonce,
        step_num,
        &actor,
        instance.actor_class.actor_type(),
      ) {
        failed_steps = checked_semantic_increment(failed_steps)
          .expect("semantic counter bound is precluded by admission");
        let retry = failure.retry;
        let control = resolve_step_control(StepResult::Failed(retry), step.on_error);
        Self::record_simulation_step(&mut trace, step_num, SimulationStepOutcome::Failed(retry));
        Self::deposit_event(Event::StepFailed {
          actor_id,
          cycle_nonce,
          step_index: step_num,
          retry_class: retry,
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
      let successful_result = if matches!(&step.task, ActorTask::StopCycle) {
        StepResult::Stopped
      } else {
        StepResult::Completed
      };
      let successful_control = resolve_step_control(successful_result, step.on_error);
      let simulation_outcome = if successful_control == StepControl::CompleteCycle {
        Self::record_stop_cycle_event(actor_id, cycle_nonce, step_num);
        SimulationStepOutcome::Stopped
      } else {
        debug_assert_eq!(successful_control, StepControl::Advance);
        SimulationStepOutcome::Executed
      };
      Self::record_simulation_step(&mut trace, step_num, simulation_outcome);
      executed_steps = checked_semantic_increment(executed_steps)
        .expect("semantic counter bound is precluded by admission");
      attempt_executed_steps = checked_semantic_increment(attempt_executed_steps)
        .expect("semantic counter bound is precluded by admission");
      if successful_control != StepControl::CompleteCycle {
        committed_effectful_tasks = checked_semantic_increment(committed_effectful_tasks)
          .expect("semantic counter bound is precluded by admission");
      }
      if successful_control == StepControl::CompleteCycle {
        break;
      }
    }
    let attempt_weight = if attempt_executed_steps == 0 {
      base_weight
    } else {
      T::WeightInfo::step_orchestration(attempt_executed_steps)
    };
    let mut failure_already_recorded = false;
    if let Some((cursor, suspension_reason)) = suspended_at {
      let consecutive_failures = ActorHot::<T>::mutate(actor_id, |maybe| {
        let Some(hot) = maybe.as_mut() else {
          return 0;
        };
        hot.consecutive_failures = hot
          .consecutive_failures
          .checked_add(1)
          .expect("attempt admission excludes an exhausted failure counter");
        hot.consecutive_failures
      });
      failure_already_recorded = true;
      let unsuccessful_attempts_at_cursor = if is_continuation && cursor == start_cursor {
        prior_unsuccessful_attempts_at_cursor
          .checked_add(1)
          .expect("admitted cursor-local retry counter remains below its u32 bound")
      } else {
        1
      };
      let max_attempts = execution_plan[cursor as usize]
        .on_error
        .retry_max_attempts()
        .expect("suspension requires bounded RetryLater");
      let local_limit_reached = unsuccessful_attempts_at_cursor >= max_attempts;
      let global_limit_reached = Self::failure_limit_reached(consecutive_failures);
      if !local_limit_reached && !global_limit_reached {
        let cumulative_outcomes = OutcomeTotals {
          executed_steps,
          committed_effectful_tasks,
          skipped_conditions,
          skipped_resolution,
          skipped_funding_unavailable,
          failed_steps,
        };
        Self::persist_continuation_suspension(
          actor_id,
          cycle_nonce,
          ContinuationState {
            cursor,
            attempt,
            unsuccessful_attempts_at_cursor,
            last_attempt_block: now,
            opening_snapshot: Self::trim_opening_snapshot(
              execution_plan,
              cursor as usize,
              &opening_snapshot,
            ),
            funding_snapshot: funding_snapshot.clone(),
            cumulative_outcomes,
          },
          suspension_reason,
        )
        .expect("admitted mutable RetryLater plan has a valid unresolved cursor");
        return (attempt_weight, false, None, fee_collection_failed);
      }
      failure_close_reason = Some(if local_limit_reached {
        CloseReason::RetryAttemptsExhausted
      } else {
        CloseReason::ConsecutiveFailures
      });
      execution_plan_failed = true;
    }
    if is_continuation {
      Self::write_continuation_state(actor_id, None)
        .expect("terminal Continuation can be cleared atomically");
    }
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let Some(hot) = maybe.as_mut() else {
        return;
      };
      if execution_plan_failed {
        if !failure_already_recorded {
          hot.consecutive_failures = checked_semantic_increment(hot.consecutive_failures)
            .expect("failure bound (MaxConsecutiveFailures) is precluded by admission");
        }
      } else {
        hot.consecutive_failures = 0;
      }
    });
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: if execution_plan_failed {
        CycleResult::Failed
      } else {
        CycleResult::Completed
      },
      outcomes: OutcomeTotals {
        executed_steps,
        committed_effectful_tasks,
        skipped_conditions,
        skipped_resolution,
        skipped_funding_unavailable,
        failed_steps,
      },
    });
    let mut terminal_close_reason = None;
    if execution_plan_failed {
      if let Some(inst) = Self::active_actor_view(actor_id) {
        let close_reason = failure_close_reason.or_else(|| {
          Self::failure_limit_reached(inst.consecutive_failures)
            .then_some(CloseReason::ConsecutiveFailures)
        });
        if !inst.lifecycle.is_paused() {
          if let Some(reason) = close_reason {
            Self::finalize_actor(actor_id, &inst, reason)
              .expect("fresh execution snapshot satisfies terminal preconditions");
            terminal_close_reason = Some(reason);
          }
        }
      }
    } else if let Some(inst) = Self::active_actor_view(actor_id) {
      if inst.completion_policy == CompletionPolicy::CloseAfterProductiveCycle
        && committed_effectful_tasks > 0
      {
        Self::finalize_actor(actor_id, &inst, CloseReason::ProductiveCycleCompleted)
          .expect("fresh productive execution snapshot satisfies terminal preconditions");
        terminal_close_reason = Some(CloseReason::ProductiveCycleCompleted);
      } else if let Some(target_nonce) = inst.auto_close_at_cycle_nonce {
        if cycle_nonce >= target_nonce {
          Self::finalize_actor(actor_id, &inst, CloseReason::AutoCloseNonceReached)
            .expect("fresh execution snapshot satisfies terminal preconditions");
          terminal_close_reason = Some(CloseReason::AutoCloseNonceReached);
        }
      }
    }
    (
      attempt_weight,
      execution_plan_failed,
      terminal_close_reason,
      fee_collection_failed,
    )
  }

  pub fn simulate_current_program(
    actor_id: ActorId,
    expected_type: ActorType,
    expected_mutability: Mutability,
    expected_program: ProgramInputOf<T>,
    mode: SimulationMode,
  ) -> Result<SimulationResultOf<T>, SimulationError> {
    let instance = Self::active_actor_for_classification(actor_id)
      .map_err(|error| match error {
        ActorClassificationError::ActorInvariant => SimulationError::ActorInvariant,
        ActorClassificationError::ContinuationInvariant => SimulationError::ContinuationInvariant,
        ActorClassificationError::ComputationOverflow => SimulationError::ComputationOverflow,
      })?
      .ok_or(SimulationError::ActorNotFound)?;
    let stored_funding = ActorFunding::<T>::get(actor_id).ok_or(SimulationError::ActorInvariant)?;
    let continuation = ContinuationStateStore::<T>::get(actor_id);
    if (instance.cycle_state == CycleState::Suspended) != continuation.is_some() {
      return Err(SimulationError::ContinuationInvariant);
    }
    if continuation
      .as_ref()
      .is_some_and(|state| state.attempt.checked_add(1).is_none())
    {
      return Err(SimulationError::ComputationOverflow);
    }
    if instance.actor_class.actor_type() != expected_type {
      return Err(SimulationError::TypeMismatch);
    }
    if instance.mutability != expected_mutability {
      return Err(SimulationError::MutabilityMismatch);
    }
    let ProgramInput::Active(ActiveProgramInput {
      schedule,
      schedule_window,
      execution_plan,
      completion_policy,
      funding_source_policy,
      auto_close_at_cycle_nonce,
    }) = expected_program
    else {
      return Err(SimulationError::ProgramMismatch);
    };
    if instance.schedule != schedule
      || instance.schedule_window != schedule_window
      || instance.execution_plan != execution_plan
      || instance.completion_policy != completion_policy
      || stored_funding.funding_source_policy != funding_source_policy
      || instance.auto_close_at_cycle_nonce != auto_close_at_cycle_nonce
    {
      return Err(SimulationError::ProgramMismatch);
    }
    match mode {
      SimulationMode::FreshCurrentPlan if instance.cycle_state != CycleState::Idle => {
        return Err(SimulationError::ModeCycleStateMismatch);
      }
      SimulationMode::CurrentContinuation if instance.cycle_state != CycleState::Suspended => {
        return Err(SimulationError::ModeCycleStateMismatch);
      }
      _ => {}
    }
    let (cycle_nonce, attempt, start_cursor, initial_outcomes) = match mode {
      SimulationMode::FreshCurrentPlan => (
        instance.cycle_nonce.saturating_add(1),
        0,
        0,
        OutcomeTotals::default(),
      ),
      SimulationMode::CurrentContinuation => {
        let continuation = continuation
          .as_ref()
          .ok_or(SimulationError::ContinuationInvariant)?;
        (
          instance.cycle_nonce,
          continuation
            .attempt
            .checked_add(1)
            .ok_or(SimulationError::ComputationOverflow)?,
          continuation.cursor,
          continuation.cumulative_outcomes,
        )
      }
    };
    let classification =
      Self::classify_actor(actor_id, &instance).map_err(|error| match error {
        ActorClassificationError::ActorInvariant => SimulationError::ActorInvariant,
        ActorClassificationError::ContinuationInvariant => SimulationError::ContinuationInvariant,
        ActorClassificationError::ComputationOverflow => SimulationError::ComputationOverflow,
      })?;
    if classification.execution_phase == ActorExecutionPhase::GlobalCircuitBreaker {
      return Err(SimulationError::GlobalCircuitBreaker);
    }
    if let Some(reason) = classification.terminal_reason {
      return Ok(SimulationResult {
        status: SimulationStatus::Closed(reason),
        cycle_nonce: instance.cycle_nonce,
        attempt: attempt.saturating_sub(u32::from(mode == SimulationMode::CurrentContinuation)),
        start_cursor,
        continuation_cursor: None,
        unsuccessful_attempts_at_cursor: None,
        cumulative_outcomes: initial_outcomes,
        steps: BoundedVec::default(),
      });
    }
    match classification.execution_phase {
      ActorExecutionPhase::Paused => return Err(SimulationError::Paused),
      ActorExecutionPhase::Ready => {}
      ActorExecutionPhase::WaitingRetry(_)
      | ActorExecutionPhase::WaitingTemporal(_)
      | ActorExecutionPhase::WaitingSignal => return Err(SimulationError::NotReady),
      ActorExecutionPhase::GlobalCircuitBreaker => unreachable!("handled above"),
    }
    let now = frame_system::Pallet::<T>::block_number();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let mut trace = alloc::vec::Vec::new();
      let (_, failed, close_reason, fee_collection_failed) =
        Self::execute_single_cycle_traced(actor_id, instance, now, Some(&mut trace));
      if fee_collection_failed {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          SimulationError::FeeCollectionFailed,
        ));
      }
      let continuation = ContinuationStateStore::<T>::get(actor_id);
      let status = if continuation.is_some() {
        SimulationStatus::Suspended
      } else if let Some(reason) = close_reason {
        SimulationStatus::Closed(reason)
      } else if failed {
        SimulationStatus::Failed
      } else {
        SimulationStatus::Completed
      };
      let continuation_cursor = continuation.as_ref().map(|state| state.cursor);
      let unsuccessful_attempts_at_cursor = continuation
        .as_ref()
        .map(|state| state.unsuccessful_attempts_at_cursor);
      let mut cumulative_outcomes = initial_outcomes;
      for record in &trace {
        match record.outcome {
          SimulationStepOutcome::Executed => {
            cumulative_outcomes.executed_steps =
              cumulative_outcomes.executed_steps.saturating_add(1);
            cumulative_outcomes.committed_effectful_tasks = cumulative_outcomes
              .committed_effectful_tasks
              .saturating_add(1);
          }
          SimulationStepOutcome::Stopped => {
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
      let Ok(steps) = BoundedVec::try_from(trace) else {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          SimulationError::ContinuationInvariant,
        ));
      };
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(SimulationResult {
        status,
        cycle_nonce,
        attempt,
        start_cursor,
        continuation_cursor,
        unsuccessful_attempts_at_cursor,
        cumulative_outcomes,
        steps,
      }))
    })
  }

  pub(crate) fn failure_limit_reached(consecutive_failures: u32) -> bool {
    let max_failures = T::MaxConsecutiveFailures::get();
    max_failures > 0 && consecutive_failures >= max_failures
  }

  pub(crate) fn compute_eval_fee_checked(num_conditions: u32) -> Result<BalanceOf<T>, Error<T>> {
    let evaluation_weight = T::WeightInfo::condition_set_evaluation(num_conditions)
      .saturating_add(T::WeightInfo::fee_collection());
    Ok(T::WeightToFee::weight_to_fee(&evaluation_weight))
  }

  #[cfg(test)]
  pub(crate) fn compute_eval_fee(num_conditions: u32) -> BalanceOf<T> {
    Self::compute_eval_fee_checked(num_conditions)
      .expect("admitted execution plans have checked evaluation fees")
  }

  fn settle_user_step_fee(
    reservation: &mut T::Balance,
    step: &super::StepFeeEnvelope<T::Balance>,
    charge_kind: FeeChargeKind,
  ) -> T::Balance {
    let settlement = settle_attempt_fee_step(ActorType::User, *reservation, step, charge_kind)
      .expect("admitted User plans preserve their fee reservation");
    *reservation = settlement.reservation_remaining;
    settlement.charged
  }

  fn collect_user_step_fee(actor: &T::AccountId, fee: T::Balance) -> DispatchResult {
    if fee.is_zero() {
      return Ok(());
    }
    let native = T::FeeNativeAssetId::get();
    if T::AssetOps::balance(actor, native) < fee {
      return Err(DispatchError::Other("StepFeeBalanceInvariant"));
    }
    T::FeeCollector::collect_fee(actor, &T::FeeSink::get(), native, fee)
      .map_err(|_| DispatchError::Other("StepFeeTransferFailed"))
  }

  fn apply_error_policy(
    _actor_id: ActorId,
    _cycle_nonce: u64,
    _step: u32,
    policy: StepErrorPolicy,
    _failure: DispatchError,
  ) -> bool {
    resolve_step_control(StepResult::Failed(RetryClass::Permanent), policy)
      == StepControl::Terminate
  }

  fn push_trigger_surface(
    amount: &AmountResolution<T::Balance>,
    surface: OpeningSurface<T::AssetId>,
    surfaces: &mut alloc::vec::Vec<OpeningSurface<T::AssetId>>,
  ) {
    if matches!(amount, AmountResolution::PercentageAtOpening(_)) && !surfaces.contains(&surface) {
      surfaces.push(surface);
    }
  }

  fn collect_percentage_opening_surfaces(
    task: &TaskOf<T>,
    surfaces: &mut alloc::vec::Vec<OpeningSurface<T::AssetId>>,
  ) {
    match task {
      ActorTask::Transfer { asset, amount, .. }
      | ActorTask::SplitTransfer { asset, amount, .. }
      | ActorTask::Burn { asset, amount } => {
        Self::push_trigger_surface(amount, OpeningSurface::PreservableAsset(*asset), surfaces)
      }
      ActorTask::Mint { asset, amount } => {
        Self::push_trigger_surface(amount, OpeningSurface::TargetAsset(*asset), surfaces)
      }
      ActorTask::RemoveLiquidity {
        lp_asset: asset,
        lp_amount,
        ..
      } => Self::push_trigger_surface(
        lp_amount,
        OpeningSurface::PreservableAsset(*asset),
        surfaces,
      ),
      ActorTask::SwapIn {
        asset_in,
        amount_in,
        ..
      } => Self::push_trigger_surface(
        amount_in,
        OpeningSurface::PreservableAsset(*asset_in),
        surfaces,
      ),
      ActorTask::SwapOut {
        asset_out,
        amount_out,
        ..
      } => Self::push_trigger_surface(
        amount_out,
        OpeningSurface::TargetAsset(*asset_out),
        surfaces,
      ),
      ActorTask::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
        ..
      } => {
        Self::push_trigger_surface(
          amount_a,
          OpeningSurface::PreservableAsset(*asset_a),
          surfaces,
        );
        Self::push_trigger_surface(
          amount_b,
          OpeningSurface::PreservableAsset(*asset_b),
          surfaces,
        );
      }
      ActorTask::Stake { asset, amount } => {
        Self::push_trigger_surface(amount, OpeningSurface::PreservableAsset(*asset), surfaces);
      }
      ActorTask::DonateLiquidity {
        asset_a,
        max_amount_a,
        ..
      } => {
        Self::push_trigger_surface(
          max_amount_a,
          OpeningSurface::PreservableAsset(*asset_a),
          surfaces,
        );
      }
      ActorTask::Unstake { asset, shares } => {
        Self::push_trigger_surface(shares, OpeningSurface::StakingShares(*asset), surfaces)
      }
      ActorTask::StopCycle => {}
    }
  }

  pub(crate) fn opening_surfaces(
    execution_plan: &ExecutionPlanOf<T>,
    start_cursor: usize,
  ) -> alloc::vec::Vec<OpeningSurface<T::AssetId>> {
    let mut surfaces = alloc::vec::Vec::new();
    for step_index in start_cursor..execution_plan.len() {
      Self::collect_percentage_opening_surfaces(&execution_plan[step_index].task, &mut surfaces);
    }
    surfaces
  }

  fn capture_opening_snapshot(
    actor_type: ActorType,
    actor: &T::AccountId,
    execution_plan: &ExecutionPlanOf<T>,
    reserved: T::Balance,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::opening_surfaces(execution_plan, 0) {
      let balance = match surface {
        OpeningSurface::PreservableAsset(asset) => {
          Self::preservable_balance(actor_type, actor, asset, reserved)
        }
        OpeningSurface::TargetAsset(asset) => Self::spendable_balance(actor, asset, reserved),
        OpeningSurface::StakingShares(asset) => T::StakingOps::share_balance(actor, asset),
      };
      snapshot
        .try_insert(surface, balance)
        .unwrap_or_else(|_| panic!("trigger surfaces fit MaxOpeningSnapshotEntries"));
    }
    snapshot
  }

  fn trim_opening_snapshot(
    execution_plan: &ExecutionPlanOf<T>,
    start_cursor: usize,
    source: &ContinuationSnapshotOf<T>,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::opening_surfaces(execution_plan, start_cursor) {
      if let Some(balance) = source.get(&surface) {
        snapshot
          .try_insert(surface, *balance)
          .unwrap_or_else(|_| panic!("suffix surfaces fit MaxOpeningSnapshotEntries"));
      }
    }
    snapshot
  }

  fn opening_balance(
    opening_snapshot: &ContinuationSnapshotOf<T>,
    surface: OpeningSurface<T::AssetId>,
  ) -> Result<T::Balance, DispatchError> {
    opening_snapshot
      .get(&surface)
      .copied()
      .ok_or(Error::<T>::SnapshotUnavailable.into())
  }

  fn prepare_task(
    task: &TaskOf<T>,
    actor: &T::AccountId,
    actor_type: ActorType,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &FundingSnapshotOf<T>,
  ) -> Result<PreparedTaskOutcome<T>, DispatchError> {
    match task {
      ActorTask::Transfer { to, asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          actor_type,
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
      ActorTask::SplitTransfer {
        asset,
        amount,
        legs,
      } => {
        Self::validate_split_transfer_legs(legs)?;
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          actor_type,
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
        let mut has_effective_leg = false;
        for leg in legs.as_slice() {
          if !leg.share.mul_floor(resolved).is_zero() {
            has_effective_leg = true;
            break;
          }
        }
        if resolved.is_zero() || !has_effective_leg {
          // A zero total or a positive total rounded to zero across every leg is an explicit
          // resolution skip: no preflight, balance mutation, or execution event occurs.
          return Ok(PreparedTaskOutcome::Skipped);
        }
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::SplitTransfer {
            asset: *asset,
            total: resolved,
            legs: legs.clone(),
          },
        ))
      }
      ActorTask::Burn { asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          actor_type,
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
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Burn {
          asset: *asset,
          amount: resolved,
        }))
      }
      ActorTask::Mint { asset, amount } => {
        ensure!(
          actor_type == ActorType::System,
          Error::<T>::MintNotAllowedForUserActor
        );
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          actor_type,
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
      ActorTask::SwapIn {
        asset_in,
        amount_in,
        asset_out,
        slippage_tolerance,
      } => {
        let resolved = match Self::resolve_for_task(
          amount_in,
          *asset_in,
          actor,
          actor_type,
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
        Ok(PreparedTaskOutcome::Executable(PreparedTask::SwapIn {
          asset_in: *asset_in,
          amount_in: resolved,
          asset_out: *asset_out,
          slippage_tolerance: *slippage_tolerance,
        }))
      }
      ActorTask::SwapOut {
        asset_out,
        amount_out,
        asset_in,
        input_limit,
        slippage_tolerance,
      } => {
        let resolved = match Self::resolve_for_task(
          amount_out,
          *asset_out,
          actor,
          actor_type,
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
        let preservable_input_capacity =
          Self::preservable_balance(actor_type, actor, *asset_in, reserved);
        let max_amount_in = match input_limit {
          InputLimit::LiveQuote => preservable_input_capacity,
          InputLimit::Absolute(authored_max) => (*authored_max).min(preservable_input_capacity),
        };
        if max_amount_in.is_zero() {
          return Ok(PreparedTaskOutcome::FundingUnavailable);
        }
        Ok(PreparedTaskOutcome::Executable(PreparedTask::SwapOut {
          asset_out: *asset_out,
          amount_out: resolved,
          asset_in: *asset_in,
          max_amount_in,
          slippage_tolerance: *slippage_tolerance,
        }))
      }
      ActorTask::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
        min_lp_out,
      } => {
        let outcome_a = Self::resolve_for_task(
          amount_a,
          *asset_a,
          actor,
          actor_type,
          reserved,
          trigger_balances,
          funding_snapshots,
          AmountResolutionPolicy::PreserveSpend,
        )?;
        let outcome_b = Self::resolve_for_task(
          amount_b,
          *asset_b,
          actor,
          actor_type,
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
              min_lp_out: *min_lp_out,
            },
          )),
        }
      }
      ActorTask::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount,
        min_amount_a,
        min_amount_b,
      } => {
        let resolved = match Self::resolve_for_task(
          lp_amount,
          *lp_asset,
          actor,
          actor_type,
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
            asset_a: *asset_a,
            asset_b: *asset_b,
            lp_amount: resolved,
            min_amount_a: *min_amount_a,
            min_amount_b: *min_amount_b,
          },
        ))
      }
      ActorTask::Stake { asset, amount } => {
        let resolved = match Self::resolve_for_task(
          amount,
          *asset,
          actor,
          actor_type,
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
      ActorTask::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a,
        max_ratio_error,
      } => {
        let resolved = match Self::resolve_for_task(
          max_amount_a,
          *asset_a,
          actor,
          actor_type,
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
            max_amount_b: Self::preservable_balance(actor_type, actor, *asset_b, reserved),
            max_ratio_error: *max_ratio_error,
          },
        ))
      }
      ActorTask::Unstake { asset, shares } => {
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
      ActorTask::StopCycle => Ok(PreparedTaskOutcome::Executable(PreparedTask::StopCycle)),
    }
  }

  fn execute_prepared_task(
    task: PreparedTask<T>,
    actor_id: ActorId,
    cycle_nonce: u64,
    step_index: u32,
    actor: &T::AccountId,
    actor_type: ActorType,
  ) -> Result<(), TaskFailure> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> Result<(), TaskFailure> {
        match task {
          PreparedTask::Transfer { to, asset, amount } => {
            T::AssetOps::transfer(actor, &to, asset, amount)?;
            Self::deposit_event(Event::TransferExecuted {
              actor_id,
              cycle_nonce,
              step_index,
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
              T::AssetOps::preflight_transfer(actor, &leg.to, asset, leg_amount)?;
              effective_distributed = effective_distributed.saturating_add(leg_amount);
              normalized_transfers.push((leg.to.clone(), leg_amount));
            }
            let retained = total.saturating_sub(effective_distributed);
            for (to, leg_amount) in normalized_transfers.iter() {
              T::AssetOps::transfer(actor, to, asset, *leg_amount)?;
            }
            Self::deposit_event(Event::SplitTransferExecuted {
              actor_id,
              cycle_nonce,
              step_index,
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
              actor_id,
              cycle_nonce,
              step_index,
              asset,
              amount,
            });
          }
          PreparedTask::Mint { asset, amount } => {
            T::AssetOps::mint(actor, asset, amount)?;
            Self::deposit_event(Event::MintExecuted {
              actor_id,
              cycle_nonce,
              step_index,
              asset,
              amount,
            });
          }
          PreparedTask::SwapIn {
            asset_in,
            amount_in,
            asset_out,
            slippage_tolerance,
          } => {
            let amount_out = T::DexOps::swap_exact_in(
              crate::ExecutionContext::new(actor, actor_type),
              asset_in,
              asset_out,
              amount_in,
              slippage_tolerance,
            )?;
            if amount_out.is_zero() {
              return Err(TaskFailure::permanent(DispatchError::Other(
                "ZeroSwapOutput",
              )));
            }
            Self::deposit_event(Event::SwapExecuted {
              actor_id,
              cycle_nonce,
              step_index,
              asset_in,
              asset_out,
              amount_in,
              amount_out,
            });
          }
          PreparedTask::SwapOut {
            asset_out,
            amount_out,
            asset_in,
            max_amount_in,
            slippage_tolerance,
          } => {
            let amount_in = T::DexOps::swap_exact_out(
              crate::ExecutionContext::new(actor, actor_type),
              asset_in,
              asset_out,
              amount_out,
              max_amount_in,
              slippage_tolerance,
            )?;
            if amount_in.is_zero() || amount_in > max_amount_in {
              return Err(TaskFailure::permanent(DispatchError::Other(
                "InvalidSwapInput",
              )));
            }
            Self::deposit_event(Event::SwapExecuted {
              actor_id,
              cycle_nonce,
              step_index,
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
            min_lp_out,
          } => {
            let (used_a, used_b, lp_minted) = T::LiquidityOps::add_liquidity(
              actor, asset_a, asset_b, amount_a, amount_b, min_lp_out,
            )?;
            Self::deposit_event(Event::LiquidityAdded {
              actor_id,
              cycle_nonce,
              step_index,
              asset_a,
              asset_b,
              amount_a: used_a,
              amount_b: used_b,
              lp_minted,
            });
          }
          PreparedTask::RemoveLiquidity {
            lp_asset,
            asset_a,
            asset_b,
            lp_amount,
            min_amount_a,
            min_amount_b,
          } => {
            // The stable ordered pair is host-owned; an admitted LP token must not be
            // silently reinterpreted. Execution rejects a changed/mismatched binding
            // before mutation.
            let registered = T::LiquidityOps::lp_assets(lp_asset)
              .ok_or_else(|| TaskFailure::permanent(DispatchError::Other("UnregisteredLpAsset")))?;
            if registered != (asset_a, asset_b) {
              return Err(TaskFailure::permanent(DispatchError::Other(
                "LiquidityPairBindingMismatch",
              )));
            }
            let (out_a, out_b) = T::LiquidityOps::remove_liquidity(
              actor,
              lp_asset,
              asset_a,
              asset_b,
              lp_amount,
              min_amount_a,
              min_amount_b,
            )?;
            Self::deposit_event(Event::LiquidityRemoved {
              actor_id,
              cycle_nonce,
              step_index,
              lp_asset,
              lp_amount,
              asset_a,
              asset_b,
              amount_a: out_a,
              amount_b: out_b,
            });
          }
          PreparedTask::Stake { asset, amount } => {
            T::StakingOps::stake(actor, asset, amount)?;
            Self::deposit_event(Event::StakeExecuted {
              actor_id,
              cycle_nonce,
              step_index,
              asset,
              amount,
            });
          }
          PreparedTask::DonateLiquidity {
            asset_a,
            asset_b,
            amount,
            max_amount_b,
            max_ratio_error,
          } => {
            let (amount_a, amount_b) = T::LiquidityOps::donate_liquidity(
              actor,
              asset_a,
              asset_b,
              amount,
              max_amount_b,
              max_ratio_error,
            )?;
            Self::deposit_event(Event::LiquidityDonated {
              actor_id,
              cycle_nonce,
              step_index,
              asset_a,
              asset_b,
              max_amount_a: amount,
              max_amount_b,
              amount_a,
              amount_b,
            });
          }
          PreparedTask::Unstake { asset, shares } => {
            T::StakingOps::unstake(actor, asset, shares)?;
            Self::deposit_event(Event::UnstakeExecuted {
              actor_id,
              cycle_nonce,
              step_index,
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
    actor_type: ActorType,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &FundingSnapshotOf<T>,
    policy: AmountResolutionPolicy,
  ) -> Result<Result<T::Balance, TaskResolutionOutcome>, DispatchError> {
    Ok(
      match Self::resolve_amount_with_policy(
        spec,
        asset,
        actor,
        actor_type,
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
    condition: &Condition<T::AssetId, T::Balance, u32, T::ObservationFeedId>,
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
      Condition::ObservationAbove {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value > *threshold),
      Condition::ObservationBelow {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value < *threshold),
      Condition::ObservationEquals {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value == *threshold),
      Condition::ObservationNotEquals {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value != *threshold),
    })
  }

  fn fresh_observation_value(
    feed: T::ObservationFeedId,
    max_age_blocks: u32,
  ) -> Result<Option<u128>, DispatchError> {
    let now = frame_system::Pallet::<T>::block_number();
    match T::ObservationProvider::observe(&feed, now, max_age_blocks) {
      ScalarObservationState::Fresh { value, observed_at } => {
        let maximum_age: BlockNumberFor<T> = max_age_blocks.saturated_into();
        ensure!(
          observed_at <= now && now.saturating_sub(observed_at) <= maximum_age,
          Error::<T>::InvalidCondition
        );
        Ok(Some(value))
      }
      ScalarObservationState::Unavailable
      | ScalarObservationState::Uninitialized
      | ScalarObservationState::Stale => Ok(None),
    }
  }

  /// Balance visible to Actors resolution — adapter-visible balance minus Actors-local reserved fees
  fn spendable_balance(who: &T::AccountId, asset: T::AssetId, reserved: T::Balance) -> T::Balance {
    let raw = T::AssetOps::balance(who, asset);
    if asset == T::FeeNativeAssetId::get() {
      raw.saturating_sub(reserved)
    } else {
      raw
    }
  }

  fn protected_minimum(actor_type: ActorType, asset: T::AssetId) -> T::Balance {
    fee_native_protected_minimum(
      actor_type,
      asset == T::FeeNativeAssetId::get(),
      T::AssetOps::minimum_balance(asset),
      T::MinUserBalance::get(),
    )
  }

  fn preservable_balance(
    actor_type: ActorType,
    who: &T::AccountId,
    asset: T::AssetId,
    reserved: T::Balance,
  ) -> T::Balance {
    Self::spendable_balance(who, asset, reserved)
      .saturating_sub(Self::protected_minimum(actor_type, asset))
  }

  fn resolve_unstake_shares(
    spec: &AmountResolution<T::Balance>,
    position_asset: T::AssetId,
    who: &T::AccountId,
    trigger_share_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &FundingSnapshotOf<T>,
  ) -> Result<AmountResolutionOutcome<T::Balance>, DispatchError> {
    let current_shares = T::StakingOps::share_balance(who, position_asset);
    let resolved = match spec {
      AmountResolution::Fixed(shares) => *shares,
      AmountResolution::AllAvailable => current_shares,
      AmountResolution::PercentageOfCurrent(pct) => pct.mul_floor(current_shares),
      AmountResolution::PercentageAtOpening(pct) => pct.mul_floor(Self::opening_balance(
        trigger_share_balances,
        OpeningSurface::StakingShares(position_asset),
      )?),
      AmountResolution::PercentageOfLastFunding(pct) => {
        let share_asset =
          T::StakingOps::share_asset(position_asset).ok_or(Error::<T>::InvalidAmountResolution)?;
        let Some(snapshot) = funding_snapshots.get(&share_asset) else {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        };
        if snapshot.is_zero() {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        }
        pct.mul_floor(*snapshot)
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
    actor_type: ActorType,
    reserved: T::Balance,
    trigger_balances: &ContinuationSnapshotOf<T>,
    funding_snapshots: &FundingSnapshotOf<T>,
    policy: AmountResolutionPolicy,
  ) -> Result<AmountResolutionOutcome<T::Balance>, DispatchError> {
    let spendable_current = Self::spendable_balance(who, asset, reserved);
    let policy_spend_limit = if policy == AmountResolutionPolicy::PreserveSpend {
      Self::preservable_balance(actor_type, who, asset, reserved)
    } else {
      spendable_current
    };
    let resolved = match spec {
      AmountResolution::Fixed(amount) => *amount,
      AmountResolution::AllAvailable => policy_spend_limit,
      AmountResolution::PercentageOfCurrent(pct) => {
        let value = pct.mul_floor(policy_spend_limit);
        if !pct.is_zero() && !policy_spend_limit.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
      AmountResolution::PercentageAtOpening(pct) => {
        let surface = if policy == AmountResolutionPolicy::PreserveSpend {
          OpeningSurface::PreservableAsset(asset)
        } else {
          OpeningSurface::TargetAsset(asset)
        };
        let opening_balance = Self::opening_balance(trigger_balances, surface)?;
        let value = pct.mul_floor(opening_balance);
        if !pct.is_zero() && !opening_balance.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
      AmountResolution::PercentageOfLastFunding(pct) => {
        let Some(snapshot) = funding_snapshots.get(&asset) else {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        };
        if snapshot.is_zero() {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        }
        let value = pct.mul_floor(*snapshot);
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

  const RETRY: StepErrorPolicy = StepErrorPolicy::RetryLater { max_attempts: 3 };

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
      (StepResult::Completed, RETRY, StepControl::Advance),
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
      (StepResult::Stopped, RETRY, StepControl::CompleteCycle),
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
        RETRY,
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
    for (result, policy, expected) in fixed_cases {
      assert_eq!(resolve_step_control(result, policy), expected);
    }
    assert_eq!(
      resolve_step_control(StepResult::Failed(RetryClass::Temporary), RETRY),
      StepControl::SuspendCurrent,
    );
  }

  #[test]
  fn funding_unavailable_uses_the_same_control_owner() {
    for policy in [
      StepErrorPolicy::ContinueNextStep,
      StepErrorPolicy::AbortCycle,
    ] {
      assert_eq!(
        resolve_step_control(StepResult::FundingUnavailable, policy),
        StepControl::Advance,
      );
    }
    assert_eq!(
      resolve_step_control(StepResult::FundingUnavailable, RETRY),
      StepControl::SuspendCurrent,
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

      let opening_snapshot = ContinuationSnapshotOf::<Test>::default();
      let funding_snapshots = FundingSnapshotOf::<Test>::default();
      let before_resolution = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Pallet::<Test>::resolve_amount_with_policy(
          &AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
          TestAsset::Native,
          &ALICE,
          ActorType::System,
          0,
          &opening_snapshot,
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
