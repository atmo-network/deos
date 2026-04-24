use super::pallet::*;
use super::types::Task as AaaTask;
use super::{AssetOps, DexOps, LiquidityDonationOps, StakingOps};
use alloc::vec::Vec;
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
  Noop,
}

enum PreparedTaskOutcome<T: Config> {
  Executable(PreparedTask<T>),
  Skipped,
  FundingUnavailable,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_single_cycle(aaa_id: AaaId) -> Weight {
    let base_weight = T::DbWeight::get()
      .reads(1)
      .saturating_add(T::DbWeight::get().writes(1));
    let now = frame_system::Pallet::<T>::block_number();
    let instance = match AaaInstances::<T>::get(aaa_id) {
      Some(inst) => inst,
      None => return base_weight,
    };
    if instance.cycle_nonce == u64::MAX {
      if instance.aaa_type == AaaType::User {
        let _ = Self::close_actor(aaa_id, &instance, CloseReason::CycleNonceExhausted);
      } else {
        AaaInstances::<T>::mutate(aaa_id, |maybe| {
          if let Some(inst) = maybe.as_mut() {
            inst.is_paused = true;
            inst.pause_reason = Some(PauseReason::CycleNonceExhausted);
            inst.updated_at = now;
          }
        });
        Self::sync_readiness_state(aaa_id);
        // Ringless: no need to remove from ring - scheduler checks is_paused flag
        Self::deposit_event(Event::AaaPaused {
          aaa_id,
          reason: PauseReason::CycleNonceExhausted,
        });
      }
      return base_weight;
    }
    let cycle_nonce = AaaInstances::<T>::mutate(aaa_id, |maybe| {
      let inst = maybe.as_mut().expect("instance verified above");
      inst.cycle_nonce = inst.cycle_nonce.saturating_add(1);
      inst.manual_trigger_pending = false;
      inst.last_cycle_block = now;
      inst.updated_at = now;
      inst.cycle_nonce
    });
    Self::sync_readiness_state(aaa_id);
    if matches!(instance.schedule.trigger, Trigger::OnAddressEvent { .. }) {
      Self::consume_address_event(aaa_id);
    }
    Self::deposit_event(Event::CycleStarted {
      aaa_id,
      cycle_nonce,
    });
    let actor = instance.sovereign_account.clone();
    let is_user = instance.aaa_type == AaaType::User;
    let execution_plan = &instance.execution_plan;
    let funding_snapshots = &instance.funding_snapshots;
    let mut executed_steps: u32 = 0;
    let mut skipped_conditions: u32 = 0;
    let mut skipped_resolution: u32 = 0;
    let mut skipped_funding_unavailable: u32 = 0;
    let mut failed_steps: u32 = 0;
    let mut execution_plan_failed = false;
    let mut reserved_fee_remaining = if is_user {
      Self::cycle_fee_upper_bound(&instance)
    } else {
      T::Balance::zero()
    };
    let trigger_balances =
      Self::capture_trigger_balances(&actor, execution_plan, reserved_fee_remaining);
    let mut intent_buffer: Vec<PreparedTask<T>> = Vec::new();
    for (step_idx, step) in execution_plan.iter().enumerate() {
      let step_num = step_idx as u32;
      if is_user {
        let eval_fee = Self::compute_eval_fee(step.conditions.len() as u32);
        if !eval_fee.is_zero() {
          reserved_fee_remaining = reserved_fee_remaining.saturating_sub(eval_fee);
          let native = T::NativeAssetId::get();
          let balance = T::AssetOps::balance(&actor, native);
          let fee_sink = T::FeeSink::get();
          if balance < eval_fee {
            let err = DispatchError::from(Error::<T>::InsufficientFee);
            failed_steps = failed_steps.saturating_add(1);
            Self::deposit_event(Event::StepFailed {
              aaa_id,
              cycle_nonce,
              step_index: step_num,
              error: err,
            });
            execution_plan_failed =
              Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, err);
            if execution_plan_failed {
              break;
            }
            continue;
          }
          if T::AssetOps::transfer(&actor, &fee_sink, native, eval_fee).is_err() {
            failed_steps = failed_steps.saturating_add(1);
            Self::deposit_event(Event::StepFailed {
              aaa_id,
              cycle_nonce,
              step_index: step_num,
              error: DispatchError::Other("EvaluationFeeTransferFailed"),
            });
            execution_plan_failed = Self::apply_error_policy(
              aaa_id,
              cycle_nonce,
              step_num,
              step.on_error,
              DispatchError::Other("EvaluationFeeTransferFailed"),
            );
            if execution_plan_failed {
              break;
            }
            continue;
          }
        }
      }
      let condition_result =
        Self::evaluate_conditions(&step.conditions, &actor, reserved_fee_remaining);
      match condition_result {
        Ok(true) => {}
        Ok(false) => {
          if is_user && !matches!(step.task, AaaTask::Noop) {
            let skip_exec_fee =
              T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(skip_exec_fee);
          }
          skipped_conditions = skipped_conditions.saturating_add(1);
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ConditionsNotMet,
          });
          continue;
        }
        Err(e) => {
          if is_user && !matches!(step.task, AaaTask::Noop) {
            let skip_exec_fee =
              T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(skip_exec_fee);
          }
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: e,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, e);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      }
      let exec_fee = T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
      let charge_exec_fee = is_user && !matches!(step.task, AaaTask::Noop) && !exec_fee.is_zero();
      let prepared_task = match Self::prepare_task(
        &step.task,
        &actor,
        instance.aaa_type,
        reserved_fee_remaining,
        &trigger_balances,
        funding_snapshots,
      ) {
        Ok(PreparedTaskOutcome::Executable(task)) => task,
        Ok(PreparedTaskOutcome::Skipped) => {
          if charge_exec_fee {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          }
          skipped_resolution = skipped_resolution.saturating_add(1);
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::ResolutionSkipped,
          });
          continue;
        }
        Ok(PreparedTaskOutcome::FundingUnavailable) => {
          skipped_funding_unavailable = skipped_funding_unavailable.saturating_add(1);
          Self::deposit_event(Event::StepSkipped {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            reason: StepSkippedReason::FundingUnavailable,
          });
          continue;
        }
        Err(e) => {
          if charge_exec_fee {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          }
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: e,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, e);
          if execution_plan_failed {
            break;
          }
          continue;
        }
      };
      if charge_exec_fee {
        let native = T::NativeAssetId::get();
        let balance = T::AssetOps::balance(&actor, native);
        let fee_sink = T::FeeSink::get();
        if balance < exec_fee {
          reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          let err = DispatchError::from(Error::<T>::InsufficientFee);
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: err,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, err);
          if execution_plan_failed {
            break;
          }
          continue;
        }
        if T::AssetOps::transfer(&actor, &fee_sink, native, exec_fee).is_err() {
          reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error: DispatchError::Other("ExecutionFeeTransferFailed"),
          });
          execution_plan_failed = Self::apply_error_policy(
            aaa_id,
            cycle_nonce,
            step_num,
            step.on_error,
            DispatchError::Other("ExecutionFeeTransferFailed"),
          );
          if execution_plan_failed {
            break;
          }
          continue;
        }
        reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
      }
      intent_buffer.push(prepared_task);
      for task in intent_buffer.drain(..) {
        if let Err(error) = Self::execute_prepared_task(task, aaa_id, &actor) {
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::StepFailed {
            aaa_id,
            cycle_nonce,
            step_index: step_num,
            error,
          });
          execution_plan_failed =
            Self::apply_error_policy(aaa_id, cycle_nonce, step_num, step.on_error, error);
          break;
        }
        executed_steps = executed_steps.saturating_add(1);
      }
      if execution_plan_failed {
        break;
      }
    }
    if !execution_plan_failed {
      AaaInstances::<T>::mutate(aaa_id, |maybe| {
        if let Some(inst) = maybe.as_mut() {
          inst.consecutive_failures = 0;
        }
      });
    } else {
      AaaInstances::<T>::mutate(aaa_id, |maybe| {
        if let Some(inst) = maybe.as_mut() {
          inst.consecutive_failures = inst.consecutive_failures.saturating_add(1);
        }
      });
      if let Some(inst) = AaaInstances::<T>::get(aaa_id) {
        if !inst.is_paused && Self::failure_limit_reached(inst.consecutive_failures) {
          let _ = Self::close_actor(aaa_id, &inst, CloseReason::ConsecutiveFailures);
        }
      }
    }
    Self::deposit_event(Event::CycleSummary {
      aaa_id,
      cycle_nonce,
      executed_steps,
      skipped_conditions,
      skipped_resolution,
      skipped_funding_unavailable,
      failed_steps,
    });
    if !execution_plan_failed {
      if let Some(inst) = AaaInstances::<T>::get(aaa_id) {
        if let Some(target_nonce) = inst.auto_close_at_cycle_nonce {
          if cycle_nonce >= target_nonce {
            let _ = Self::close_actor(aaa_id, &inst, CloseReason::AutoCloseNonceReached);
          }
        }
      }
    }
    base_weight.saturating_add(Weight::from_parts(
      5_000_000u64.saturating_mul(executed_steps as u64 + 1),
      1000u64.saturating_mul(executed_steps as u64 + 1),
    ))
  }

  pub(crate) fn failure_limit_reached(consecutive_failures: u32) -> bool {
    let max_failures = T::MaxConsecutiveFailures::get();
    max_failures > 0 && consecutive_failures >= max_failures
  }

  pub(crate) fn execute_on_close_execution_plan(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    mut reserved_fee_remaining: T::Balance,
  ) {
    let actor = &instance.sovereign_account;
    let execution_plan = &instance.on_close_execution_plan;
    let funding_snapshots = &instance.funding_snapshots;
    let is_user = instance.aaa_type == AaaType::User;
    let trigger_balances =
      Self::capture_trigger_balances(actor, execution_plan, reserved_fee_remaining);
    let mut intent_buffer: Vec<PreparedTask<T>> = Vec::new();
    let mut executed_steps = 0u32;
    let mut skipped_steps = 0u32;
    let mut failed_steps = 0u32;
    for (step_idx, step) in execution_plan.iter().enumerate() {
      let step_num = step_idx as u32;
      if is_user {
        let eval_fee = Self::compute_eval_fee(step.conditions.len() as u32);
        if !eval_fee.is_zero() {
          let native = T::NativeAssetId::get();
          let balance = T::AssetOps::balance(actor, native);
          let fee_sink = T::FeeSink::get();
          if balance < eval_fee {
            let error = DispatchError::from(Error::<T>::InsufficientFee);
            failed_steps = failed_steps.saturating_add(1);
            Self::deposit_event(Event::OnCloseStepFailed {
              aaa_id,
              step_index: step_num,
              error,
            });
            continue;
          }
          if T::AssetOps::transfer(actor, &fee_sink, native, eval_fee).is_err() {
            let error = DispatchError::Other("EvaluationFeeTransferFailed");
            failed_steps = failed_steps.saturating_add(1);
            Self::deposit_event(Event::OnCloseStepFailed {
              aaa_id,
              step_index: step_num,
              error,
            });
            continue;
          }
          reserved_fee_remaining = reserved_fee_remaining.saturating_sub(eval_fee);
        }
      }
      match Self::evaluate_conditions(&step.conditions, actor, reserved_fee_remaining) {
        Ok(true) => {}
        Ok(false) => {
          if is_user && !matches!(step.task, AaaTask::Noop) {
            let skip_exec_fee =
              T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(skip_exec_fee);
          }
          skipped_steps = skipped_steps.saturating_add(1);
          continue;
        }
        Err(error) => {
          if is_user && !matches!(step.task, AaaTask::Noop) {
            let skip_exec_fee =
              T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(skip_exec_fee);
          }
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::OnCloseStepFailed {
            aaa_id,
            step_index: step_num,
            error,
          });
          if Self::apply_error_policy(aaa_id, instance.cycle_nonce, step_num, step.on_error, error)
          {
            break;
          }
          continue;
        }
      }
      let exec_fee = T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
      let charge_exec_fee = is_user && !matches!(step.task, AaaTask::Noop) && !exec_fee.is_zero();
      let prepared_task = match Self::prepare_task(
        &step.task,
        actor,
        instance.aaa_type,
        reserved_fee_remaining,
        &trigger_balances,
        funding_snapshots,
      ) {
        Ok(PreparedTaskOutcome::Executable(task)) => task,
        Ok(PreparedTaskOutcome::Skipped | PreparedTaskOutcome::FundingUnavailable) => {
          if charge_exec_fee {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          }
          skipped_steps = skipped_steps.saturating_add(1);
          continue;
        }
        Err(error) => {
          if charge_exec_fee {
            reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
          }
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::OnCloseStepFailed {
            aaa_id,
            step_index: step_num,
            error,
          });
          if Self::apply_error_policy(aaa_id, instance.cycle_nonce, step_num, step.on_error, error)
          {
            break;
          }
          continue;
        }
      };
      if charge_exec_fee {
        let native = T::NativeAssetId::get();
        let balance = T::AssetOps::balance(actor, native);
        let fee_sink = T::FeeSink::get();
        if balance < exec_fee {
          let error = DispatchError::from(Error::<T>::InsufficientFee);
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::OnCloseStepFailed {
            aaa_id,
            step_index: step_num,
            error,
          });
          continue;
        }
        if T::AssetOps::transfer(actor, &fee_sink, native, exec_fee).is_err() {
          let error = DispatchError::Other("ExecutionFeeTransferFailed");
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::OnCloseStepFailed {
            aaa_id,
            step_index: step_num,
            error,
          });
          continue;
        }
        reserved_fee_remaining = reserved_fee_remaining.saturating_sub(exec_fee);
      }
      intent_buffer.push(prepared_task);
      let mut abort = false;
      for task in intent_buffer.drain(..) {
        if let Err(error) = Self::execute_prepared_task(task, aaa_id, actor) {
          failed_steps = failed_steps.saturating_add(1);
          Self::deposit_event(Event::OnCloseStepFailed {
            aaa_id,
            step_index: step_num,
            error,
          });
          abort =
            Self::apply_error_policy(aaa_id, instance.cycle_nonce, step_num, step.on_error, error);
          if abort {
            break;
          }
          continue;
        }
        executed_steps = executed_steps.saturating_add(1);
      }
      if abort {
        break;
      }
    }
    Self::deposit_event(Event::OnCloseExecutionPlanSummary {
      aaa_id,
      executed_steps,
      skipped_steps,
      failed_steps,
    });
  }

  pub(crate) fn compute_eval_fee(num_conditions: u32) -> BalanceOf<T> {
    let base = T::StepBaseFee::get();
    let per_cond = T::ConditionReadFee::get();
    base.saturating_add(per_cond.saturating_mul(num_conditions.into()))
  }

  fn apply_error_policy(
    _aaa_id: AaaId,
    _cycle_nonce: u64,
    _step: u32,
    policy: StepErrorPolicy,
    _error: DispatchError,
  ) -> bool {
    match policy {
      StepErrorPolicy::AbortCycle => true,
      StepErrorPolicy::ContinueNextStep => false,
    }
  }

  fn push_asset_once(assets: &mut alloc::vec::Vec<T::AssetId>, asset: T::AssetId) {
    if !assets.contains(&asset) {
      assets.push(asset);
    }
  }

  fn push_trigger_asset(
    amount: &AmountResolution<T::Balance>,
    asset: T::AssetId,
    assets: &mut alloc::vec::Vec<T::AssetId>,
  ) {
    if matches!(amount, AmountResolution::PercentageOfTrigger(_)) {
      Self::push_asset_once(assets, asset);
    }
  }

  fn collect_percentage_trigger_assets(task: &TaskOf<T>, assets: &mut alloc::vec::Vec<T::AssetId>) {
    match task {
      AaaTask::Transfer { asset, amount, .. }
      | AaaTask::SplitTransfer { asset, amount, .. }
      | AaaTask::Burn { asset, amount }
      | AaaTask::Mint { asset, amount }
      | AaaTask::RemoveLiquidity {
        lp_asset: asset,
        amount,
      } => {
        Self::push_trigger_asset(amount, *asset, assets);
      }
      AaaTask::SwapExactIn {
        asset_in,
        amount_in,
        ..
      } => {
        Self::push_trigger_asset(amount_in, *asset_in, assets);
      }
      AaaTask::SwapExactOut {
        asset_out,
        amount_out,
        ..
      } => {
        Self::push_trigger_asset(amount_out, *asset_out, assets);
      }
      AaaTask::AddLiquidity {
        asset_a,
        asset_b,
        amount_a,
        amount_b,
      } => {
        Self::push_trigger_asset(amount_a, *asset_a, assets);
        Self::push_trigger_asset(amount_b, *asset_b, assets);
      }
      AaaTask::Stake { asset, amount } => {
        Self::push_trigger_asset(amount, *asset, assets);
      }
      AaaTask::DonateLiquidity {
        asset_a, amount, ..
      } => {
        Self::push_trigger_asset(amount, *asset_a, assets);
      }
      AaaTask::Noop | AaaTask::Unstake { .. } => {}
    }
  }

  fn capture_trigger_balances(
    actor: &T::AccountId,
    execution_plan: &ExecutionPlanOf<T>,
    reserved: T::Balance,
  ) -> alloc::vec::Vec<(T::AssetId, T::Balance)> {
    let mut assets: alloc::vec::Vec<T::AssetId> = alloc::vec::Vec::new();
    for step in execution_plan.iter() {
      Self::collect_percentage_trigger_assets(&step.task, &mut assets);
    }
    let mut balances: alloc::vec::Vec<(T::AssetId, T::Balance)> =
      alloc::vec::Vec::with_capacity(assets.len());
    for asset in assets.into_iter() {
      let trigger_balance = Self::spendable_balance(actor, asset, reserved);
      balances.push((asset, trigger_balance));
    }
    balances
  }

  fn trigger_balance(
    trigger_balances: &[(T::AssetId, T::Balance)],
    asset: T::AssetId,
  ) -> Option<T::Balance> {
    trigger_balances.iter().find_map(|(stored_asset, balance)| {
      if *stored_asset == asset {
        Some(*balance)
      } else {
        None
      }
    })
  }

  fn prepare_task(
    task: &TaskOf<T>,
    actor: &T::AccountId,
    aaa_type: AaaType,
    reserved: T::Balance,
    trigger_balances: &[(T::AssetId, T::Balance)],
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingSnapshot<T::Balance, BlockNumberFor<T>>,
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
        Ok(PreparedTaskOutcome::Executable(
          PreparedTask::SwapExactOut {
            asset_in: *asset_in,
            asset_out: *asset_out,
            amount_out: resolved,
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
        let resolved_a = match Self::resolve_for_task(
          amount_a,
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
        let resolved_b = match Self::resolve_for_task(
          amount_b,
          *asset_b,
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
          PreparedTask::AddLiquidity {
            asset_a: *asset_a,
            asset_b: *asset_b,
            amount_a: resolved_a,
            amount_b: resolved_b,
          },
        ))
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
        let resolved = match Self::resolve_for_task(
          shares,
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
        Ok(PreparedTaskOutcome::Executable(PreparedTask::Unstake {
          asset: *asset,
          shares: resolved,
        }))
      }
      AaaTask::Noop => Ok(PreparedTaskOutcome::Executable(PreparedTask::Noop)),
    }
  }

  fn execute_prepared_task(
    task: PreparedTask<T>,
    aaa_id: AaaId,
    actor: &T::AccountId,
  ) -> DispatchResult {
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
        ensure!(actor_balance >= total, Error::<T>::InsufficientBalance);
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
        let transfer_result = polkadot_sdk::frame_support::storage::with_transaction(|| {
          for (to, leg_amount) in normalized_transfers.iter() {
            if let Err(err) = T::AssetOps::transfer(actor, to, asset, *leg_amount) {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(err));
            }
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
        });
        transfer_result?;
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
        slippage_tolerance,
      } => {
        let amount_in =
          T::DexOps::swap_exact_out(actor, asset_in, asset_out, amount_out, slippage_tolerance)?;
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
      PreparedTask::Noop => {}
    }
    Ok(())
  }

  fn resolve_for_task(
    spec: &AmountResolution<T::Balance>,
    asset: T::AssetId,
    actor: &T::AccountId,
    reserved: T::Balance,
    trigger_balances: &[(T::AssetId, T::Balance)],
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingSnapshot<T::Balance, BlockNumberFor<T>>,
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

  pub(crate) fn evaluate_conditions(
    conditions: &BoundedVec<Condition<T::AssetId, T::Balance>, T::MaxConditionsPerStep>,
    who: &T::AccountId,
    reserved: T::Balance,
  ) -> Result<bool, DispatchError> {
    for cond in conditions.iter() {
      let pass = match cond {
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
      };
      if !pass {
        return Ok(false);
      }
    }
    Ok(true)
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

  fn resolve_amount_with_policy(
    spec: &AmountResolution<T::Balance>,
    asset: T::AssetId,
    who: &T::AccountId,
    reserved: T::Balance,
    trigger_balances: &[(T::AssetId, T::Balance)],
    funding_snapshots: &BoundedBTreeMap<
      T::AssetId,
      FundingSnapshot<T::Balance, BlockNumberFor<T>>,
      T::MaxFundingTrackedAssets,
    >,
    policy: AmountResolutionPolicy,
  ) -> Result<AmountResolutionOutcome<T::Balance>, DispatchError> {
    let spendable_current = Self::spendable_balance(who, asset, reserved);
    let resolved = match spec {
      AmountResolution::Fixed(amount) => *amount,
      AmountResolution::AllBalance => {
        if policy == AmountResolutionPolicy::PreserveSpend {
          spendable_current.saturating_sub(T::AssetOps::minimum_balance(asset))
        } else {
          spendable_current
        }
      }
      AmountResolution::PercentageOfCurrent(pct) => {
        let value = pct.mul_floor(spendable_current);
        if !pct.is_zero() && !spendable_current.is_zero() && value.is_zero() {
          return Ok(AmountResolutionOutcome::Skipped);
        }
        value
      }
      AmountResolution::PercentageOfTrigger(pct) => {
        let Some(trigger_balance) = Self::trigger_balance(trigger_balances, asset) else {
          return Ok(AmountResolutionOutcome::Skipped);
        };
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
        if policy == AmountResolutionPolicy::PreserveSpend && value > spendable_current {
          return Ok(AmountResolutionOutcome::FundingUnavailable);
        }
        value
      }
    };
    if resolved.is_zero() {
      return Ok(AmountResolutionOutcome::Skipped);
    }
    if policy == AmountResolutionPolicy::PreserveSpend && resolved > spendable_current {
      return Ok(AmountResolutionOutcome::FundingUnavailable);
    }
    Ok(AmountResolutionOutcome::Resolved(resolved))
  }
}
