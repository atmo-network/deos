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
/// (`MaxContractSteps * (MaxRetryAttempts + 1)` for outcome totals) precludes
/// overflow; a violation fails closed before mutation with an invariant error rather
/// than silently saturating (spec 4.4).
fn checked_semantic_increment(counter: u32) -> Result<u32, DispatchError> {
  counter
    .checked_add(1)
    .ok_or(DispatchError::Other("SemanticCounterOverflow"))
}

#[derive(Clone, Copy)]
pub(crate) enum FailureStreakTransition {
  UnsuccessfulAttempt,
  Reset,
}

pub(crate) fn transition_failure_streak(
  current: u32,
  transition: FailureStreakTransition,
) -> Option<u32> {
  match transition {
    FailureStreakTransition::UnsuccessfulAttempt => current.checked_add(1),
    FailureStreakTransition::Reset => Some(0),
  }
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
enum StepControl {
  Advance,
  CompleteCycle,
  Terminate,
  SuspendCurrent,
}

pub(crate) fn evaluate_precondition_with<P, MaxClauses, MaxPerClause, E, Evaluate>(
  precondition: &Precondition<P, MaxClauses, MaxPerClause>,
  mut evaluate: Evaluate,
) -> Result<bool, E>
where
  MaxClauses: Get<u32>,
  MaxPerClause: Get<u32>,
  Evaluate: FnMut(&TimedPredicate<P>) -> Result<bool, E>,
{
  let clauses = &precondition.clauses;
  let mut expression_passes = false;
  let mut first_error = None;
  for clause in clauses {
    let mut clause_passes = true;
    for predicate in clause {
      match evaluate(predicate) {
        Ok(pass) => clause_passes &= pass,
        Err(error) => {
          clause_passes = false;
          if first_error.is_none() {
            first_error = Some(error);
          }
        }
      }
    }
    expression_passes |= clause_passes;
  }
  if let Some(error) = first_error {
    return Err(error);
  }
  Ok(expression_passes)
}

fn resolve_step_control(outcome: &StepOutcome, error_policy: StepErrorPolicy) -> StepControl {
  match (outcome, error_policy) {
    (StepOutcome::Executed | StepOutcome::Skipped(_), _) => StepControl::Advance,
    (StepOutcome::Stopped, _) => StepControl::CompleteCycle,
    (StepOutcome::FundingUnavailable, StepErrorPolicy::RetryLater { .. }) => {
      StepControl::SuspendCurrent
    }
    (StepOutcome::FundingUnavailable, _) => StepControl::Advance,
    (StepOutcome::Failed(_), StepErrorPolicy::ContinueNextStep) => StepControl::Advance,
    (
      StepOutcome::Failed(TaskFailure {
        retry: RetryClass::Temporary,
        ..
      }),
      StepErrorPolicy::RetryLater { .. },
    ) => StepControl::SuspendCurrent,
    (StepOutcome::Failed(_), StepErrorPolicy::AbortCycle | StepErrorPolicy::RetryLater { .. }) => {
      StepControl::Terminate
    }
  }
}

pub(crate) struct AttemptExecution {
  weight: Weight,
  disposition: AttemptDisposition,
  fee_collection_failed: bool,
  outcomes: OutcomeTotals,
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
    let identity = ActorIdentities::<T>::get(actor_id).ok_or(Error::<T>::ContinuationInvariant)?;
    ensure!(identity.cycle_nonce > 0, Error::<T>::ContinuationInvariant);
    Self::wakeup_substrate_invalidate_inner(actor_id)
      .map_err(|_| Error::<T>::ContinuationInvariant)?;
    ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
      let hot = maybe.as_mut().ok_or(Error::<T>::ContinuationInvariant)?;
      ensure!(
        hot.cycle_state == CycleState::Suspended,
        Error::<T>::ContinuationInvariant
      );
      hot.cycle_state = CycleState::Idle;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
      Ok(())
    })?;
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
    let loaded = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(Error::<T>::ActorNotFound.into());
      }
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let identity = loaded.identity;
    if let Some(continuation) = state.as_ref() {
      let contract = loaded.contract;
      ensure!(
        identity.mutability == Mutability::Mutable
          && continuation.cursor < contract.steps.len() as u32,
        Error::<T>::ContinuationInvariant
      );
      let max_attempts = contract.steps[continuation.cursor as usize]
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
      let hot_update = ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
        maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?.cycle_state = if state.is_some() {
          CycleState::Suspended
        } else {
          CycleState::Idle
        };
        Ok(())
      });
      if let Err(error) = hot_update {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
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
    let cursor = state.cursor;
    let cumulative_outcomes = state.cumulative_outcomes;
    Self::write_continuation_state(actor_id, Some(state))?;
    Self::deposit_event(Event::CycleSuspended {
      actor_id,
      cycle_nonce,
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
  ) -> Option<ContinuationStateOf<T>> {
    let updated = ContinuationStateStore::<T>::mutate(actor_id, |maybe| {
      let Some(continuation) = maybe.as_mut() else {
        return false;
      };
      continuation.last_attempt_block = now;
      true
    });
    if !updated {
      return None;
    }
    let continuation = ContinuationStateStore::<T>::get(actor_id)?;
    Self::deposit_event(Event::CycleContinued {
      actor_id,
      cycle_nonce,
      cursor: continuation.cursor,
    });
    Some(continuation)
  }

  pub(crate) fn record_stop_cycle_event(actor_id: ActorId, cycle_nonce: u64, step_index: u32) {
    Self::deposit_event(Event::CycleStopped {
      actor_id,
      cycle_nonce,
      step_index,
    });
  }

  fn record_step_outcome(
    trace: &mut Option<&mut alloc::vec::Vec<SimulationStepRecord>>,
    step_index: u32,
    outcome: StepOutcome,
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
    (result.weight, result.fee_collection_failed)
  }

  pub(crate) fn execute_single_cycle_traced(
    actor_id: ActorId,
    instance: ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
    mut trace: Option<&mut alloc::vec::Vec<SimulationStepRecord>>,
  ) -> AttemptExecution {
    let base_weight = T::WeightInfo::cycle_orchestration();
    let is_continuation = instance.cycle_state == CycleState::Suspended;
    let actor = instance.sovereign_account.clone();
    let contract_steps = &instance.steps;
    let Ok(fee_envelope) = Self::attempt_fee_envelope(
      instance.actor_class.actor_type(),
      contract_steps,
      if is_continuation {
        Self::continuation_state(actor_id).map_or(0, |state| state.cursor as usize)
      } else {
        0
      },
    ) else {
      return AttemptExecution {
        weight: base_weight,
        disposition: AttemptDisposition::Failed,
        fee_collection_failed: false,
        outcomes: OutcomeTotals::default(),
      };
    };
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
      prior_unsuccessful_attempts_at_cursor,
      cumulative_outcomes,
      funding_snapshot,
      opening_snapshot,
      opening_predicate_results,
    ) = if is_continuation {
      let Some(continuation) =
        Self::begin_continuation_attempt(actor_id, instance.cycle_nonce, now)
      else {
        return AttemptExecution {
          weight: base_weight,
          disposition: AttemptDisposition::Failed,
          fee_collection_failed: false,
          outcomes: OutcomeTotals::default(),
        };
      };
      (
        instance.cycle_nonce,
        continuation.cursor,
        continuation.unsuccessful_attempts_at_cursor,
        continuation.cumulative_outcomes,
        continuation.funding_snapshot,
        continuation.opening_snapshot,
        continuation.opening_predicate_results,
      )
    } else {
      if instance.cycle_nonce == u64::MAX {
        Self::finalize_actor(actor_id, &instance, CloseReason::CycleNonceExhausted)
          .expect("fresh execution snapshot satisfies terminal preconditions");
        return AttemptExecution {
          weight: base_weight,
          disposition: AttemptDisposition::Closed(CloseReason::CycleNonceExhausted),
          fee_collection_failed: false,
          outcomes: OutcomeTotals::default(),
        };
      }
      let funding_snapshot = ActorFunding::<T>::get(actor_id)
        .map(|funding| funding.funding_accumulated)
        .unwrap_or_default();
      let opening_snapshot = Self::capture_opening_snapshot(
        instance.actor_class.actor_type(),
        &actor,
        contract_steps,
        reserved_fee_remaining,
      );
      let opening_predicate_results =
        Self::capture_opening_predicate_results(&actor, contract_steps, reserved_fee_remaining);
      let Some(cycle_nonce) = ActorIdentities::<T>::mutate(actor_id, |maybe| {
        let identity = maybe.as_mut()?;
        identity.cycle_nonce = identity.cycle_nonce.checked_add(1)?;
        Some(identity.cycle_nonce)
      }) else {
        return AttemptExecution {
          weight: base_weight,
          disposition: AttemptDisposition::Failed,
          fee_collection_failed: false,
          outcomes: OutcomeTotals::default(),
        };
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
        OutcomeTotals::default(),
        funding_snapshot,
        opening_snapshot,
        opening_predicate_results,
      )
    };
    let is_user = instance.actor_class.actor_type() == ActorType::User;
    let funding_snapshots = &funding_snapshot;
    let mut executed_steps = cumulative_outcomes.executed_steps;
    let mut committed_effectful_tasks = cumulative_outcomes.committed_effectful_tasks;
    let mut precondition_skips = cumulative_outcomes.precondition_skips;
    let mut skipped_resolution = cumulative_outcomes.skipped_resolution;
    let mut skipped_funding_unavailable = cumulative_outcomes.skipped_funding_unavailable;
    let mut failed_steps = cumulative_outcomes.failed_steps;
    let mut attempt_executed_steps: u32 = 0;
    let mut contract_steps_failed = false;
    let mut failure_close_reason = None;
    let mut suspended_at: Option<(u32, SuspensionReason)> = None;
    if !is_continuation {
      Self::deposit_event(Event::CycleStarted {
        actor_id,
        cycle_nonce,
      });
    }
    let mut opening_predicate_index =
      Self::opening_predicate_count_before(contract_steps, start_cursor as usize);
    for step_idx in start_cursor as usize..contract_steps.len() {
      let step = &contract_steps[step_idx];
      let step_num = step_idx as u32;
      let step_fee = &fee_envelope.steps[step_idx - start_cursor as usize];
      // Fee settlement can fail at six points in this step walk, and every one resolves the step
      // identically: count the failure, record and emit it as Permanent, then terminate or advance
      // per `StepErrorPolicy`. It stays a macro because that resolution ends in `break` or
      // `continue` against this loop, which a helper function cannot express, and it lives inside
      // the loop so `step` and `step_num` resolve without threading them through parameters.
      macro_rules! fail_step_with_fee_error {
        ($error:expr) => {{
          let step_error = $error;
          failed_steps = checked_semantic_increment(failed_steps)
            .expect("semantic counter bound is precluded by admission");
          let failure = TaskFailure::permanent(step_error);
          let outcome = StepOutcome::Failed(failure.clone());
          Self::record_step_outcome(&mut trace, step_num, outcome.clone());
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            retry_class: failure.retry,
            error: step_error,
          });
          contract_steps_failed =
            resolve_step_control(&outcome, step.on_error) == StepControl::Terminate;
          if contract_steps_failed {
            break;
          }
          continue;
        }};
      }
      match Self::evaluate_step_precondition(
        step.precondition.as_ref(),
        &actor,
        reserved_fee_remaining,
        &opening_predicate_results,
        &mut opening_predicate_index,
      ) {
        Ok(true) => {}
        Ok(false) => {
          if is_user {
            let charged_fee = Self::settle_user_step_fee(
              &mut reserved_fee_remaining,
              step_fee,
              FeeChargeKind::EvaluationOnly,
            );
            if let Err(error) = collect_step_fee!(charged_fee) {
              fail_step_with_fee_error!(error);
            }
          }
          precondition_skips = checked_semantic_increment(precondition_skips)
            .expect("semantic counter bound is precluded by admission");
          Self::record_step_outcome(
            &mut trace,
            step_num,
            StepOutcome::Skipped(StepSkippedReason::PreconditionFalse),
          );
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::PreconditionFalse,
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
          fail_step_with_fee_error!(charged_error);
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
              fail_step_with_fee_error!(error);
            }
          }
          skipped_resolution = checked_semantic_increment(skipped_resolution)
            .expect("semantic counter bound is precluded by admission");
          Self::record_step_outcome(
            &mut trace,
            step_num,
            StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped),
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
              fail_step_with_fee_error!(error);
            }
          }
          let outcome = StepOutcome::FundingUnavailable;
          Self::record_step_outcome(&mut trace, step_num, outcome.clone());
          match resolve_step_control(&outcome, step.on_error) {
            StepControl::Advance => {}
            StepControl::CompleteCycle => {
              unreachable!("FundingUnavailable cannot complete a cycle")
            }
            StepControl::Terminate => {
              contract_steps_failed = true;
              break;
            }
            StepControl::SuspendCurrent => {
              suspended_at = Some((step_num, SuspensionReason::FundingUnavailable));
              break;
            }
          }
          skipped_funding_unavailable = checked_semantic_increment(skipped_funding_unavailable)
            .expect("semantic counter bound is precluded by admission");
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
          fail_step_with_fee_error!(charged_error);
        }
      };
      if is_user {
        let charged_fee = Self::settle_user_step_fee(
          &mut reserved_fee_remaining,
          step_fee,
          FeeChargeKind::Attempted,
        );
        if let Err(error) = collect_step_fee!(charged_fee) {
          fail_step_with_fee_error!(error);
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
        let outcome = StepOutcome::Failed(failure.clone());
        let control = resolve_step_control(&outcome, step.on_error);
        Self::record_step_outcome(&mut trace, step_num, outcome);
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
            contract_steps_failed = true;
            break;
          }
          StepControl::SuspendCurrent => {
            suspended_at = Some((step_num, SuspensionReason::Temporary));
            break;
          }
        }
      }
      let outcome = if matches!(&step.task, ActorTask::StopCycle) {
        StepOutcome::Stopped
      } else {
        StepOutcome::Executed
      };
      let successful_control = resolve_step_control(&outcome, step.on_error);
      if successful_control == StepControl::CompleteCycle {
        Self::record_stop_cycle_event(actor_id, cycle_nonce, step_num);
      } else {
        debug_assert_eq!(successful_control, StepControl::Advance);
      }
      Self::record_step_outcome(&mut trace, step_num, outcome);
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
      let unsuccessful_attempt_streak = ActorHot::<T>::mutate(actor_id, |maybe| {
        let Some(hot) = maybe.as_mut() else {
          return 0;
        };
        hot.unsuccessful_attempt_streak = transition_failure_streak(
          hot.unsuccessful_attempt_streak,
          FailureStreakTransition::UnsuccessfulAttempt,
        )
        .expect("attempt admission excludes an exhausted failure counter");
        hot.unsuccessful_attempt_streak
      });
      failure_already_recorded = true;
      let unsuccessful_attempts_at_cursor = if is_continuation && cursor == start_cursor {
        prior_unsuccessful_attempts_at_cursor
          .checked_add(1)
          .expect("admitted cursor-local retry counter remains below its u32 bound")
      } else {
        1
      };
      let max_attempts = contract_steps[cursor as usize]
        .on_error
        .retry_max_attempts()
        .expect("suspension requires bounded RetryLater");
      let local_limit_reached = unsuccessful_attempts_at_cursor >= max_attempts;
      let global_limit_reached = Self::failure_limit_reached(unsuccessful_attempt_streak);
      if !local_limit_reached && !global_limit_reached {
        let cumulative_outcomes = OutcomeTotals {
          executed_steps,
          committed_effectful_tasks,
          precondition_skips,
          skipped_resolution,
          skipped_funding_unavailable,
          failed_steps,
        };
        Self::persist_continuation_suspension(
          actor_id,
          cycle_nonce,
          ContinuationState {
            cursor,
            unsuccessful_attempts_at_cursor,
            last_attempt_block: now,
            opening_snapshot: Self::trim_opening_snapshot(
              contract_steps,
              cursor as usize,
              &opening_snapshot,
            ),
            opening_predicate_results: opening_predicate_results.clone(),
            funding_snapshot: funding_snapshot.clone(),
            cumulative_outcomes,
          },
          suspension_reason,
        )
        .expect("admitted mutable RetryLater plan has a valid unresolved cursor");
        return AttemptExecution {
          weight: attempt_weight,
          disposition: AttemptDisposition::Suspended,
          fee_collection_failed,
          outcomes: cumulative_outcomes,
        };
      }
      failure_close_reason = Some(if local_limit_reached {
        CloseReason::RetryAttemptsExhausted
      } else {
        CloseReason::ConsecutiveFailures
      });
      contract_steps_failed = true;
    }
    if is_continuation {
      Self::write_continuation_state(actor_id, None)
        .expect("terminal Continuation can be cleared atomically");
    }
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let Some(hot) = maybe.as_mut() else {
        return;
      };
      if contract_steps_failed && failure_already_recorded {
        return;
      }
      let transition = if contract_steps_failed {
        FailureStreakTransition::UnsuccessfulAttempt
      } else {
        FailureStreakTransition::Reset
      };
      hot.unsuccessful_attempt_streak =
        transition_failure_streak(hot.unsuccessful_attempt_streak, transition)
          .expect("failure bound (MaxConsecutiveFailures) is precluded by admission");
    });
    let outcomes = OutcomeTotals {
      executed_steps,
      committed_effectful_tasks,
      precondition_skips,
      skipped_resolution,
      skipped_funding_unavailable,
      failed_steps,
    };
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: if contract_steps_failed {
        CycleResult::Failed
      } else {
        CycleResult::Completed
      },
      outcomes,
    });
    let mut terminal_close_reason = None;
    if contract_steps_failed {
      if let Some(inst) = Self::active_actor_view(actor_id) {
        let close_reason = failure_close_reason.or_else(|| {
          Self::failure_limit_reached(inst.unsuccessful_attempt_streak)
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
      if inst.completion == CompletionPolicy::CloseAfterProductiveCycle
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
    let disposition = if let Some(reason) = terminal_close_reason {
      AttemptDisposition::Closed(reason)
    } else if contract_steps_failed {
      AttemptDisposition::Failed
    } else {
      AttemptDisposition::Completed
    };
    AttemptExecution {
      weight: attempt_weight,
      disposition,
      fee_collection_failed,
      outcomes,
    }
  }

  pub fn simulate_current_contract(
    actor_id: ActorId,
    expected_type: ActorType,
    expected_mutability: Mutability,
    expected_contract: ActorContractOf<T>,
    mode: SimulationMode,
  ) -> Result<SimulationResultOf<T>, SimulationError> {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(SimulationError::ActorNotFound);
      }
      LoadedActorStateOf::Corrupt => {
        return Err(SimulationError::Classification(
          ActorClassificationError::ActorInvariant,
        ));
      }
    };
    let continuation = state.continuation;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    if instance.actor_class.actor_type() != expected_type {
      return Err(SimulationError::TypeMismatch);
    }
    if instance.mutability != expected_mutability {
      return Err(SimulationError::MutabilityMismatch);
    }
    if ActorContracts::<T>::get(actor_id) != Some(expected_contract) {
      return Err(SimulationError::ContractMismatch);
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
    let classification =
      Self::classify_actor(actor_id, &instance).map_err(SimulationError::Classification)?;
    if classification.execution_phase == ActorExecutionPhase::GlobalCircuitBreaker {
      return Err(SimulationError::GlobalCircuitBreaker);
    }
    if let Some(reason) = classification.terminal_reason {
      let (start_cursor, cumulative_outcomes) = match mode {
        SimulationMode::FreshCurrentPlan => (0, OutcomeTotals::default()),
        SimulationMode::CurrentContinuation => {
          let continuation = continuation
            .as_ref()
            .ok_or(SimulationError::Classification(
              ActorClassificationError::ContinuationInvariant,
            ))?;
          (continuation.cursor, continuation.cumulative_outcomes)
        }
      };
      return Ok(SimulationResult {
        status: AttemptDisposition::Closed(reason),
        cycle_nonce: instance.cycle_nonce,
        start_cursor,
        continuation_cursor: None,
        unsuccessful_attempts_at_cursor: None,
        cumulative_outcomes,
        steps: BoundedVec::default(),
      });
    }
    let (cycle_nonce, start_cursor) = match mode {
      SimulationMode::FreshCurrentPlan => (
        instance
          .cycle_nonce
          .checked_add(1)
          .ok_or(SimulationError::Classification(
            ActorClassificationError::ActorInvariant,
          ))?,
        0,
      ),
      SimulationMode::CurrentContinuation => {
        let continuation = continuation
          .as_ref()
          .ok_or(SimulationError::Classification(
            ActorClassificationError::ContinuationInvariant,
          ))?;
        (instance.cycle_nonce, continuation.cursor)
      }
    };
    match classification.execution_phase {
      ActorExecutionPhase::Paused => return Err(SimulationError::Paused),
      ActorExecutionPhase::Ready => {}
      ActorExecutionPhase::WaitingRetry(_)
      | ActorExecutionPhase::WaitingBlock(_)
      | ActorExecutionPhase::WaitingCadenceTick(_)
      | ActorExecutionPhase::WaitingSignal => return Err(SimulationError::NotReady),
      ActorExecutionPhase::GlobalCircuitBreaker => unreachable!("handled above"),
    }
    let now = frame_system::Pallet::<T>::block_number();
    polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
      let mut trace = alloc::vec::Vec::new();
      let attempt = Self::execute_single_cycle_traced(actor_id, instance, now, Some(&mut trace));
      if attempt.fee_collection_failed {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          SimulationError::FeeCollectionFailed,
        ));
      }
      let continuation = ContinuationStateStore::<T>::get(actor_id);
      let continuation_cursor = continuation.as_ref().map(|state| state.cursor);
      let unsuccessful_attempts_at_cursor = continuation
        .as_ref()
        .map(|state| state.unsuccessful_attempts_at_cursor);
      if let Some(state) = continuation.as_ref() {
        debug_assert_eq!(state.cumulative_outcomes, attempt.outcomes);
      }
      debug_assert!(trace.len() <= T::MaxContractSteps::get() as usize);
      let Ok(steps) = BoundedVec::try_from(trace) else {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          SimulationError::Classification(ActorClassificationError::ContinuationInvariant),
        ));
      };
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(SimulationResult {
        status: attempt.disposition,
        cycle_nonce,
        start_cursor,
        continuation_cursor,
        unsuccessful_attempts_at_cursor,
        cumulative_outcomes: attempt.outcomes,
        steps,
      }))
    })
    .map_err(|()| SimulationError::TransactionDepthExceeded)?
  }

  pub(crate) fn failure_limit_reached(unsuccessful_attempt_streak: u32) -> bool {
    let max_failures = T::MaxConsecutiveFailures::get();
    max_failures > 0 && unsuccessful_attempt_streak >= max_failures
  }

  pub(crate) fn predicate_evaluation_weight(evaluation_units: u32) -> Weight {
    let component_bound = T::MaxPredicatesPerStep::get();
    if component_bound == 0 {
      return Weight::MAX;
    }
    let mut remaining = evaluation_units;
    let mut weight = Weight::zero();
    while remaining > 0 {
      let chunk = remaining.min(component_bound);
      weight = weight.saturating_add(T::WeightInfo::predicate_set_evaluation(chunk));
      remaining = remaining.saturating_sub(chunk);
    }
    weight
  }

  pub(crate) fn compute_eval_fee_checked(evaluation_units: u32) -> Result<BalanceOf<T>, Error<T>> {
    let evaluation_weight = Self::predicate_evaluation_weight(evaluation_units)
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
    contract_steps: &ContractSteps<T>,
    start_cursor: usize,
  ) -> alloc::vec::Vec<OpeningSurface<T::AssetId>> {
    let mut surfaces = alloc::vec::Vec::new();
    for step_index in start_cursor..contract_steps.len() {
      Self::collect_percentage_opening_surfaces(&contract_steps[step_index].task, &mut surfaces);
    }
    surfaces
  }

  fn opening_predicate_count_before(contract_steps: &ContractSteps<T>, end_step: usize) -> usize {
    contract_steps
      .iter()
      .take(end_step)
      .map(|step| {
        step.precondition.as_ref().map_or(0, |precondition| {
          precondition.opening_predicate_count() as usize
        })
      })
      .sum()
  }

  fn capture_opening_predicate_results(
    actor: &T::AccountId,
    contract_steps: &ContractSteps<T>,
    reserved: T::Balance,
  ) -> OpeningPredicateResultsOf<T> {
    let mut results = OpeningPredicateResultsOf::<T>::default();
    for step in contract_steps {
      let Some(precondition) = &step.precondition else {
        continue;
      };
      let clauses = &precondition.clauses;
      for timed in clauses.iter().flat_map(|clause| clause.iter()) {
        if timed.timing != ObservationTiming::Opening {
          continue;
        }
        let result = Self::evaluate_atomic_predicate(&timed.predicate, actor, reserved);
        results
          .try_push(result)
          .unwrap_or_else(|_| panic!("admitted opening predicates fit MaxOpeningPredicateResults"));
      }
    }
    results
  }

  fn capture_opening_snapshot(
    actor_type: ActorType,
    actor: &T::AccountId,
    contract_steps: &ContractSteps<T>,
    reserved: T::Balance,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::opening_surfaces(contract_steps, 0) {
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
    contract_steps: &ContractSteps<T>,
    start_cursor: usize,
    source: &ContinuationSnapshotOf<T>,
  ) -> ContinuationSnapshotOf<T> {
    let mut snapshot = ContinuationSnapshotOf::<T>::default();
    for surface in Self::opening_surfaces(contract_steps, start_cursor) {
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
              effective_distributed = effective_distributed
                .checked_add(&leg_amount)
                .ok_or_else(|| TaskFailure::permanent(Error::<T>::InvalidSplitTransfer))?;
              normalized_transfers.push((leg.to.clone(), leg_amount));
            }
            let retained = total
              .checked_sub(&effective_distributed)
              .ok_or_else(|| TaskFailure::permanent(Error::<T>::InvalidSplitTransfer))?;
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
            let outcome = T::DexOps::swap_exact_in(
              crate::ExecutionContext::new(actor, actor_type),
              asset_in,
              asset_out,
              amount_in,
              slippage_tolerance,
            )?;
            if outcome.recipient_amount_out.is_zero() || outcome.total_amount_in != amount_in {
              return Err(TaskFailure::permanent(DispatchError::Other(
                "InvalidSwapOutcome",
              )));
            }
            Self::deposit_event(Event::SwapExecuted {
              actor_id,
              cycle_nonce,
              step_index,
              asset_in,
              asset_out,
              amount_in: outcome.total_amount_in,
              amount_out: outcome.recipient_amount_out,
            });
          }
          PreparedTask::SwapOut {
            asset_out,
            amount_out,
            asset_in,
            max_amount_in,
            slippage_tolerance,
          } => {
            let outcome = T::DexOps::swap_exact_out(
              crate::ExecutionContext::new(actor, actor_type),
              asset_in,
              asset_out,
              amount_out,
              max_amount_in,
              slippage_tolerance,
            )?;
            if outcome.total_amount_in.is_zero()
              || outcome.total_amount_in > max_amount_in
              || outcome.recipient_amount_out < amount_out
            {
              return Err(TaskFailure::permanent(DispatchError::Other(
                "InvalidSwapOutcome",
              )));
            }
            Self::deposit_event(Event::SwapExecuted {
              actor_id,
              cycle_nonce,
              step_index,
              asset_in,
              asset_out,
              amount_in: outcome.total_amount_in,
              amount_out: outcome.recipient_amount_out,
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

  fn evaluate_step_precondition(
    precondition: Option<&PreconditionOf<T>>,
    who: &T::AccountId,
    reserved: T::Balance,
    opening_results: &OpeningPredicateResultsOf<T>,
    opening_index: &mut usize,
  ) -> Result<bool, DispatchError> {
    let Some(precondition) = precondition else {
      return Ok(true);
    };
    evaluate_precondition_with(precondition, |timed| match timed.timing {
      ObservationTiming::Current => {
        Self::evaluate_atomic_predicate(&timed.predicate, who, reserved)
          .map_err(|_| Error::<T>::InvalidPredicate.into())
      }
      ObservationTiming::Opening => {
        let result = opening_results
          .get(*opening_index)
          .copied()
          .ok_or(Error::<T>::SnapshotUnavailable)?;
        *opening_index = opening_index
          .checked_add(1)
          .ok_or(Error::<T>::ComputationOverflow)?;
        result.map_err(|_| Error::<T>::InvalidPredicate.into())
      }
    })
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn evaluate_precondition(
    precondition: &PreconditionOf<T>,
    who: &T::AccountId,
    reserved: T::Balance,
  ) -> Result<bool, DispatchError> {
    let opening_results = OpeningPredicateResultsOf::<T>::default();
    Self::evaluate_step_precondition(Some(precondition), who, reserved, &opening_results, &mut 0)
  }

  fn evaluate_atomic_predicate(
    condition: &Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>,
    who: &T::AccountId,
    reserved: T::Balance,
  ) -> Result<bool, PredicateError> {
    Ok(match condition {
      Predicate::BalanceAbove { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) > *threshold
      }
      Predicate::BalanceBelow { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) < *threshold
      }
      Predicate::BalanceEquals { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) == *threshold
      }
      Predicate::BalanceNotEquals { asset, threshold } => {
        Self::spendable_balance(who, *asset, reserved) != *threshold
      }
      Predicate::BlockNumberAbove { threshold } => {
        let now: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
        now > *threshold
      }
      Predicate::BlockNumberBelow { threshold } => {
        let now: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
        now < *threshold
      }
      Predicate::ObservationAbove {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value > *threshold),
      Predicate::ObservationBelow {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value < *threshold),
      Predicate::ObservationEquals {
        feed,
        threshold,
        max_age_blocks,
      } => Self::fresh_observation_value(*feed, *max_age_blocks)?
        .is_some_and(|value| value == *threshold),
      Predicate::ObservationNotEquals {
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
  ) -> Result<Option<u128>, PredicateError> {
    let now = frame_system::Pallet::<T>::block_number();
    match T::ObservationProvider::observe(&feed, now, max_age_blocks) {
      ScalarObservationState::Fresh { value, observed_at } => {
        let maximum_age: BlockNumberFor<T> = max_age_blocks.saturated_into();
        ensure!(observed_at <= now, PredicateError::InvalidObservation);
        let observation_age = now
          .checked_sub(&observed_at)
          .ok_or(PredicateError::InvalidObservation)?;
        ensure!(
          observation_age <= maximum_age,
          PredicateError::InvalidObservation
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
    let permanent = StepOutcome::Failed(TaskFailure::permanent(DispatchError::Other("test")));
    let temporary = StepOutcome::Failed(TaskFailure::temporary(DispatchError::Other("test")));
    let fixed_cases = [
      (
        &StepOutcome::Executed,
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        &StepOutcome::Executed,
        StepErrorPolicy::AbortCycle,
        StepControl::Advance,
      ),
      (&StepOutcome::Executed, RETRY, StepControl::Advance),
      (
        &StepOutcome::Stopped,
        StepErrorPolicy::ContinueNextStep,
        StepControl::CompleteCycle,
      ),
      (
        &StepOutcome::Stopped,
        StepErrorPolicy::AbortCycle,
        StepControl::CompleteCycle,
      ),
      (&StepOutcome::Stopped, RETRY, StepControl::CompleteCycle),
      (
        &permanent,
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        &permanent,
        StepErrorPolicy::AbortCycle,
        StepControl::Terminate,
      ),
      (&permanent, RETRY, StepControl::Terminate),
      (
        &temporary,
        StepErrorPolicy::ContinueNextStep,
        StepControl::Advance,
      ),
      (
        &temporary,
        StepErrorPolicy::AbortCycle,
        StepControl::Terminate,
      ),
    ];
    for (outcome, policy, expected) in fixed_cases {
      assert_eq!(resolve_step_control(outcome, policy), expected);
    }
    assert_eq!(
      resolve_step_control(&temporary, RETRY),
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
        resolve_step_control(&StepOutcome::FundingUnavailable, policy),
        StepControl::Advance,
      );
    }
    assert_eq!(
      resolve_step_control(&StepOutcome::FundingUnavailable, RETRY),
      StepControl::SuspendCurrent,
    );
  }

  #[test]
  fn precondition_dnf_never_short_circuits_truth_or_error() {
    use polkadot_sdk::frame_support::traits::ConstU32;

    let clause = |predicates| BoundedVec::try_from(predicates).expect("predicates fit");
    let timed = |predicate| TimedPredicate {
      timing: ObservationTiming::Current,
      predicate,
    };
    let precondition = Precondition::<u8, ConstU32<4>, ConstU32<4>> {
      clauses: BoundedVec::try_from(alloc::vec![
        clause(alloc::vec![timed(1), timed(2)]),
        clause(alloc::vec![timed(3)]),
      ])
      .expect("clauses fit"),
    };
    let mut visited = alloc::vec::Vec::new();
    let result = evaluate_precondition_with(&precondition, |timed| {
      visited.push(timed.predicate);
      match timed.predicate {
        1 => Ok(true),
        2 => Err("predicate failed"),
        _ => Ok(false),
      }
    });
    assert_eq!(result, Err("predicate failed"));
    assert_eq!(visited, alloc::vec![1, 2, 3]);
  }

  #[test]
  fn conditions_and_amount_resolution_do_not_write_storage() {
    use crate::mock::{ALICE, TEST_INITIAL_BALANCE, Test, TestAsset, new_test_ext};
    use polkadot_sdk::sp_runtime::StateVersion;

    new_test_ext().execute_with(|| {
      let precondition = Precondition {
        clauses: BoundedVec::try_from(alloc::vec![
          BoundedVec::try_from(alloc::vec![TimedPredicate {
            timing: ObservationTiming::Current,
            predicate: Predicate::BalanceAbove {
              asset: TestAsset::Native,
              threshold: 1,
            },
          }])
          .expect("one predicate fits"),
        ])
        .expect("one clause fits"),
      };
      let before_conditions = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      assert_eq!(
        Pallet::<Test>::evaluate_precondition(&precondition, &ALICE, 0),
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
