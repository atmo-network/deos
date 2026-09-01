use super::pallet::*;
use super::types::{InputLimit, Task as ActorTask};
use super::{
  AssetOps, DexOps, FeeAssetClass, FeeCollector, LiquidityOps, ObservationProvider as _,
  RetryClass, ScalarObservationState, StakingOps, TaskEffectExecution, TaskFailure,
  WeightInfo as _, fee_native_protected_minimum,
};
use crate::scheduler::AttemptTransactionError;
use frame::prelude::*;
use polkadot_sdk::sp_runtime::{
  Perbill,
  traits::{SaturatedConversion, Zero},
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

#[derive(Clone, Copy)]
enum LoadedFundingDisposition {
  Preserve,
  Clear,
}

impl LoadedFundingDisposition {
  fn clears(self) -> bool {
    matches!(self, Self::Clear)
  }
}

pub(crate) enum LoadedCancellationContext<T: Config> {
  RetainedFrame {
    admission: ActorAdmissionCertificateOf<T>,
    state: ActiveActorStateOf<T>,
  },
  ConsumedFrame {
    admission: ActorAdmissionCertificateOf<T>,
    state: ActiveActorStateOf<T>,
  },
}

impl<T: Config> LoadedCancellationContext<T> {
  pub(crate) fn admission(&self) -> &ActorAdmissionCertificateOf<T> {
    match self {
      Self::RetainedFrame { admission, .. } | Self::ConsumedFrame { admission, .. } => admission,
    }
  }
}

impl<T: Config> Pallet<T> {
  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn commit_cycle_nonce_from_identity(
    actor_id: ActorId,
    identity: &ActorIdentityOf<T>,
    cycle_nonce: u64,
  ) -> DispatchResult {
    ensure!(
      identity.cycle_nonce.checked_add(1) == Some(cycle_nonce),
      Error::<T>::ActorRunInvariant
    );
    let mut updated = identity.clone();
    updated.cycle_nonce = cycle_nonce;
    let frame_primary_exists = ActorControlLocators::<T>::contains_key(actor_id);
    if frame_primary_exists {
      Self::update_existing_frame_control_identity(actor_id, &updated)
        .map_err(|_| Error::<T>::ActorRunInvariant)?;
    }
    Ok(())
  }

  pub(crate) fn cancel_run_internal(
    actor_id: ActorId,
    reason: CancellationReason,
    outcomes: Option<OutcomeTotals>,
  ) -> Result<bool, DispatchError> {
    let (state, admission, _) =
      Self::load_frame_actor_service_state(actor_id).ok_or(Error::<T>::ActorRunInvariant)?;
    let identity = state.identity.clone();
    Self::cancel_run_internal_loaded(
      actor_id,
      &identity,
      reason,
      outcomes,
      LoadedCancellationContext::RetainedFrame { admission, state },
    )
  }

  pub(crate) fn cancel_run_internal_loaded(
    actor_id: ActorId,
    identity: &ActorIdentityOf<T>,
    reason: CancellationReason,
    outcomes: Option<OutcomeTotals>,
    context: LoadedCancellationContext<T>,
  ) -> Result<bool, DispatchError> {
    let (admission, mut state, consumed) = match context {
      LoadedCancellationContext::RetainedFrame { admission, state } => (admission, state, false),
      LoadedCancellationContext::ConsumedFrame { admission, state } => (admission, state, true),
    };
    let Some(run_state) = state.run_state.clone() else {
      ensure!(
        state.hot.cycle_state == CycleState::Idle,
        Error::<T>::ActorRunInvariant
      );
      return Ok(false);
    };
    ensure!(
      state.identity == *identity
        && (identity.cycle_nonce == run_state.cycle_nonce
          || identity.cycle_nonce.checked_add(1) == Some(run_state.cycle_nonce))
        && matches!(
          state.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        )
        && (!consumed || !ActorControlLocators::<T>::contains_key(actor_id)),
      Error::<T>::ActorRunInvariant
    );
    if state.hot.wakeup_pointer.is_some() {
      Self::wakeup_substrate_invalidate_loaded(actor_id, state.clone(), &admission)
        .map_err(|_| Error::<T>::ActorRunInvariant)?;
    }
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| Error::<T>::ActorRunInvariant)?;
    }
    state.hot.cycle_state = CycleState::Idle;
    state.hot.queue_ticket = None;
    state.hot.wakeup_pointer = None;
    state.identity.cycle_nonce = run_state.cycle_nonce;
    ActorRunStateStore::<T>::remove(actor_id);
    if !matches!(reason, CancellationReason::Closing(_)) {
      let resources = if state.contract.steps.is_empty() {
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        }
      } else {
        Self::load_current_step_with_admission(actor_id, 0, &admission)
          .map(|loaded| loaded.resources)
          .ok_or(Error::<T>::ActorRunInvariant)?
      };
      if state.hot.pending_signal && !state.hot.lifecycle.is_paused() {
        let plan = Self::preflight_paged_enqueue_authority(
          actor_id,
          state.hot,
          &state.identity,
          None,
          &admission,
          resources,
        )
        .map_err(Self::placement_error)?;
        Self::commit_paged_enqueue(plan).map_err(Self::placement_error)?;
      } else {
        Self::restore_unsignaled_from_authority(
          actor_id,
          state.hot,
          &state.identity,
          None,
          &admission,
          resources,
        )
        .map_err(|_| Error::<T>::ActorRunInvariant)?;
      }
      Self::reconcile_actor_state_hold_with_authority(actor_id)?;
    }
    Self::deposit_event(Event::CycleCancelled {
      actor_id,
      cycle_nonce: run_state.cycle_nonce,
      reason,
    });
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce: run_state.cycle_nonce,
      result: CycleResult::Cancelled,
      outcomes: outcomes.unwrap_or(run_state.cumulative_outcomes),
    });
    Ok(true)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn write_run_state(
    actor_id: ActorId,
    state: Option<ActorRunStateOf<T>>,
  ) -> DispatchResult {
    let Some((loaded, admission, _)) = Self::load_current_step_service_state(actor_id) else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    let identity = loaded.identity;
    let contract = loaded.contract;
    let existing_run = loaded.run_state;
    let step_count = ActorContractHeads::<T>::get(actor_id)
      .ok_or(Error::<T>::ActorRunInvariant)?
      .header
      .step_count;
    let target_step = state
      .as_ref()
      .and_then(|run| Self::load_current_step_from_storage(actor_id, run.cursor));
    let expected_cycle_nonce = identity
      .cycle_nonce
      .checked_add(1)
      .ok_or(Error::<T>::ActorRunInvariant)?;
    if let Some(run_state) = state.as_ref() {
      ensure!(
        run_state.cycle_nonce == expected_cycle_nonce
          && existing_run.as_ref().is_none_or(|existing| {
            existing.cycle_nonce == run_state.cycle_nonce
              && existing.opening_snapshot == run_state.opening_snapshot
              && existing.opening_predicate_results == run_state.opening_predicate_results
              && existing.funding_snapshot == run_state.funding_snapshot
          })
          && run_state.cursor < step_count
          && run_state.has_contract_authority(
            admission.semantic_contract_id,
            admission.body_commitment,
            admission.admission_identity,
          )
          && target_step.is_some(),
        Error::<T>::ActorRunInvariant
      );
      if run_state.suspension.is_some() {
        ensure!(
          identity.mutability == Mutability::Mutable && run_state.suspension_is_coherent(),
          Error::<T>::ActorRunInvariant
        );
        let max_attempts = target_step
          .as_ref()
          .ok_or(Error::<T>::ActorRunInvariant)?
          .step
          .on_error
          .retry_max_attempts()
          .ok_or(Error::<T>::ActorRunInvariant)?;
        let expected_eligible_at = Self::suspension_eligible_at(
          contract.cooldown_blocks,
          contract.window,
          run_state.last_attempt_block,
          run_state.unsuccessful_attempts_at_cursor,
        )
        .map_err(|_| Error::<T>::ActorRunInvariant)?;
        ensure!(
          run_state.unsuccessful_attempts_at_cursor > 0
            && run_state.unsuccessful_attempts_at_cursor < max_attempts
            && run_state.eligible_at == expected_eligible_at,
          Error::<T>::ActorRunInvariant
        );
      } else {
        ensure!(
          run_state.unsuccessful_attempts_at_cursor == 0 && run_state.running_is_coherent(),
          Error::<T>::ActorRunInvariant
        );
      }
    } else {
      let run_state = existing_run.as_ref().ok_or(Error::<T>::ActorRunInvariant)?;
      ensure!(
        run_state.cycle_nonce == expected_cycle_nonce,
        Error::<T>::ActorRunInvariant
      );
    }
    let next_cycle_state = state.as_ref().map(|run| {
      if run.suspension.is_some() {
        CycleState::Suspended
      } else {
        CycleState::Running
      }
    });
    ensure!(
      identity.actor_class.actor_type() == ActorType::System
        || ActorControlLocators::<T>::contains_key(actor_id),
      Error::<T>::StateHoldInvariant
    );
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if state.is_none()
        && let Err(error) =
          Self::commit_cycle_nonce_from_identity(actor_id, &identity, expected_cycle_nonce)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let hot_update = Self::try_mutate_control_hot_with_authority(
        actor_id,
        Error::<T>::ActorNotFound,
        |hot| -> DispatchResult {
          hot.cycle_state = next_cycle_state.unwrap_or(CycleState::Idle);
          Ok(())
        },
      );
      if let Err(error) = hot_update {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let (location, mut cell) = match Self::load_primary_control_cell(actor_id) {
        Ok(primary) => primary,
        Err(_) => {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorRunInvariant.into(),
          ));
        }
      };
      cell.cursor = state.as_ref().map_or(0, |run| run.cursor);
      let Some(step) = Self::load_current_step_with_admission(actor_id, cell.cursor, &admission)
      else {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorRunInvariant.into(),
        ));
      };
      cell.resources = step.resources;
      let mut placement_hot = loaded.hot.clone();
      placement_hot.cycle_state = next_cycle_state.unwrap_or(CycleState::Idle);
      let placement_run = state.as_ref().map(|run| (run.cursor, run.eligible_at));
      if let Some(run) = state.as_ref()
        && matches!(location, ActorControlLocation::Ready { .. })
      {
        cell.eligible_at = Some(run.eligible_at);
      }
      let primary_publication = if placement_run.is_none() {
        Self::remove_primary_control_cell_inner(actor_id).and_then(|_| {
          if let ActorControlLocation::Waiting { key, .. } = location {
            match key.clock() {
              WakeupClock::Block => cell.hot.wakeup_pointer = None,
              WakeupClock::Tick => cell.hot.trigger_wakeup_pointer = None,
            }
          }
          if cell.hot.pending_signal && !cell.hot.lifecycle.is_paused() {
            cell.eligible_at = Some(frame_system::Pallet::<T>::block_number());
            Self::control_append_ready(cell).map(|_| ())
          } else {
            cell.eligible_at = None;
            ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
            ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
            Ok(())
          }
        })
      } else {
        Self::store_primary_control_cell(location, cell)
      };
      if primary_publication.is_err() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorRunInvariant.into(),
        ));
      }
      if let Some(run_state) = state {
        ActorRunStateStore::<T>::insert(actor_id, run_state);
      } else {
        ActorRunStateStore::<T>::remove(actor_id);
      }
      if let Some((cursor, eligible_at)) = placement_run {
        if Self::try_wakeup_substrate_schedule_transition_with_authority(
          actor_id,
          WakeupKey::Block(eligible_at),
          placement_hot,
          &identity,
          cursor,
          &admission,
          step.resources,
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorRunInvariant.into(),
          ));
        }
      }
      if identity.actor_class.actor_type() == ActorType::User
        && let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn persist_run_progress(
    actor_id: ActorId,
    state: ActorRunStateOf<T>,
  ) -> DispatchResult {
    ensure!(state.running_is_coherent(), Error::<T>::ActorRunInvariant);
    Self::write_run_state(actor_id, Some(state))
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn persist_run_suspension(
    actor_id: ActorId,
    state: ActorRunStateOf<T>,
  ) -> DispatchResult {
    let cycle_nonce = state.cycle_nonce;
    let cursor = state.cursor;
    let cumulative_outcomes = state.cumulative_outcomes;
    let reason = state.suspension.ok_or(Error::<T>::ActorRunInvariant)?;
    Self::write_run_state(actor_id, Some(state))?;
    Self::deposit_event(Event::CycleSuspended {
      actor_id,
      cycle_nonce,
      cursor,
      reason,
      cumulative_outcomes,
    });
    Ok(())
  }

  pub(crate) fn record_stop_cycle_event(actor_id: ActorId, cycle_nonce: u64, step_index: u32) {
    Self::deposit_event(Event::CycleStopped {
      actor_id,
      cycle_nonce,
      step_index,
    });
  }

  fn commit_loaded_single_step_suspension(
    actor_id: ActorId,
    mut plan: CurrentStepPlanOf<T>,
    run: ActorRunStateOf<T>,
    unsuccessful_attempt_streak: u32,
    funding_disposition: LoadedFundingDisposition,
    effect_execution: TaskEffectExecution,
  ) -> Result<
    (
      CurrentStepPlanOf<T>,
      TaskEffectExecution,
      AttemptDisposition,
      OutcomeTotals,
      Option<BlockNumberFor<T>>,
    ),
    AttemptTransactionError,
  > {
    if !run.suspension_is_coherent() {
      return Err(AttemptTransactionError::Invariant);
    }
    let cycle_nonce = run.cycle_nonce;
    let cursor = run.cursor;
    let eligible_at = run.eligible_at;
    let outcomes = run.cumulative_outcomes;
    let reason = run.suspension.ok_or(AttemptTransactionError::Invariant)?;
    let deferred_signal = plan.hot.cycle_state == CycleState::Suspended && plan.hot.pending_signal;
    plan.hot.cycle_state = CycleState::Suspended;
    plan.hot.unsuccessful_attempt_streak = unsuccessful_attempt_streak;
    plan.hot.pending_signal = deferred_signal;
    plan.hot.queue_ticket = None;
    if funding_disposition.clears() {
      plan.funding.funding_accumulated.clear();
      ActorFunding::<T>::insert(actor_id, &plan.funding);
    }
    plan.last_step_outcome = run.last_step_outcome.clone();
    ActorRunStateStore::<T>::insert(actor_id, run.clone());
    plan.run = Some(run);
    Self::deposit_event(Event::CycleSuspended {
      actor_id,
      cycle_nonce,
      cursor,
      reason,
      cumulative_outcomes: outcomes,
    });
    Ok((
      plan,
      effect_execution,
      AttemptDisposition::Suspended,
      outcomes,
      Some(eligible_at),
    ))
  }

  fn prepare_loaded_resuspension(
    instance: &ActiveActorViewOf<T>,
    run: &mut ActorRunStateOf<T>,
    max_attempts: u32,
    unsuccessful_attempt_streak: u32,
    now: BlockNumberFor<T>,
    reason: SuspensionReason,
    last_step_outcome: StepOutcome,
    outcomes: OutcomeTotals,
  ) -> Result<(u32, bool), AttemptTransactionError> {
    let attempts = run
      .unsuccessful_attempts_at_cursor
      .checked_add(1)
      .ok_or(AttemptTransactionError::Invariant)?;
    let streak = transition_failure_streak(
      unsuccessful_attempt_streak,
      FailureStreakTransition::UnsuccessfulAttempt,
    )
    .ok_or(AttemptTransactionError::Invariant)?;
    let terminal = attempts >= max_attempts || Self::failure_limit_reached(streak);
    run.unsuccessful_attempts_at_cursor = attempts;
    run.last_attempt_block = now;
    run.cumulative_outcomes = outcomes;
    run.last_step_outcome = Some(last_step_outcome);
    run.suspension = Some(reason);
    if terminal {
      return Ok((streak, true));
    }
    run.eligible_at =
      Self::suspension_eligible_at(instance.cooldown_blocks, instance.window, now, attempts)
        .map_err(|_| AttemptTransactionError::Invariant)?;
    Ok((streak, false))
  }

  fn commit_loaded_single_step_failure(
    actor_id: ActorId,
    mut plan: CurrentStepPlanOf<T>,
    cycle_nonce: u64,
    unsuccessful_attempt_streak: u32,
    outcomes: OutcomeTotals,
    funding_disposition: LoadedFundingDisposition,
    effect_execution: TaskEffectExecution,
    now: BlockNumberFor<T>,
  ) -> Result<
    (
      CurrentStepPlanOf<T>,
      TaskEffectExecution,
      AttemptDisposition,
      OutcomeTotals,
      Option<BlockNumberFor<T>>,
    ),
    AttemptTransactionError,
  > {
    let deferred_signal = plan.hot.cycle_state != CycleState::Idle && plan.hot.pending_signal;
    ActorRunStateStore::<T>::remove(actor_id);
    plan.run = None;
    plan.identity.cycle_nonce = cycle_nonce;
    plan.hot.cycle_state = CycleState::Idle;
    plan.hot.pending_signal = deferred_signal;
    plan.hot.queue_ticket = None;
    plan.hot.last_cycle_block = Some(now);
    plan.hot.unsuccessful_attempt_streak = unsuccessful_attempt_streak;
    if funding_disposition.clears() {
      plan.funding.funding_accumulated.clear();
      ActorFunding::<T>::insert(actor_id, &plan.funding);
    }
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Failed,
      outcomes,
    });
    Ok((
      plan,
      effect_execution,
      AttemptDisposition::Failed,
      outcomes,
      None,
    ))
  }

  fn execute_suspended_single_step_core(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    mut plan: CurrentStepPlanOf<T>,
    now: BlockNumberFor<T>,
    step_count: u32,
  ) -> Result<
    (
      CurrentStepPlanOf<T>,
      TaskEffectExecution,
      AttemptDisposition,
      OutcomeTotals,
      Option<BlockNumberFor<T>>,
    ),
    AttemptTransactionError,
  > {
    let step = plan.loaded_step.step.clone();
    let mut run = plan.run.take().ok_or(AttemptTransactionError::Invariant)?;
    let deferred_signal = plan.hot.pending_signal;
    if instance.cycle_state != CycleState::Suspended
      || run.cursor >= step_count
      || step_count == 0
      || plan.hot.wakeup_pointer.is_some()
      || plan.ticket.actor_id != actor_id
      || plan.ticket.cursor != run.cursor
      || plan.loaded_step.cursor != run.cursor
      || plan.ticket.cycle_nonce != run.cycle_nonce
      || plan.ticket.eligible_at != run.eligible_at
      || now < run.eligible_at
      || run.cycle_nonce
        != plan
          .identity
          .cycle_nonce
          .checked_add(1)
          .ok_or(AttemptTransactionError::Invariant)?
      || !run.suspension_is_coherent()
      || !matches!(step.on_error, StepErrorPolicy::RetryLater { .. })
      || !plan.admission.has_valid_identity()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let expected_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      plan.loaded_step.resources,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if plan.maximum_fee != expected_fee {
      return Err(AttemptTransactionError::Invariant);
    }
    let cycle_nonce = run.cycle_nonce;
    let cursor = run.cursor;
    run.last_attempt_block = now;
    Self::deposit_event(Event::CycleContinued {
      actor_id,
      cycle_nonce,
      cursor,
    });
    let mut predicate_index = run.opening_predicate_cursor as usize;
    let predicate_result = Self::evaluate_step_precondition(
      step.precondition.as_ref(),
      &instance.sovereign_account,
      plan.maximum_fee.total_fee,
      &run.opening_predicate_results,
      &mut predicate_index,
    );
    let predicate_error = predicate_result.as_ref().err().cloned();
    let predicate_matches = predicate_result.unwrap_or(true);
    let mut resolution_skipped = false;
    let effect_execution = if predicate_matches {
      match predicate_error.map_or_else(
        || {
          Self::prepare_task(
            &step.task,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
            plan.maximum_fee.total_fee,
            &run.opening_snapshot,
            &run.funding_snapshot,
          )
        },
        Err,
      ) {
        Err(error) => {
          let failure = TaskFailure::permanent(error);
          plan.last_step_outcome = Some(StepOutcome::Failed(failure.clone()));
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: cursor,
            retry_class: failure.retry,
            error: failure.error,
          });
          let mut outcomes = run.cumulative_outcomes;
          outcomes.failed_steps = checked_semantic_increment(outcomes.failed_steps)
            .map_err(|_| AttemptTransactionError::Invariant)?;
          let streak = transition_failure_streak(
            plan.hot.unsuccessful_attempt_streak,
            FailureStreakTransition::UnsuccessfulAttempt,
          )
          .ok_or(AttemptTransactionError::Invariant)?;
          return Self::commit_loaded_single_step_failure(
            actor_id,
            plan,
            cycle_nonce,
            streak,
            outcomes,
            LoadedFundingDisposition::Preserve,
            TaskEffectExecution::NotInvoked,
            now,
          );
        }
        Ok(PreparedTaskOutcome::Executable(prepared)) => {
          if let Err(failure) = Self::execute_prepared_task(
            prepared,
            actor_id,
            cycle_nonce,
            cursor,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
          ) {
            let mut outcomes = run.cumulative_outcomes;
            outcomes.failed_steps = checked_semantic_increment(outcomes.failed_steps)
              .map_err(|_| AttemptTransactionError::Invariant)?;
            let failed_outcome = StepOutcome::Failed(failure.clone());
            plan.last_step_outcome = Some(failed_outcome.clone());
            Self::deposit_event(Event::StepFailed {
              actor_id,
              cycle_nonce,
              step_index: cursor,
              retry_class: failure.retry,
              error: failure.error,
            });
            if failure.retry != RetryClass::Temporary {
              let streak = transition_failure_streak(
                plan.hot.unsuccessful_attempt_streak,
                FailureStreakTransition::UnsuccessfulAttempt,
              )
              .ok_or(AttemptTransactionError::Invariant)?;
              return Self::commit_loaded_single_step_failure(
                actor_id,
                plan,
                cycle_nonce,
                streak,
                outcomes,
                LoadedFundingDisposition::Preserve,
                TaskEffectExecution::Invoked,
                now,
              );
            }
            let (streak, terminal) = Self::prepare_loaded_resuspension(
              instance,
              &mut run,
              step
                .on_error
                .retry_max_attempts()
                .ok_or(AttemptTransactionError::Invariant)?,
              plan.hot.unsuccessful_attempt_streak,
              now,
              SuspensionReason::Temporary,
              failed_outcome,
              outcomes,
            )?;
            if terminal {
              return Self::commit_loaded_single_step_failure(
                actor_id,
                plan,
                cycle_nonce,
                streak,
                outcomes,
                LoadedFundingDisposition::Preserve,
                TaskEffectExecution::Invoked,
                now,
              );
            }
            return Self::commit_loaded_single_step_suspension(
              actor_id,
              plan,
              run,
              streak,
              LoadedFundingDisposition::Preserve,
              TaskEffectExecution::Invoked,
            );
          }
          TaskEffectExecution::Invoked
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          plan.last_step_outcome = Some(StepOutcome::FundingUnavailable);
          let outcomes = run.cumulative_outcomes;
          let (streak, terminal) = Self::prepare_loaded_resuspension(
            instance,
            &mut run,
            step
              .on_error
              .retry_max_attempts()
              .ok_or(AttemptTransactionError::Invariant)?,
            plan.hot.unsuccessful_attempt_streak,
            now,
            SuspensionReason::FundingUnavailable,
            StepOutcome::FundingUnavailable,
            outcomes,
          )?;
          if terminal {
            return Self::commit_loaded_single_step_failure(
              actor_id,
              plan,
              cycle_nonce,
              streak,
              outcomes,
              LoadedFundingDisposition::Preserve,
              TaskEffectExecution::NotInvoked,
              now,
            );
          }
          return Self::commit_loaded_single_step_suspension(
            actor_id,
            plan,
            run,
            streak,
            LoadedFundingDisposition::Preserve,
            TaskEffectExecution::NotInvoked,
          );
        }
        Ok(PreparedTaskOutcome::Skipped) => {
          resolution_skipped = true;
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: cursor,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          TaskEffectExecution::NotInvoked
        }
      }
    } else {
      let mut outcomes = run.cumulative_outcomes;
      outcomes.precondition_skips = checked_semantic_increment(outcomes.precondition_skips)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      Self::deposit_event(Event::StepSkipped {
        actor_id,
        cycle_nonce,
        step_index: cursor,
        reason: StepSkippedReason::PreconditionFalse,
      });
      run.cumulative_outcomes = outcomes;
      TaskEffectExecution::NotInvoked
    };
    let mut outcomes = run.cumulative_outcomes;
    if resolution_skipped {
      outcomes.skipped_resolution = checked_semantic_increment(outcomes.skipped_resolution)
        .map_err(|_| AttemptTransactionError::Invariant)?;
    } else if matches!(effect_execution, TaskEffectExecution::Invoked) {
      outcomes.executed_steps = checked_semantic_increment(outcomes.executed_steps)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      if matches!(&step.task, ActorTask::StopCycle) {
        Self::record_stop_cycle_event(actor_id, cycle_nonce, cursor);
      } else {
        outcomes.committed_effectful_tasks =
          checked_semantic_increment(outcomes.committed_effectful_tasks)
            .map_err(|_| AttemptTransactionError::Invariant)?;
      }
    }
    plan.last_step_outcome = Some(if resolution_skipped {
      StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped)
    } else if !predicate_matches {
      StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
    } else if matches!(&step.task, ActorTask::StopCycle) {
      StepOutcome::Stopped
    } else {
      StepOutcome::Executed
    });
    let next_cursor = cursor
      .checked_add(1)
      .ok_or(AttemptTransactionError::Invariant)?;
    if (matches!(&step.task, ActorTask::StopCycle)
      && matches!(effect_execution, TaskEffectExecution::Invoked))
      || next_cursor >= step_count
    {
      ActorRunStateStore::<T>::remove(actor_id);
      plan.run = None;
      plan.identity.cycle_nonce = cycle_nonce;
      plan.hot.cycle_state = CycleState::Idle;
      plan.hot.pending_signal = deferred_signal;
      plan.hot.queue_ticket = None;
      plan.hot.last_cycle_block = Some(now);
      plan.hot.unsuccessful_attempt_streak = 0;
      Self::deposit_event(Event::CycleSummary {
        actor_id,
        cycle_nonce,
        result: CycleResult::Completed,
        outcomes,
      });
      let deferred_eligible_at = if deferred_signal {
        Some(
          now
            .checked_add(&One::one())
            .ok_or(AttemptTransactionError::Invariant)?,
        )
      } else {
        None
      };
      return Ok((
        plan,
        effect_execution,
        AttemptDisposition::Completed,
        outcomes,
        deferred_eligible_at,
      ));
    }
    let eligible_at = now
      .checked_add(&One::one())
      .ok_or(AttemptTransactionError::Invariant)?;
    run.cursor = next_cursor;
    run.opening_predicate_cursor =
      u32::try_from(predicate_index).map_err(|_| AttemptTransactionError::Invariant)?;
    run.unsuccessful_attempts_at_cursor = 0;
    run.last_committed_step_block = Some(now);
    run.eligible_at = eligible_at;
    run.cumulative_outcomes = outcomes;
    run.last_step_outcome = Some(if resolution_skipped {
      StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped)
    } else if predicate_matches {
      StepOutcome::Executed
    } else {
      StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
    });
    run.suspension = None;
    if !run.running_is_coherent() {
      return Err(AttemptTransactionError::Invariant);
    }
    ActorRunStateStore::<T>::insert(actor_id, run.clone());
    plan.run = Some(run);
    plan.hot.cycle_state = CycleState::Running;
    plan.hot.pending_signal = deferred_signal;
    plan.hot.queue_ticket = None;
    plan.hot.unsuccessful_attempt_streak = 0;
    Ok((
      plan,
      effect_execution,
      AttemptDisposition::Continued,
      outcomes,
      Some(eligible_at),
    ))
  }

  fn execute_running_current_step_core(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    mut plan: CurrentStepPlanOf<T>,
    now: BlockNumberFor<T>,
    step_count: u32,
  ) -> Result<
    (
      CurrentStepPlanOf<T>,
      TaskEffectExecution,
      AttemptDisposition,
      OutcomeTotals,
      Option<BlockNumberFor<T>>,
    ),
    AttemptTransactionError,
  > {
    let step = plan.loaded_step.step.clone();
    let direct_task_policy = matches!(
      step.on_error,
      StepErrorPolicy::ContinueNextStep
        | StepErrorPolicy::AbortCycle
        | StepErrorPolicy::RetryLater { .. }
    );
    let mut run = plan.run.take().ok_or(AttemptTransactionError::Invariant)?;
    let deferred_signal = plan.hot.pending_signal;
    if instance.cycle_state != CycleState::Running
      || plan.hot.wakeup_pointer.is_some()
      || plan.ticket.actor_id != actor_id
      || plan.ticket.cursor != run.cursor
      || plan.loaded_step.cursor != run.cursor
      || plan.ticket.cycle_nonce != run.cycle_nonce
      || plan.ticket.eligible_at != run.eligible_at
      || now < run.eligible_at
      || run.cursor == 0
      || run.cursor >= step_count
      || run.cycle_nonce
        != plan
          .identity
          .cycle_nonce
          .checked_add(1)
          .ok_or(AttemptTransactionError::Invariant)?
      || !run.running_is_coherent()
      || !direct_task_policy
      || !plan.admission.has_valid_identity()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let expected_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      plan.loaded_step.resources,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if plan.maximum_fee != expected_fee {
      return Err(AttemptTransactionError::Invariant);
    }
    let cycle_nonce = run.cycle_nonce;
    let cursor = run.cursor;
    run.last_attempt_block = now;
    let mut predicate_index = run.opening_predicate_cursor as usize;
    let predicate_result = Self::evaluate_step_precondition(
      step.precondition.as_ref(),
      &instance.sovereign_account,
      plan.maximum_fee.total_fee,
      &run.opening_predicate_results,
      &mut predicate_index,
    );
    let predicate_error = predicate_result.as_ref().err().cloned();
    let predicate_matches = predicate_result.unwrap_or(true);
    let mut resolution_skipped = false;
    let mut funding_unavailable = false;
    let mut execution_failure = None;
    let mut abort_cycle = false;
    let effect_execution = if predicate_matches {
      match predicate_error.map_or_else(
        || {
          Self::prepare_task(
            &step.task,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
            plan.maximum_fee.total_fee,
            &run.opening_snapshot,
            &run.funding_snapshot,
          )
        },
        Err,
      ) {
        Err(error) => {
          let failure = TaskFailure::permanent(error);
          plan.last_step_outcome = Some(StepOutcome::Failed(failure.clone()));
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: cursor,
            retry_class: failure.retry,
            error: failure.error,
          });
          abort_cycle = !matches!(step.on_error, StepErrorPolicy::ContinueNextStep);
          execution_failure = Some(failure);
          TaskEffectExecution::NotInvoked
        }
        Ok(PreparedTaskOutcome::Executable(prepared)) => {
          if let Err(failure) = Self::execute_prepared_task(
            prepared,
            actor_id,
            cycle_nonce,
            cursor,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
          ) {
            let failed_outcome = StepOutcome::Failed(failure.clone());
            plan.last_step_outcome = Some(failed_outcome.clone());
            Self::deposit_event(Event::StepFailed {
              actor_id,
              cycle_nonce,
              step_index: cursor,
              retry_class: failure.retry,
              error: failure.error,
            });
            if matches!(step.on_error, StepErrorPolicy::ContinueNextStep) {
              execution_failure = Some(failure);
            } else if matches!(step.on_error, StepErrorPolicy::AbortCycle) {
              execution_failure = Some(failure);
              abort_cycle = true;
            } else if failure.retry != RetryClass::Temporary {
              execution_failure = Some(failure);
              abort_cycle = true;
            } else {
              let mut outcomes = run.cumulative_outcomes;
              outcomes.failed_steps = checked_semantic_increment(outcomes.failed_steps)
                .map_err(|_| AttemptTransactionError::Invariant)?;
              let (streak, terminal) = Self::prepare_loaded_resuspension(
                instance,
                &mut run,
                step
                  .on_error
                  .retry_max_attempts()
                  .ok_or(AttemptTransactionError::Invariant)?,
                plan.hot.unsuccessful_attempt_streak,
                now,
                SuspensionReason::Temporary,
                failed_outcome,
                outcomes,
              )?;
              if terminal {
                return Self::commit_loaded_single_step_failure(
                  actor_id,
                  plan,
                  cycle_nonce,
                  streak,
                  outcomes,
                  LoadedFundingDisposition::Preserve,
                  TaskEffectExecution::Invoked,
                  now,
                );
              }
              return Self::commit_loaded_single_step_suspension(
                actor_id,
                plan,
                run,
                streak,
                LoadedFundingDisposition::Preserve,
                TaskEffectExecution::Invoked,
              );
            }
          }
          TaskEffectExecution::Invoked
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          plan.last_step_outcome = Some(StepOutcome::FundingUnavailable);
          if step.on_error.retry_max_attempts().is_none() {
            funding_unavailable = true;
            Self::deposit_event(Event::StepSkipped {
              actor_id,
              cycle_nonce,
              step_index: cursor,
              reason: StepSkippedReason::FundingUnavailable,
            });
            TaskEffectExecution::NotInvoked
          } else {
            let outcomes = run.cumulative_outcomes;
            let (streak, terminal) = Self::prepare_loaded_resuspension(
              instance,
              &mut run,
              step
                .on_error
                .retry_max_attempts()
                .ok_or(AttemptTransactionError::Invariant)?,
              plan.hot.unsuccessful_attempt_streak,
              now,
              SuspensionReason::FundingUnavailable,
              StepOutcome::FundingUnavailable,
              outcomes,
            )?;
            if terminal {
              return Self::commit_loaded_single_step_failure(
                actor_id,
                plan,
                cycle_nonce,
                streak,
                outcomes,
                LoadedFundingDisposition::Preserve,
                TaskEffectExecution::NotInvoked,
                now,
              );
            }
            return Self::commit_loaded_single_step_suspension(
              actor_id,
              plan,
              run,
              streak,
              LoadedFundingDisposition::Preserve,
              TaskEffectExecution::NotInvoked,
            );
          }
        }
        Ok(PreparedTaskOutcome::Skipped) => {
          resolution_skipped = true;
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: cursor,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          TaskEffectExecution::NotInvoked
        }
      }
    } else {
      Self::deposit_event(Event::StepSkipped {
        actor_id,
        cycle_nonce,
        step_index: cursor,
        reason: StepSkippedReason::PreconditionFalse,
      });
      TaskEffectExecution::NotInvoked
    };
    let mut outcomes = run.cumulative_outcomes;
    let last_step_outcome = if let Some(failure) = execution_failure {
      outcomes.failed_steps = checked_semantic_increment(outcomes.failed_steps)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      StepOutcome::Failed(failure)
    } else if funding_unavailable {
      outcomes.skipped_funding_unavailable =
        checked_semantic_increment(outcomes.skipped_funding_unavailable)
          .map_err(|_| AttemptTransactionError::Invariant)?;
      StepOutcome::FundingUnavailable
    } else if resolution_skipped {
      outcomes.skipped_resolution = checked_semantic_increment(outcomes.skipped_resolution)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped)
    } else if predicate_matches {
      outcomes.executed_steps = checked_semantic_increment(outcomes.executed_steps)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      if matches!(&step.task, ActorTask::StopCycle) {
        Self::record_stop_cycle_event(actor_id, cycle_nonce, cursor);
        StepOutcome::Stopped
      } else {
        outcomes.committed_effectful_tasks =
          checked_semantic_increment(outcomes.committed_effectful_tasks)
            .map_err(|_| AttemptTransactionError::Invariant)?;
        StepOutcome::Executed
      }
    } else {
      outcomes.precondition_skips = checked_semantic_increment(outcomes.precondition_skips)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
    };
    plan.last_step_outcome = Some(last_step_outcome.clone());
    if abort_cycle {
      let unsuccessful_attempt_streak = transition_failure_streak(
        plan.hot.unsuccessful_attempt_streak,
        FailureStreakTransition::UnsuccessfulAttempt,
      )
      .ok_or(AttemptTransactionError::Invariant)?;
      return Self::commit_loaded_single_step_failure(
        actor_id,
        plan,
        cycle_nonce,
        unsuccessful_attempt_streak,
        outcomes,
        LoadedFundingDisposition::Preserve,
        effect_execution,
        now,
      );
    }
    let next_cursor = cursor
      .checked_add(1)
      .ok_or(AttemptTransactionError::Invariant)?;
    if matches!(last_step_outcome, StepOutcome::Stopped) || next_cursor >= step_count {
      ActorRunStateStore::<T>::remove(actor_id);
      plan.run = None;
      plan.identity.cycle_nonce = cycle_nonce;
      plan.hot.cycle_state = CycleState::Idle;
      plan.hot.pending_signal = deferred_signal;
      plan.hot.queue_ticket = None;
      plan.hot.last_cycle_block = Some(now);
      plan.hot.unsuccessful_attempt_streak = 0;
      Self::deposit_event(Event::CycleSummary {
        actor_id,
        cycle_nonce,
        result: CycleResult::Completed,
        outcomes,
      });
      let deferred_eligible_at = if deferred_signal {
        Some(
          now
            .checked_add(&One::one())
            .ok_or(AttemptTransactionError::Invariant)?,
        )
      } else {
        None
      };
      return Ok((
        plan,
        effect_execution,
        AttemptDisposition::Completed,
        outcomes,
        deferred_eligible_at,
      ));
    }
    let eligible_at = now
      .checked_add(&One::one())
      .ok_or(AttemptTransactionError::Invariant)?;
    run.cursor = next_cursor;
    run.opening_predicate_cursor =
      u32::try_from(predicate_index).map_err(|_| AttemptTransactionError::Invariant)?;
    run.unsuccessful_attempts_at_cursor = 0;
    run.last_committed_step_block = Some(now);
    run.eligible_at = eligible_at;
    run.cumulative_outcomes = outcomes;
    run.last_step_outcome = Some(last_step_outcome);
    run.suspension = None;
    if !run.running_is_coherent() {
      return Err(AttemptTransactionError::Invariant);
    }
    ActorRunStateStore::<T>::insert(actor_id, run.clone());
    plan.run = Some(run);
    plan.hot.cycle_state = CycleState::Running;
    plan.hot.pending_signal = deferred_signal;
    plan.hot.queue_ticket = None;
    plan.hot.unsuccessful_attempt_streak = 0;
    Ok((
      plan,
      effect_execution,
      AttemptDisposition::Continued,
      outcomes,
      Some(eligible_at),
    ))
  }

  /// Returns the persisted successor Run in `plan.run`; ticket and loaded Step still identify
  /// the attempted Step. Terminal outcomes return no Run authority.
  pub(crate) fn execute_loaded_single_step_core(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    mut plan: CurrentStepPlanOf<T>,
    now: BlockNumberFor<T>,
    step_count: u32,
  ) -> Result<
    (
      CurrentStepPlanOf<T>,
      TaskEffectExecution,
      AttemptDisposition,
      OutcomeTotals,
      Option<BlockNumberFor<T>>,
    ),
    AttemptTransactionError,
  > {
    if instance.cycle_state == CycleState::Running {
      return Self::execute_running_current_step_core(actor_id, instance, plan, now, step_count);
    }
    if instance.cycle_state == CycleState::Suspended {
      return Self::execute_suspended_single_step_core(actor_id, instance, plan, now, step_count);
    }
    let step = plan.loaded_step.step.clone();
    let direct_task_policy = matches!(
      step.on_error,
      StepErrorPolicy::ContinueNextStep
        | StepErrorPolicy::AbortCycle
        | StepErrorPolicy::RetryLater { .. }
    );
    if instance.cycle_state != CycleState::Idle
      || instance.steps.is_empty()
      || instance.steps.len() != step_count as usize
      || plan.run.is_some()
      || ActorRunHeads::<T>::contains_key(actor_id)
      || ActorRunPayloads::<T>::contains_key(actor_id)
      || !plan.hot.pending_signal
      || plan.ticket.actor_id != actor_id
      || plan.ticket.cursor != 0
      || plan.loaded_step.cursor != 0
      || plan.ticket.cycle_nonce
        != plan
          .identity
          .cycle_nonce
          .checked_add(1)
          .ok_or(AttemptTransactionError::Invariant)?
      || instance.steps.first() != Some(&step)
      || step_count == 0
      || !direct_task_policy
      || !plan.admission.has_valid_identity()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let expected_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      plan.loaded_step.resources,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if plan.maximum_fee != expected_fee {
      return Err(AttemptTransactionError::Invariant);
    }
    let opening_snapshot = Self::capture_opening_snapshot(
      instance.actor_class.actor_type(),
      &instance.sovereign_account,
      &instance.steps,
      plan.maximum_fee.total_fee,
    );
    let opening_predicate_results = Self::capture_opening_predicate_results(
      &instance.sovereign_account,
      &instance.steps,
      plan.maximum_fee.total_fee,
    );
    let funding_snapshot = plan.funding.funding_accumulated.clone();
    let cycle_nonce = plan.ticket.cycle_nonce;
    plan.hot.last_cycle_block = Some(now);
    Self::deposit_event(Event::CycleStarted {
      actor_id,
      cycle_nonce,
    });
    let mut opening_predicate_index = 0usize;
    let predicate_result = Self::evaluate_step_precondition(
      step.precondition.as_ref(),
      &instance.sovereign_account,
      plan.maximum_fee.total_fee,
      &opening_predicate_results,
      &mut opening_predicate_index,
    );
    let predicate_error = predicate_result.as_ref().err().cloned();
    let predicate_matches = predicate_result.unwrap_or(true);
    let mut abort_cycle = false;
    let (effect_execution, outcomes, last_step_outcome) = if predicate_matches {
      match predicate_error.map_or_else(
        || {
          Self::prepare_task(
            &step.task,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
            plan.maximum_fee.total_fee,
            &opening_snapshot,
            &funding_snapshot,
          )
        },
        Err,
      ) {
        Err(error) => {
          let failure = TaskFailure::permanent(error);
          plan.last_step_outcome = Some(StepOutcome::Failed(failure.clone()));
          Self::deposit_event(Event::StepFailed {
            actor_id,
            cycle_nonce,
            step_index: 0,
            retry_class: failure.retry,
            error: failure.error,
          });
          abort_cycle = !matches!(step.on_error, StepErrorPolicy::ContinueNextStep);
          (
            TaskEffectExecution::NotInvoked,
            OutcomeTotals {
              failed_steps: 1,
              ..Default::default()
            },
            StepOutcome::Failed(failure),
          )
        }
        Ok(PreparedTaskOutcome::Skipped) => {
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: 0,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          (
            TaskEffectExecution::NotInvoked,
            OutcomeTotals {
              skipped_resolution: 1,
              ..Default::default()
            },
            StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped),
          )
        }
        Ok(PreparedTaskOutcome::FundingUnavailable)
          if step.on_error.retry_max_attempts().is_none() =>
        {
          Self::deposit_event(Event::StepSkipped {
            actor_id,
            cycle_nonce,
            step_index: 0,
            reason: StepSkippedReason::FundingUnavailable,
          });
          (
            TaskEffectExecution::NotInvoked,
            OutcomeTotals {
              skipped_funding_unavailable: 1,
              ..Default::default()
            },
            StepOutcome::FundingUnavailable,
          )
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          plan.last_step_outcome = Some(StepOutcome::FundingUnavailable);
          let max_attempts = step
            .on_error
            .retry_max_attempts()
            .ok_or(AttemptTransactionError::Invariant)?;
          let unsuccessful_attempts_at_cursor = 1u32;
          let unsuccessful_attempt_streak = transition_failure_streak(
            plan.hot.unsuccessful_attempt_streak,
            FailureStreakTransition::UnsuccessfulAttempt,
          )
          .ok_or(AttemptTransactionError::Invariant)?;
          let outcomes = OutcomeTotals::default();
          if unsuccessful_attempts_at_cursor >= max_attempts
            || Self::failure_limit_reached(unsuccessful_attempt_streak)
          {
            return Self::commit_loaded_single_step_failure(
              actor_id,
              plan,
              cycle_nonce,
              unsuccessful_attempt_streak,
              outcomes,
              LoadedFundingDisposition::Clear,
              TaskEffectExecution::NotInvoked,
              now,
            );
          }
          let eligible_at = Self::suspension_eligible_at(
            instance.cooldown_blocks,
            instance.window,
            now,
            unsuccessful_attempts_at_cursor,
          )
          .map_err(|_| AttemptTransactionError::Invariant)?;
          let run = ActorRunState {
            contract_authority: ActorRunAuthority {
              semantic_contract_id: plan.admission.semantic_contract_id,
              body_commitment: plan.admission.body_commitment,
              admission_identity: plan.admission.admission_identity,
            },
            cycle_nonce,
            cursor: 0,
            opening_predicate_cursor: 0,
            unsuccessful_attempts_at_cursor,
            last_attempt_block: now,
            last_committed_step_block: None,
            eligible_at,
            opening_snapshot,
            opening_predicate_results,
            funding_snapshot,
            cumulative_outcomes: outcomes,
            last_step_outcome: Some(StepOutcome::FundingUnavailable),
            suspension: Some(SuspensionReason::FundingUnavailable),
          };
          return Self::commit_loaded_single_step_suspension(
            actor_id,
            plan,
            run,
            unsuccessful_attempt_streak,
            LoadedFundingDisposition::Clear,
            TaskEffectExecution::NotInvoked,
          );
        }
        Ok(PreparedTaskOutcome::Executable(prepared)) => {
          if let Err(failure) = Self::execute_prepared_task(
            prepared,
            actor_id,
            cycle_nonce,
            0,
            &instance.sovereign_account,
            instance.actor_class.actor_type(),
          ) {
            let last_step_outcome = StepOutcome::Failed(failure.clone());
            plan.last_step_outcome = Some(last_step_outcome.clone());
            Self::deposit_event(Event::StepFailed {
              actor_id,
              cycle_nonce,
              step_index: 0,
              retry_class: failure.retry,
              error: failure.error,
            });
            if matches!(step.on_error, StepErrorPolicy::ContinueNextStep) {
              (
                TaskEffectExecution::Invoked,
                OutcomeTotals {
                  failed_steps: 1,
                  ..Default::default()
                },
                last_step_outcome,
              )
            } else if matches!(step.on_error, StepErrorPolicy::AbortCycle) {
              abort_cycle = true;
              (
                TaskEffectExecution::Invoked,
                OutcomeTotals {
                  failed_steps: 1,
                  ..Default::default()
                },
                last_step_outcome,
              )
            } else if failure.retry != RetryClass::Temporary {
              abort_cycle = true;
              (
                TaskEffectExecution::Invoked,
                OutcomeTotals {
                  failed_steps: 1,
                  ..Default::default()
                },
                last_step_outcome,
              )
            } else {
              let max_attempts = step
                .on_error
                .retry_max_attempts()
                .ok_or(AttemptTransactionError::Invariant)?;
              let unsuccessful_attempts_at_cursor = 1u32;
              let unsuccessful_attempt_streak = transition_failure_streak(
                plan.hot.unsuccessful_attempt_streak,
                FailureStreakTransition::UnsuccessfulAttempt,
              )
              .ok_or(AttemptTransactionError::Invariant)?;
              let outcomes = OutcomeTotals {
                failed_steps: 1,
                ..Default::default()
              };
              if unsuccessful_attempts_at_cursor >= max_attempts
                || Self::failure_limit_reached(unsuccessful_attempt_streak)
              {
                return Self::commit_loaded_single_step_failure(
                  actor_id,
                  plan,
                  cycle_nonce,
                  unsuccessful_attempt_streak,
                  outcomes,
                  LoadedFundingDisposition::Clear,
                  TaskEffectExecution::Invoked,
                  now,
                );
              }
              let eligible_at = Self::suspension_eligible_at(
                instance.cooldown_blocks,
                instance.window,
                now,
                unsuccessful_attempts_at_cursor,
              )
              .map_err(|_| AttemptTransactionError::Invariant)?;
              let run = ActorRunState {
                contract_authority: ActorRunAuthority {
                  semantic_contract_id: plan.admission.semantic_contract_id,
                  body_commitment: plan.admission.body_commitment,
                  admission_identity: plan.admission.admission_identity,
                },
                cycle_nonce,
                cursor: 0,
                opening_predicate_cursor: 0,
                unsuccessful_attempts_at_cursor,
                last_attempt_block: now,
                last_committed_step_block: None,
                eligible_at,
                opening_snapshot,
                opening_predicate_results,
                funding_snapshot,
                cumulative_outcomes: outcomes,
                last_step_outcome: Some(last_step_outcome),
                suspension: Some(SuspensionReason::Temporary),
              };
              return Self::commit_loaded_single_step_suspension(
                actor_id,
                plan,
                run,
                unsuccessful_attempt_streak,
                LoadedFundingDisposition::Clear,
                TaskEffectExecution::Invoked,
              );
            }
          } else {
            let committed_effectful_tasks = if matches!(&step.task, ActorTask::StopCycle) {
              Self::record_stop_cycle_event(actor_id, cycle_nonce, 0);
              0
            } else {
              1
            };
            (
              TaskEffectExecution::Invoked,
              OutcomeTotals {
                executed_steps: 1,
                committed_effectful_tasks,
                ..Default::default()
              },
              if matches!(&step.task, ActorTask::StopCycle) {
                StepOutcome::Stopped
              } else {
                StepOutcome::Executed
              },
            )
          }
        }
      }
    } else {
      Self::deposit_event(Event::StepSkipped {
        actor_id,
        cycle_nonce,
        step_index: 0,
        reason: StepSkippedReason::PreconditionFalse,
      });
      (
        TaskEffectExecution::NotInvoked,
        OutcomeTotals {
          precondition_skips: 1,
          ..Default::default()
        },
        StepOutcome::Skipped(StepSkippedReason::PreconditionFalse),
      )
    };
    plan.last_step_outcome = Some(last_step_outcome.clone());
    if abort_cycle {
      let unsuccessful_attempt_streak = transition_failure_streak(
        plan.hot.unsuccessful_attempt_streak,
        FailureStreakTransition::UnsuccessfulAttempt,
      )
      .ok_or(AttemptTransactionError::Invariant)?;
      return Self::commit_loaded_single_step_failure(
        actor_id,
        plan,
        cycle_nonce,
        unsuccessful_attempt_streak,
        outcomes,
        LoadedFundingDisposition::Clear,
        effect_execution,
        now,
      );
    }
    if step_count > 1 && !matches!(last_step_outcome, StepOutcome::Stopped) {
      let eligible_at = now
        .checked_add(&One::one())
        .ok_or(AttemptTransactionError::Invariant)?;
      let run = ActorRunState {
        contract_authority: ActorRunAuthority {
          semantic_contract_id: plan.admission.semantic_contract_id,
          body_commitment: plan.admission.body_commitment,
          admission_identity: plan.admission.admission_identity,
        },
        cycle_nonce,
        cursor: 1,
        opening_predicate_cursor: u32::try_from(opening_predicate_index)
          .map_err(|_| AttemptTransactionError::Invariant)?,
        unsuccessful_attempts_at_cursor: 0,
        last_attempt_block: now,
        last_committed_step_block: Some(now),
        eligible_at,
        opening_snapshot,
        opening_predicate_results,
        funding_snapshot,
        cumulative_outcomes: outcomes,
        last_step_outcome: Some(last_step_outcome),
        suspension: None,
      };
      if !run.running_is_coherent() {
        return Err(AttemptTransactionError::Invariant);
      }
      ActorRunStateStore::<T>::insert(actor_id, run.clone());
      plan.run = Some(run);
      plan.hot.cycle_state = CycleState::Running;
      plan.hot.pending_signal = false;
      plan.hot.queue_ticket = None;
      plan.hot.last_cycle_block = Some(now);
      plan.hot.unsuccessful_attempt_streak = 0;
      plan.funding.funding_accumulated.clear();
      ActorFunding::<T>::insert(actor_id, &plan.funding);
      return Ok((
        plan,
        effect_execution,
        AttemptDisposition::Continued,
        outcomes,
        Some(eligible_at),
      ));
    }
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Completed,
      outcomes,
    });
    plan.identity.cycle_nonce = cycle_nonce;
    plan.hot.cycle_state = CycleState::Idle;
    plan.hot.pending_signal = false;
    plan.hot.queue_ticket = None;
    plan.hot.last_cycle_block = Some(now);
    plan.hot.unsuccessful_attempt_streak = 0;
    plan.funding.funding_accumulated.clear();
    ActorFunding::<T>::insert(actor_id, &plan.funding);
    Ok((
      plan,
      effect_execution,
      AttemptDisposition::Completed,
      outcomes,
      None,
    ))
  }

  pub fn simulate_current_contract(
    actor_id: ActorId,
    expected_type: ActorType,
    expected_mutability: Mutability,
    expected_contract: ActorContractOf<T>,
    mode: SimulationMode,
    budget: SimulationBudget,
  ) -> Result<SimulationResult, SimulationError> {
    Self::validate_trigger(
      &expected_contract.trigger,
      expected_contract.cooldown_blocks,
    )
    .map_err(|_| SimulationError::InvalidContract)?;
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
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
    if state.contract != expected_contract {
      return Err(SimulationError::ContractMismatch);
    }
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    if instance.actor_class.actor_type() != expected_type {
      return Err(SimulationError::TypeMismatch);
    }
    if instance.mutability != expected_mutability {
      return Err(SimulationError::MutabilityMismatch);
    }
    match mode {
      SimulationMode::FreshCurrentPlan if instance.cycle_state != CycleState::Idle => {
        return Err(SimulationError::ModeCycleStateMismatch);
      }
      SimulationMode::CurrentRun
        if !matches!(
          instance.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) =>
      {
        return Err(SimulationError::ModeCycleStateMismatch);
      }
      _ => {}
    }
    let classification = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(SimulationError::Classification)?;
    if classification.execution_phase == ActorExecutionPhase::GlobalCircuitBreaker {
      return Err(SimulationError::GlobalCircuitBreaker);
    }
    budget
      .checked_limits()
      .map_err(|_| SimulationError::InvalidBudget)?;
    if classification.terminal_reason.is_none() {
      match classification.execution_phase {
        ActorExecutionPhase::Ready => {}
        ActorExecutionPhase::Paused => return Err(SimulationError::Paused),
        ActorExecutionPhase::GlobalCircuitBreaker => {
          return Err(SimulationError::GlobalCircuitBreaker);
        }
        _ => return Err(SimulationError::NotReady),
      }
    }
    polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err(|| {
      let result = Self::simulate_actor_service(actor_id, budget);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
    })
    .map_err(|()| SimulationError::TransactionDepthExceeded)?
  }

  pub(crate) fn failure_limit_reached(unsuccessful_attempt_streak: u32) -> bool {
    let max_failures = T::MaxConsecutiveFailures::get();
    max_failures > 0 && unsuccessful_attempt_streak >= max_failures
  }

  pub(crate) fn collect_user_step_fee(actor: &T::AccountId, fee: T::Balance) -> DispatchResult {
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

  pub(crate) fn capture_opening_predicate_results(
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

  pub(crate) fn capture_opening_snapshot(
    actor_type: ActorType,
    actor: &T::AccountId,
    contract_steps: &ContractSteps<T>,
    reserved: T::Balance,
  ) -> RunOpeningSnapshotOf<T> {
    let mut snapshot = RunOpeningSnapshotOf::<T>::default();
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

  fn opening_balance(
    opening_snapshot: &RunOpeningSnapshotOf<T>,
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
    trigger_balances: &RunOpeningSnapshotOf<T>,
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
    trigger_balances: &RunOpeningSnapshotOf<T>,
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

  pub(crate) fn evaluate_step_precondition(
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
      if asset == T::FeeNativeAssetId::get() {
        FeeAssetClass::FeeNative
      } else {
        FeeAssetClass::Other
      },
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
    trigger_share_balances: &RunOpeningSnapshotOf<T>,
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
    trigger_balances: &RunOpeningSnapshotOf<T>,
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

      let opening_snapshot = RunOpeningSnapshotOf::<Test>::default();
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
