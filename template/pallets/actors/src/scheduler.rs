use super::pallet::*;
use super::{
  AddressEvent, AdmissionCertificateAuthorityProvider as _, AssetOps, BlockResourceDomain,
  BlockResourceLimits, BlockResourceState, FinancialWaitBackingProvider as _,
  FundingAuthority as _, IngressFailure, RetryClass, StepControlExecution, StepControlOutcome,
  StepControlPhase, StepControlPlacement, StepControlWeightContext, StepControlWeightProvider as _,
  TaskEffectWeightProvider as _, weights::WeightInfo,
};
#[cfg(test)]
use alloc::vec;
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err;
use polkadot_sdk::sp_runtime::traits::{One, Zero};
use polkadot_sdk::sp_weights::WeightMeter;

#[derive(Clone, Copy)]
enum QueueMutation {
  Enqueue,
  Head,
}

/// Transient authenticated ingress view; no hint becomes a persistent scheduling owner.
enum FinancialIngressState<T: Config> {
  BalanceOnly,
  Waiting {
    actor: ActorRef,
    state: ActiveActorStateOf<T>,
    process: ActorProcessOf<T>,
    episode: crate::FinancialWaitEpisode<T::Balance>,
    terminal_reason: Option<CloseReason>,
    cleanup: Weight,
  },
}

/// Storage-free classification of the next canonical process residence.
/// Publication remains separate so planning cannot transiently create dual authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NextWorkPlan<BlockNumber> {
  Disabled(ProcessDisablement<BlockNumber>),
  Service(ServiceResidenceKind),
  Wakeup(BlockNumber),
}

/// Transient Idle destination; physical Deadline/Park transfer remains separately owned.
/// Park borrows its authored activation rather than copying or persisting another plan.
pub(crate) enum PreparedIdleServicePlacement<'a, T: Config> {
  Disabled {
    process: ActorProcessOf<T>,
    disablement: ProcessDisablement<BlockNumberFor<T>>,
  },
  Deadline {
    at: BlockNumberFor<T>,
  },
  Parked {
    kind: ServiceResidenceKind,
    activation: &'a ParkedBalanceActivationOf<T>,
    review_at: BlockNumberFor<T>,
  },
}

/// Complete generation-bound destination selected before atomic publication.
/// This plan owns no storage and cannot become a second committer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedProcessDestination<BlockNumber> {
  Disabled(ActorProcess<BlockNumber>),
  Service {
    process: ActorProcess<BlockNumber>,
    admission_round: BlockNumber,
  },
  Deadline {
    process: ActorProcess<BlockNumber>,
    handle: DeadlineHandle<BlockNumber>,
  },
  Parked {
    process: ActorProcess<BlockNumber>,
    review: DeadlineHandle<BlockNumber>,
  },
}

/// Storage-free publication plan for the two independent physical obligations
/// owned by one active Actor: exactly one process residence and, when its
/// temporal Trigger is armed, one additional Tick deadline.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannedActorPublication<BlockNumber> {
  hot: ActorHotState<BlockNumber>,
  process: PlannedProcessDestination<BlockNumber>,
  trigger_deadline: Option<DeadlineHandle<BlockNumber>>,
  resources: ActorStepResourceEnvelope,
}

pub(crate) enum CancellationPublicationAuthority {
  Current(ActorStepResourceEnvelope),
  /// The enclosing Contract replacement supersedes this carrier before another Attempt.
  Retiring(ActorStepResourceEnvelope),
}

struct QueueTopology {
  head: QueueTicket,
  tail: QueueTicket,
  occupancy: u32,
}

pub(crate) struct QueueAppendPlan<T: Config> {
  publications: Vec<PreparedReadyPublication<T>>,
  next_tail: QueueTicket,
  next_occupancy: u32,
}

struct PreparedReadyPublication<T: Config> {
  ticket: QueueTicket,
  cell: ActorControlCellOf<T>,
}

/// Authenticated header/policy source shared by qualification and intrinsic Step control.
struct EffectfulStepSource<T: Config> {
  state: ActiveActorStateOf<T>,
  step_count: u32,
  behavior: Option<LoadedBehaviorPolicyOf<T>>,
}

struct PreparedEffectfulStepControl<T: Config> {
  source: EffectfulStepSource<T>,
  plan: CurrentStepPlanOf<T>,
  step: StepOf<T>,
  execution_instance: ActiveActorViewOf<T>,
  control_context: StepControlWeightContext,
}

/// Carrier-neutral semantic result and next-residence intent of one Step attempt.
/// Physical placement remains owned by the caller's encompassing transaction.
struct EffectfulStepTransition<T: Config> {
  plan: CurrentStepPlanOf<T>,
  execution_instance: ActiveActorViewOf<T>,
  step: StepOf<T>,
  control_context: StepControlWeightContext,
  reserved_control_weight: Weight,
  effect_execution: super::TaskEffectExecution,
  actual_effect_weight: Weight,
  disposition: AttemptDisposition,
  attempt: Option<ActorAttemptEvidence>,
  financial_wait: Option<crate::UnpaidActorService>,
  next_residence: NextResidence<T>,
  eligible_at: Option<BlockNumberFor<T>>,
  parked_balance_reserved: bool,
  behavior: Option<BehaviorPolicyCandidateOf<T>>,
}

/// Carrier-neutral intent selected after a semantic transition. The legacy
/// FIFO adapter may realize this intent physically or fail closed when its
/// scheduler namespace is exhausted.
enum NextResidence<T: Config> {
  Close {
    state: ActiveActorStateOf<T>,
    reason: CloseReason,
  },
  Publish {
    state: ActiveActorStateOf<T>,
  },
}

/// Carrier-neutral semantic result of completing an admitted empty Contract.
/// The legacy FIFO adapter decides whether the still-live Actor is closed or
/// republished after the semantic cycle has completed.
struct ZeroStepTransition<T: Config> {
  normal_cycle_completion: NormalCycleCompletion,
  next_residence: NextResidence<T>,
  cycle_nonce: u64,
  behavior: Option<BehaviorPolicyCandidateOf<T>>,
}

#[derive(Clone, Copy)]
pub(crate) enum ServiceCutoff {
  Open,
  #[cfg(test)]
  Snapshotted,
}

impl ServiceCutoff {
  fn is_snapshotted(self) -> bool {
    #[cfg(test)]
    {
      return matches!(self, Self::Snapshotted);
    }
    #[cfg(not(test))]
    false
  }
}

/// Closed outcome of one canonical FIFO placement attempt. Queue capacity
/// exhaustion may preserve readiness through an exact later wakeup; monotonic
/// ticket/page namespace exhaustion and corruption are not retryable and fail
/// closed through the public error surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
  AlreadyLive,
  CapacityUnavailable,
  TicketExhausted,
  SchedulerIndexExhausted,
  WakeupCapacityExhausted,
  WakeupIndexExhausted,
  InvalidParkedBalanceActivation,
  CorruptedTopology,
}

impl From<DeadlineMutationError> for EnqueueOutcome {
  fn from(error: DeadlineMutationError) -> Self {
    match error {
      DeadlineMutationError::NamespaceExhausted => Self::SchedulerIndexExhausted,
      DeadlineMutationError::CapacityExceeded | DeadlineMutationError::PageFull => {
        Self::WakeupCapacityExhausted
      }
      _ => Self::CorruptedTopology,
    }
  }
}

/// Semantic result of admitting one trigger activation through the canonical
/// pending latch and FIFO/wakeup substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivationOutcome {
  Latched,
}

const MAX_RETRY_BACKOFF_BLOCKS: u32 = 8;

#[cfg(test)]
std::thread_local! {
  static FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
  static QUEUE_APPEND_COMMITS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttemptTransactionError {
  FeeCollection,
  StateHold,
  /// Preserve canonical placement causes across the enclosing whole-Step rollback.
  Deadline(DeadlineMutationError),
  ParkedBalance(ParkedBalanceTransitionError),
  Invariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActorControlTransitionError {
  Invariant,
  IndexExhausted,
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for ActorControlTransitionError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::Invariant
  }
}

#[derive(Clone, Copy)]
pub(crate) enum ActorWaitingAuthority {
  Trigger,
  Service,
}

/// A semantic rollback does not erase resource work. Control on a failed commit keeps its
/// admitted bound; a valid effect observation survives independently of storage rollback.
pub(crate) struct StepRollback {
  pub(crate) cause: AttemptTransactionError,
  pub(crate) actual_effect_weight: Option<Weight>,
}

pub(crate) struct StepCommitEvidence {
  pub(crate) actual_control_weight: Weight,
  pub(crate) actual_effect_weight: Weight,
  pub(crate) attempt: Option<ActorAttemptEvidence>,
}

pub(crate) struct ActorAttemptEvidence {
  pub(crate) normal_cycle_completion: Option<NormalCycleCompletion>,
  pub(crate) clean_cycle_step: CleanCycleStep,
  status: AttemptDisposition,
  cycle_nonce: u64,
  start_cursor: u32,
  run_cursor: Option<u32>,
  unsuccessful_attempts_at_cursor: Option<u32>,
  cumulative_outcomes: OutcomeTotals,
  step: Option<SimulationStepRecord>,
}

impl ActorAttemptEvidence {
  pub(crate) fn is_closed(&self) -> bool {
    matches!(self.status, AttemptDisposition::Closed(_))
  }
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for AttemptTransactionError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::Invariant
  }
}

/// One bounded actor-service pass over the canonical FIFO: consumed weight plus the starve signal
/// derived from the terminal block reason (spec 8.6.3).
pub(crate) struct CyclePass {
  pub(crate) consumed: Weight,
  pub(crate) effect_consumed: Weight,
  pub(crate) effect_reconciliation_uncertain: bool,
  /// Service, a completed bound or an accounting fault must justify a telemetry update.
  pub(crate) starvation_observed: bool,
  pub(crate) starved: bool,
}

impl CyclePass {
  pub(crate) fn reconciled_domains(&self) -> Option<(Weight, Weight)> {
    if self.effect_reconciliation_uncertain {
      return None;
    }
    self
      .consumed
      .checked_sub(&self.effect_consumed)
      .map(|control| (control, self.effect_consumed))
  }
}

impl<T: Config> Pallet<T> {
  pub fn next_queue_ticket() -> QueueTicket {
    ActorReadyTail::<T>::get()
  }

  pub fn queue_head() -> QueueTicket {
    ActorReadyHead::<T>::get()
  }

  pub fn queue_tail() -> QueueTicket {
    ActorReadyTail::<T>::get()
  }

  pub fn queue_occupancy() -> u32 {
    ActorReadyOccupancy::<T>::get()
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> CyclePass {
    Self::execute_cycle_to_cutoff(remaining_weight, ActorReadyTail::<T>::get())
  }

  pub(crate) fn execute_cycle_to_cutoff(
    remaining_weight: Weight,
    _cutoff: QueueTicket,
  ) -> CyclePass {
    Self::execute_cycle_to_cutoff_inner(remaining_weight, None)
  }

  pub(crate) fn execute_cycle_to_cutoff_with_resources(
    remaining_weight: Weight,
    _cutoff: QueueTicket,
    state: &mut BlockResourceState<BlockNumberFor<T>>,
    limits: BlockResourceLimits,
    effect_domain: BlockResourceDomain,
    control_maximum: Weight,
  ) -> CyclePass {
    Self::execute_cycle_to_cutoff_inner(
      remaining_weight,
      Some((state, limits, effect_domain, control_maximum)),
    )
  }

  fn execute_cycle_to_cutoff_inner(
    remaining_weight: Weight,
    mut resources: Option<(
      &mut BlockResourceState<BlockNumberFor<T>>,
      BlockResourceLimits,
      BlockResourceDomain,
      Weight,
    )>,
  ) -> CyclePass {
    if remaining_weight.is_zero() {
      return CyclePass {
        consumed: Weight::zero(),
        effect_consumed: Weight::zero(),
        effect_reconciliation_uncertain: false,
        starvation_observed: false,
        starved: false,
      };
    }
    let mut pass_control_reservation = match resources.as_mut() {
      Some((state, limits, _, control_maximum)) => {
        match state.reserve(*limits, BlockResourceDomain::ActorControl, *control_maximum) {
          Ok(reservation) => Some(reservation),
          Err(_) => {
            state.halt_optional_actor_work();
            return CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              starvation_observed: true,
              starved: true,
            };
          }
        }
      }
      None => None,
    };
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let now = frame_system::Pallet::<T>::block_number();
    let max_executions = T::MaxExecutionsPerBlock::get();
    let max_scanned = T::MaxQueueEntriesScannedPerBlock::get();
    let mut executed = 0u32;
    let mut scanned = 0u32;
    let mut effect_consumed = Weight::zero();
    let mut effect_reconciliation_uncertain = false;
    let mut starvation_observed = false;
    let mut starved = false;
    while executed < max_executions && scanned < max_scanned {
      let control_remaining = match resources.as_ref() {
        Some((_, _, _, maximum)) => match cycle_meter
          .consumed()
          .checked_sub(&effect_consumed)
          .and_then(|control| maximum.checked_sub(&control))
        {
          Some(remaining) => remaining,
          None => {
            effect_reconciliation_uncertain = true;
            starvation_observed = true;
            starved = executed == 0;
            break;
          }
        },
        None => Weight::zero(),
      };
      // The canonical encounter owns discovery as well as execution. No outer peek may read or
      // mutate round authority before that owner's component-wise admission.
      let result = match resources.as_mut() {
        Some((state, limits, domain, _)) => {
          let effect_before = state.usage().actor_effect_used();
          let result = Self::service_canonical_round_head_with_reserved_control(
            &mut cycle_meter,
            now,
            &mut **state,
            *limits,
            *domain,
            control_remaining,
          );
          // Semantic rejection can retain incurred effects; reconcile both result paths.
          match state
            .usage()
            .actor_effect_used()
            .checked_sub(&effect_before)
            .and_then(|effect| effect_consumed.checked_add(&effect))
          {
            Some(total) => effect_consumed = total,
            None => {
              effect_reconciliation_uncertain = true;
              state.halt_optional_actor_work();
            }
          }
          result
        }
        None => Self::service_canonical_round_head(&mut cycle_meter, now),
      };
      match result {
        Ok(ServiceRoundEncounter::Eligible(_)) => {
          scanned = scanned.saturating_add(1);
          executed = executed.saturating_add(1);
          starvation_observed = true;
        }
        Ok(ServiceRoundEncounter::NoWork(_)) => {
          scanned = scanned.saturating_add(1);
        }
        Ok(
          ServiceRoundEncounter::Empty
          | ServiceRoundEncounter::Closed
          | ServiceRoundEncounter::TerminallyClosed(_)
          | ServiceRoundEncounter::AlreadyAttempted(_),
        ) => {
          starvation_observed = true;
          break;
        }
        Ok(ServiceRoundEncounter::BreakerRefused(_))
        | Err(ServiceRoundError::DiscoveryUnavailable) => break,
        Err(_) => {
          starvation_observed = true;
          starved = executed == 0;
          break;
        }
      }
    }
    // Completing an admitted bounded scan is the existing healthy cap exit, even when every
    // visited member was Idle. A capacity refusal before that boundary remains unobserved.
    if scanned > 0 && scanned == max_scanned {
      starvation_observed = true;
    }
    let pass = CyclePass {
      consumed: cycle_meter.consumed(),
      effect_consumed,
      effect_reconciliation_uncertain,
      starvation_observed,
      starved,
    };
    if let (Some((state, _, _, control_maximum)), Some(reservation)) =
      (resources.as_mut(), pass_control_reservation.as_mut())
    {
      let actual_control = pass
        .reconciled_domains()
        .map(|(actual, _)| actual)
        .unwrap_or(*control_maximum);
      if state.settle(reservation, actual_control).is_err() {
        // Preserve the admitted maximum while releasing transition authority. This second
        // settlement is deterministic because the reservation still owns that exact maximum.
        let _ = state.settle(reservation, *control_maximum);
        state.halt_optional_actor_work();
      } else if pass.reconciled_domains().is_none() {
        state.halt_optional_actor_work();
      }
    }
    pass
  }

  pub(crate) fn charge_pipeline_opening(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<(), AttemptTransactionError> {
    if instance.cycle_state != CycleState::Idle
      || instance.actor_class.actor_type() != ActorType::User
    {
      return Ok(());
    }
    let breakdown =
      Self::collect_pipeline_fee(actor_id, ActorType::User, &instance.sovereign_account)
        .map_err(|_| AttemptTransactionError::FeeCollection)?;
    Self::deposit_event(Event::PipelineFeeCharged {
      actor_id,
      fee: breakdown.total_fee,
    });
    Ok(())
  }

  pub(crate) fn deposit_action_fee_receipt(
    actor_id: ActorId,
    cycle_nonce: u64,
    step_index: u32,
    actual_effect_weight: Weight,
    fee: T::Balance,
  ) {
    Self::deposit_event(Event::ActionFeeCharged {
      actor_id,
      cycle_nonce,
      step_index,
      actual_effect_weight,
      fee,
    });
  }

  fn prepare_cadenced_rearm_hot(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    mut hot: ActorHotStateOf<T>,
  ) -> Result<ActorHotStateOf<T>, AttemptTransactionError> {
    let Trigger::Cadenced { every_ticks } = &instance.trigger else {
      return Err(AttemptTransactionError::Invariant);
    };
    let anchor_tick = instance
      .temporal_anchor_tick
      .ok_or(AttemptTransactionError::Invariant)?;
    let now_tick =
      Self::current_scheduler_tick().map_err(|_| AttemptTransactionError::Invariant)?;
    let due_tick = next_cadence_due_tick(anchor_tick, *every_ticks, now_tick)
      .ok_or(AttemptTransactionError::Invariant)?;
    let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
      && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      && ActorProcesses::<T>::contains_key(actor_id);
    if canonical {
      // Opening follows a consumed occurrence commit, so no temporal Trigger member or pointer may
      // remain. Re-register the next cadence deadline in the canonical `TriggerDeadlineHandles`
      // carrier so the shared `service_due_deadline_frontiers` frontier observes it; the legacy
      // `ActorWaitingEntry` reference substrate is never read by canonical production `on_idle`.
      if hot.trigger_wakeup_pointer.is_some() {
        return Err(AttemptTransactionError::Invariant);
      }
      let actor = Self::load_actor_ref(actor_id).ok_or(AttemptTransactionError::Invariant)?;
      let handle = Self::plan_deadline_destination(actor, WakeupKey::Tick(due_tick))
        .map_err(AttemptTransactionError::Deadline)?;
      let pointer = TriggerWakeupPointer {
        tick: due_tick,
        page_id: handle.page,
        slot: u32::from(handle.slot),
      };
      let Some(ActorSemanticState::Active(current)) = ActorSemanticStates::<T>::get(actor_id)
      else {
        return Err(AttemptTransactionError::Invariant);
      };
      if current.generation != actor.generation || current.admission != *admission {
        return Err(AttemptTransactionError::Invariant);
      }
      let mut replacement = current;
      replacement.hot.trigger_wakeup_pointer = Some(pointer);
      ActorSemanticStates::<T>::insert(actor_id, ActorSemanticState::Active(replacement));
      Self::insert_trigger_deadline_member(handle).map_err(AttemptTransactionError::Deadline)?;
      hot.trigger_wakeup_pointer = Some(pointer);
      return Ok(hot);
    }
    if let Some(pointer) = hot.trigger_wakeup_pointer {
      Self::invalidate_wakeup_reference(
        actor_id,
        WakeupPointer {
          block: WakeupKey::Tick(pointer.tick),
          page_id: pointer.page_id,
          slot: pointer.slot,
        },
        admission.admission_identity,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?;
      hot.trigger_wakeup_pointer = None;
    }
    let (page_id, slot) = Self::schedule_fresh_wakeup_reference(
      actor_id,
      WakeupKey::Tick(due_tick),
      admission.admission_identity,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    Ok(Self::with_wakeup_pointer(
      hot,
      WakeupKey::Tick(due_tick),
      page_id,
      slot,
    ))
  }

  fn prepare_opening_rearm_hot(
    actor_id: ActorId,
    opening: &ActiveActorViewOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    mut hot: ActorHotStateOf<T>,
  ) -> Result<ActorHotStateOf<T>, AttemptTransactionError> {
    match &opening.trigger {
      Trigger::AtTime { .. } => {
        let TriggerRuntimeState::AtTime { consumed, .. } = &mut hot.trigger_runtime_state else {
          return Err(AttemptTransactionError::Invariant);
        };
        // Replacement may preserve a paid latch from a different Trigger family.
        *consumed = true;
        // A canonical replacement that armed the AtTime deadline while a different-family latch was
        // pending leaves that one-shot Trigger member behind. This Opening consumes the occurrence,
        // so the deadline must be released here; otherwise the shared deadline frontier observes a
        // consumed occurrence and its member can never be serviced. The ordinary AtTime path already
        // removed its member before the occurrence commit reached this point.
        let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
          && ActorProcesses::<T>::contains_key(actor_id);
        if canonical && hot.trigger_wakeup_pointer.is_some() {
          let actor = Self::load_actor_ref(actor_id).ok_or(AttemptTransactionError::Invariant)?;
          Self::remove_trigger_deadline_member(actor)
            .map_err(|_| AttemptTransactionError::Invariant)?;
          hot.trigger_wakeup_pointer = None;
          let Some(ActorSemanticState::Active(mut semantic)) =
            ActorSemanticStates::<T>::get(actor_id)
          else {
            return Err(AttemptTransactionError::Invariant);
          };
          semantic.hot = hot.clone();
          ActorSemanticStates::<T>::insert(actor_id, ActorSemanticState::Active(semantic));
          Self::reconcile_actor_state_hold_with_authority(actor_id)
            .map_err(|_| AttemptTransactionError::StateHold)?;
        }
        Ok(hot)
      }
      Trigger::Cadenced { .. } => {
        Self::prepare_cadenced_rearm_hot(actor_id, opening, admission, hot)
      }
      Trigger::Manual | Trigger::AddressEvent { .. } => Ok(hot),
    }
  }

  /// Reserve lifecycle work independently of the host's Step/placement model. Authored policies
  /// and potential namespace-bearing placement conservatively cover their reachable Steps.
  /// Nonterminal success releases this allowance; Close or rollback retains it.
  pub(crate) fn service_terminal_control_upper(
    state: &ActiveActorStateOf<T>,
    step: Option<&StepOf<T>>,
  ) -> Weight {
    let cycle_nonce = state.run_state.as_ref().map_or_else(
      || state.identity.cycle_nonce.saturating_add(1),
      |run| run.cycle_nonce,
    );
    let authored = state
      .contract
      .auto_close_at_cycle_nonce
      .is_some_and(|target| cycle_nonce >= target)
      || state.contract.close_when.is_some();
    let failed = step.is_some_and(|step| {
      Self::failure_limit_reached(state.hot.unsuccessful_attempt_streak.saturating_add(1))
        || step.on_error.retry_max_attempts().is_some_and(|maximum| {
          state.run_state.as_ref().map_or(1, |run| {
            run.unsuccessful_attempts_at_cursor.saturating_add(1)
          }) >= maximum
        })
    });
    // With zero/one-block cooldown the first retry stays in B+1 Service. Later backoff or a
    // longer cooldown can publish a Deadline. A ready frontier's window can only narrow it.
    // Financial Action recovery can also restore a detached cadence source.
    let later_retry = step.is_some_and(|step| {
      step.on_error.retry_max_attempts().is_some()
        && (state.contract.cooldown_blocks > One::one()
          || state
            .run_state
            .as_ref()
            .is_some_and(|run| run.unsuccessful_attempts_at_cursor > 0))
    });
    let placement = later_retry
      || (state.hot.cycle_state == CycleState::Idle
        || state.contract.on_unfunded == OnUnfunded::AwaitFunding)
        && matches!(state.contract.trigger, Trigger::Cadenced { .. })
      || step.is_some() && state.contract.parked_balance_activation.is_some();
    if authored || failed || placement {
      Self::close_control_weight_for_contract(&state.contract).unwrap_or(Weight::MAX)
    } else {
      Weight::zero()
    }
  }

  /// Resolve only a raw namespace cause after the attempt transaction has rolled back. The
  /// caller already admitted this independent cleanup allowance in the same frozen Control owner.
  pub(crate) fn finalize_service_namespace_rollback(
    actor: ActorRef,
    cause: AttemptTransactionError,
    cleanup_allowance: Weight,
  ) -> Result<ActorAttemptEvidence, AttemptTransactionError> {
    if !matches!(
      cause,
      AttemptTransactionError::Deadline(DeadlineMutationError::NamespaceExhausted)
        | AttemptTransactionError::ParkedBalance(ParkedBalanceTransitionError::Dependency(
          DependencyRegistrationError::DeadlineNamespaceExhausted
        ))
    ) {
      return Err(cause);
    }
    if !Self::close_dispatch_weight_upper().all_lte(cleanup_allowance) {
      return Err(AttemptTransactionError::Invariant);
    }
    // A current-Step reconstruction or failed successor is not full terminal authority.
    let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor.actor_id) else {
      return Err(AttemptTransactionError::Invariant);
    };
    if state.contract.close_when.is_some()
      && !Self::close_control_weight_for_contract(&state.contract)
        .is_some_and(|weight| weight.all_lte(cleanup_allowance))
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let run = state.run_state.clone();
    let cycle_nonce = run
      .as_ref()
      .map_or(state.identity.cycle_nonce, |run| run.cycle_nonce);
    let cursor = run.as_ref().map_or(0, |run| run.cursor);
    let outcomes = run
      .as_ref()
      .map_or_else(OutcomeTotals::default, |run| run.cumulative_outcomes);
    Self::remove_actor_publication_and_finalize(
      actor,
      state,
      run.as_ref(),
      CloseReason::SchedulerIndexExhausted,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    Ok(Self::step_simulation_evidence(
      cycle_nonce,
      cursor,
      AttemptDisposition::Closed(CloseReason::SchedulerIndexExhausted),
      outcomes,
      None,
      None,
    ))
  }

  fn load_zero_step_behavior_source(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
  ) -> Result<Option<LoadedBehaviorPolicyOf<T>>, AttemptTransactionError> {
    if !matches!(
      state.contract.trigger,
      Trigger::Manual
        | Trigger::AddressEvent { .. }
        | Trigger::AtTime { .. }
        | Trigger::Cadenced { .. }
    ) || !state.contract.steps.is_empty()
      || state.hot.cycle_state != CycleState::Idle
      || state.run_state.is_some()
      || ActorControlLocators::<T>::contains_key(actor_id)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    if state.contract.close_when.is_some() {
      let head =
        ActorContractHeads::<T>::get(actor_id).ok_or(AttemptTransactionError::Invariant)?;
      Self::load_step_behavior_policy(actor_id, &head.header)
        .ok_or(AttemptTransactionError::Invariant)
    } else {
      Ok(None)
    }
  }

  fn execute_zero_step_transition(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ZeroStepTransition<T>, Weight), AttemptTransactionError> {
    let behavior = Self::load_zero_step_behavior_source(actor_id, &state)?;
    let reserved_control_weight = Self::zero_step_resource_envelope(&state.contract)
      .ok_or(AttemptTransactionError::Invariant)?
      .control;
    if behavior.is_some() {
      Self::close_control_weight_for_contract(&state.contract)
        .ok_or(AttemptTransactionError::Invariant)?;
    }
    if let Some(source) = &behavior {
      let context = Self::behavior_control_weight_context(&source.policy)
        .ok_or(AttemptTransactionError::Invariant)?;
      if T::StepControlWeight::maximum_behavior_zero_step_control_weight(context)
        != reserved_control_weight.checked_sub(&Self::idle_window_placement_weight(&state.contract))
      {
        return Err(AttemptTransactionError::Invariant);
      }
    }
    Ok((
      Self::execute_zero_step_control(actor_id, state, admission, now, behavior)?,
      reserved_control_weight,
    ))
  }

  fn execute_zero_step_control(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    behavior: Option<LoadedBehaviorPolicyOf<T>>,
  ) -> Result<ZeroStepTransition<T>, AttemptTransactionError> {
    state.hot.queue_ticket = None;
    let opening = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    state.hot = Self::prepare_opening_rearm_hot(actor_id, &opening, admission, state.hot)?;
    Self::charge_pipeline_opening(actor_id, &opening)?;
    let cycle_nonce = state
      .identity
      .cycle_nonce
      .checked_add(1)
      .ok_or(AttemptTransactionError::Invariant)?;
    state.identity.cycle_nonce = cycle_nonce;
    state.hot.pending_signal = false;
    state.hot.last_cycle_block = Some(now);
    state.hot.unsuccessful_attempt_streak = 0;
    Self::deposit_event(Event::CycleStarted {
      actor_id,
      cycle_nonce,
    });
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Completed,
      outcomes: OutcomeTotals::default(),
    });
    let behavior = match behavior {
      Some(source) => Some(
        Self::reduce_step_behavior_policy(
          source,
          CycleState::Idle,
          CycleState::Idle,
          cycle_nonce,
          BehaviorObservation::default(),
          CleanCycleStep::Neutral,
          Some(NormalCycleCompletion::PlanEnd),
        )
        .ok_or(AttemptTransactionError::Invariant)?,
      ),
      None => None,
    };
    let policy_satisfied = behavior
      .as_ref()
      .is_some_and(|candidate| candidate.satisfied);
    let next_residence = if policy_satisfied {
      NextResidence::Close {
        state,
        reason: CloseReason::BehaviorPolicySatisfied,
      }
    } else if state
      .contract
      .auto_close_at_cycle_nonce
      .is_some_and(|target| cycle_nonce >= target)
    {
      NextResidence::Close {
        state,
        reason: CloseReason::AutoCloseNonceReached,
      }
    } else {
      NextResidence::Publish { state }
    };
    Ok(ZeroStepTransition {
      normal_cycle_completion: NormalCycleCompletion::PlanEnd,
      next_residence,
      cycle_nonce,
      behavior,
    })
  }

  /// Measures authenticated empty-cycle control and candidate publication, not resource pricing,
  /// Service placement, destruction or final Weight settlement. The caller owns the transaction
  /// and must finish or roll back the returned residence intent.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_zero_step_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Option<CloseReason>), AttemptTransactionError> {
    let behavior = Self::load_zero_step_behavior_source(actor_id, &state)?;
    let transition = Self::execute_zero_step_control(actor_id, state, admission, now, behavior)?;
    if let Some(candidate) = &transition.behavior {
      if !Self::commit_step_behavior_candidate(candidate) {
        return Err(AttemptTransactionError::Invariant);
      }
    }
    Ok(match transition.next_residence {
      NextResidence::Publish { state } => (state, None),
      NextResidence::Close { state, reason } => (state, Some(reason)),
    })
  }

  /// Read-only caller authentication, separate from zero-Step transition and resource pricing.
  pub(crate) fn authenticate_zero_step_service_source(
    actor: ActorRef,
    kind: ServiceResidenceKind,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<ActorSemanticRecordOf<T>, AttemptTransactionError> {
    let semantic = Self::load_service_actor_semantic_state(actor, kind)
      .map_err(|_| AttemptTransactionError::Invariant)?;
    if semantic.identity != state.identity
      || semantic.hot != state.hot
      || semantic.admission != *admission
      || Self::load_actor_contract(actor.actor_id).as_ref() != Some(&state.contract)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok(semantic)
  }

  /// Executes and commits one generation-bound zero-Step Service attempt without restoring legacy
  /// FIFO authority. Refusal rolls back semantic, lifecycle, process, and ring mutations together.
  pub(crate) fn execute_zero_step_on_service(
    actor: ActorRef,
    kind: ServiceResidenceKind,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepCommitEvidence, AttemptTransactionError> {
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let result = (|| {
        let semantic = Self::authenticate_zero_step_service_source(actor, kind, &state, admission)?;
        let recovery = if state.contract.on_unfunded == OnUnfunded::AwaitFunding {
          let receipt = Self::prepare_financial_service_recovery(actor, &semantic)?;
          if receipt.is_some_and(|receipt| {
            !matches!(
              receipt.unpaid_service,
              crate::UnpaidActorService::PipelineOpening { .. }
            )
          }) {
            return Err(AttemptTransactionError::Invariant);
          }
          receipt
        } else {
          None
        };
        let placement_reserve = Self::idle_window_placement_weight(&state.contract);
        let (transition, reserved_control_weight) =
          Self::execute_zero_step_transition(actor.actor_id, state, admission, now)?;
        let ZeroStepTransition {
          normal_cycle_completion,
          next_residence,
          cycle_nonce,
          behavior,
        } = transition;
        if let Some(candidate) = &behavior {
          if !Self::commit_step_behavior_candidate(candidate) {
            return Err(AttemptTransactionError::Invariant);
          }
        }
        let (status, placement) = match next_residence {
          NextResidence::Publish { state, .. } => {
            let placement = Self::publish_idle_cycle_completion(actor, kind, &state, now)?;
            (AttemptDisposition::Completed, placement)
          }
          NextResidence::Close { state, reason } => {
            // Commit semantic/lifecycle cleanup before unlinking the cursor-owning ring member.
            Self::finalize_actor_from_consumed_state(actor.actor_id, state, admission, reason)
              .map_err(|_| AttemptTransactionError::Invariant)?;
            Self::retire_service_member(actor).map_err(|_| AttemptTransactionError::Invariant)?;
            (
              AttemptDisposition::Closed(reason),
              StepControlPlacement::None,
            )
          }
        };
        let host_reserved_control = reserved_control_weight
          .checked_sub(&placement_reserve)
          .ok_or(AttemptTransactionError::Invariant)?;
        let actual_control_weight = match &behavior {
          Some(candidate) => Self::behavior_control_weight_context(&candidate.source.policy)
            .and_then(|context| {
              T::StepControlWeight::actual_behavior_zero_step_control_weight(
                context,
                host_reserved_control,
                matches!(status, AttemptDisposition::Closed(_)),
              )
            })
            .filter(|actual| actual.all_lte(host_reserved_control))
            .ok_or(AttemptTransactionError::Invariant)?,
          None => host_reserved_control,
        }
        .checked_add(&if placement == StepControlPlacement::Wakeup {
          placement_reserve
        } else {
          Weight::zero()
        })
        .filter(|actual| actual.all_lte(reserved_control_weight))
        .ok_or(AttemptTransactionError::Invariant)?;
        let mut evidence = Self::step_simulation_evidence(
          cycle_nonce,
          0,
          status,
          OutcomeTotals::default(),
          None,
          None,
        );
        evidence.normal_cycle_completion = Some(normal_cycle_completion);
        if let Some(receipt) = recovery {
          Self::deposit_event(Event::FinancialWaitRecovered { receipt });
        }
        Ok(StepCommitEvidence {
          actual_control_weight,
          actual_effect_weight: Weight::zero(),
          attempt: Some(evidence),
        })
      })();
      match result {
        Ok(evidence) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(evidence))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  /// Publishes completed Idle-cycle semantics and its passive destination. The caller owns
  /// the encompassing attempt transaction, including any subsequent Weight-settlement refusal.
  pub(crate) fn publish_idle_cycle_completion(
    actor: ActorRef,
    kind: ServiceResidenceKind,
    state: &ActiveActorStateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepControlPlacement, AttemptTransactionError> {
    Self::try_store_service_control_state(actor, kind, state.identity.clone(), state.hot.clone())
      .map_err(|_| AttemptTransactionError::Invariant)?;
    Self::place_idle_service_after_cycle(actor, state, now)
  }

  fn load_effectful_step_source(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<EffectfulStepSource<T>, AttemptTransactionError> {
    let header =
      ActorContractHeads::<T>::get(actor_id).ok_or(AttemptTransactionError::Invariant)?;
    if header.header.admission_identity != admission.admission_identity {
      return Err(AttemptTransactionError::Invariant);
    }
    let step_count = header.header.step_count;
    let behavior = Self::load_step_behavior_policy(actor_id, &header.header)
      .ok_or(AttemptTransactionError::Invariant)?;
    Ok(EffectfulStepSource {
      state,
      step_count,
      behavior,
    })
  }

  fn prepare_effectful_step_control(
    actor_id: ActorId,
    source: EffectfulStepSource<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    requires_detached_primary: bool,
  ) -> Result<PreparedEffectfulStepControl<T>, AttemptTransactionError> {
    let state = &source.state;
    let step = plan.loaded_step.step.clone();
    let direct_task_policy = matches!(
      step.on_error,
      StepErrorPolicy::ContinueNextStep
        | StepErrorPolicy::AbortCycle
        | StepErrorPolicy::RetryLater { .. }
    );
    if requires_detached_primary && ActorControlLocators::<T>::contains_key(actor_id)
      || plan.identity != state.identity
      || plan.hot != state.hot
      || plan.admission != *admission
      || !direct_task_policy
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let execution_instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let control_context =
      Self::execution_step_control_weight_context(&execution_instance, &plan.loaded_step)
        .ok_or(AttemptTransactionError::Invariant)?;
    Ok(PreparedEffectfulStepControl {
      source,
      plan,
      step,
      execution_instance,
      control_context,
    })
  }

  fn execute_effectful_step_transition(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    requires_detached_primary: bool,
    retained_effect_weight: &mut Option<Weight>,
  ) -> Result<EffectfulStepTransition<T>, AttemptTransactionError> {
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    // Production qualification remains before preparation and all semantic mutation.
    if source.behavior.is_some() {
      Self::close_control_weight_for_contract(&source.state.contract)
        .ok_or(AttemptTransactionError::Invariant)?;
    }
    let prepared = Self::prepare_effectful_step_control(
      actor_id,
      source,
      plan,
      admission,
      requires_detached_primary,
    )?;
    if let Some(source) = &prepared.source.behavior {
      let context = Self::behavior_control_weight_context(&source.policy)
        .ok_or(AttemptTransactionError::Invariant)?;
      T::StepControlWeight::maximum_behavior_control_weight(
        prepared.control_context,
        &prepared.step,
        context,
      )
      .filter(|maximum| maximum.all_lte(prepared.plan.loaded_step.resources.control))
      .ok_or(AttemptTransactionError::Invariant)?;
    }
    Self::execute_effectful_step_control(actor_id, prepared, admission, now, retained_effect_weight)
  }

  fn execute_effectful_step_control(
    actor_id: ActorId,
    prepared: PreparedEffectfulStepControl<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    retained_effect_weight: &mut Option<Weight>,
  ) -> Result<EffectfulStepTransition<T>, AttemptTransactionError> {
    let PreparedEffectfulStepControl {
      source: EffectfulStepSource {
        mut state,
        step_count,
        behavior,
      },
      mut plan,
      step,
      execution_instance,
      control_context,
    } = prepared;
    let reserved_control_weight = plan.loaded_step.resources.control;
    let reserved_effect_weight = plan.loaded_step.resources.effect;
    let retry_attempt_limit_reached = if let Some(max_attempts) = step.on_error.retry_max_attempts()
    {
      plan
        .run
        .as_ref()
        .map_or(0, |run| run.unsuccessful_attempts_at_cursor)
        .checked_add(1)
        .ok_or(AttemptTransactionError::Invariant)?
        >= max_attempts
    } else {
      false
    };
    if execution_instance.cycle_state == CycleState::Idle {
      plan.hot.queue_ticket = None;
      plan.hot =
        Self::prepare_opening_rearm_hot(actor_id, &execution_instance, admission, plan.hot)?;
    }
    Self::charge_pipeline_opening(actor_id, &execution_instance)?;
    let (mut plan, effect_execution, disposition, outcomes, eligible_at) =
      Self::execute_loaded_single_step_core(actor_id, &execution_instance, plan, now, step_count)?;
    let actual_effect_weight =
      T::TaskEffectWeight::actual_effect_weight(&step.task, effect_execution)
        .filter(|actual| actual.all_lte(reserved_effect_weight))
        .ok_or(AttemptTransactionError::Invariant)?;
    // Valid performed work survives every subsequent policy, placement or fee rollback.
    *retained_effect_weight = Some(actual_effect_weight);
    let behavior = if plan.financial_wait.is_some() {
      None
    } else {
      match behavior {
        Some(source) => Some(
          Self::reduce_step_behavior_policy(
            source,
            execution_instance.cycle_state,
            plan.hot.cycle_state,
            plan.ticket.cycle_nonce,
            plan.behavior_observation,
            plan.clean_cycle_step,
            plan.normal_cycle_completion,
          )
          .ok_or(AttemptTransactionError::Invariant)?,
        ),
        None => None,
      }
    };
    let placement_run = plan.run.take();
    let attempt = if plan.financial_wait.is_some() {
      None
    } else {
      let mut attempt = Self::step_simulation_evidence(
        plan.ticket.cycle_nonce,
        plan.loaded_step.cursor,
        disposition,
        outcomes,
        placement_run.as_ref(),
        Some(
          plan
            .last_step_outcome
            .take()
            .ok_or(AttemptTransactionError::Invariant)?,
        ),
      );
      attempt.normal_cycle_completion = plan.normal_cycle_completion;
      attempt.clean_cycle_step = plan.clean_cycle_step;
      Some(attempt)
    };
    let failure_close_reason = if disposition != AttemptDisposition::Failed {
      None
    } else if retry_attempt_limit_reached {
      Some(CloseReason::RetryAttemptsExhausted)
    } else if Self::failure_limit_reached(plan.hot.unsuccessful_attempt_streak) {
      Some(CloseReason::ConsecutiveFailures)
    } else {
      None
    };
    let nonce_close_reason = if !matches!(
      disposition,
      AttemptDisposition::Completed | AttemptDisposition::Failed
    ) {
      None
    } else {
      state
        .contract
        .auto_close_at_cycle_nonce
        .filter(|target_nonce| plan.identity.cycle_nonce >= *target_nonce)
        .map(|_| CloseReason::AutoCloseNonceReached)
    };
    let parked_balance_reserved = state.contract.parked_balance_activation.is_some();
    state.identity = plan.identity.clone();
    state.hot = plan.hot.clone();
    state.run_state = placement_run;
    let policy_close_reason = behavior
      .as_ref()
      .is_some_and(|candidate| candidate.satisfied)
      .then_some(CloseReason::BehaviorPolicySatisfied);
    let next_residence = match failure_close_reason
      .or(policy_close_reason)
      .or(nonce_close_reason)
    {
      Some(reason) => NextResidence::Close { state, reason },
      None => NextResidence::Publish { state },
    };
    let financial_wait = plan.financial_wait;
    Ok(EffectfulStepTransition {
      plan,
      execution_instance,
      step,
      control_context,
      reserved_control_weight,
      effect_execution,
      actual_effect_weight,
      disposition,
      attempt,
      financial_wait,
      next_residence,
      eligible_at,
      parked_balance_reserved,
      behavior,
    })
  }

  /// Intrinsic successful Transfer and policy reduction. The returned flag requires clean-Cycle
  /// authority publication before Close; otherwise the caller publishes the surviving state.
  /// Placement/destruction and Action-fee settlement stay atomic; no control price is supplied.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_transfer_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Option<CloseReason>, Weight, bool), AttemptTransactionError>
  {
    if !matches!(
      plan.loaded_step.step.task,
      super::types::Task::Transfer { .. }
    ) || plan.loaded_step.step.precondition.is_some()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
      || !transition
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.step.as_ref())
        .is_some_and(|step| matches!(step.outcome, StepOutcome::Executed))
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let (state, reason) = match transition.next_residence {
      NextResidence::Publish { state } => (state, None),
      NextResidence::Close {
        state,
        reason: CloseReason::BehaviorPolicySatisfied,
      } => (state, Some(CloseReason::BehaviorPolicySatisfied)),
      _ => return Err(AttemptTransactionError::Invariant),
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied != reason.is_some() || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((
      state,
      reason,
      transition.actual_effect_weight,
      candidate.source.policy.plan.requires_clean_cycle(),
    ))
  }

  /// Intrinsic real liquidity-bound failure followed by funded recovery; no policy prices.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_liquidity_retry_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Weight), AttemptTransactionError> {
    let recovering = state.hot.cycle_state == CycleState::Suspended;
    if !matches!(
      state.hot.cycle_state,
      CycleState::Running | CycleState::Suspended
    ) || plan.loaded_step.cursor != 1
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::AddLiquidity { .. }
      )
      || plan
        .loaded_step
        .step
        .on_error
        .retry_max_attempts()
        .is_none()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    let outcome = transition
      .attempt
      .as_ref()
      .and_then(|attempt| attempt.step.as_ref())
      .ok_or(AttemptTransactionError::Invariant)?;
    let valid = if recovering {
      transition.disposition == AttemptDisposition::Continued
        && matches!(outcome.outcome, StepOutcome::Executed)
    } else {
      transition.disposition == AttemptDisposition::Suspended
        && matches!(&outcome.outcome, StepOutcome::Failed(failure) if failure.retry == super::RetryClass::Temporary)
    };
    if !valid
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((state, transition.actual_effect_weight))
  }

  /// Real invoked liquidity failure under ContinueNextStep or AbortCycle, without Close.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_liquidity_failure_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    expected_failure: super::RetryClass,
  ) -> Result<(ActiveActorStateOf<T>, Weight), AttemptTransactionError> {
    let policy = plan.loaded_step.step.on_error;
    let aborting = !matches!(policy, StepErrorPolicy::ContinueNextStep);
    let allowed_policy = matches!(policy, StepErrorPolicy::ContinueNextStep)
      || match expected_failure {
        super::RetryClass::Temporary => matches!(policy, StepErrorPolicy::AbortCycle),
        super::RetryClass::Permanent => matches!(policy, StepErrorPolicy::RetryLater { .. }),
      };
    if state.hot.cycle_state != CycleState::Running
      || plan.loaded_step.cursor != 1
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::AddLiquidity { .. }
      )
      || !allowed_policy
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition != if aborting { AttemptDisposition::Failed } else { AttemptDisposition::Continued }
      || transition.plan.behavior_observation.action != Some(false)
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
      || !transition.attempt.as_ref().and_then(|attempt| attempt.step.as_ref()).is_some_and(|step|
        matches!(&step.outcome, StepOutcome::Failed(failure) if failure.retry == expected_failure))
    { return Err(AttemptTransactionError::Invariant); }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    if state.hot.cycle_state
      != if aborting {
        CycleState::Idle
      } else {
        CycleState::Running
      }
      || state.run_state.is_none() != aborting
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((state, transition.actual_effect_weight))
  }

  /// Genuine Task-input shortfall: advancing skip, repeated suspension or bounded Close.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_funding_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    expected: AttemptDisposition,
  ) -> Result<(ActiveActorStateOf<T>, Option<CloseReason>, bool), AttemptTransactionError> {
    let opening = expected == AttemptDisposition::Continued;
    if !matches!(
      expected,
      AttemptDisposition::Continued | AttemptDisposition::Suspended | AttemptDisposition::Failed
    ) || state.hot.cycle_state
      != if opening {
        CycleState::Idle
      } else {
        CycleState::Suspended
      }
      || plan.loaded_step.cursor != 0
      || plan.loaded_step.step.precondition.is_some()
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::Transfer { .. }
      )
      || if opening {
        !matches!(plan.loaded_step.step.on_error, StepErrorPolicy::AbortCycle)
      } else {
        plan
          .loaded_step
          .step
          .on_error
          .retry_max_attempts()
          .is_none()
      }
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition != expected
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::NotInvoked
      || transition.actual_effect_weight != Weight::zero()
      || retained != Some(Weight::zero())
      || transition.plan.behavior_observation.action.is_some()
      || !transition
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.step.as_ref())
        .is_some_and(|step| {
          matches!(
            step.outcome,
            StepOutcome::FundingUnavailable(crate::FundingUnavailableReason::TaskInput)
          )
        })
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let (state, reason) = match transition.next_residence {
      NextResidence::Publish { state } if expected != AttemptDisposition::Failed => (state, None),
      NextResidence::Close {
        state,
        reason: reason @ (CloseReason::RetryAttemptsExhausted | CloseReason::ConsecutiveFailures),
      } if expected == AttemptDisposition::Failed => (state, Some(reason)),
      _ => return Err(AttemptTransactionError::Invariant),
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((
      state,
      reason,
      candidate.source.policy.plan.requires_clean_cycle(),
    ))
  }

  /// Resolution skip or Task-input retry without invoking Transfer or charging an Action.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_opening_no_action_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    retry: bool,
  ) -> Result<ActiveActorStateOf<T>, AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Idle
      || plan.loaded_step.cursor != 0
      || plan.loaded_step.step.precondition.is_some()
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::Transfer { .. }
      )
      || if retry {
        plan
          .loaded_step
          .step
          .on_error
          .retry_max_attempts()
          .is_none()
      } else {
        !matches!(plan.loaded_step.step.on_error, StepErrorPolicy::AbortCycle)
      }
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition
      != if retry {
        AttemptDisposition::Suspended
      } else {
        AttemptDisposition::Continued
      }
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::NotInvoked
      || transition.actual_effect_weight != Weight::zero()
      || retained != Some(Weight::zero())
      || transition.plan.behavior_observation.action.is_some()
      || !transition
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.step.as_ref())
        .is_some_and(|step| {
          if retry {
            matches!(
              step.outcome,
              StepOutcome::FundingUnavailable(crate::FundingUnavailableReason::TaskInput)
            )
          } else {
            matches!(
              step.outcome,
              StepOutcome::Skipped(super::StepSkippedReason::ResolutionSkipped)
            )
          }
        })
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok(state)
  }

  /// Close-only User ServiceFee shortage after a committed native-spend prefix.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_running_service_fee_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    retry: bool,
  ) -> Result<ActiveActorStateOf<T>, AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Running
      || plan.loaded_step.cursor != 1
      || state.identity.actor_class.actor_type() != ActorType::User
      || state.contract.on_unfunded != OnUnfunded::Close
      || plan.loaded_step.step.precondition.is_some()
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::Transfer { .. }
      )
      || if retry {
        plan
          .loaded_step
          .step
          .on_error
          .retry_max_attempts()
          .is_none()
      } else {
        !matches!(
          plan.loaded_step.step.on_error,
          StepErrorPolicy::ContinueNextStep
        )
      }
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition
      != if retry {
        AttemptDisposition::Suspended
      } else {
        AttemptDisposition::Continued
      }
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::NotInvoked
      || transition.actual_effect_weight != Weight::zero()
      || retained != Some(Weight::zero())
      || transition.plan.behavior_observation.action.is_some()
      || !transition
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.step.as_ref())
        .is_some_and(|step| {
          matches!(
            step.outcome,
            StepOutcome::FundingUnavailable(crate::FundingUnavailableReason::ServiceFee)
          )
        })
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok(state)
  }

  /// Genuine pre-effect Unstake failure; caller publishes surviving Idle or continued Run.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_opening_pre_effect_failure_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<ActiveActorStateOf<T>, AttemptTransactionError> {
    let aborting = matches!(plan.loaded_step.step.on_error, StepErrorPolicy::AbortCycle);
    if state.hot.cycle_state != CycleState::Idle
      || plan.loaded_step.cursor != 0
      || plan.loaded_step.step.precondition.is_some()
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::Unstake { .. }
      )
      || !matches!(
        plan.loaded_step.step.on_error,
        StepErrorPolicy::AbortCycle | StepErrorPolicy::ContinueNextStep
      )
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition != if aborting { AttemptDisposition::Failed } else { AttemptDisposition::Continued }
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::NotInvoked
      || transition.actual_effect_weight != Weight::zero() || retained != Some(Weight::zero())
      || transition.plan.behavior_observation.action.is_some()
      || !transition.attempt.as_ref().and_then(|attempt| attempt.step.as_ref()).is_some_and(|step|
        matches!(&step.outcome, StepOutcome::Failed(failure) if failure.retry == super::RetryClass::Permanent))
    { return Err(AttemptTransactionError::Invariant); }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    if state.hot.cycle_state
      != if aborting {
        CycleState::Idle
      } else {
        CycleState::Running
      }
      || state.run_state.is_none() != aborting
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok(state)
  }

  /// Exhausted retry witness; caller joins consumed destruction and the Action suffix.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_liquidity_exhaustion_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Weight, bool), AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Suspended
      || plan.loaded_step.cursor != 1
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::AddLiquidity { .. }
      )
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition != AttemptDisposition::Failed || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
      || !transition.attempt.as_ref().and_then(|attempt| attempt.step.as_ref()).is_some_and(|step|
        matches!(&step.outcome, StepOutcome::Failed(failure) if failure.retry == super::RetryClass::Temporary))
    { return Err(AttemptTransactionError::Invariant); }
    let NextResidence::Close {
      state,
      reason: CloseReason::RetryAttemptsExhausted,
    } = transition.next_residence
    else {
      return Err(AttemptTransactionError::Invariant);
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((
      state,
      transition.actual_effect_weight,
      candidate.source.policy.plan.requires_clean_cycle(),
    ))
  }

  /// Late-retry witness; caller publishes the real Deadline destination.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_liquidity_late_retry_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Weight), AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Suspended
      || plan.loaded_step.cursor != 1
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::AddLiquidity { .. }
      )
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.disposition != AttemptDisposition::Suspended
      || transition.financial_wait.is_some()
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
      || !transition.attempt.as_ref().and_then(|attempt| attempt.step.as_ref()).is_some_and(|step|
        matches!(&step.outcome, StepOutcome::Failed(failure) if failure.retry == super::RetryClass::Temporary))
    { return Err(AttemptTransactionError::Invariant); }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    let next = now
      .checked_add(&One::one())
      .ok_or(AttemptTransactionError::Invariant)?;
    if state
      .run_state
      .as_ref()
      .is_none_or(|run| run.eligible_at <= next)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok((state, transition.actual_effect_weight))
  }

  /// Running successor whose real predicate rejects before AddLiquidity invocation.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_running_predicate_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<ActiveActorStateOf<T>, AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Running
      || plan.loaded_step.cursor == 0
      || plan.loaded_step.step.precondition.is_none()
      || !matches!(
        plan.loaded_step.step.task,
        super::types::Task::AddLiquidity { .. }
      )
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.financial_wait.is_some()
      || transition.disposition != AttemptDisposition::Continued
      || transition.effect_execution != super::TaskEffectExecution::NotInvoked
      || transition.actual_effect_weight != Weight::zero()
      || retained != Some(Weight::zero())
      || !transition
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.step.as_ref())
        .is_some_and(|step| {
          matches!(
            step.outcome,
            StepOutcome::Skipped(super::StepSkippedReason::PreconditionFalse)
          )
        })
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let NextResidence::Publish { state } = transition.next_residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    let candidate = transition
      .behavior
      .as_ref()
      .ok_or(AttemptTransactionError::Invariant)?;
    if candidate.satisfied || !Self::commit_step_behavior_candidate(candidate) {
      return Err(AttemptTransactionError::Invariant);
    }
    Ok(state)
  }

  /// Intrinsic StopCycle control for measurement, without supplying optional policy prices.
  /// The caller owns the transaction and must publish or destroy/retire the returned state.
  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_stop_cycle_control(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(ActiveActorStateOf<T>, Option<CloseReason>, Weight), AttemptTransactionError> {
    if !matches!(plan.loaded_step.step.task, super::types::Task::StopCycle) {
      return Err(AttemptTransactionError::Invariant);
    }
    let source = Self::load_effectful_step_source(actor_id, state, admission)?;
    let prepared = Self::prepare_effectful_step_control(actor_id, source, plan, admission, false)?;
    let mut retained = None;
    let transition =
      Self::execute_effectful_step_control(actor_id, prepared, admission, now, &mut retained)?;
    if transition.financial_wait.is_some()
      || transition.disposition != AttemptDisposition::Completed
      || transition.effect_execution != super::TaskEffectExecution::Invoked
      || retained != Some(transition.actual_effect_weight)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    if let Some(candidate) = &transition.behavior {
      if !Self::commit_step_behavior_candidate(candidate) {
        return Err(AttemptTransactionError::Invariant);
      }
    }
    let (state, reason) = match transition.next_residence {
      NextResidence::Publish { state } => (state, None),
      NextResidence::Close { state, reason } => (state, Some(reason)),
    };
    Ok((state, reason, transition.actual_effect_weight))
  }

  pub(crate) fn execute_effectful_step_on_service_with_deadline(
    actor: ActorRef,
    kind: ServiceResidenceKind,
    mut state: ActiveActorStateOf<T>,
    mut plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
    deadline: Option<WakeupKey<BlockNumberFor<T>>>,
  ) -> Result<StepCommitEvidence, StepRollback> {
    let mut retained_effect_weight = None;
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let result = (|| {
        let semantic = Self::load_service_actor_semantic_state(actor, kind)
          .map_err(|_| AttemptTransactionError::Invariant)?;
        if semantic.identity != state.identity
          || semantic.hot != state.hot
          || semantic.admission != *admission
          || Self::consider_service_head(now).map_err(|_| AttemptTransactionError::Invariant)?
            != ServiceRoundEncounter::Eligible(actor)
        {
          return Err(AttemptTransactionError::Invariant);
        }
        let financial_recovery = if state.contract.on_unfunded == OnUnfunded::AwaitFunding {
          let receipt = Self::prepare_financial_service_recovery(actor, &semantic)?;
          if receipt.is_some_and(|receipt| {
            !matches!(
              receipt.unpaid_service,
              crate::UnpaidActorService::PipelineOpening { .. }
                | crate::UnpaidActorService::RunningAction { .. }
                | crate::UnpaidActorService::SuspendedAction { .. }
            )
          }) {
            return Err(AttemptTransactionError::Invariant);
          }
          receipt
        } else {
          None
        };
        if financial_recovery.is_some_and(|receipt| {
          matches!(
            receipt.unpaid_service,
            crate::UnpaidActorService::RunningAction { .. }
              | crate::UnpaidActorService::SuspendedAction { .. }
          )
        }) && matches!(state.contract.trigger, Trigger::Cadenced { .. })
        {
          // Waiting detached the busy temporal source. Restore only the next current cadence
          // point in this paid boundary transaction; rollback restores the original wait.
          let instance = Self::derive_active_actor_view(
            state.identity.clone(),
            state.hot.clone(),
            state.contract.clone(),
          );
          state.hot =
            Self::prepare_cadenced_rearm_hot(actor.actor_id, &instance, admission, state.hot)?;
          plan.hot = state.hot.clone();
        }
        let transition = Self::execute_effectful_step_transition(
          actor.actor_id,
          state,
          plan,
          admission,
          now,
          false,
          &mut retained_effect_weight,
        )?;
        let EffectfulStepTransition {
          plan,
          execution_instance,
          step,
          control_context,
          reserved_control_weight,
          effect_execution,
          actual_effect_weight,
          disposition,
          mut attempt,
          financial_wait,
          next_residence,
          eligible_at,
          parked_balance_reserved,
          behavior,
        } = transition;
        if let Some(unpaid) = financial_wait {
          if attempt.is_some()
            || behavior.is_some()
            || actual_effect_weight != Weight::zero()
            || effect_execution != super::TaskEffectExecution::NotInvoked
            || financial_recovery.is_some_and(|receipt| {
              !matches!(
                receipt.unpaid_service,
                crate::UnpaidActorService::PipelineOpening { .. }
              )
            })
          {
            return Err(AttemptTransactionError::Invariant);
          }
          let NextResidence::Publish { state } = next_residence else {
            return Err(AttemptTransactionError::Invariant);
          };
          Self::try_store_service_control_state(actor, kind, state.identity, state.hot)
            .map_err(|_| AttemptTransactionError::Invariant)?;
          if let Some(receipt) = financial_recovery {
            Self::deposit_event(Event::FinancialWaitRecovered { receipt });
          }
          Self::publish_financial_wait(actor, unpaid).map_err(|cause| match cause {
            crate::FinancialWaitPublicationError::Deadline(error) => {
              AttemptTransactionError::Deadline(error)
            }
            crate::FinancialWaitPublicationError::StateHold => AttemptTransactionError::StateHold,
            _ => AttemptTransactionError::Invariant,
          })?;
          let entry = T::AdmissionCertificateAuthority::financial_wait_entry_weight(
            u32::try_from(crate::FinancialWaitEpisode::<T::Balance>::max_encoded_len())
              .map_err(|_| AttemptTransactionError::Invariant)?,
          )
          .ok_or(AttemptTransactionError::Invariant)?;
          return Ok(StepCommitEvidence {
            actual_control_weight: reserved_control_weight
              .checked_add(&entry)
              .ok_or(AttemptTransactionError::Invariant)?,
            actual_effect_weight: Weight::zero(),
            attempt: None,
          });
        }
        let mut attempt = attempt.take().ok_or(AttemptTransactionError::Invariant)?;
        if financial_recovery.is_some()
          && attempt.step.as_ref().is_some_and(|record| {
            matches!(
              record.outcome,
              StepOutcome::FundingUnavailable(FundingUnavailableReason::ServiceFee)
            )
          })
        {
          return Err(AttemptTransactionError::Invariant);
        }
        if let Some(candidate) = &behavior {
          if !Self::commit_step_behavior_candidate(candidate) {
            return Err(AttemptTransactionError::Invariant);
          }
        }
        let exhaustion_reason = match &next_residence {
          NextResidence::Close {
            reason:
              reason @ (CloseReason::RetryAttemptsExhausted | CloseReason::ConsecutiveFailures),
            ..
          } => Some(*reason),
          _ => None,
        };
        let later_retry_destination = if matches!(next_residence, NextResidence::Close { .. }) {
          None
        } else {
          match (disposition, eligible_at, deadline) {
            // Success discards a supplied retry key. A completed Idle successor independently
            // chooses passive residence; a continuing Run retains canonical Service.
            (
              AttemptDisposition::Completed | AttemptDisposition::Continued,
              Some(eligible_at),
              _,
            ) if now.checked_add(&One::one()) == Some(eligible_at) => None,
            (AttemptDisposition::Completed, None, _) => None,
            (AttemptDisposition::Failed, None, _)
              if matches!(step.on_error, StepErrorPolicy::RetryLater { .. })
                && exhaustion_reason.is_some() =>
            {
              None
            }
            // A terminal permanent failure or abort removes the Run, so a supplied retry key is
            // irrelevant and must not cause deadline work.
            (AttemptDisposition::Failed, None, _)
              if matches!(step.on_error, StepErrorPolicy::AbortCycle)
                || matches!(step.on_error, StepErrorPolicy::RetryLater { .. })
                  && attempt.step.as_ref().is_some_and(|record| {
                    matches!(
                      record.outcome,
                      StepOutcome::Failed(ref failure) if failure.retry == RetryClass::Permanent
                    )
                  }) =>
            {
              None
            }
            (AttemptDisposition::Suspended, Some(eligible_at), None)
              if now.checked_add(&One::one()) == Some(eligible_at) =>
            {
              None
            }
            (AttemptDisposition::Suspended, Some(eligible_at), Some(key))
              if key == WakeupKey::Block(eligible_at)
                && now.checked_add(&One::one()) != Some(eligible_at) =>
            {
              Some(
                Self::plan_deadline_destination(actor, key)
                  .map_err(AttemptTransactionError::Deadline)?,
              )
            }
            _ => return Err(AttemptTransactionError::Invariant),
          }
        };
        let control_outcome = match disposition {
          AttemptDisposition::Continued => StepControlOutcome::Continued,
          AttemptDisposition::Completed => StepControlOutcome::Completed,
          AttemptDisposition::Suspended => StepControlOutcome::Suspended,
          AttemptDisposition::Failed => StepControlOutcome::Failed,
          _ => return Err(AttemptTransactionError::Invariant),
        };
        let mut parked_balance_completion = false;
        let placement = match next_residence {
          NextResidence::Publish { state, .. } => {
            let idle = state.hot.cycle_state == CycleState::Idle
              && !state.hot.pending_signal
              && state.run_state.is_none();
            if idle {
              parked_balance_completion = state.contract.parked_balance_activation.is_some();
              Self::publish_idle_cycle_completion(actor, kind, &state, now)?
            } else {
              Self::try_store_service_control_state(
                actor,
                kind,
                state.identity.clone(),
                state.hot.clone(),
              )
              .map_err(|_| AttemptTransactionError::Invariant)?;
              if let Some(destination) = later_retry_destination {
                Self::transfer_service_member_to_deadline(actor, destination)
                  .map_err(AttemptTransactionError::Deadline)?;
                StepControlPlacement::Wakeup
              } else {
                Self::advance_service_head(actor, now)
                  .map_err(|_| AttemptTransactionError::Invariant)?;
                StepControlPlacement::Queue
              }
            }
          }
          NextResidence::Close { state, reason } => {
            if behavior
              .as_ref()
              .is_some_and(|candidate| candidate.source.policy.plan.requires_clean_cycle())
            {
              Self::try_store_service_control_state(
                actor,
                kind,
                state.identity.clone(),
                state.hot.clone(),
              )
              .map_err(|_| AttemptTransactionError::Invariant)?;
            }
            Self::finalize_actor_from_consumed_state(actor.actor_id, state, admission, reason)
              .map_err(|_| AttemptTransactionError::Invariant)?;
            Self::retire_service_member(actor).map_err(|_| AttemptTransactionError::Invariant)?;
            attempt.status = AttemptDisposition::Closed(reason);
            attempt.run_cursor = None;
            attempt.unsuccessful_attempts_at_cursor = None;
            StepControlPlacement::None
          }
        };
        let actual_fee = Self::maximum_current_action_fee(
          execution_instance.actor_class.actor_type(),
          &step,
          ActorStepResourceEnvelope {
            control: Weight::zero(),
            effect: actual_effect_weight,
          },
        )
        .map_err(|_| AttemptTransactionError::Invariant)?;
        let action_fee_collected = execution_instance.actor_class.actor_type() == ActorType::User
          && !actual_fee.total_fee.is_zero();
        let control_execution = StepControlExecution {
          phase: match execution_instance.cycle_state {
            CycleState::Idle => StepControlPhase::Opening,
            CycleState::Running => StepControlPhase::Running,
            CycleState::Suspended => StepControlPhase::Suspended,
          },
          outcome: control_outcome,
          placement,
          task_effect: effect_execution,
          action_fee_collected,
        };
        let host_reserved_control = if parked_balance_reserved {
          reserved_control_weight
            .checked_sub(&T::WeightInfo::complete_cycle_to_parked_balance())
            .ok_or(AttemptTransactionError::Invariant)?
        } else {
          reserved_control_weight
        };
        let host_actual_control = match behavior.as_ref() {
          Some(candidate) => Self::behavior_control_weight_context(&candidate.source.policy)
            .and_then(|behavior| {
              T::StepControlWeight::actual_behavior_control_weight(
                control_context,
                &step,
                behavior,
                host_reserved_control,
                control_execution,
              )
            }),
          None => T::StepControlWeight::actual_control_weight(
            control_context,
            &step,
            host_reserved_control,
            control_execution,
          ),
        };
        let actual_control_weight = host_actual_control
          .map(|actual| {
            if parked_balance_completion {
              actual.saturating_add(T::WeightInfo::complete_cycle_to_parked_balance())
            } else {
              actual
            }
          })
          .filter(|actual| actual.all_lte(reserved_control_weight))
          .ok_or(AttemptTransactionError::Invariant)?;
        if action_fee_collected {
          Self::collect_user_step_fee(&execution_instance.sovereign_account, actual_fee.total_fee)?;
        }
        if !matches!(step.task, super::types::Task::StopCycle)
          && matches!(effect_execution, super::TaskEffectExecution::Invoked)
        {
          Self::deposit_action_fee_receipt(
            actor.actor_id,
            plan.ticket.cycle_nonce,
            plan.loaded_step.cursor,
            actual_effect_weight,
            actual_fee.effect_fee,
          );
        }
        if let Some(receipt) = financial_recovery {
          Self::deposit_event(Event::FinancialWaitRecovered { receipt });
        }
        Ok(StepCommitEvidence {
          actual_control_weight,
          actual_effect_weight,
          attempt: Some(attempt),
        })
      })();
      match result {
        Ok(evidence) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(evidence))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|cause| StepRollback {
      cause,
      actual_effect_weight: retained_effect_weight,
    })
  }

  fn control_blank_control_chunk() -> Result<ActorControlChunkOf<T>, ActorControlTransitionError> {
    BoundedVec::try_from(alloc::vec![None; 32]).map_err(|_| ActorControlTransitionError::Invariant)
  }

  fn append_waiting_entry(
    key: WakeupKey<BlockNumberFor<T>>,
    entry: impl FnOnce(WakeupPageId, WakeupSlot) -> ActorWaitingEntry<ActorControlCellOf<T>>,
  ) -> Result<(WakeupPageId, WakeupSlot), ActorControlTransitionError> {
    let occupancy = ActorWaitingOccupancies::<T>::get(key);
    let tail = ActorWaitingTails::<T>::get(key);
    Self::validate_waiting_directory(key, occupancy)?;
    let next_tail = tail
      .checked_add(1)
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let next_occupancy = occupancy
      .checked_add(1)
      .filter(|value| *value <= T::MaxActiveActors::get())
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let page_id = tail / 32;
    let slot = (tail % 32) as u32;
    let mut page = if slot == 0 {
      if ActorWaitingFrameChunks::<T>::contains_key((key, page_id)) {
        return Err(ActorControlTransitionError::Invariant);
      }
      let previous_page = if occupancy == 0 {
        None
      } else {
        tail.checked_sub(1).map(|tail| tail / 32)
      };
      if let Some(previous_id) = previous_page {
        let mut previous = ActorWaitingFrameChunks::<T>::get((key, previous_id))
          .ok_or(ActorControlTransitionError::Invariant)?;
        if previous.next_page.is_some() || previous.live_entries == 0 {
          return Err(ActorControlTransitionError::Invariant);
        }
        previous.next_page = Some(page_id);
        ActorWaitingFrameChunks::<T>::insert((key, previous_id), previous);
      }
      ActorWaitingPageOf::<T> {
        entries: BoundedVec::try_from(alloc::vec![None; 32])
          .map_err(|_| ActorControlTransitionError::Invariant)?,
        live_entries: 0,
        scan_slot: 0,
        previous_page,
        next_page: None,
      }
    } else {
      ActorWaitingFrameChunks::<T>::get((key, page_id))
        .ok_or(ActorControlTransitionError::Invariant)?
    };
    if page.next_page.is_some() || page.entries.get(slot as usize).is_none_or(Option::is_some) {
      return Err(ActorControlTransitionError::Invariant);
    }
    page.entries[slot as usize] = Some(entry(page_id, slot));
    page.live_entries = page
      .live_entries
      .checked_add(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    ActorWaitingFrameChunks::<T>::insert((key, page_id), page);
    if occupancy == 0 {
      ActorWaitingHeads::<T>::insert(key, tail);
    }
    ActorWaitingTails::<T>::insert(key, next_tail);
    ActorWaitingOccupancies::<T>::insert(key, next_occupancy);
    if occupancy == 0 && !Self::wakeup_cursor_insert_inner(key) {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok((page_id, slot))
  }

  fn remove_waiting_entry(
    pointer: WakeupPointer<BlockNumberFor<T>>,
  ) -> Result<ActorWaitingEntry<ActorControlCellOf<T>>, ActorControlTransitionError> {
    let key = pointer.block;
    Self::validate_waiting_directory(key, ActorWaitingOccupancies::<T>::get(key))?;
    let mut page = ActorWaitingFrameChunks::<T>::get((key, pointer.page_id))
      .ok_or(ActorControlTransitionError::Invariant)?;
    // ActorWaitingChunkOf is type-bounded to 32 slots, including malformed short pages.
    if page.entries.iter(/* deos-bypass: bounded-iter */).filter(|entry| entry.is_some()).count()
      != page.live_entries as usize
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let entry = page
      .entries
      .get_mut(pointer.slot as usize)
      .and_then(Option::take)
      .ok_or(ActorControlTransitionError::Invariant)?;
    page.live_entries = page
      .live_entries
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    let occupancy = ActorWaitingOccupancies::<T>::get(key)
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    if (occupancy == 0)
      != (page.live_entries == 0 && page.previous_page.is_none() && page.next_page.is_none())
      || occupancy < page.live_entries
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    if page.live_entries > 0 {
      ActorWaitingFrameChunks::<T>::insert((key, pointer.page_id), page);
      ActorWaitingOccupancies::<T>::insert(key, occupancy);
      return Ok(entry);
    }
    if let Some(previous_id) = page.previous_page {
      let mut previous = ActorWaitingFrameChunks::<T>::get((key, previous_id))
        .ok_or(ActorControlTransitionError::Invariant)?;
      if previous.next_page != Some(pointer.page_id) {
        return Err(ActorControlTransitionError::Invariant);
      }
      previous.next_page = page.next_page;
      ActorWaitingFrameChunks::<T>::insert((key, previous_id), previous);
    }
    if let Some(next_id) = page.next_page {
      let mut next = ActorWaitingFrameChunks::<T>::get((key, next_id))
        .ok_or(ActorControlTransitionError::Invariant)?;
      if next.previous_page != Some(pointer.page_id) {
        return Err(ActorControlTransitionError::Invariant);
      }
      next.previous_page = page.previous_page;
      if page.previous_page.is_none() {
        ActorWaitingHeads::<T>::insert(
          key,
          next_id
            .checked_mul(32)
            .and_then(|start| start.checked_add(u64::from(next.scan_slot)))
            .ok_or(ActorControlTransitionError::IndexExhausted)?,
        );
      }
      ActorWaitingFrameChunks::<T>::insert((key, next_id), next);
    } else if let Some(previous_id) = page.previous_page {
      ActorWaitingTails::<T>::insert(
        key,
        previous_id
          .checked_add(1)
          .and_then(|next| next.checked_mul(32))
          .ok_or(ActorControlTransitionError::IndexExhausted)?,
      );
    }
    ActorWaitingFrameChunks::<T>::remove((key, pointer.page_id));
    if occupancy == 0 {
      if page.previous_page.is_some()
        || page.next_page.is_some()
        || !Self::control_wakeup_cursor_release(key)
      {
        return Err(ActorControlTransitionError::Invariant);
      }
      ActorWaitingHeads::<T>::remove(key);
      ActorWaitingTails::<T>::remove(key);
      ActorWaitingOccupancies::<T>::remove(key);
    } else {
      ActorWaitingOccupancies::<T>::insert(key, occupancy);
    }
    Ok(entry)
  }

  fn validate_waiting_directory(
    key: WakeupKey<BlockNumberFor<T>>,
    occupancy: u32,
  ) -> Result<(), ActorControlTransitionError> {
    // The draining owner advances head before consuming its last live slot, so
    // head == tail is a valid in-transaction intermediate, not a missing directory.
    let head_present = ActorWaitingHeads::<T>::contains_key(key);
    let tail_present = ActorWaitingTails::<T>::contains_key(key);
    if occupancy == 0 {
      if head_present || tail_present {
        return Err(ActorControlTransitionError::Invariant);
      }
    } else if !head_present
      || !tail_present
      || ActorWaitingHeads::<T>::get(key) > ActorWaitingTails::<T>::get(key)
      || ActorWaitingCursorIndices::<T>::get(key).is_none()
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok(())
  }

  pub(crate) fn control_append_waiting(
    mut cell: ActorControlCellOf<T>,
    key: WakeupKey<BlockNumberFor<T>>,
    authority: ActorWaitingAuthority,
  ) -> Result<ActorControlLocation<BlockNumberFor<T>>, ActorControlTransitionError> {
    let actor_id = cell.actor_id;
    if ActorControlLocators::<T>::contains_key(actor_id)
      || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let pointer = match authority {
      ActorWaitingAuthority::Trigger => {
        if !matches!(key, WakeupKey::Tick(_))
          || cell.hot.cycle_state != CycleState::Idle
          || cell.hot.pending_signal
          || cell.eligible_at.is_some()
          || cell.hot.wakeup_pointer.is_some()
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        cell
          .hot
          .trigger_wakeup_pointer
          .map(|pointer| WakeupPointer {
            block: WakeupKey::Tick(pointer.tick),
            page_id: pointer.page_id,
            slot: pointer.slot,
          })
      }
      ActorWaitingAuthority::Service => {
        let terminal_idle = cell.hot.cycle_state == CycleState::Idle
          && !cell.hot.pending_signal
          && cell
            .hot
            .terminal_at
            .is_some_and(|terminal| cell.eligible_at.is_some_and(|at| at >= terminal));
        if !matches!(key, WakeupKey::Block(_))
          || cell.eligible_at.is_none()
          || !(matches!(
            cell.hot.cycle_state,
            CycleState::Running | CycleState::Suspended
          ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
            || terminal_idle)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        cell.hot.wakeup_pointer
      }
    };
    if let Some(pointer) = pointer {
      if pointer.block != key {
        return Err(ActorControlTransitionError::Invariant);
      }
      if let Some(mut page) = ActorWaitingFrameChunks::<T>::get((key, pointer.page_id)) {
        if let Some(Some(ActorWaitingEntry::Reference(reference))) =
          page.entries.get(pointer.slot as usize)
        {
          if reference.actor_id != actor_id
            || reference.admission_identity != cell.admission.admission_identity
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          let slot =
            u8::try_from(pointer.slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          page.entries[pointer.slot as usize] = Some(ActorWaitingEntry::Primary(cell));
          let location = ActorControlLocation::Waiting {
            key,
            page: pointer.page_id,
            slot,
          };
          ActorWaitingFrameChunks::<T>::insert((key, pointer.page_id), page);
          ActorControlLocators::<T>::insert(actor_id, location);
          Self::replace_active_semantics_from_primary(actor_id, location)?;
          return Ok(location);
        }
        if page
          .entries
          .get(pointer.slot as usize)
          .is_none_or(Option::is_some)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
      }
    }
    let (page, slot) = Self::append_waiting_entry(key, |page_id, slot| {
      match key {
        WakeupKey::Block(_) => {
          cell.hot.wakeup_pointer = Some(WakeupPointer {
            block: key,
            page_id,
            slot,
          })
        }
        WakeupKey::Tick(tick) => {
          cell.hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
            tick,
            page_id,
            slot,
          })
        }
      }
      ActorWaitingEntry::Primary(cell)
    })?;
    let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
    let location = ActorControlLocation::Waiting { key, page, slot };
    ActorControlLocators::<T>::insert(actor_id, location);
    Self::replace_active_semantics_from_primary(actor_id, location)?;
    Ok(location)
  }

  pub(crate) fn control_append_ready(
    cell: ActorControlCellOf<T>,
  ) -> Result<(ActorId, QueueTicket), ActorControlTransitionError> {
    const CHUNK_SIZE: QueueTicket = 32;
    if cell.eligible_at.is_none() {
      return Err(ActorControlTransitionError::Invariant);
    }
    let actor_id = cell.actor_id;
    let tail = ActorReadyTail::<T>::get();
    let occupancy = ActorReadyOccupancy::<T>::get();
    if tail
      .checked_sub(ActorReadyHead::<T>::get())
      .is_none_or(|span| span >= u64::from(T::MaxQueueLength::get()))
      || ActorControlLocators::<T>::contains_key(actor_id)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let next_tail = tail
      .checked_add(1)
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let next_occupancy = occupancy
      .checked_add(1)
      .filter(|value| *value <= T::MaxActiveActors::get())
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    if Self::project_control_cell(&cell, ActorControlLocation::Ready { ticket: tail }).is_none() {
      return Err(ActorControlTransitionError::Invariant);
    }
    let page = tail / CHUNK_SIZE;
    let slot = (tail % CHUNK_SIZE) as usize;
    let mut chunk = match ActorReadyFrameChunks::<T>::get(page) {
      Some(chunk) => chunk,
      None => Self::control_blank_control_chunk()?,
    };
    let Some(target) = chunk.get_mut(slot) else {
      return Err(ActorControlTransitionError::Invariant);
    };
    if target.replace(cell).is_some() {
      return Err(ActorControlTransitionError::Invariant);
    }
    ActorReadyFrameChunks::<T>::insert(page, chunk);
    ActorReadyTail::<T>::put(next_tail);
    ActorReadyOccupancy::<T>::put(next_occupancy);
    let location = ActorControlLocation::Ready { ticket: tail };
    ActorControlLocators::<T>::insert(actor_id, location);
    Self::replace_active_semantics_from_primary(actor_id, location)?;
    Ok((actor_id, tail))
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_stage_unsignaled_temporal(
    actor_id: ActorId,
    due_tick: SchedulerTick,
  ) -> Result<ActorControlLocation<BlockNumberFor<T>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        if ActorControlLocators::<T>::get(actor_id) != Some(ActorControlLocation::Unsignaled) {
          return Err(ActorControlTransitionError::Invariant);
        }
        let cell = ActorUnsignaledControlCells::<T>::get(actor_id)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if Self::project_control_cell(&cell, ActorControlLocation::Unsignaled).is_none() {
          return Err(ActorControlTransitionError::Invariant);
        }
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| ActorControlTransitionError::Invariant)?;
        let location = Self::control_append_waiting(
          cell,
          WakeupKey::Tick(due_tick),
          ActorWaitingAuthority::Trigger,
        )?;
        Ok(location)
      };
      match transition() {
        Ok(location) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(location))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_latch_temporal_waiting_page(
    due_tick: SchedulerTick,
    page: u64,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<Vec<ActorId>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        const CHUNK_SIZE: u64 = 32;
        let source_key = WakeupKey::Tick(due_tick);
        if due_tick > now_tick
          || Self::wakeup_cursor_peek_key(WakeupClock::Tick) != Some(source_key)
          || ActorWaitingHeads::<T>::get(source_key) / CHUNK_SIZE != page
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let mut chunk = ActorWaitingFrameChunks::<T>::get((source_key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let eligible_at = now
          .checked_add(&One::one())
          .ok_or(ActorControlTransitionError::IndexExhausted)?;
        let destination_key = WakeupKey::Block(eligible_at);
        let mut moved = Vec::new();
        for (slot, maybe_cell) in chunk
          .entries
          .iter_mut(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          let Some(entry) = maybe_cell.take() else {
            continue;
          };
          let mut cell = entry
            .into_primary()
            .ok_or(ActorControlTransitionError::Invariant)?;
          let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          let location = ActorControlLocation::Waiting {
            key: source_key,
            page,
            slot,
          };
          if Self::project_control_cell(&cell, location).is_none()
            || cell.hot.wakeup_pointer.is_some()
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          cell.hot.trigger_wakeup_pointer = None;
          cell.hot.pending_signal = true;
          cell.eligible_at = Some(eligible_at);
          let actor_id = cell.actor_id;
          Self::remove_primary_control_cell_inner(actor_id)?;
          Self::control_append_waiting(cell, destination_key, ActorWaitingAuthority::Service)?;
          moved.push(actor_id);
        }
        Ok(moved)
      };
      match transition() {
        Ok(moved) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(moved)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_promote_due_waiting_page(
    eligible_at: BlockNumberFor<T>,
    page: u64,
    now: BlockNumberFor<T>,
  ) -> Result<Vec<(ActorId, QueueTicket)>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        const CHUNK_SIZE: u64 = 32;
        if eligible_at > now {
          return Err(ActorControlTransitionError::Invariant);
        }
        let source_key = WakeupKey::Block(eligible_at);
        if Self::wakeup_cursor_peek_key(WakeupClock::Block) != Some(source_key)
          || ActorWaitingHeads::<T>::get(source_key) / CHUNK_SIZE != page
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let mut chunk = ActorWaitingFrameChunks::<T>::get((source_key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let mut moved = Vec::new();
        for (slot, maybe_cell) in chunk
          .entries
          .iter_mut(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          let Some(entry) = maybe_cell.take() else {
            continue;
          };
          let mut cell = entry
            .into_primary()
            .ok_or(ActorControlTransitionError::Invariant)?;
          let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          let location = ActorControlLocation::Waiting {
            key: source_key,
            page,
            slot,
          };
          if Self::project_control_cell(&cell, location).is_none()
            || cell.eligible_at.is_none_or(|value| value > now)
            || cell.hot.wakeup_pointer.is_none()
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          cell.hot.wakeup_pointer = None;
          Self::remove_primary_control_cell_inner(cell.actor_id)?;
          moved.push(Self::control_append_ready(cell)?);
        }
        Ok(moved)
      };
      match transition() {
        Ok(moved) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(moved)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn step_simulation_evidence(
    cycle_nonce: u64,
    start_cursor: u32,
    status: AttemptDisposition,
    cumulative_outcomes: OutcomeTotals,
    run: Option<&ActorRunStateOf<T>>,
    outcome: Option<StepOutcome>,
  ) -> ActorAttemptEvidence {
    ActorAttemptEvidence {
      normal_cycle_completion: None,
      clean_cycle_step: CleanCycleStep::Neutral,
      status,
      cycle_nonce,
      start_cursor,
      run_cursor: run.map(|run| run.cursor),
      unsuccessful_attempts_at_cursor: run.map(|run| run.unsuccessful_attempts_at_cursor),
      cumulative_outcomes,
      step: outcome.map(|outcome| SimulationStepRecord {
        step_index: start_cursor,
        outcome,
      }),
    }
  }

  /// Called only within the simulation's outer rollback. Selects one real primary without
  /// advancing the FIFO Head or servicing any other Actor.
  pub(crate) fn simulate_actor_service(
    actor_id: ActorId,
    budget: SimulationBudget,
    terminal_reason: Option<CloseReason>,
    terminal_source: ActiveActorStateOf<T>,
  ) -> Result<SimulationResult, SimulationError> {
    let limits = budget
      .checked_limits()
      .map_err(|_| SimulationError::InvalidBudget)?;
    let now = frame_system::Pallet::<T>::block_number();
    let mut resources = BlockResourceState::new(now);
    resources
      .begin_prepass(
        limits
          .into_budget()
          .map_err(|_| SimulationError::InvalidBudget)?,
      )
      .map_err(|_| SimulationError::InvalidBudget)?;
    resources
      .open_external_phase()
      .map_err(|_| SimulationError::InvalidBudget)?;
    resources
      .begin_drain()
      .map_err(|_| SimulationError::InvalidBudget)?;
    let mut cycle_meter = WeightMeter::with_limit(
      budget
        .actor_control
        .checked_add(&budget.shared_economic)
        .ok_or(SimulationError::InvalidBudget)?,
    );
    // Fresh-genesis runtime state has one canonical Service/Deadline owner. A retained legacy
    // locator is malformed compatibility state, not an alternate simulation executor.
    if ActorControlLocators::<T>::contains_key(actor_id) {
      return Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant,
      ));
    }

    let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<T>::get(actor_id) else {
      return Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant,
      ));
    };
    let actor = ActorRef {
      actor_id,
      generation: semantic.generation,
    };
    let (_, process) = Self::load_canonical_actor_semantic_state(actor)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
    // A funded hint only offers recovery service; the original unpaid boundary is still waiting.
    if process.status == ProcessStatus::AwaitingFunding && terminal_reason.is_none() {
      return Err(SimulationError::NotReady);
    }
    match process.residence {
      Some(ProcessResidence::Deadline {
        key: WakeupKey::Block(due),
        ..
      }) if due <= now => {
        Self::return_due_deadline_member_to_service(actor, ServiceResidenceKind::Live, now)
          .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
      }
      Some(ProcessResidence::Service(_)) => {}
      _ => {
        // Independent terminals remain actionable without eligible Service, including while a
        // financial Deadline is still future. Admit the selector and complete Close before the
        // shared atomic destructor; ordinary waiting has no terminal and remains NotReady.
        let Some(reason) = terminal_reason else {
          return Err(SimulationError::NotReady);
        };
        let selector_envelope = T::WeightInfo::service_round_begin_populated()
          .saturating_add(T::WeightInfo::service_round_probe_eligible());
        let cleanup = Self::close_control_weight_for_contract(&terminal_source.contract)
          .ok_or(SimulationError::ResourceDeferred)?;
        let close_envelope = selector_envelope
          .checked_add(&cleanup)
          .ok_or(SimulationError::ResourceDeferred)?;
        let total = close_envelope;
        let mut reservation = resources
          .reserve(limits, BlockResourceDomain::ActorControl, close_envelope)
          .map_err(|_| SimulationError::ResourceDeferred)?;
        if !cycle_meter.can_consume(total) {
          return Err(SimulationError::ResourceDeferred);
        }
        cycle_meter.consume(total);
        let run = terminal_source.run_state.clone();
        let (cycle_nonce, start_cursor, cumulative_outcomes) = run.as_ref().map_or(
          (semantic.identity.cycle_nonce, 0, OutcomeTotals::default()),
          |run| (run.cycle_nonce, run.cursor, run.cumulative_outcomes),
        );
        Self::remove_actor_publication_and_finalize(actor, terminal_source, run.as_ref(), reason)
          .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
        resources
          .settle(&mut reservation, close_envelope)
          .map_err(|_| SimulationError::ResourceDeferred)?;
        return Ok(Self::simulation_attempt_result(
          Self::step_simulation_evidence(
            cycle_nonce,
            start_cursor,
            AttemptDisposition::Closed(reason),
            cumulative_outcomes,
            None,
            None,
          ),
        ));
      }
    }
    Self::begin_service_round(now)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
    if Self::consider_service_head(now)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?
      != ServiceRoundEncounter::Eligible(actor)
    {
      return Err(SimulationError::NotReady);
    }
    let (encounter, attempt) = Self::service_canonical_round_head_inner(
      &mut cycle_meter,
      now,
      Some((&mut resources, limits)),
      BlockResourceDomain::ActorDrainEffect,
      None,
    )
    .map_err(|error| match error {
      ServiceRoundError::DiscoveryUnavailable
      | ServiceRoundError::InsufficientWeight
      | ServiceRoundError::ResourceUnavailable => SimulationError::ResourceDeferred,
      ServiceRoundError::FeeCollection => SimulationError::FeeCollectionFailed,
      _ => SimulationError::Classification(ActorClassificationError::ActorInvariant),
    })?;
    return match encounter {
      ServiceRoundEncounter::Eligible(_) => attempt
        .map(Self::simulation_attempt_result)
        .ok_or(SimulationError::NotReady),
      ServiceRoundEncounter::TerminallyClosed(reason) => Ok(Self::simulation_attempt_result(
        Self::step_simulation_evidence(
          semantic.identity.cycle_nonce,
          0,
          AttemptDisposition::Closed(reason),
          OutcomeTotals::default(),
          None,
          None,
        ),
      )),
      _ => Err(SimulationError::NotReady),
    };
  }

  fn simulation_attempt_result(attempt: ActorAttemptEvidence) -> SimulationResult {
    SimulationResult {
      status: attempt.status,
      cycle_nonce: attempt.cycle_nonce,
      start_cursor: attempt.start_cursor,
      run_cursor: attempt.run_cursor,
      unsuccessful_attempts_at_cursor: attempt.unsuccessful_attempts_at_cursor,
      cumulative_outcomes: attempt.cumulative_outcomes,
      steps: BoundedVec::truncate_from(attempt.step.into_iter().collect()),
    }
  }

  #[cfg(test)]
  pub(crate) fn enqueue(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    match Self::try_paged_enqueue(actor_id) {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => {
        // Queue saturation preserves readiness through an exact next-block wakeup
        // (spec 8.1.4). A failure to place that wakeup must fail closed rather than
        // silently leave the actor with neither a live ticket nor a wakeup.
        let next_block = frame_system::Pallet::<T>::block_number()
          .checked_add(&One::one())
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        Self::defer_retained_wakeup(actor_id, next_block)
      }
      Err(other) => Err(other),
    }
  }

  fn queue_topology_preflight(_mutation: QueueMutation) -> Result<QueueTopology, EnqueueOutcome> {
    let head = ActorReadyHead::<T>::get();
    let tail = ActorReadyTail::<T>::get();
    let occupancy = ActorReadyOccupancy::<T>::get();
    let span = tail
      .checked_sub(head)
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    if span > u64::from(T::MaxQueueLength::get())
      || u64::from(occupancy) > span
      || occupancy > T::MaxActiveActors::get()
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    if head < tail {
      let chunk =
        ActorReadyFrameChunks::<T>::get(head / 32).ok_or(EnqueueOutcome::CorruptedTopology)?;
      // ActorControlChunkOf has at most 32 slots; inspect only the consumed head prefix.
      if chunk.len() != 32
        || chunk.iter(/* deos-bypass: bounded-iter */).take((head % 32) as usize).any(Option::is_some)
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    }
    if let Some(chunk) = ActorReadyFrameChunks::<T>::get(tail / 32) {
      // ActorControlChunkOf has at most 32 slots; inspect only the unused tail suffix.
      if chunk.len() != 32
        || chunk.iter(/* deos-bypass: bounded-iter */).skip((tail % 32) as usize).any(Option::is_some)
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    } else if head < tail && !tail.is_multiple_of(32) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(QueueTopology {
      head,
      tail,
      occupancy,
    })
  }

  pub fn combined_queue_occupancy() -> u64 {
    u64::from(ActorReadyOccupancy::<T>::get())
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  /// Appends one actor to the canonical FIFO using the global ticket allocator.
  pub fn paged_enqueue(actor_id: ActorId) -> bool {
    matches!(
      Self::try_paged_enqueue(actor_id),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  #[cfg(test)]
  pub(crate) fn preflight_paged_enqueue_cohort_with_authority(
    actors: Vec<(ActorId, ActorHotStateOf<T>)>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    if actors.is_empty() || actors.len() > T::MaxQueueLength::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    let mut plan = Self::new_queue_append_plan()?;
    for (actor_id, hot) in actors.into_iter(/* deos-bypass: bounded-iter */) {
      let (state, admission, loaded_step) =
        Self::load_frame_actor_service_state(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
      let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
      let resources = if state.contract.steps.is_empty() {
        Self::zero_step_resource_envelope(&state.contract)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      } else {
        loaded_step
          .filter(|loaded| loaded.cursor == cursor)
          .map(|loaded| loaded.resources)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      };
      Self::reserve_following_paged_enqueue_with_authority(
        &mut plan,
        actor_id,
        hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        resources,
      )?;
    }
    Ok(plan)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn preflight_paged_enqueue_actor_state(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    loaded_step: Option<&LoadedActorStepOf<T>>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
    let resources = if state.contract.steps.is_empty() {
      Self::zero_step_resource_envelope(&state.contract).ok_or(EnqueueOutcome::CorruptedTopology)?
    } else {
      loaded_step
        .filter(|loaded| loaded.cursor == cursor)
        .map(|loaded| loaded.resources)
        .ok_or(EnqueueOutcome::CorruptedTopology)?
    };
    Self::preflight_paged_enqueue_authority(
      actor_id,
      state.hot.clone(),
      &state.identity,
      state.run_state.as_ref(),
      admission,
      resources,
    )
  }

  fn new_queue_append_plan() -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let topology = Self::queue_topology_preflight(QueueMutation::Enqueue)?;
    Ok(QueueAppendPlan {
      publications: Vec::new(),
      next_tail: topology.tail,
      next_occupancy: topology.occupancy,
    })
  }

  pub(crate) fn preflight_paged_enqueue_authority(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let mut plan = Self::new_queue_append_plan()?;
    Self::reserve_following_paged_enqueue_with_authority(
      &mut plan, actor_id, hot, identity, run_state, admission, resources,
    )?;
    Ok(plan)
  }

  fn preflight_ready_publication_capacity(plan: &QueueAppendPlan<T>) -> Result<(), EnqueueOutcome> {
    if plan.next_tail.saturating_sub(ActorReadyHead::<T>::get())
      >= u64::from(T::MaxQueueLength::get())
      || plan.next_occupancy >= T::MaxActiveActors::get()
    {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    plan
      .next_tail
      .checked_add(1)
      .ok_or(EnqueueOutcome::TicketExhausted)?;
    Ok(())
  }

  fn reserve_ready_publication(
    plan: &mut QueueAppendPlan<T>,
    cell: ActorControlCellOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let ticket = plan.next_tail;
    let next_tail = ticket
      .checked_add(1)
      .ok_or(EnqueueOutcome::TicketExhausted)?;
    let next_occupancy = plan
      .next_occupancy
      .checked_add(1)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    plan
      .publications
      .push(PreparedReadyPublication { ticket, cell });
    plan.next_tail = next_tail;
    plan.next_occupancy = next_occupancy;
    Ok(())
  }

  fn reserve_following_paged_enqueue_with_authority(
    plan: &mut QueueAppendPlan<T>,
    actor_id: ActorId,
    mut hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if hot.queue_ticket.is_some()
      || plan.publications.iter(/* deos-bypass: bounded-iter */)
        .any(|publication| publication.cell.actor_id == actor_id)
    {
      return Err(EnqueueOutcome::AlreadyLive);
    }
    Self::preflight_ready_publication_capacity(plan)?;
    let ticket = plan.next_tail;
    hot.queue_ticket = Some(ticket);
    let now = frame_system::Pallet::<T>::block_number();
    let eligible_at = match run_state {
      Some(run) => run.eligible_at,
      None if hot.last_cycle_block == Some(now) => now
        .checked_add(&One::one())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?,
      None => now,
    };
    let step_ticket = Self::build_actor_step_ticket(
      actor_id,
      ticket,
      eligible_at,
      identity,
      &hot,
      run_state,
      admission,
    )
    .ok_or(EnqueueOutcome::CorruptedTopology)?;
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
      cursor: step_ticket.cursor,
      eligible_at: Some(step_ticket.eligible_at),
      admission: admission.clone(),
      resources,
    };
    Self::reserve_ready_publication(plan, cell)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn preflight_retained_paged_enqueue(
    actor_id: ActorId,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return if Self::control_hot_exists(actor_id) {
        Err(EnqueueOutcome::CorruptedTopology)
      } else {
        Err(EnqueueOutcome::CapacityUnavailable)
      };
    };
    Self::preflight_paged_enqueue_actor_state(actor_id, &state, &admission, loaded_step.as_ref())
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_queue_pair(
    first: ActorId,
    second: ActorId,
  ) -> Result<[QueueTicket; 2], EnqueueOutcome> {
    let first_hot = match Self::load_actor_state(first) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let second_hot = match Self::load_actor_state(second) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(vec![
      (first, first_hot),
      (second, second_hot),
    ])?;
    let first_ticket = plan.publications[0].ticket;
    let second_ticket = plan.publications[1].ticket;
    Ok([first_ticket, second_ticket])
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_queue_quartet(
    actors: [ActorId; 4],
  ) -> Result<[QueueTicket; 4], EnqueueOutcome> {
    let first_hot = match Self::load_actor_state(actors[0]) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let mut cohort = vec![(actors[0], first_hot)];
    for actor_id in actors.iter(/* deos-bypass: bounded-iter */).skip(1) {
      let hot = match Self::load_actor_state(*actor_id) {
        LoadedActorStateOf::Active(state) => state.hot,
        _ => return Err(EnqueueOutcome::CorruptedTopology),
      };
      cohort.push((*actor_id, hot));
    }
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(cohort)?;
    let mut tickets = [0; 4];
    for (index, publication) in plan
      .publications
      .iter(/* deos-bypass: bounded-iter */)
      .enumerate()
    {
      tickets[index] = publication.ticket;
    }
    Ok(tickets)
  }

  #[cfg(test)]
  pub(crate) fn test_commit_queue_quartet(actors: [ActorId; 4]) -> Result<(), EnqueueOutcome> {
    let mut cohort = Vec::new();
    for actor_id in actors {
      let mut hot = match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot,
        _ => return Err(EnqueueOutcome::CorruptedTopology),
      };
      hot.pending_signal = true;
      cohort.push((actor_id, hot));
    }
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(cohort)?;
    Self::commit_paged_enqueue(plan)
  }

  #[cfg(test)]
  pub(crate) fn test_reset_queue_append_commits() {
    QUEUE_APPEND_COMMITS.with(|count| count.set(0));
  }

  #[cfg(test)]
  pub(crate) fn test_queue_append_commits() -> u32 {
    QUEUE_APPEND_COMMITS.with(core::cell::Cell::get)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn update_existing_frame_control_identity(
    actor_id: ActorId,
    identity: &ActorIdentityOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.identity = Self::control_identity_from_scalar(identity.clone())
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn update_existing_frame_control_hot(
    actor_id: ActorId,
    hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.hot = Self::control_hot_from_scalar(hot.clone());
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn restore_unsignaled_from_authority(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    }
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
      cursor: run_state.map_or(0, |run| run.cursor),
      eligible_at: None,
      admission: admission.clone(),
      resources,
    };
    ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
    let location = ActorControlLocation::Unsignaled;
    ActorControlLocators::<T>::insert(actor_id, location);
    Self::replace_active_semantics_from_primary(actor_id, location)
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn detach_primary_for_successor(
    actor_id: ActorId,
    successor_hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    if let ActorControlLocation::Waiting { key, page, slot } = location {
      let pointer = WakeupPointer {
        block: key,
        page_id: page,
        slot: u32::from(slot),
      };
      if Self::wakeup_pointer_for_clock(successor_hot, key.clock()) == Some(pointer) {
        let mut stored = ActorWaitingFrameChunks::<T>::get((key, page))
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let target = stored
          .entries
          .get_mut(slot as usize)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        if target.as_ref().and_then(ActorWaitingEntry::primary) != Some(&cell) {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        *target = Some(ActorWaitingEntry::Reference(ActorWakeupReference {
          actor_id,
          admission_identity: cell.admission.admission_identity,
        }));
        ActorWaitingFrameChunks::<T>::insert((key, page), stored);
        ActorControlLocators::<T>::remove(actor_id);
        return Ok(());
      }
    }
    Self::remove_primary_control_cell_inner(actor_id)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  fn consume_waiting_from_supplied_authority(
    actor_id: ActorId,
    key: WakeupKey<BlockNumberFor<T>>,
    hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let Some(location) = ActorControlLocators::<T>::get(actor_id) else {
      return Ok(());
    };
    if let ActorControlLocation::Waiting {
      key: waiting_key, ..
    } = location
      && waiting_key == key
    {
      let mut cell = Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      // The caller validates the exact physical pointer before consuming its primary.
      let consumed_trigger_source = matches!(key, WakeupKey::Tick(tick)
        if cell.hot.trigger_wakeup_pointer.is_some_and(|pointer| pointer.tick == tick));
      if consumed_trigger_source {
        if hot.cycle_state != CycleState::Idle
          || hot.pending_signal
          || hot.queue_ticket.is_some()
          || hot.trigger_wakeup_pointer.is_some()
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        cell.hot = Self::control_hot_from_scalar(hot.clone());
        cell.cursor = 0;
        cell.eligible_at = None;
        ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
        let destination = ActorControlLocation::Unsignaled;
        ActorControlLocators::<T>::insert(actor_id, destination);
        Self::replace_active_semantics_from_primary(actor_id, destination)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      }
      return Ok(());
    }
    let (_, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.hot = Self::control_hot_from_scalar(hot.clone());
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn commit_paged_enqueue(plan: QueueAppendPlan<T>) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    QUEUE_APPEND_COMMITS.with(|count| count.set(count.get().saturating_add(1)));
    for PreparedReadyPublication { ticket, cell } in plan.publications {
      let actor_id = cell.actor_id;
      let hot = Self::control_hot_to_scalar(&cell.hot, Some(ticket));
      if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
        let (_, source) = Self::load_primary_control_cell(actor_id)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        let source_hot_matches = if source.hot == cell.hot {
          true
        } else {
          let mut pre_activation_hot = cell.hot.clone();
          pre_activation_hot.pending_signal = false;
          source.hot == pre_activation_hot
        };
        if matches!(location, ActorControlLocation::Ready { .. })
          || source.actor_id != cell.actor_id
          || source.identity != cell.identity
          || !source_hot_matches
          || source.cursor != cell.cursor
          || source.admission != cell.admission
          || source.resources != cell.resources
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        Self::detach_primary_for_successor(actor_id, &hot)?;
      }
      if ActorReadyTail::<T>::get() != ticket || cell.eligible_at.is_none() {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let (stored_actor, stored_ticket) =
        Self::control_append_ready(cell).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      if (stored_actor, stored_ticket) != (actor_id, ticket) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    }
    if ActorReadyTail::<T>::get() != plan.next_tail
      || ActorReadyOccupancy::<T>::get() != plan.next_occupancy
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(())
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn try_paged_enqueue(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| match Self::preflight_retained_paged_enqueue(actor_id) {
      Ok(plan) => match Self::commit_paged_enqueue(plan) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn scheduler_index_is_exhausted(outcome: EnqueueOutcome) -> bool {
    matches!(
      outcome,
      EnqueueOutcome::TicketExhausted
        | EnqueueOutcome::SchedulerIndexExhausted
        | EnqueueOutcome::WakeupIndexExhausted
    )
  }

  /// Maps a placement result to the public error surface for extrinsic boundaries.
  pub fn enqueue_outcome_error(outcome: Result<(), EnqueueOutcome>) -> Result<(), DispatchError> {
    match outcome {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => Err(Error::<T>::QueueCapacityUnavailable.into()),
      Err(EnqueueOutcome::TicketExhausted) => Err(Error::<T>::QueueTicketExhausted.into()),
      Err(EnqueueOutcome::SchedulerIndexExhausted) => {
        Err(Error::<T>::SchedulerIndexExhausted.into())
      }
      Err(EnqueueOutcome::WakeupCapacityExhausted) => {
        Err(Error::<T>::QueueCapacityUnavailable.into())
      }
      Err(EnqueueOutcome::WakeupIndexExhausted) => Err(Error::<T>::SchedulerIndexExhausted.into()),
      Err(EnqueueOutcome::InvalidParkedBalanceActivation) => {
        Err(Error::<T>::InvalidParkedBalanceActivation.into())
      }
      Err(EnqueueOutcome::CorruptedTopology) => Err(Error::<T>::SchedulerIndexExhausted.into()),
    }
  }

  /// Extracts the public error from a failed placement outcome for `map_err` sites.
  pub fn placement_error(outcome: EnqueueOutcome) -> DispatchError {
    match Self::enqueue_outcome_error(Err(outcome)) {
      // Placement owners normally normalize AlreadyLive to success before `map_err`.
      // A missed normalization fails closed instead of panicking in consensus execution.
      Ok(()) => Error::<T>::QueueCapacityUnavailable.into(),
      Err(error) => error,
    }
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn paged_invalidate(actor_id: ActorId) -> Option<QueueTicket> {
    Self::try_invalidate_ready_to_unsignaled(actor_id)
      .ok()
      .flatten()
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn invalidate_ready_to_unsignaled_inner(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return if Self::control_hot_exists(actor_id) {
        Err(EnqueueOutcome::CorruptedTopology)
      } else {
        Ok(None)
      };
    };
    let ticket = state.hot.queue_ticket;
    if ticket.is_some() {
      let resources = if state.contract.steps.is_empty() {
        Self::zero_step_resource_envelope(&state.contract)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      } else {
        let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
        loaded_step
          .filter(|loaded| loaded.cursor == cursor)
          .map(|loaded| loaded.resources)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      };
      let mut hot = state.hot;
      if !hot.lifecycle.is_paused() {
        hot.pending_signal = false;
      }
      hot.queue_ticket = None;
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      Self::restore_unsignaled_from_authority(
        actor_id,
        hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        resources,
      )?;
    }
    Ok(ticket)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn try_invalidate_ready_to_unsignaled(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    with_transaction_opaque_err(
      || match Self::invalidate_ready_to_unsignaled_inner(actor_id) {
        Ok(ticket) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(ticket)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
    )
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub fn paged_head_entry() -> Option<(QueueTicket, QueueEntry<BlockNumberFor<T>>)> {
    let head = ActorReadyHead::<T>::get();
    if head >= ActorReadyTail::<T>::get() {
      return None;
    }
    let chunk = ActorReadyFrameChunks::<T>::get(head / 32)?;
    let cell = chunk.get((head % 32) as usize)?.as_ref()?;
    let entry = ActorStepTicket {
      actor_id: cell.actor_id,
      // Discovery must retain an exhausted head for mandatory terminal cleanup.
      // Opening still requires a checked next nonce in the execution ticket builder.
      cycle_nonce: cell.identity.cycle_nonce.saturating_add(1),
      cursor: cell.cursor,
      ticket: head,
      eligible_at: cell.eligible_at?,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: cell.admission.semantic_contract_id,
        body_commitment: cell.admission.body_commitment,
      },
    };
    Some((head, entry))
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn consume_ready_primary(actor_id: ActorId, ticket: QueueTicket) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::get(actor_id) != Some(ActorControlLocation::Ready { ticket }) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Self::remove_primary_control_cell_inner(actor_id)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn paged_consume_head_at(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(position)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn paged_consume_head_at_inner(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let transition = || -> Result<(), EnqueueOutcome> {
        let topology = Self::queue_topology_preflight(QueueMutation::Head)?;
        if position != topology.head || position >= topology.tail {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let entry = Self::paged_head_entry()
          .map(|(_, entry)| entry)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let (location, cell) = Self::load_primary_control_cell(entry.actor_id)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        if location
          != (ActorControlLocation::Ready {
            ticket: entry.ticket,
          })
          || cell.actor_id != entry.actor_id
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        Self::consume_ready_primary(entry.actor_id, entry.ticket)?;
        let next_head = position
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        ActorReadyHead::<T>::put(next_head);
        if next_head.is_multiple_of(32) || next_head == topology.tail {
          ActorReadyFrameChunks::<T>::remove(position / 32);
        }
        Ok(())
      };
      match transition() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn paged_consume_head(ticket: QueueTicket) -> bool {
    let Some((position, entry)) = Self::paged_head_entry() else {
      return false;
    };
    entry.ticket == ticket && Self::paged_consume_head_at(position).is_ok()
  }

  pub fn paged_drain_tombstones(
    cutoff: QueueTicket,
    scan_limit: u32,
  ) -> Result<QueueDrainStats, EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let transition = || -> Result<QueueDrainStats, EnqueueOutcome> {
        let topology = Self::queue_topology_preflight(QueueMutation::Head)?;
        let mut stats = QueueDrainStats::default();
        let mut head = topology.head;
        let limit = scan_limit.min(T::MaxQueueEntriesScannedPerBlock::get());
        while head < topology.tail && head < cutoff && stats.entries_scanned < limit {
          let page_id = head / 32;
          let chunk =
            ActorReadyFrameChunks::<T>::get(page_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
          if chunk.len() != 32 {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
          stats.pages_touched = stats.pages_touched.saturating_add(1);
          while head < topology.tail
            && head < cutoff
            && head / 32 == page_id
            && stats.entries_scanned < limit
          {
            stats.entries_scanned = stats.entries_scanned.saturating_add(1);
            if let Some(cell) = &chunk[(head % 32) as usize] {
              if ActorControlLocators::<T>::get(cell.actor_id)
                != Some(ActorControlLocation::Ready { ticket: head })
                || Self::project_control_cell(cell, ActorControlLocation::Ready { ticket: head })
                  .is_none()
              {
                return Err(EnqueueOutcome::CorruptedTopology);
              }
              if head != topology.head {
                ActorReadyHead::<T>::put(head);
              }
              return Ok(stats);
            }
            stats.tombstones_skipped = stats.tombstones_skipped.saturating_add(1);
            head = head
              .checked_add(1)
              .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
          }
          if head.is_multiple_of(32) || head == topology.tail {
            ActorReadyFrameChunks::<T>::remove(page_id);
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
        }
        if head != topology.head {
          ActorReadyHead::<T>::put(head);
        }
        Ok(stats)
      };
      match transition() {
        Ok(stats) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub(crate) fn wakeup_page_entry_matches(
    pointer: WakeupPointer<BlockNumberFor<T>>,
    actor_id: ActorId,
  ) -> bool {
    ActorWaitingFrameChunks::<T>::get((pointer.block, pointer.page_id))
      .and_then(|page| page.entries.get(pointer.slot as usize).cloned().flatten())
      .is_some_and(|entry| match entry {
        ActorWaitingEntry::Primary(cell) => cell.actor_id == actor_id,
        ActorWaitingEntry::Reference(reference) => reference.actor_id == actor_id,
      })
  }

  fn wakeup_pointer_for_clock(
    hot: &ActorHotStateOf<T>,
    clock: WakeupClock,
  ) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    match clock {
      WakeupClock::Block => hot.wakeup_pointer,
      WakeupClock::Tick => hot.trigger_wakeup_pointer.map(|pointer| WakeupPointer {
        block: WakeupKey::Tick(pointer.tick),
        page_id: pointer.page_id,
        slot: pointer.slot,
      }),
    }
  }

  fn clear_wakeup_pointer_for_clock(hot: &mut ActorHotStateOf<T>, clock: WakeupClock) {
    match clock {
      WakeupClock::Block => hot.wakeup_pointer = None,
      WakeupClock::Tick => hot.trigger_wakeup_pointer = None,
    }
  }

  pub(crate) fn wakeup_substrate_invalidate_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, admission, WakeupClock::Block)
  }

  pub(crate) fn trigger_wakeup_substrate_invalidate_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, admission, WakeupClock::Tick)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn wakeup_substrate_invalidate_clock_inner(
    actor_id: ActorId,
    clock: WakeupClock,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let Some((state, admission, _)) = Self::load_frame_actor_service_state(actor_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, &admission, clock)
  }

  fn wakeup_substrate_invalidate_clock_loaded(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    clock: WakeupClock,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let Some((_, frame_identity, frame_hot, frame_admission)) =
      Self::load_frame_control_authority(actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if frame_identity != state.identity || frame_hot != state.hot || frame_admission != *admission {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let Some(pointer) = Self::wakeup_pointer_for_clock(&state.hot, clock) else {
      return Ok(None);
    };
    Self::invalidate_wakeup_reference(actor_id, pointer, admission.admission_identity)?;
    Self::clear_wakeup_pointer_for_clock(&mut state.hot, clock);
    {
      Self::consume_waiting_from_supplied_authority(actor_id, pointer.block, &state.hot)?;
    }
    Ok(Some(pointer))
  }

  pub(crate) fn invalidate_wakeup_reference(
    actor_id: ActorId,
    pointer: WakeupPointer<BlockNumberFor<T>>,
    admission_identity: [u8; 32],
  ) -> Result<(), EnqueueOutcome> {
    let page = ActorWaitingFrameChunks::<T>::get((pointer.block, pointer.page_id))
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    match page
      .entries
      .get(pointer.slot as usize)
      .and_then(Option::as_ref)
    {
      Some(ActorWaitingEntry::Primary(cell))
        if cell.actor_id == actor_id && cell.admission.admission_identity == admission_identity =>
      {
        // The caller transfers primary authority immediately after clearing this pointer.
        // Only that transfer may remove the primary slot and its locator.
        Ok(())
      }
      Some(ActorWaitingEntry::Reference(reference))
        if reference.actor_id == actor_id && reference.admission_identity == admission_identity =>
      {
        Self::remove_waiting_entry(pointer)
          .map(|_| ())
          .map_err(|_| EnqueueOutcome::CorruptedTopology)
      }
      _ => Err(EnqueueOutcome::CorruptedTopology),
    }
  }

  pub(crate) fn load_primary_control_cell(
    actor_id: ActorId,
  ) -> Result<
    (
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    ),
    ActorControlTransitionError,
  > {
    let location =
      ActorControlLocators::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
    let cell = match location {
      ActorControlLocation::Unsignaled => ActorUnsignaledControlCells::<T>::get(actor_id),
      ActorControlLocation::Ready { ticket } => ActorReadyFrameChunks::<T>::get(ticket / 32)
        .and_then(|chunk| chunk.get((ticket % 32) as usize).cloned().flatten()),
      ActorControlLocation::Waiting { key, page, slot } => {
        ActorWaitingFrameChunks::<T>::get((key, page))
          .and_then(|page| page.entries.get(slot as usize).cloned().flatten())
          .and_then(ActorWaitingEntry::into_primary)
      }
    }
    .ok_or(ActorControlTransitionError::Invariant)?;
    if cell.actor_id != actor_id
      || !cell.admission.has_valid_identity()
      || cell.pipeline_service_identity
        != pipeline_service_identity(cell.admission.admission_identity)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok((location, cell))
  }

  pub(crate) fn replace_active_semantics_from_primary(
    actor_id: ActorId,
    location: ActorControlLocation<BlockNumberFor<T>>,
  ) -> Result<(), ActorControlTransitionError> {
    let Some(ActorSemanticState::Active(current)) = ActorSemanticStates::<T>::get(actor_id) else {
      return Err(ActorControlTransitionError::Invariant);
    };
    let (_, cell) = Self::load_primary_control_cell(actor_id)?;
    let (identity, hot, admission) =
      Self::project_control_cell(&cell, location).ok_or(ActorControlTransitionError::Invariant)?;
    ActorSemanticStates::<T>::insert(
      actor_id,
      ActorSemanticState::Active(ActorSemanticRecord {
        identity,
        generation: current.generation,
        hot,
        admission,
      }),
    );
    Ok(())
  }

  pub(crate) fn store_primary_control_cell(
    location: ActorControlLocation<BlockNumberFor<T>>,
    cell: ActorControlCellOf<T>,
  ) -> Result<(), ActorControlTransitionError> {
    let actor_id = cell.actor_id;
    if ActorControlLocators::<T>::get(actor_id) != Some(location)
      || !matches!(
        ActorSemanticStates::<T>::get(actor_id),
        Some(ActorSemanticState::Active(_))
      )
      || Self::project_control_cell(&cell, location).is_none()
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    match location {
      ActorControlLocation::Unsignaled => {
        if ActorUnsignaledControlCells::<T>::get(actor_id)
          .is_none_or(|stored| stored.actor_id != actor_id)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
      }
      ActorControlLocation::Ready { ticket } => {
        let page = ticket / 32;
        let slot = (ticket % 32) as usize;
        let mut chunk =
          ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .get_mut(slot)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored.as_ref().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = Some(cell);
        ActorReadyFrameChunks::<T>::insert(page, chunk);
      }
      ActorControlLocation::Waiting { key, page, slot } => {
        let mut chunk = ActorWaitingFrameChunks::<T>::get((key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .entries
          .get_mut(slot as usize)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored
          .as_ref()
          .and_then(ActorWaitingEntry::primary)
          .map(|stored| stored.actor_id)
          != Some(actor_id)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = Some(ActorWaitingEntry::Primary(cell));
        ActorWaitingFrameChunks::<T>::insert((key, page), chunk);
      }
    }
    Self::replace_active_semantics_from_primary(actor_id, location)
  }

  pub(crate) fn remove_primary_control_cell_inner(
    actor_id: ActorId,
  ) -> Result<ActorControlCellOf<T>, ActorControlTransitionError> {
    let (location, cell) = Self::load_primary_control_cell(actor_id)?;
    match location {
      ActorControlLocation::Unsignaled => {
        ActorUnsignaledControlCells::<T>::remove(actor_id);
      }
      ActorControlLocation::Ready { ticket } => {
        let page = ticket / 32;
        let slot = (ticket % 32) as usize;
        let mut chunk =
          ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .get_mut(slot)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored.as_ref().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = None;
        ActorReadyFrameChunks::<T>::insert(page, chunk);
        ActorReadyOccupancy::<T>::try_mutate(|occupancy| {
          *occupancy = occupancy
            .checked_sub(1)
            .ok_or(ActorControlTransitionError::Invariant)?;
          Ok::<(), ActorControlTransitionError>(())
        })?;
      }
      ActorControlLocation::Waiting { key, page, slot } => {
        let removed = Self::remove_waiting_entry(WakeupPointer {
          block: key,
          page_id: page,
          slot: u32::from(slot),
        })?;
        if removed.primary().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
      }
    }
    ActorControlLocators::<T>::remove(actor_id);
    Ok(cell)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  #[allow(
    dead_code,
    reason = "legacy waiting-substrate invalidation is reachable only from runtime-benchmark fixtures after the canonical deadline carrier cutover"
  )]
  pub(crate) fn trigger_wakeup_substrate_invalidate_inner(
    actor_id: ActorId,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_inner(actor_id, WakeupClock::Tick)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn wakeup_substrate_invalidate(
    actor_id: ActorId,
  ) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let result: Result<WakeupPointer<BlockNumberFor<T>>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_invalidate_clock_inner(actor_id, WakeupClock::Block) {
          Ok(Some(pointer)) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(pointer))
          }
          Ok(None) | Err(_) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
            Err(Error::<T>::ActorNotFound.into()),
          ),
        }
      });
    result.ok()
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn wakeup_substrate_schedule_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> bool {
    matches!(
      Self::try_wakeup_substrate_schedule_key_inner(actor_id, wakeup_key),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn try_wakeup_substrate_schedule_key_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::schedule_retained_wakeup_transition(actor_id, wakeup_key) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn schedule_retained_wakeup_transition(
    actor_id: ActorId,
    wakeup_block: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some((_, frame_identity, frame_hot, frame_admission)) =
      Self::load_frame_control_authority(actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if frame_identity != state.identity || frame_hot != state.hot || frame_admission != admission {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let resources = if state.contract.steps.is_empty() {
      Self::zero_step_resource_envelope(&state.contract).ok_or(EnqueueOutcome::CorruptedTopology)?
    } else {
      loaded_step
        .ok_or(EnqueueOutcome::CorruptedTopology)?
        .resources
    };
    Self::try_wakeup_substrate_schedule_transition_with_authority(
      actor_id,
      wakeup_block,
      state.hot,
      &state.identity,
      state.run_state.as_ref().map_or(0, |run| run.cursor),
      &admission,
      resources,
    )
  }

  fn publish_waiting_from_authority(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::contains_key(actor_id) {
      return Err(EnqueueOutcome::AlreadyLive);
    }
    let (authority, eligible_at) = match wakeup_key {
      WakeupKey::Block(block) => (ActorWaitingAuthority::Service, Some(block)),
      WakeupKey::Tick(_) => (ActorWaitingAuthority::Trigger, None),
    };
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
      cursor,
      eligible_at,
      admission: admission.clone(),
      resources,
    };
    Self::control_append_waiting(cell, wakeup_key, authority)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn try_wakeup_substrate_schedule_transition_with_authority(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::schedule_wakeup_transition_with_authority_inner(
        actor_id, wakeup_key, hot, identity, cursor, admission, resources,
      ) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn schedule_wakeup_transition_with_authority_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    mut hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    let clock = wakeup_key.clock();
    if let Some(pointer) = Self::wakeup_pointer_for_clock(&hot, clock) {
      if pointer.block == wakeup_key && Self::wakeup_page_entry_matches(pointer, actor_id) {
        if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
          let (_, mut source) = Self::load_primary_control_cell(actor_id)
            .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          if source.admission != *admission {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
          source.identity = Self::control_identity_from_scalar(identity.clone())
            .ok_or(EnqueueOutcome::CorruptedTopology)?;
          source.hot = Self::control_hot_from_scalar(hot);
          source.cursor = cursor;
          source.resources = resources;
          return Self::store_primary_control_cell(location, source)
            .map_err(|_| EnqueueOutcome::CorruptedTopology);
        }
        return Self::publish_waiting_from_authority(
          actor_id, wakeup_key, hot, identity, cursor, admission, resources,
        );
      }
      Self::invalidate_wakeup_reference(actor_id, pointer, admission.admission_identity)?;
      Self::clear_wakeup_pointer_for_clock(&mut hot, clock);
    }
    let mut retained_primary = None;
    if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
      let (_, source) =
        Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      let mut expected_identity = Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      expected_identity.cycle_nonce = source.identity.cycle_nonce;
      if source.identity != expected_identity || source.admission != *admission {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      if location == ActorControlLocation::Unsignaled
        || matches!(location, ActorControlLocation::Waiting { key, .. } if key.clock() == clock)
      {
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      } else {
        retained_primary = Some((location, source));
      }
    }
    let (page_id, slot) =
      Self::schedule_fresh_wakeup_reference(actor_id, wakeup_key, admission.admission_identity)?;
    hot = Self::with_wakeup_pointer(hot, wakeup_key, page_id, slot);
    if let Some((location, mut source)) = retained_primary {
      source.identity = Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      source.hot = Self::control_hot_from_scalar(hot);
      source.cursor = cursor;
      source.resources = resources;
      return Self::store_primary_control_cell(location, source)
        .map_err(|_| EnqueueOutcome::CorruptedTopology);
    }
    Self::publish_waiting_from_authority(
      actor_id, wakeup_key, hot, identity, cursor, admission, resources,
    )
  }

  fn schedule_fresh_wakeup_reference(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    admission_identity: [u8; 32],
  ) -> Result<(WakeupPageId, WakeupSlot), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    Self::append_waiting_entry(wakeup_key, |_, _| {
      ActorWaitingEntry::Reference(ActorWakeupReference {
        actor_id,
        admission_identity,
      })
    })
    .map_err(|error| match error {
      ActorControlTransitionError::IndexExhausted => EnqueueOutcome::WakeupIndexExhausted,
      _ => EnqueueOutcome::CorruptedTopology,
    })
  }

  fn with_wakeup_pointer(
    mut hot: ActorHotStateOf<T>,
    block: WakeupKey<BlockNumberFor<T>>,
    page_id: WakeupPageId,
    slot: WakeupSlot,
  ) -> ActorHotStateOf<T> {
    match block {
      WakeupKey::Block(_) => {
        hot.wakeup_pointer = Some(WakeupPointer {
          block,
          page_id,
          slot,
        });
      }
      WakeupKey::Tick(tick) => {
        hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
          tick,
          page_id,
          slot,
        });
      }
    }
    hot
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn wakeup_substrate_schedule(actor_id: ActorId, wakeup_block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_substrate_schedule_inner(actor_id, WakeupKey::Block(wakeup_block)) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_substrate_drain_block_inner(
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> Option<(
    BoundedVec<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >,
    WakeupDrainStats,
  )> {
    let mut ready = BoundedVec::<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >::default();
    let mut stats = WakeupDrainStats::default();
    let limit = max_entries_scanned.min(T::MaxWakeupsPerBlock::get());
    let mut last_page = None;
    while stats.entries_scanned < limit && ActorWaitingOccupancies::<T>::get(wakeup_key) > 0 {
      let cursor_index = ActorWaitingCursorIndices::<T>::get(wakeup_key)?;
      if Self::wakeup_cursor_get(wakeup_key.clock(), cursor_index) != Some(wakeup_key) {
        return None;
      }
      let head = ActorWaitingHeads::<T>::get(wakeup_key);
      let page_id = head / 32;
      let mut page = ActorWaitingFrameChunks::<T>::get((wakeup_key, page_id))?;
      if page.previous_page.is_some()
        || page.entries.len() != 32
        || page.live_entries == 0
        || page.entries.iter(/* deos-bypass: bounded-iter */).filter(|entry| entry.is_some()).count()
          != page.live_entries as usize
      {
        return None;
      }
      if last_page != Some(page_id) {
        stats.pages_touched = stats.pages_touched.saturating_add(1);
        last_page = Some(page_id);
      }
      let slot = page.scan_slot;
      if slot >= 32 || head % 32 != u64::from(slot) {
        return None;
      }
      let entry = page.entries.get(slot as usize)?.clone();
      let pointer = WakeupPointer {
        block: wakeup_key,
        page_id,
        slot,
      };
      let frozen = entry.as_ref().and_then(|entry| {
        let actor_id = match entry {
          ActorWaitingEntry::Primary(cell) => cell.actor_id,
          ActorWaitingEntry::Reference(reference) => reference.actor_id,
        };
        Self::load_frame_actor_service_state(actor_id)
      });
      page.scan_slot = slot.checked_add(1)?;
      stats.entries_scanned = stats.entries_scanned.saturating_add(1);
      ActorWaitingFrameChunks::<T>::insert((wakeup_key, page_id), page);
      ActorWaitingHeads::<T>::insert(wakeup_key, head.checked_add(1)?);
      let Some(entry) = entry else {
        continue;
      };
      let (actor_id, expected_admission, is_primary) = match entry {
        ActorWaitingEntry::Primary(cell) => {
          (cell.actor_id, cell.admission.admission_identity, true)
        }
        ActorWaitingEntry::Reference(reference) => {
          (reference.actor_id, reference.admission_identity, false)
        }
      };
      let (mut state, admission, loaded_step) = match frozen {
        Some((state, admission, loaded_step))
          if admission.admission_identity == expected_admission
            && Self::admission_authorizes_contract_wake(&admission, &state.contract)
            && Self::wakeup_pointer_for_clock(&state.hot, wakeup_key.clock()) == Some(pointer) =>
        {
          (state, admission, loaded_step)
        }
        None if !is_primary && !ActorControlLocators::<T>::contains_key(actor_id) => {
          Self::remove_waiting_entry(pointer).ok()?;
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
          continue;
        }
        Some((state, _, _))
          if !is_primary
            && Self::wakeup_pointer_for_clock(&state.hot, wakeup_key.clock()).is_none() =>
        {
          Self::remove_waiting_entry(pointer).ok()?;
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
          continue;
        }
        _ => return None,
      };
      Self::clear_wakeup_pointer_for_clock(&mut state.hot, wakeup_key.clock());
      if !is_primary {
        Self::remove_waiting_entry(pointer).ok()?;
      }
      Self::consume_waiting_from_supplied_authority(actor_id, wakeup_key, &state.hot).ok()?;
      ready
        .try_push((actor_id, state, admission, loaded_step))
        .ok()?;
      stats.ready_entries = stats.ready_entries.saturating_add(1);
      if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
        stats.pages_deleted = stats.pages_deleted.saturating_add(1);
      }
    }
    Some((ready, stats))
  }

  pub(crate) fn wakeup_substrate_drain_key(
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> (
    BoundedVec<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >,
    WakeupDrainStats,
  ) {
    let result: Result<_, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_drain_block_inner(wakeup_key, max_entries_scanned) {
          Some(result) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(result))
          }
          None => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorNotFound.into(),
          )),
        }
      });
    result.unwrap_or_default()
  }

  pub fn wakeup_substrate_drain_block(
    wakeup_block: BlockNumberFor<T>,
    max_entries_scanned: u32,
  ) -> (BoundedVec<ActorId, T::MaxWakeupsPerBlock>, WakeupDrainStats) {
    let (loaded, stats) =
      Self::wakeup_substrate_drain_key(WakeupKey::Block(wakeup_block), max_entries_scanned);
    let ready = BoundedVec::truncate_from(
      loaded
        .into_iter()
        .map(|(actor_id, _, _, _)| actor_id)
        .collect(),
    );
    (ready, stats)
  }

  fn wakeup_cursor_page_and_slot(index: WakeupCursorIndex) -> (WakeupPageId, usize) {
    let page_size = T::WakeupPageSize::get().max(1);
    (u64::from(index / page_size), (index % page_size) as usize)
  }

  pub(crate) fn wakeup_cursor_get(
    clock: WakeupClock,
    index: WakeupCursorIndex,
  ) -> Option<WakeupKey<BlockNumberFor<T>>> {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    WakeupCursorPages::<T>::get((clock, page_id)).and_then(|page| page.get(slot).copied())
  }

  fn wakeup_cursor_set(
    clock: WakeupClock,
    index: WakeupCursorIndex,
    block: WakeupKey<BlockNumberFor<T>>,
  ) -> bool {
    if block.clock() != clock {
      return false;
    }
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let mut page = WakeupCursorPages::<T>::get((clock, page_id)).unwrap_or_default();
    if slot < page.len() {
      page[slot] = block;
    } else if slot == page.len() {
      if page.try_push(block).is_err() {
        return false;
      }
    } else {
      return false;
    }
    WakeupCursorPages::<T>::insert((clock, page_id), page);
    true
  }

  fn wakeup_cursor_remove_tail(clock: WakeupClock, index: WakeupCursorIndex) -> bool {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let Some(mut page) = WakeupCursorPages::<T>::get((clock, page_id)) else {
      return false;
    };
    if slot.checked_add(1) != Some(page.len()) {
      return false;
    }
    page.pop();
    if page.is_empty() {
      WakeupCursorPages::<T>::remove((clock, page_id));
    } else {
      WakeupCursorPages::<T>::insert((clock, page_id), page);
    }
    true
  }

  pub(crate) fn wakeup_cursor_owner_index(
    key: WakeupKey<BlockNumberFor<T>>,
  ) -> Option<WakeupCursorIndex> {
    ActorWaitingCursorIndices::<T>::get(key)
  }

  fn wakeup_cursor_has_owner(key: WakeupKey<BlockNumberFor<T>>) -> bool {
    ActorWaitingOccupancies::<T>::get(key) > 0 || ActorWaitingCursorIndices::<T>::contains_key(key)
  }

  fn wakeup_cursor_write_owner_index(
    key: WakeupKey<BlockNumberFor<T>>,
    index: Option<WakeupCursorIndex>,
  ) -> bool {
    let mut wrote = false;
    if ActorWaitingOccupancies::<T>::get(key) > 0
      || ActorWaitingCursorIndices::<T>::contains_key(key)
    {
      match index {
        Some(index) => ActorWaitingCursorIndices::<T>::insert(key, index),
        None => ActorWaitingCursorIndices::<T>::remove(key),
      }
      wrote = true;
    }
    wrote
  }

  fn wakeup_cursor_swap(
    clock: WakeupClock,
    left: WakeupCursorIndex,
    right: WakeupCursorIndex,
  ) -> bool {
    let Some(left_block) = Self::wakeup_cursor_get(clock, left) else {
      return false;
    };
    let Some(right_block) = Self::wakeup_cursor_get(clock, right) else {
      return false;
    };
    if Self::wakeup_cursor_owner_index(left_block) != Some(left)
      || Self::wakeup_cursor_owner_index(right_block) != Some(right)
    {
      return false;
    }
    if !Self::wakeup_cursor_set(clock, left, right_block)
      || !Self::wakeup_cursor_set(clock, right, left_block)
    {
      return false;
    }
    Self::wakeup_cursor_write_owner_index(right_block, Some(left))
      && Self::wakeup_cursor_write_owner_index(left_block, Some(right))
  }

  fn wakeup_cursor_height_bound() -> u32 {
    u32::BITS.saturating_sub(T::MaxActiveActors::get().max(1).leading_zeros())
  }

  fn wakeup_cursor_insert_inner(block: WakeupKey<BlockNumberFor<T>>) -> bool {
    let clock = block.clock();
    if !Self::wakeup_cursor_has_owner(block) {
      return false;
    }
    if let Some(index) = Self::wakeup_cursor_owner_index(block) {
      return Self::wakeup_cursor_get(clock, index) == Some(block)
        && Self::wakeup_cursor_write_owner_index(block, Some(index));
    }
    let len = WakeupCursorLen::<T>::get(clock);
    let Some(next_len) = len.checked_add(1) else {
      return false;
    };
    if len >= T::MaxActiveActors::get() || !Self::wakeup_cursor_set(clock, len, block) {
      return false;
    }
    if !Self::wakeup_cursor_write_owner_index(block, Some(len)) {
      return false;
    }
    WakeupCursorLen::<T>::insert(clock, next_len);
    let mut current = len;
    for _ in 0..Self::wakeup_cursor_height_bound() {
      if current == 0 {
        break;
      }
      let parent = current.saturating_sub(1) / 2;
      let Some(parent_block) = Self::wakeup_cursor_get(clock, parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, parent, current) {
        return false;
      }
      current = parent;
    }
    true
  }

  pub fn wakeup_cursor_insert(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_cursor_insert_inner(WakeupKey::Block(block)) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  pub(crate) fn wakeup_cursor_peek_key(clock: WakeupClock) -> Option<WakeupKey<BlockNumberFor<T>>> {
    (WakeupCursorLen::<T>::get(clock) > 0)
      .then(|| Self::wakeup_cursor_get(clock, 0))
      .flatten()
  }

  pub fn wakeup_cursor_peek() -> Option<BlockNumberFor<T>> {
    match Self::wakeup_cursor_peek_key(WakeupClock::Block)? {
      WakeupKey::Block(block) => Some(block),
      WakeupKey::Tick(_) => None,
    }
  }

  fn wakeup_cursor_remove_inner(block: WakeupKey<BlockNumberFor<T>>) -> bool {
    let clock = block.clock();
    let Some(index) = Self::wakeup_cursor_owner_index(block) else {
      return false;
    };
    let len = WakeupCursorLen::<T>::get(clock);
    if index >= len || Self::wakeup_cursor_get(clock, index) != Some(block) {
      return false;
    }
    let Some(last_index) = len.checked_sub(1) else {
      return false;
    };
    let Some(last_block) = Self::wakeup_cursor_get(clock, last_index) else {
      return false;
    };
    if Self::wakeup_cursor_owner_index(last_block) != Some(last_index)
      || !Self::wakeup_cursor_remove_tail(clock, last_index)
      || !Self::wakeup_cursor_write_owner_index(block, None)
    {
      return false;
    }
    WakeupCursorLen::<T>::insert(clock, last_index);
    if index == last_index {
      return true;
    }
    if !Self::wakeup_cursor_set(clock, index, last_block)
      || !Self::wakeup_cursor_write_owner_index(last_block, Some(index))
    {
      return false;
    }

    let mut current = index;
    for _ in 0..Self::wakeup_cursor_height_bound() {
      if current == 0 {
        break;
      }
      let parent = current.saturating_sub(1) / 2;
      let Some(parent_block) = Self::wakeup_cursor_get(clock, parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, parent, current) {
        return false;
      }
      current = parent;
    }
    if current != index {
      return true;
    }

    for _ in 0..Self::wakeup_cursor_height_bound() {
      let left = current.saturating_mul(2).saturating_add(1);
      if left >= last_index {
        break;
      }
      let right = left.saturating_add(1);
      let mut smallest = left;
      let Some(left_block) = Self::wakeup_cursor_get(clock, left) else {
        return false;
      };
      if right < last_index {
        let Some(right_block) = Self::wakeup_cursor_get(clock, right) else {
          return false;
        };
        if right_block < left_block {
          smallest = right;
        }
      }
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      let Some(smallest_block) = Self::wakeup_cursor_get(clock, smallest) else {
        return false;
      };
      if current_block <= smallest_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, current, smallest) {
        return false;
      }
      current = smallest;
    }
    true
  }

  pub(crate) fn control_wakeup_cursor_release(key: WakeupKey<BlockNumberFor<T>>) -> bool {
    let Some(index) = ActorWaitingCursorIndices::<T>::get(key) else {
      return false;
    };
    if Self::wakeup_cursor_get(key.clock(), index) != Some(key) {
      return false;
    }
    Self::wakeup_cursor_remove_inner(key)
  }

  pub fn wakeup_cursor_remove(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      let key = WakeupKey::Block(block);
      if ActorWaitingOccupancies::<T>::get(key) > 0 {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ));
      }
      if Self::wakeup_cursor_remove_inner(key) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_cursor_pop_min_inner(clock: WakeupClock) -> Option<WakeupKey<BlockNumberFor<T>>> {
    let min_block = Self::wakeup_cursor_get(clock, 0)?;
    if ActorWaitingOccupancies::<T>::get(min_block) > 0 {
      return None;
    }
    Self::wakeup_cursor_remove_inner(min_block).then_some(min_block)
  }

  pub fn wakeup_cursor_pop_min() -> Option<BlockNumberFor<T>> {
    let result: Result<BlockNumberFor<T>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_cursor_pop_min_inner(WakeupClock::Block) {
          Some(WakeupKey::Block(block)) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(block))
          }
          Some(WakeupKey::Tick(_)) | None => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Error::<T>::ActorNotFound.into(),
            ))
          }
        }
      });
    result.ok()
  }

  fn initial_trigger_wakeup_tick(
    instance: &ActiveActorViewOf<T>,
  ) -> Result<Option<SchedulerTick>, EnqueueOutcome> {
    Ok(match instance.trigger {
      Trigger::AtTime { after_ticks } if !instance.temporal_occurrence_consumed => {
        Some(instance.temporal_anchor_tick.map_or(Ok(0), |anchor_tick| {
          anchor_tick
            .checked_add(after_ticks)
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
        })?)
      }
      Trigger::Cadenced { every_ticks } => Some(
        instance
          .temporal_anchor_tick
          .map(|anchor_tick| {
            next_cadence_due_tick(anchor_tick, every_ticks, Self::current_scheduler_tick()?)
              .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
          })
          .transpose()?
          .unwrap_or(0),
      ),
      Trigger::Manual | Trigger::AddressEvent { .. } | Trigger::AtTime { .. } => None,
    })
  }

  fn window_expiry_wakeup(instance: &ActiveActorViewOf<T>) -> Option<BlockNumberFor<T>> {
    instance
      .window
      .map(|window| Self::window_terminal_at(&window))
  }

  #[cfg(test)]
  fn defer_retained_wakeup(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (state, admission, loaded_step) =
      Self::load_frame_actor_service_state(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let resources = if state.contract.steps.is_empty() {
      Self::zero_step_resource_envelope(&state.contract).ok_or(EnqueueOutcome::CorruptedTopology)?
    } else {
      loaded_step
        .ok_or(EnqueueOutcome::CorruptedTopology)?
        .resources
    };
    Self::defer_wakeup_with_authority(
      actor_id,
      wakeup_block,
      &instance,
      state.hot,
      &state.identity,
      state.run_state.as_ref(),
      &admission,
      resources,
    )
  }

  #[cfg(test)]
  fn defer_wakeup_with_authority(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
    instance: &ActiveActorViewOf<T>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    let target = Self::window_expiry_wakeup(instance)
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    match with_transaction_opaque_err(|| {
      match Self::try_wakeup_substrate_schedule_transition_with_authority(
        actor_id,
        WakeupKey::Block(target),
        hot,
        identity,
        run_state.map_or(0, |run| run.cursor),
        admission,
        resources,
      ) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
    {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  /// Component-wise conservative owner across every complete canonical outer Service profile.
  pub fn scheduler_complete_outer_weight_upper() -> Weight {
    T::WeightInfo::scheduler_service_successful_interior()
      .max(T::WeightInfo::scheduler_service_retry_to_deadline())
      .max(T::WeightInfo::scheduler_service_retry_to_deadline_new_key())
      .max(T::WeightInfo::scheduler_due_deadline_to_service())
      .max(T::WeightInfo::scheduler_due_deadline_to_service_deep_index())
      .max(T::WeightInfo::scheduler_service_late_refusal_rollback())
      .max(T::WeightInfo::scheduler_service_terminal_retain_close())
      .max(T::WeightInfo::scheduler_service_minimal_apoptosis())
  }

  /// Baseline scheduler envelope reserved ahead of one actor run plus pure cleanup.
  /// Explicit permissionless repair sweeps remain dispatch-owned and do not consume every block's
  /// guaranteed scheduler envelope.
  pub fn scheduler_admission_overhead() -> Weight {
    T::WeightInfo::scheduler_on_idle_base()
      .saturating_add(Self::scheduler_actor_state_probe_weight_upper())
  }

  /// Host-owned bound for complete lifecycle destruction, independently admitted from
  /// recurring service. This is the Close cost owner, not another service fee, and it stays the
  /// plain owner so ordinary Close dispatch annotations and reservations keep their meaning. The
  /// policy-bearing owner supplements it at admission, where an authored policy actually exists.
  pub fn close_cleanup_weight_upper() -> Weight {
    T::WeightInfo::close_actor()
  }

  pub fn scheduler_actor_probe_weight_upper() -> Weight {
    Self::scheduler_actor_state_probe_weight_upper()
  }

  pub fn scheduler_actor_state_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_state_probe()
  }

  /// The funded Trigger retires its original wait only with the canonical paid occurrence.
  pub(crate) fn execute_financial_at_time_recovery_on_service(
    actor: ActorRef,
    kind: ServiceResidenceKind,
    state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    loaded_step: Option<LoadedActorStepOf<T>>,
    now: BlockNumberFor<T>,
  ) -> Result<(), AttemptTransactionError> {
    if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return Err(AttemptTransactionError::Invariant);
    }
    let semantic = Self::load_service_actor_semantic_state(actor, kind)
      .map_err(|_| AttemptTransactionError::Invariant)?;
    if semantic.identity != state.identity
      || semantic.hot != state.hot
      || semantic.admission != admission
      || Self::consider_service_head(now).map_err(|_| AttemptTransactionError::Invariant)?
        != ServiceRoundEncounter::Eligible(actor)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let receipt = Self::prepare_financial_service_recovery(actor, &semantic)?
      .filter(|receipt| {
        matches!(
          receipt.unpaid_service,
          crate::UnpaidActorService::AtTimeTrigger { .. }
        )
      })
      .ok_or(AttemptTransactionError::Invariant)?;
    let now_tick =
      Self::current_scheduler_tick().map_err(|_| AttemptTransactionError::Invariant)?;
    if Self::process_due_temporal_occurrence_loaded(
      actor.actor_id,
      state,
      admission,
      loaded_step,
      now_tick,
    )
    .map_err(|cause| match cause {
      TemporalOccurrenceError::Dispatch(error)
        if error == Error::<T>::TriggerFeeCollectionFailed.into() =>
      {
        AttemptTransactionError::FeeCollection
      }
      TemporalOccurrenceError::Deadline(error) => AttemptTransactionError::Deadline(error),
      _ => AttemptTransactionError::Invariant,
    })? {
      return Err(AttemptTransactionError::Invariant);
    }
    Self::deposit_event(Event::FinancialWaitRecovered { receipt });
    Ok(())
  }

  pub(crate) fn process_due_temporal_occurrence_loaded(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    loaded_step: Option<LoadedActorStepOf<T>>,
    now_tick: SchedulerTick,
  ) -> Result<bool, TemporalOccurrenceError> {
    if !Self::admission_authorizes_contract_wake(&admission, &state.contract) {
      return Err(DispatchError::Other("temporal wake qualification is corrupt").into());
    }
    let resources = if state.contract.steps.is_empty() {
      Self::zero_step_resource_envelope(&state.contract).ok_or(DispatchError::Other(
        "temporal zero-Step control is unavailable",
      ))?
    } else {
      let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
      loaded_step
        .filter(|loaded| loaded.cursor == cursor)
        .map(|loaded| loaded.resources)
        .ok_or(DispatchError::Other("temporal loaded Step is corrupt"))?
    };
    if state.hot.pending_signal {
      return Ok(false);
    }
    let (delay_ticks, trigger_family, occurrence_weight) = match state.contract.trigger {
      Trigger::AtTime { after_ticks } => (
        after_ticks,
        TriggerFamily::AtTime,
        T::WeightInfo::at_time_trigger_occurrence(),
      ),
      Trigger::Cadenced { every_ticks } => (
        every_ticks,
        TriggerFamily::Cadenced,
        T::WeightInfo::cadenced_trigger_occurrence(),
      ),
      Trigger::Manual | Trigger::AddressEvent { .. } => {
        return Err(DispatchError::Other("tick wakeup owner is not temporal").into());
      }
    };
    if state
      .hot
      .trigger_runtime_state
      .temporal_anchor_tick()
      .is_none()
    {
      let Some(anchor_tick) = Self::temporal_anchor_tick(&state.contract.trigger)
        .map_err(|_| DispatchError::Other("genesis temporal anchor failed"))?
      else {
        return Err(DispatchError::Other("genesis temporal anchor failed").into());
      };
      let due_tick = anchor_tick
        .checked_add(delay_ticks)
        .ok_or(DispatchError::Other("genesis temporal deadline failed"))?;
      let initialized = match state.contract.trigger {
        Trigger::AtTime { .. } => TriggerRuntimeState::AtTime {
          anchor_tick: Some(anchor_tick),
          consumed: false,
        },
        Trigger::Cadenced { .. } => TriggerRuntimeState::Cadenced {
          anchor_tick: Some(anchor_tick),
        },
        _ => return Err(DispatchError::Other("tick wakeup owner changed trigger").into()),
      };
      // Genesis installation records `None` as the bounded bootstrap marker because no consensus
      // timestamp exists yet and `plan_actor_publication` publishes a Tick(0) bootstrap deadline;
      // the first observed timestamp initializes the anchor here. A canonically published Actor
      // therefore re-anchors and places its deadline through the canonical carrier, while a
      // pre-cutover carrier keeps the legacy waiting-substrate transition.
      let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
        && ActorProcesses::<T>::contains_key(actor_id);
      if canonical {
        let actor = Self::load_actor_ref(actor_id).ok_or(DispatchError::Other(
          "genesis temporal generation authority is missing",
        ))?;
        let handle = Self::plan_deadline_destination(actor, WakeupKey::Tick(due_tick))
          .map_err(TemporalOccurrenceError::Deadline)?;
        let Some(ActorSemanticState::Active(current)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(
            DispatchError::Other("genesis temporal semantic authority is missing").into(),
          );
        };
        if current.generation != actor.generation
          || current.identity != state.identity
          || current.hot != state.hot
          || current.admission != admission
        {
          return Err(
            DispatchError::Other("genesis temporal semantic authority is corrupt").into(),
          );
        }
        let mut replacement = current.clone();
        replacement.hot.trigger_runtime_state = initialized;
        replacement.hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
          tick: due_tick,
          page_id: handle.page,
          slot: u32::from(handle.slot),
        });
        ActorSemanticStates::<T>::insert(actor_id, ActorSemanticState::Active(replacement));
        Self::insert_trigger_deadline_member(handle).map_err(TemporalOccurrenceError::Deadline)?;
        return Ok(false);
      }
      state.hot.trigger_runtime_state = initialized;
      Self::try_store_control_hot_with_authority(actor_id, state.hot.clone())
        .map_err(|_| DispatchError::Other("genesis temporal authority update failed"))?;
      let placement = {
        Self::try_wakeup_substrate_schedule_transition_with_authority(
          actor_id,
          WakeupKey::Tick(due_tick),
          state.hot.clone(),
          &state.identity,
          state.run_state.as_ref().map_or(0, |run| run.cursor),
          &admission,
          resources,
        )
      };
      if let Err(error) = placement {
        if !Self::scheduler_index_is_exhausted(error) {
          return Err(DispatchError::Other("genesis temporal placement failed").into());
        }
        Self::finalize_actor_from_retained_state(
          actor_id,
          state.clone(),
          &admission,
          CloseReason::SchedulerIndexExhausted,
        )?;
        return Ok(true);
      }
      return Ok(false);
    }
    let anchor_tick = state
      .hot
      .trigger_runtime_state
      .temporal_anchor_tick()
      .ok_or(DispatchError::Other(
        "initialized temporal anchor is missing",
      ))?;
    match state.hot.trigger_runtime_state {
      TriggerRuntimeState::AtTime {
        anchor_tick,
        consumed: false,
      } => {
        let Some(ActorSemanticState::Active(current)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(DispatchError::Other("AtTime semantic authority is missing").into());
        };
        if current.identity != state.identity
          || current.hot != state.hot
          || current.admission != admission
        {
          return Err(DispatchError::Other("AtTime semantic authority is corrupt").into());
        }
        let mut replacement = current.clone();
        replacement.hot.trigger_runtime_state = TriggerRuntimeState::AtTime {
          anchor_tick,
          consumed: true,
        };
        Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected: ActorSemanticState::Active(current),
            replacement: ActorSemanticState::Active(replacement),
          },
        )
        .map_err(|_| DispatchError::Other("AtTime progression commit failed"))?;
      }
      TriggerRuntimeState::Cadenced { .. } => {
        let next_due_tick = next_cadence_due_tick(anchor_tick, delay_ticks, now_tick)
          .ok_or(DispatchError::Other("cadence deadline failed"))?;
        let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
          && ActorProcesses::<T>::contains_key(actor_id);
        if canonical {
          let actor = Self::load_actor_ref(actor_id).ok_or(DispatchError::Other(
            "cadence generation authority is missing",
          ))?;
          let handle = Self::plan_deadline_destination(actor, WakeupKey::Tick(next_due_tick))
            .map_err(TemporalOccurrenceError::Deadline)?;
          let Some(ActorSemanticState::Active(current)) = ActorSemanticStates::<T>::get(actor_id)
          else {
            return Err(DispatchError::Other("cadence semantic authority is missing").into());
          };
          if current.generation != actor.generation
            || current.identity != state.identity
            || current.hot != state.hot
            || current.admission != admission
          {
            return Err(DispatchError::Other("cadence semantic authority is corrupt").into());
          }
          let mut replacement = current.clone();
          replacement.hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
            tick: next_due_tick,
            page_id: handle.page,
            slot: u32::from(handle.slot),
          });
          ActorSemanticStates::<T>::insert(actor_id, ActorSemanticState::Active(replacement));
          Self::insert_trigger_deadline_member(handle)
            .map_err(TemporalOccurrenceError::Deadline)?;
        } else {
          let placement = Self::try_wakeup_substrate_schedule_transition_with_authority(
            actor_id,
            WakeupKey::Tick(next_due_tick),
            state.hot.clone(),
            &state.identity,
            state.run_state.as_ref().map_or(0, |run| run.cursor),
            &admission,
            resources,
          );
          if let Err(error) = placement {
            if !Self::scheduler_index_is_exhausted(error) {
              return Err(DispatchError::Other("cadence rearm failed").into());
            }
            Self::finalize_actor_from_retained_state(
              actor_id,
              state.clone(),
              &admission,
              CloseReason::SchedulerIndexExhausted,
            )?;
            return Ok(true);
          }
        }
      }
      TriggerRuntimeState::AtTime { consumed: true, .. } | TriggerRuntimeState::Stateless => {
        return Err(DispatchError::Other("temporal runtime state is incompatible").into());
      }
    }
    Self::reconcile_actor_state_hold_with_authority(actor_id)
      .map_err(|_| DispatchError::Other("temporal state hold reconciliation failed"))?;
    let loaded_state = Self::load_actor_service_state_with_authority(actor_id);
    let Some((state, _admission, _)) = loaded_state else {
      return Err(DispatchError::Other("temporal progression state is corrupt").into());
    };
    if matches!(
      state.hot.cycle_state,
      CycleState::Running | CycleState::Suspended
    ) {
      return Ok(false);
    }
    let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
      && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      && ActorProcesses::<T>::contains_key(actor_id);
    // Fresh genesis publishes canonical Actors only, so a temporal occurrence that is not
    // canonically owned is an incoherent pre-cutover carrier state rather than a supported
    // legacy activation path. Canonical publication remains the sole temporal placement owner.
    if !canonical {
      return Err(DispatchError::Other("temporal owner is not canonically published").into());
    }
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let actor = Self::load_actor_ref(actor_id).ok_or(DispatchError::Other(
      "temporal generation authority is missing",
    ))?;
    let classification =
      Self::classify_actor_loaded(&instance, state.run_state.as_ref()).map_err(|error| {
        TemporalOccurrenceError::Dispatch(Self::classification_dispatch_error(error).into())
      })?;
    if let Some(reason) = classification.terminal_reason {
      Self::remove_actor_publication_and_finalize(actor, state, None, reason)
        .map_err(|_| DispatchError::Other("temporal terminal substitution failed"))?;
      return Ok(true);
    }
    let actor_type = state.identity.actor_class.actor_type();
    let breakdown = Self::trigger_fee_for_actor(
      actor_id,
      actor_type,
      state.contract.on_unfunded,
      trigger_family,
      occurrence_weight,
    )
    .map_err(DispatchError::from)?;
    if trigger_family == TriggerFamily::AtTime {
      let temporal_capacity = Self::trigger_occurrence_capacity_sufficient(
        actor_type,
        &state.identity.sovereign_account,
        breakdown,
      )
      .map_err(|_| DispatchError::Other("temporal capacity calculation failed"))?;
      if !temporal_capacity {
        if state.contract.on_unfunded == OnUnfunded::AwaitFunding {
          return Err(TemporalOccurrenceError::Dispatch(
            Error::<T>::AwaitFundingUnavailable.into(),
          ));
        }
        Self::remove_actor_publication_and_finalize(
          actor,
          state,
          None,
          CloseReason::TriggerAdmissionInsufficient,
        )
        .map_err(|_| DispatchError::Other("underfunded temporal apoptosis failed"))?;
        return Ok(true);
      }
    }
    let sovereign_account = state.identity.sovereign_account.clone();
    match Self::commit_canonical_trigger_occurrence_with_authority(
      actor,
      actor_type,
      &sovereign_account,
      breakdown,
      state,
      frame_system::Pallet::<T>::block_number(),
    ) {
      Ok(_) => Ok(false),
      Err(error)
        if trigger_family == TriggerFamily::Cadenced
          && error == Error::<T>::InsufficientFee.into() =>
      {
        Ok(false)
      }
      Err(error) => Err(error.into()),
    }
  }

  pub(crate) fn current_scheduler_tick() -> Result<SchedulerTick, EnqueueOutcome> {
    scheduler_tick_floor(
      <T::Time as polkadot_sdk::frame_support::traits::Time>::now(),
      T::CadenceTickMillis::get(),
    )
    .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
  }

  pub(crate) fn temporal_anchor_tick(
    trigger: &TriggerOf<T>,
  ) -> Result<Option<SchedulerTick>, EnqueueOutcome> {
    if !matches!(trigger, Trigger::AtTime { .. } | Trigger::Cadenced { .. }) {
      return Ok(None);
    }
    scheduler_tick_ceil(
      <T::Time as polkadot_sdk::frame_support::traits::Time>::now(),
      T::CadenceTickMillis::get(),
    )
    .map(Some)
    .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
  }

  /// The Active-epoch block anchor. Set to the current block (clamped to window
  /// start) at Active installation and schedule replacement; reactivation with
  /// `cycle_nonce > 0` uses it as the conservative cooldown anchor when no
  /// active-epoch `last_cycle_block` exists.
  pub(crate) fn schedule_anchor_at(
    schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    now: BlockNumberFor<T>,
  ) -> BlockNumberFor<T> {
    schedule_window
      .map(|window| now.max(window.start))
      .unwrap_or(now)
  }

  fn next_eligible_at(
    instance: &ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let cooldown_anchor = instance
      .last_cycle_block
      .unwrap_or(instance.schedule_anchor);
    let cooldown_eligible_at = if instance.cycle_nonce == 0 && instance.last_cycle_block.is_none() {
      instance.schedule_anchor
    } else {
      cooldown_anchor
        .checked_add(&instance.cooldown_blocks.into())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?
    };
    let window_floor = instance
      .window
      .map(|window| window.start)
      .unwrap_or_else(Zero::zero);
    Ok(now.max(cooldown_eligible_at).max(window_floor))
  }

  /// A paid Idle latch that cannot be consumed next round owns a future Block deadline,
  /// clamped by independent window expiry. It must not poll Service during the known delay.
  pub(crate) fn latched_idle_deadline(
    instance: &ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<Option<BlockNumberFor<T>>, EnqueueOutcome> {
    if instance.cycle_state != CycleState::Idle || !instance.pending_signal {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let eligible_at = Self::next_eligible_at(instance, now)?;
    let due = Self::window_expiry_wakeup(instance)
      .map_or(eligible_at, |terminal| eligible_at.min(terminal));
    let next_round = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    Ok((due > next_round).then_some(due))
  }

  pub(crate) fn retry_backoff_blocks(cursor_local_attempt: u32) -> u32 {
    1u32
      .checked_shl(cursor_local_attempt)
      .unwrap_or(MAX_RETRY_BACKOFF_BLOCKS)
      .min(MAX_RETRY_BACKOFF_BLOCKS)
  }

  #[cfg(test)]
  pub(crate) fn retry_eligible_at(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let run_state =
      ActorRunStateStore::<T>::get(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::retry_eligible_at_loaded(instance, &run_state)
  }

  pub(crate) fn suspension_eligible_at(
    cooldown_blocks: u32,
    window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    last_attempt_block: BlockNumberFor<T>,
    unsuccessful_attempts_at_cursor: u32,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let cooldown: BlockNumberFor<T> = cooldown_blocks.into();
    let cursor_local_attempt = unsuccessful_attempts_at_cursor.saturating_sub(1);
    let backoff: BlockNumberFor<T> = Self::retry_backoff_blocks(cursor_local_attempt).into();
    let retry_delay = cooldown.max(backoff);
    let mut eligible_at = last_attempt_block
      .checked_add(&retry_delay)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if let Some(window) = window {
      eligible_at = eligible_at
        .max(window.start)
        .min(Self::window_terminal_at(&window));
    }
    Ok(eligible_at)
  }

  fn retry_eligible_at_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: &ActorRunStateOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let expected = Self::suspension_eligible_at(
      instance.cooldown_blocks,
      instance.window,
      run_state.last_attempt_block,
      run_state.unsuccessful_attempts_at_cursor,
    )?;
    if run_state.eligible_at != expected {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(run_state.eligible_at)
  }

  pub(crate) fn idle_window_placement_weight(contract: &ActorContractOf<T>) -> Weight {
    contract.window.map_or(Weight::zero(), |_| {
      T::WeightInfo::service_member_to_deadline_new_key()
    })
  }

  /// A committed Cycle owns its final placement. Quiet Idle work keeps detector authority but
  /// leaves the Service ring, using the same passive plan as fresh Active publication.
  fn place_idle_service_after_cycle(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepControlPlacement, AttemptTransactionError> {
    if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return Err(AttemptTransactionError::Invariant);
    }
    let (placement, disabled) = match Self::prepare_idle_service_placement(actor, state, now)? {
      PreparedIdleServicePlacement::Disabled {
        process,
        disablement,
      } => (StepControlPlacement::None, Some((process, disablement))),
      PreparedIdleServicePlacement::Deadline { at } => {
        let destination = Self::plan_deadline_destination(actor, WakeupKey::Block(at))
          .map_err(AttemptTransactionError::Deadline)?;
        Self::transfer_service_member_to_deadline(actor, destination)
          .map_err(AttemptTransactionError::Deadline)?;
        (StepControlPlacement::Wakeup, None)
      }
      PreparedIdleServicePlacement::Parked {
        kind,
        activation,
        review_at,
      } => {
        Self::transfer_service_member_to_parked_balance(
          actor,
          kind,
          state.identity.cycle_nonce,
          activation,
          WakeupKey::Block(review_at),
        )
        .map_err(AttemptTransactionError::ParkedBalance)?;
        (StepControlPlacement::Wakeup, None)
      }
    };
    Self::finish_idle_service_placement(actor, now, disabled)?;
    Ok(placement)
  }

  /// Read-only preparation; the mutating wrapper retains its mandatory transaction guard.
  pub(crate) fn prepare_idle_service_placement<'a>(
    actor: ActorRef,
    state: &'a ActiveActorStateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<PreparedIdleServicePlacement<'a, T>, AttemptTransactionError> {
    if state.hot.cycle_state != CycleState::Idle
      || state.hot.pending_signal
      || state.run_state.is_some()
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let process =
      ActorProcesses::<T>::get(actor.actor_id).ok_or(AttemptTransactionError::Invariant)?;
    let Some(ProcessResidence::Service(kind)) = process.residence else {
      return Err(AttemptTransactionError::Invariant);
    };
    if process.generation != actor.generation || process.status != ProcessStatus::Serving {
      return Err(AttemptTransactionError::Invariant);
    }
    if let Some(activation) = &state.contract.parked_balance_activation {
      let review_at = now
        .checked_add(&One::one())
        .ok_or(AttemptTransactionError::Invariant)?;
      return Ok(PreparedIdleServicePlacement::Parked {
        kind,
        activation,
        review_at,
      });
    }
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    // The guarded unlatched Idle branch returns before cutoff-dependent admission.
    match Self::plan_next_work_loaded(&instance, None, now, ServiceCutoff::Open)
      .map_err(|_| AttemptTransactionError::Invariant)?
    {
      NextWorkPlan::Disabled(disablement) => Ok(PreparedIdleServicePlacement::Disabled {
        process,
        disablement,
      }),
      NextWorkPlan::Wakeup(at) => Ok(PreparedIdleServicePlacement::Deadline { at }),
      NextWorkPlan::Service(_) => Err(AttemptTransactionError::Invariant),
    }
  }

  /// Retires only Disabled destinations, then stamps the current process after any transfer.
  /// The caller owns the enclosing attempt transaction; no cached Service record overwrites Park
  /// or Deadline residence.
  pub(crate) fn finish_idle_service_placement(
    actor: ActorRef,
    now: BlockNumberFor<T>,
    disabled: Option<(ActorProcessOf<T>, ProcessDisablement<BlockNumberFor<T>>)>,
  ) -> Result<(), AttemptTransactionError> {
    if let Some((mut process, disablement)) = disabled {
      Self::remove_service_member(actor).map_err(|_| AttemptTransactionError::Invariant)?;
      process.status = ProcessStatus::Disabled(disablement);
      process.residence = None;
      ActorProcesses::<T>::insert(actor.actor_id, process);
    }
    ActorProcesses::<T>::try_mutate(actor.actor_id, |process| {
      let process = process.as_mut().ok_or(AttemptTransactionError::Invariant)?;
      if process.generation != actor.generation {
        return Err(AttemptTransactionError::Invariant);
      }
      process.last_attempted = Some(now);
      Ok(())
    })
  }

  fn plan_next_work_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<NextWorkPlan<BlockNumberFor<T>>, EnqueueOutcome> {
    let disabled_basis = || match instance.cycle_state {
      CycleState::Idle => Ok(SuspendedProcessBasis::Idle),
      CycleState::Running => {
        let run = run_state.ok_or(EnqueueOutcome::CorruptedTopology)?;
        if !run.running_is_coherent() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        Ok(SuspendedProcessBasis::Running {
          eligible_at: run.eligible_at,
        })
      }
      CycleState::Suspended => {
        let run = run_state.ok_or(EnqueueOutcome::CorruptedTopology)?;
        Ok(SuspendedProcessBasis::Suspended {
          not_before: Self::retry_eligible_at_loaded(instance, run)?,
        })
      }
    };
    if instance.lifecycle.is_paused() {
      return Self::window_expiry_wakeup(instance).map_or_else(
        || {
          Ok(NextWorkPlan::Disabled(ProcessDisablement {
            cause: ProcessDisableCause::OwnerPaused,
            revival_authority: ProcessRevivalAuthority::Owner,
            basis: disabled_basis()?,
          }))
        },
        |at| Ok(NextWorkPlan::Wakeup(at)),
      );
    }
    let (eligible_at, kind) = if matches!(
      instance.cycle_state,
      CycleState::Running | CycleState::Suspended
    ) {
      let run = run_state.ok_or(EnqueueOutcome::CorruptedTopology)?;
      let eligible_at = if instance.cycle_state == CycleState::Running {
        if !run.running_is_coherent() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        run.eligible_at
      } else {
        Self::retry_eligible_at_loaded(instance, run)?
      };
      (eligible_at, ServiceResidenceKind::Live)
    } else if instance.pending_signal {
      (
        Self::next_eligible_at(instance, now)?,
        ServiceResidenceKind::Pending,
      )
    } else {
      return Self::window_expiry_wakeup(instance).map_or_else(
        || {
          Ok(NextWorkPlan::Disabled(ProcessDisablement {
            cause: ProcessDisableCause::Protocol,
            revival_authority: ProcessRevivalAuthority::Protocol,
            basis: SuspendedProcessBasis::Idle,
          }))
        },
        |at| Ok(NextWorkPlan::Wakeup(at)),
      );
    };
    let wakeup_at = instance.window.map_or(eligible_at, |window| {
      eligible_at.min(Self::window_terminal_at(&window))
    });
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    Ok(
      if wakeup_at < exact_next_block || wakeup_at == exact_next_block && cutoff.is_snapshotted() {
        NextWorkPlan::Service(kind)
      } else {
        NextWorkPlan::Wakeup(wakeup_at)
      },
    )
  }

  fn plan_process_destination(
    actor: ActorRef,
    plan: NextWorkPlan<BlockNumberFor<T>>,
    now: BlockNumberFor<T>,
    last_attempted: Option<BlockNumberFor<T>>,
  ) -> Result<PlannedProcessDestination<BlockNumberFor<T>>, EnqueueOutcome> {
    let process = |status, residence| ActorProcess {
      generation: actor.generation,
      last_attempted,
      status,
      residence,
    };
    match plan {
      NextWorkPlan::Disabled(disablement) => Ok(PlannedProcessDestination::Disabled(process(
        ProcessStatus::Disabled(disablement),
        None,
      ))),
      NextWorkPlan::Service(kind) => {
        if ActorProcesses::<T>::contains_key(actor.actor_id)
          || ServiceNodes::<T>::contains_key(actor.actor_id)
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let header = ServiceHeader::<T>::get();
        if header.count >= T::MaxActiveActors::get() {
          return Err(EnqueueOutcome::CapacityUnavailable);
        }
        match (header.count, header.cursor) {
          (0, None) => {}
          (0, Some(_)) | (_, None) => return Err(EnqueueOutcome::CorruptedTopology),
          (_, Some(cursor)) => {
            let head = ServiceNodes::<T>::get(cursor.actor_id)
              .filter(|node| node.generation == cursor.generation)
              .ok_or(EnqueueOutcome::CorruptedTopology)?;
            let tail = ServiceNodes::<T>::get(head.previous.actor_id)
              .filter(|node| node.generation == head.previous.generation)
              .ok_or(EnqueueOutcome::CorruptedTopology)?;
            if tail.next != cursor {
              return Err(EnqueueOutcome::CorruptedTopology);
            }
          }
        }
        Ok(PlannedProcessDestination::Service {
          process: process(
            ProcessStatus::Serving,
            Some(ProcessResidence::Service(kind)),
          ),
          admission_round: now,
        })
      }
      NextWorkPlan::Wakeup(at) => {
        let handle = Self::plan_deadline_destination(actor, WakeupKey::Block(at))
          .map_err(EnqueueOutcome::from)?;
        Ok(PlannedProcessDestination::Deadline {
          process: process(
            ProcessStatus::Serving,
            Some(ProcessResidence::Deadline {
              key: handle.key,
              page: handle.page,
              slot: handle.slot,
            }),
          ),
          handle,
        })
      }
    }
  }

  fn plan_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<PlannedActorPublication<BlockNumberFor<T>>, EnqueueOutcome> {
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let process = if state.contract.parked_balance_activation.is_some()
      && !state.hot.lifecycle.is_paused()
      && state.hot.cycle_state == CycleState::Idle
      && !state.hot.pending_signal
      && supplied_run.is_none()
    {
      let review_at = now
        .checked_add(&One::one())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
      let review = Self::plan_deadline_destination(actor, WakeupKey::Block(review_at))
        .map_err(EnqueueOutcome::from)?;
      PlannedProcessDestination::Parked {
        process: ActorProcess {
          generation: actor.generation,
          last_attempted: None,
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Parked(ParkEvidence {
            plan_identity: admission.admission_identity,
            reason: ParkNegativeReason::ParkedBalanceBelowThreshold,
            review_at: Some(review_at),
          })),
        },
        review,
      }
    } else {
      let process_plan = Self::plan_next_work_loaded(&instance, supplied_run, now, cutoff)?;
      Self::plan_process_destination(actor, process_plan, now, None)?
    };
    let mut hot = state.hot.clone();
    let trigger_deadline = if instance.lifecycle.is_paused() {
      hot.trigger_wakeup_pointer = None;
      None
    } else {
      let trigger_tick = match instance.trigger_wakeup_pointer.as_ref() {
        Some(pointer) => Some(pointer.tick),
        None => Self::initial_trigger_wakeup_tick(&instance)?,
      };
      trigger_tick
        .map(|tick| {
          Self::plan_deadline_destination(actor, WakeupKey::Tick(tick)).inspect(|handle| {
            hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
              tick,
              page_id: handle.page,
              slot: u32::from(handle.slot),
            });
          })
        })
        .transpose()
        .map_err(EnqueueOutcome::from)?
    };
    Ok(PlannedActorPublication {
      hot,
      process,
      trigger_deadline,
      resources,
    })
  }

  /// Preflights every stable owner needed by a canonical publication. No canonical process,
  /// residence, or reverse handle may already exist. Generated current-Step resources and semantic
  /// generation are checked before the caller enters the transactional commit boundary.
  fn preflight_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<PlannedActorPublication<BlockNumberFor<T>>, EnqueueOutcome> {
    if actor.generation == 0
      || ActorProcesses::<T>::contains_key(actor.actor_id)
      || ServiceNodes::<T>::contains_key(actor.actor_id)
      || DeadlineHandles::<T>::contains_key(actor.actor_id)
      || TriggerDeadlineHandles::<T>::contains_key(actor.actor_id)
      || state.run_state.is_some() != supplied_run.is_some()
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<T>::get(actor.actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let admission = Self::canonical_control_admission(actor.actor_id, &state.contract)
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    if semantic.generation != actor.generation
      || semantic.identity != state.identity
      || semantic.hot != state.hot
      || semantic.admission != admission
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let expected_resources = if state.contract.steps.is_empty() {
      Self::zero_step_resource_envelope(&state.contract).ok_or(EnqueueOutcome::CorruptedTopology)?
    } else {
      let cursor = supplied_run.map_or(0, |run| run.cursor);
      Self::derive_step_resource_envelopes(&state.contract)
        .and_then(|envelopes| envelopes.get(cursor as usize).copied())
        .ok_or(EnqueueOutcome::CorruptedTopology)?
    };
    if resources != expected_resources {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Self::plan_actor_publication(
      actor,
      state,
      supplied_run,
      resources,
      now,
      cutoff,
      &admission,
    )
  }

  /// Atomically replaces one canonical publication with a validated lifecycle successor. The
  /// source semantic record and process residence remain authoritative until their exact carrier
  /// members have been removed; any refusal restores the complete canonical root.
  pub(crate) fn transition_actor_publication_to_successor(
    actor: ActorRef,
    source: &ActiveActorStateOf<T>,
    successor: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let result = (|| -> Result<(), EnqueueOutcome> {
        if ActorControlLocators::<T>::contains_key(actor.actor_id)
          || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
          || successor.contract != source.contract
          || successor.identity.sovereign_account != source.identity.sovereign_account
          || successor.identity.owner != source.identity.owner
          || successor.identity.actor_class != source.identity.actor_class
          || successor.identity.mutability != source.identity.mutability
          || successor.identity.cycle_nonce != source.identity.cycle_nonce
          || source.run_state.as_ref().map(|run| run.encode())
            != supplied_run.map(|run| run.encode())
          || successor.run_state.as_ref().map(|run| run.encode())
            != supplied_run.map(|run| run.encode())
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let admission = Self::build_admission_certificate(&source.contract)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let Some(ActorSemanticState::Active(mut semantic)) =
          ActorSemanticStates::<T>::get(actor.actor_id)
        else {
          return Err(EnqueueOutcome::CorruptedTopology);
        };
        if semantic.generation != actor.generation
          || semantic.identity != source.identity
          || semantic.hot != source.hot
          || semantic.admission != admission
          || Self::build_admission_certificate(&successor.contract) != Some(admission.clone())
          || ActorRunStateStore::<T>::get(actor.actor_id)
            .as_ref()
            .map(|run| run.encode())
            != supplied_run.map(|run| run.encode())
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let process = ActorProcesses::<T>::get(actor.actor_id)
          .filter(|process| process.generation == actor.generation)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;

        if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
          Self::remove_trigger_deadline_member(actor)
            .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        } else if source.hot.trigger_wakeup_pointer.is_some() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        match process.residence {
          Some(ProcessResidence::Service(_)) => {
            Self::remove_service_member(actor).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          }
          Some(ProcessResidence::Deadline { .. }) => {
            Self::remove_deadline_member(actor).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          }
          Some(ProcessResidence::Parked(evidence)) => {
            Self::release_parked_dependency_authority(actor, evidence)
              .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          }
          None if matches!(process.status, ProcessStatus::Disabled(_)) => {}
          None => return Err(EnqueueOutcome::CorruptedTopology),
        }
        ActorProcesses::<T>::remove(actor.actor_id);
        semantic.identity = successor.identity.clone();
        semantic.hot = successor.hot.clone();
        ActorSemanticStates::<T>::insert(actor.actor_id, ActorSemanticState::Active(semantic));
        Self::publish_actor_publication(actor, successor, supplied_run, resources, now, cutoff)
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  /// Atomically removes one exact canonical active publication and converges on the existing
  /// custody-neutral terminal finalizer. Trigger and process residence are independent; a refusal
  /// after either removal restores the complete storage root.
  /// Releases one canonical generation-bound process publication and its exact residence without
  /// finalizing the durable identity. On success the semantic Active record owns the released Hot
  /// state, `ActorProcesses` and its Service/Deadline/Trigger residence are gone, and no legacy
  /// authority was created. The caller owns the enclosing storage transaction.
  fn detach_actor_publication_inner(
    actor: ActorRef,
    state: ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
  ) -> Result<(ActiveActorStateOf<T>, ActorAdmissionCertificateOf<T>), DispatchError> {
    ensure!(
      !ActorControlLocators::<T>::contains_key(actor.actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id),
      Error::<T>::ActorInvariant
    );
    let admission = if state.contract.close_when.is_some()
      || state.contract.on_unfunded == OnUnfunded::AwaitFunding
    {
      let (contract, admission) =
        Self::load_admitted_contract_geometry(actor.actor_id).ok_or(Error::<T>::ActorInvariant)?;
      ensure!(
        contract == state.contract && Self::admission_matches_current_authority(&admission),
        Error::<T>::ActorInvariant
      );
      ensure!(
        Self::close_control_weight_for_contract(&contract).is_some(),
        Error::<T>::BehaviorPolicyUnavailable
      );
      ensure!(
        state.contract.on_unfunded != OnUnfunded::AwaitFunding
          || Self::load_canonical_actor_semantic_state(actor).is_ok(),
        Error::<T>::ActorInvariant
      );
      admission
    } else {
      Self::build_admission_certificate(&state.contract).ok_or(Error::<T>::ActorInvariant)?
    };
    let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<T>::get(actor.actor_id)
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    ensure!(
      actor.generation != 0
        && semantic.generation == actor.generation
        && semantic.identity == state.identity
        && semantic.hot == state.hot
        && semantic.admission == admission
        && state.run_state.as_ref().map(|run| run.encode()) == supplied_run.map(|run| run.encode())
        && ActorRunStateStore::<T>::get(actor.actor_id)
          .as_ref()
          .map(|run| run.encode())
          == supplied_run.map(|run| run.encode()),
      Error::<T>::ActorInvariant
    );
    let process = ActorProcesses::<T>::get(actor.actor_id)
      .filter(|process| process.generation == actor.generation)
      .ok_or(Error::<T>::ActorInvariant)?;

    let mut terminal_state = state;
    if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
      Self::remove_trigger_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
      terminal_state.hot.trigger_wakeup_pointer = None;
    } else {
      ensure!(
        terminal_state.hot.trigger_wakeup_pointer.is_none(),
        Error::<T>::ActorInvariant
      );
    }
    match process.residence {
      Some(ProcessResidence::Service(_)) => {
        Self::remove_service_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
      }
      Some(ProcessResidence::Deadline { .. }) => {
        Self::remove_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
        terminal_state.hot.wakeup_pointer = None;
      }
      Some(ProcessResidence::Parked(evidence)) => {
        Self::release_parked_dependency_authority(actor, evidence)
          .map_err(|_| Error::<T>::ActorInvariant)?;
      }
      None if matches!(process.status, ProcessStatus::Disabled(_)) => {}
      _ => return Err(Error::<T>::ActorInvariant.into()),
    }
    ActorProcesses::<T>::remove(actor.actor_id);
    let Some(ActorSemanticState::Active(mut terminal_semantic)) =
      ActorSemanticStates::<T>::get(actor.actor_id)
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    terminal_semantic.hot = terminal_state.hot.clone();
    ActorSemanticStates::<T>::insert(
      actor.actor_id,
      ActorSemanticState::Active(terminal_semantic),
    );
    Ok((terminal_state, admission))
  }

  /// Atomically releases one canonical process publication and residence while preserving the
  /// durable identity, used by owner-initiated deactivation. Refusal restores the complete
  /// canonical root and never creates legacy control authority.
  pub(crate) fn detach_actor_publication(
    actor: ActorRef,
    state: ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
  ) -> Result<ActiveActorStateOf<T>, DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::detach_actor_publication_inner(actor, state, supplied_run) {
        Ok((terminal_state, _admission)) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(terminal_state))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  pub(crate) fn remove_actor_publication_and_finalize(
    actor: ActorRef,
    state: ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    reason: CloseReason,
  ) -> DispatchResult {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> DispatchResult {
        let (terminal_state, admission) =
          Self::detach_actor_publication_inner(actor, state, supplied_run)?;
        Self::finalize_actor_from_consumed_state(actor.actor_id, terminal_state, &admission, reason)
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  /// Atomically replaces one canonical running publication with its cancelled Idle successor.
  /// The running residence and any independent temporal Trigger deadline are released before the
  /// successor publication is planned and committed; refusal restores the complete canonical root
  /// and no legacy control authority is ever created. `source` still carries the open Run, while
  /// `successor` carries the same stable identity with `run_state` cleared and its Hot state set to
  /// the cancelled Idle form.
  pub(crate) fn cancel_actor_publication(
    actor: ActorRef,
    source: &ActiveActorStateOf<T>,
    successor: &ActiveActorStateOf<T>,
    authority: CancellationPublicationAuthority,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<(), EnqueueOutcome> {
    let retiring = matches!(authority, CancellationPublicationAuthority::Retiring(_));
    if retiring && !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let resources = match authority {
      CancellationPublicationAuthority::Current(resources)
      | CancellationPublicationAuthority::Retiring(resources) => resources,
    };
    with_transaction_opaque_err(|| {
      let result = (|| -> Result<(), EnqueueOutcome> {
        // The control loader deliberately reconstructs a bounded current-Step Contract whose body
        // commitment differs from the stored full body for a multi-Step Actor. Cancellation
        // republishes from the durable admitted geometry, so read the stored full Contract and use
        // it as the publication authority for the pre-cancel source and the Idle successor.
        let full_contract = if source.contract.close_when.is_some() {
          source.contract.clone()
        } else {
          Self::load_actor_contract(actor.actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?
        };
        let mut source = source.clone();
        source.contract = full_contract.clone();
        let mut successor = successor.clone();
        successor.contract = full_contract;
        let source = &source;
        let successor = &successor;
        if ActorControlLocators::<T>::contains_key(actor.actor_id)
          || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
          || successor.contract != source.contract
          || successor.identity.sovereign_account != source.identity.sovereign_account
          || successor.identity.owner != source.identity.owner
          || successor.identity.actor_class != source.identity.actor_class
          || successor.identity.mutability != source.identity.mutability
          || successor.run_state.is_some()
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let run = source
          .run_state
          .as_ref()
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        if successor.identity.cycle_nonce != run.cycle_nonce {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let admission = Self::canonical_control_admission_for_publication(
          actor.actor_id,
          &source.contract,
          !retiring,
        )
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
        if retiring {
          let head = ActorContractHeads::<T>::get(actor.actor_id)
            .ok_or(EnqueueOutcome::CorruptedTopology)?;
          if head.first_step_resources != Some(resources) {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
        }
        let Some(ActorSemanticState::Active(mut semantic)) =
          ActorSemanticStates::<T>::get(actor.actor_id)
        else {
          return Err(EnqueueOutcome::CorruptedTopology);
        };
        if semantic.generation != actor.generation
          || semantic.identity != source.identity
          || semantic.hot != source.hot
          || semantic.admission != admission
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        if let Some(stored) = ActorRunStateStore::<T>::get(actor.actor_id)
          && source.run_state.as_ref().map(|run| run.encode()) != Some(stored.encode())
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let process = ActorProcesses::<T>::get(actor.actor_id)
          .filter(|process| process.generation == actor.generation)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;

        if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
          Self::remove_trigger_deadline_member(actor)
            .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        } else if source.hot.trigger_wakeup_pointer.is_some() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        match process.residence {
          Some(ProcessResidence::Service(_)) => {
            Self::remove_service_member(actor).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          }
          Some(ProcessResidence::Deadline { .. }) => {
            Self::remove_deadline_member(actor).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          }
          Some(ProcessResidence::Parked(_)) => return Err(EnqueueOutcome::CorruptedTopology),
          None => return Err(EnqueueOutcome::CorruptedTopology),
        }
        ActorProcesses::<T>::remove(actor.actor_id);
        if source.contract.close_when.is_some()
          && source.contract.on_unfunded == OnUnfunded::AwaitFunding
        {
          Self::clear_behavior_cycle_for_cancellation(
            actor.actor_id,
            &admission,
            &successor.identity,
            &successor.hot,
            false,
          )
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        }
        ActorRunStateStore::<T>::remove(actor.actor_id);
        if source.contract.on_unfunded == OnUnfunded::AwaitFunding {
          crate::ActorFinancialWaits::<T>::remove(actor.actor_id);
        }
        semantic.identity = successor.identity.clone();
        semantic.hot = successor.hot.clone();
        ActorSemanticStates::<T>::insert(actor.actor_id, ActorSemanticState::Active(semantic));
        if retiring {
          let publication = Self::plan_actor_publication(
            actor, successor, None, resources, now, cutoff, &admission,
          )?;
          Self::commit_actor_publication(actor, successor, publication)
        } else {
          Self::publish_actor_publication(actor, successor, None, resources, now, cutoff)
        }
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  /// Atomically publishes semantic Hot state, one exclusive process residence and an optional
  /// independent temporal Trigger deadline for initial activation and lifecycle republishing.
  pub(crate) fn publish_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let result = (|| -> Result<(), EnqueueOutcome> {
        let publication =
          Self::preflight_actor_publication(actor, state, supplied_run, resources, now, cutoff)?;
        Self::commit_actor_publication(actor, state, publication)
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn commit_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    publication: PlannedActorPublication<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let Some(ActorSemanticState::Active(mut semantic)) =
      ActorSemanticStates::<T>::get(actor.actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    semantic.hot = publication.hot;
    ActorSemanticStates::<T>::insert(actor.actor_id, ActorSemanticState::Active(semantic));

    let process = match publication.process {
      PlannedProcessDestination::Disabled(process) => process,
      PlannedProcessDestination::Service { process, .. } => process,
      PlannedProcessDestination::Deadline { process, .. } => process,
      PlannedProcessDestination::Parked { process, .. } => process,
    };
    ActorProcesses::<T>::insert(actor.actor_id, process);

    match publication.process {
      PlannedProcessDestination::Disabled(_) => {}
      PlannedProcessDestination::Service {
        process,
        admission_round,
      } => {
        let Some(ProcessResidence::Service(kind)) = process.residence else {
          return Err(EnqueueOutcome::CorruptedTopology);
        };
        Self::insert_service_member(actor, kind, admission_round)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      }
      PlannedProcessDestination::Deadline { handle, .. } => {
        Self::insert_deadline_member(handle).map_err(EnqueueOutcome::from)?;
      }
      PlannedProcessDestination::Parked { review, .. } => {
        let activation = state
          .contract
          .parked_balance_activation
          .as_ref()
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        Self::arm_initial_parked_balance(actor, state.identity.cycle_nonce, activation, review.key)
          .map_err(|error| match error {
            ParkedBalanceTransitionError::Certification(_) => {
              EnqueueOutcome::InvalidParkedBalanceActivation
            }
            ParkedBalanceTransitionError::Dependency(
              DependencyRegistrationError::CapacityExceeded,
            ) => EnqueueOutcome::CapacityUnavailable,
            _ => EnqueueOutcome::CorruptedTopology,
          })?;
        Self::insert_deadline_member(review).map_err(EnqueueOutcome::from)?;
      }
    }
    if let Some(handle) = publication.trigger_deadline {
      Self::insert_trigger_deadline_member(handle).map_err(EnqueueOutcome::from)?;
    }
    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn test_plan_next_work_source(
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    now: BlockNumberFor<T>,
  ) -> Result<
    (
      StepControlPlacement,
      Option<BlockNumberFor<T>>,
      Option<ProcessDisablement<BlockNumberFor<T>>>,
      Option<ServiceResidenceKind>,
    ),
    EnqueueOutcome,
  > {
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    Self::plan_next_work_loaded(&instance, supplied_run, now, ServiceCutoff::Snapshotted).map(
      |plan| match plan {
        NextWorkPlan::Disabled(disablement) => {
          (StepControlPlacement::None, None, Some(disablement), None)
        }
        NextWorkPlan::Service(kind) => (StepControlPlacement::Queue, None, None, Some(kind)),
        NextWorkPlan::Wakeup(at) => (StepControlPlacement::Wakeup, Some(at), None, None),
      },
    )
  }

  #[cfg(test)]
  pub(crate) fn test_plan_process_destination(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    now: BlockNumberFor<T>,
  ) -> Result<
    (
      ActorProcessOf<T>,
      Option<BlockNumberFor<T>>,
      Option<DeadlineHandleOf<T>>,
    ),
    EnqueueOutcome,
  > {
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let plan =
      Self::plan_next_work_loaded(&instance, supplied_run, now, ServiceCutoff::Snapshotted)?;
    Self::plan_process_destination(actor, plan, now, None).map(|destination| match destination {
      PlannedProcessDestination::Disabled(process) => (process, None, None),
      PlannedProcessDestination::Service {
        process,
        admission_round,
      } => (process, Some(admission_round), None),
      PlannedProcessDestination::Deadline { process, handle } => (process, None, Some(handle)),
      PlannedProcessDestination::Parked { process, review } => (process, None, Some(review)),
    })
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::preflight_actor_publication(
      actor,
      state,
      supplied_run,
      resources,
      now,
      ServiceCutoff::Snapshotted,
    )
    .map(|_| ())
  }

  #[cfg(test)]
  pub(crate) fn test_transition_actor_publication_to_successor(
    actor: ActorRef,
    source: &ActiveActorStateOf<T>,
    successor: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::transition_actor_publication_to_successor(
      actor,
      source,
      successor,
      supplied_run,
      resources,
      now,
      ServiceCutoff::Snapshotted,
    )
  }

  #[cfg(test)]
  pub(crate) fn test_remove_actor_publication_and_finalize(
    actor: ActorRef,
    state: ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    reason: CloseReason,
  ) -> DispatchResult {
    Self::remove_actor_publication_and_finalize(actor, state, supplied_run, reason)
  }

  #[cfg(test)]
  pub(crate) fn test_publish_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::publish_actor_publication(
      actor,
      state,
      supplied_run,
      resources,
      now,
      ServiceCutoff::Snapshotted,
    )
  }

  #[cfg(test)]
  pub(crate) fn test_plan_actor_publication(
    actor: ActorRef,
    state: &ActiveActorStateOf<T>,
    supplied_run: Option<&ActorRunStateOf<T>>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
  ) -> Result<
    (
      ActorHotStateOf<T>,
      ActorProcessOf<T>,
      Option<BlockNumberFor<T>>,
      Option<DeadlineHandleOf<T>>,
      Option<DeadlineHandleOf<T>>,
    ),
    EnqueueOutcome,
  > {
    let admission = Self::build_admission_certificate(&state.contract)
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::plan_actor_publication(
      actor,
      state,
      supplied_run,
      resources,
      now,
      ServiceCutoff::Snapshotted,
      &admission,
    )
    .map(|publication| {
      let (process, admission_round, process_deadline) = match publication.process {
        PlannedProcessDestination::Disabled(process) => (process, None, None),
        PlannedProcessDestination::Service {
          process,
          admission_round,
        } => (process, Some(admission_round), None),
        PlannedProcessDestination::Deadline { process, handle } => (process, None, Some(handle)),
        PlannedProcessDestination::Parked { process, review } => (process, None, Some(review)),
      };
      (
        publication.hot,
        process,
        admission_round,
        process_deadline,
        publication.trigger_deadline,
      )
    })
  }

  #[cfg(test)]
  fn schedule_next_work_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    loaded_authority: (
      &ActorHotStateOf<T>,
      &ActorIdentityOf<T>,
      Option<&ActorRunStateOf<T>>,
      &ActorAdmissionCertificateOf<T>,
      ActorStepResourceEnvelope,
    ),
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<StepControlPlacement, EnqueueOutcome> {
    match Self::plan_next_work_loaded(instance, loaded_authority.2, now, cutoff)? {
      NextWorkPlan::Disabled(_) => Ok(StepControlPlacement::None),
      NextWorkPlan::Service(_) => Ok(StepControlPlacement::Queue),
      NextWorkPlan::Wakeup(wakeup_at) => {
        let (hot, identity, run_state, admission, resources) = loaded_authority;
        Self::defer_wakeup_with_authority(
          actor_id,
          wakeup_at,
          instance,
          hot.clone(),
          identity,
          run_state,
          admission,
          resources,
        )
        .map(|()| StepControlPlacement::Wakeup)
      }
    }
  }

  #[cfg(test)]
  pub(crate) fn test_schedule_next_work_source(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
    supplied_run: Option<Option<&ActorRunStateOf<T>>>,
    now: BlockNumberFor<T>,
  ) -> Result<(StepControlPlacement, Vec<ActorId>), EnqueueOutcome> {
    let retained;
    let (state, admission, resources, run) = match supplied_run {
      Some(run) => (state, admission, resources, run),
      None => {
        retained = Self::load_frame_actor_service_state(actor_id)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let (state, admission, loaded_step) = &retained;
        let resources = if state.contract.steps.is_empty() {
          Self::zero_step_resource_envelope(&state.contract)
            .ok_or(EnqueueOutcome::CorruptedTopology)?
        } else {
          loaded_step
            .as_ref()
            .ok_or(EnqueueOutcome::CorruptedTopology)?
            .resources
        };
        (state, admission, resources, state.run_state.as_ref())
      }
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let authority = (&state.hot, &state.identity, run, admission, resources);
    let placement = Self::schedule_next_work_loaded(
      actor_id,
      &instance,
      authority,
      now,
      ServiceCutoff::Snapshotted,
    )?;
    let requeues = if placement == StepControlPlacement::Queue {
      vec![actor_id]
    } else {
      Vec::new()
    };
    Ok((placement, requeues))
  }

  pub(crate) fn is_window_expired(instance: &ActiveActorViewOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    instance
      .window
      .map(|window| now > window.end)
      .unwrap_or(false)
  }

  pub(crate) fn classification_dispatch_error(error: ActorClassificationError) -> Error<T> {
    match error {
      ActorClassificationError::ActorInvariant => Error::<T>::ActorInvariant,
      ActorClassificationError::RunInvariant => Error::<T>::ActorRunInvariant,
      ActorClassificationError::ComputationOverflow => Error::<T>::ComputationOverflow,
    }
  }

  /// Reuse authenticated optional Contract authority without opening public admission.
  fn canonical_control_admission(
    actor_id: ActorId,
    contract: &ActorContractOf<T>,
  ) -> Option<ActorAdmissionCertificateOf<T>> {
    Self::canonical_control_admission_for_publication(actor_id, contract, true)
  }

  fn canonical_control_admission_for_publication(
    actor_id: ActorId,
    contract: &ActorContractOf<T>,
    require_current: bool,
  ) -> Option<ActorAdmissionCertificateOf<T>> {
    if contract.close_when.is_none() && contract.on_unfunded == OnUnfunded::Close {
      return Self::build_admission_certificate(contract);
    }
    let ActorSemanticState::Active(record) = ActorSemanticStates::<T>::get(actor_id)? else {
      return None;
    };
    let admission = record.admission;
    let head = ActorContractHeads::<T>::get(actor_id)?;
    // The caller retains a loaded body across provisional monitor/Run mutations. Revalidate its
    // durable identity without reloading a monitor whose cancellation is not yet committed.
    (admission.has_valid_identity()
      && admission.semantic_contract_id == contract.semantic_contract_id()
      && admission.body_commitment == contract.body_commitment()?
      && head.header.semantic_contract_id == admission.semantic_contract_id
      && head.header.body_commitment == admission.body_commitment
      && head.header.admission_identity == admission.admission_identity
      && Self::admission_authorizes_contract_wake(&admission, contract)
      && (!require_current || Self::admission_matches_current_authority(&admission))
      && Self::close_control_weight_for_contract(contract).is_some())
    .then_some(admission)
  }

  pub(crate) fn expiry_substitution_due_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
  ) -> Result<bool, Error<T>> {
    Self::classify_actor_lifecycle_loaded(actor_id, instance, run_state)
      .map(|classification| classification.terminal_reason == Some(CloseReason::WindowExpired))
      .map_err(Self::classification_dispatch_error)
  }

  pub(crate) fn classify_actor_lifecycle_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let unpaid_trigger_due = if instance.on_unfunded == OnUnfunded::AwaitFunding {
      match crate::ActorFinancialWaits::<T>::get(actor_id) {
        Some(episode) => {
          if episode.record().scope().actor.actor_id != actor_id {
            return Err(ActorClassificationError::ActorInvariant);
          }
          let (_, process) =
            Self::load_canonical_actor_semantic_state(episode.record().scope().actor)
              .map_err(|_| ActorClassificationError::ActorInvariant)?;
          let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
            return Err(ActorClassificationError::ActorInvariant);
          };
          let current = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
          if process.status != ProcessStatus::AwaitingFunding
            || current != *instance
            || state.run_state.as_ref().map(Encode::encode) != run_state.map(Encode::encode)
          {
            return Err(ActorClassificationError::ActorInvariant);
          }
          match episode.record().unpaid_service() {
            crate::UnpaidActorService::AtTimeTrigger { due_tick } => Some(due_tick),
            _ => None,
          }
        }
        None => None,
      }
    } else {
      None
    };
    Self::classify_actor_loaded_with_unpaid_trigger(instance, run_state, unpaid_trigger_due)
  }

  #[cfg(test)]
  pub(crate) fn classify_actor(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let run_state = ActorRunStateStore::<T>::get(actor_id);
    Self::classify_actor_loaded(instance, run_state.as_ref())
  }

  pub(crate) fn classify_actor_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let cursor = run_state.as_ref().map_or(0, |state| state.cursor as usize);
    Self::classify_actor_at_current_step(instance, run_state, instance.steps.get(cursor))
  }

  /// The financial caller authenticates the owed AtTime point instead of a removed Trigger member.
  pub(crate) fn classify_actor_loaded_with_unpaid_trigger(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    unpaid_trigger_due: Option<SchedulerTick>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let cursor = run_state.as_ref().map_or(0, |state| state.cursor as usize);
    Self::classify_actor_at_current_step_with_unpaid_trigger(
      instance,
      run_state,
      instance.steps.get(cursor),
      unpaid_trigger_due,
    )
  }

  fn classify_actor_at_current_step(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    current_step: Option<&StepOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    Self::classify_actor_at_current_step_with_unpaid_trigger(
      instance,
      run_state,
      current_step,
      None,
    )
  }

  fn classify_actor_at_current_step_with_unpaid_trigger(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    current_step: Option<&StepOf<T>>,
    unpaid_trigger_due: Option<SchedulerTick>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    match (instance.cycle_state, run_state) {
      (CycleState::Idle, None) => {}
      (CycleState::Running | CycleState::Suspended, Some(state)) => {
        let expected_cycle_nonce = instance
          .cycle_nonce
          .checked_add(1)
          .ok_or(ActorClassificationError::RunInvariant)?;
        if current_step.is_none() || state.cycle_nonce != expected_cycle_nonce {
          return Err(ActorClassificationError::RunInvariant);
        }
        if instance.cycle_state == CycleState::Running {
          if state.unsuccessful_attempts_at_cursor != 0 || !state.running_is_coherent() {
            return Err(ActorClassificationError::RunInvariant);
          }
        } else if state.unsuccessful_attempts_at_cursor == 0
          || !state.suspension_is_coherent()
          || current_step
            .and_then(|step| step.on_error.retry_max_attempts())
            .is_none()
        {
          return Err(ActorClassificationError::RunInvariant);
        }
      }
      _ => return Err(ActorClassificationError::RunInvariant),
    }

    let terminal_reason = if Self::is_window_expired(instance) {
      Some(CloseReason::WindowExpired)
    } else if instance.cycle_state == CycleState::Idle && instance.cycle_nonce == u64::MAX {
      Some(CloseReason::CycleNonceExhausted)
    } else if run_state.as_ref().is_some_and(|state| {
      current_step
        .and_then(|step| step.on_error.retry_max_attempts())
        .is_some_and(|max_attempts| state.unsuccessful_attempts_at_cursor >= max_attempts)
    }) {
      Some(CloseReason::RetryAttemptsExhausted)
    } else if Self::failure_limit_reached(instance.unsuccessful_attempt_streak) {
      Some(CloseReason::ConsecutiveFailures)
    } else if instance.cycle_state == CycleState::Idle
      && instance
        .auto_close_at_cycle_nonce
        .is_some_and(|target| instance.cycle_nonce >= target)
    {
      Some(CloseReason::AutoCloseNonceReached)
    } else {
      None
    };

    let execution_phase = if GlobalCircuitBreaker::<T>::get() {
      ActorExecutionPhase::GlobalCircuitBreaker
    } else if instance.lifecycle.is_paused() {
      ActorExecutionPhase::Paused
    } else if terminal_reason.is_some() {
      ActorExecutionPhase::Ready
    } else if instance.cycle_state == CycleState::Running {
      let state = run_state.ok_or(ActorClassificationError::RunInvariant)?;
      let now = frame_system::Pallet::<T>::block_number();
      if state.eligible_at > now {
        ActorExecutionPhase::WaitingBlock(state.eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if instance.cycle_state == CycleState::Suspended {
      let eligible_at = Self::retry_eligible_at_loaded(
        instance,
        run_state.ok_or(ActorClassificationError::RunInvariant)?,
      )
      .map_err(|outcome| match outcome {
        EnqueueOutcome::SchedulerIndexExhausted => ActorClassificationError::ComputationOverflow,
        _ => ActorClassificationError::RunInvariant,
      })?;
      let now = frame_system::Pallet::<T>::block_number();
      if eligible_at > now {
        ActorExecutionPhase::WaitingRetry(eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if matches!(
      instance.trigger,
      Trigger::AtTime { .. } | Trigger::Cadenced { .. }
    ) {
      if instance.pending_signal {
        ActorExecutionPhase::Ready
      } else if instance.temporal_occurrence_consumed {
        ActorExecutionPhase::WaitingSignal
      } else {
        let Some(due_tick) = instance
          .trigger_wakeup_pointer
          .map(|pointer| pointer.tick)
          .or_else(|| {
            matches!(instance.trigger, Trigger::AtTime { .. })
              .then_some(unpaid_trigger_due)
              .flatten()
          })
        else {
          return Err(ActorClassificationError::ActorInvariant);
        };
        ActorExecutionPhase::WaitingCadenceTick(due_tick)
      }
    } else {
      let now = frame_system::Pallet::<T>::block_number();
      let eligible_at = Self::next_eligible_at(instance, now)
        .map_err(|_| ActorClassificationError::ComputationOverflow)?;
      if eligible_at > now {
        ActorExecutionPhase::WaitingBlock(eligible_at)
      } else if !instance.pending_signal {
        ActorExecutionPhase::WaitingSignal
      } else {
        ActorExecutionPhase::Ready
      }
    };
    Ok(ActorClassification {
      terminal_reason,
      execution_phase,
    })
  }

  /// Projects the canonical actor classifier without stripping temporal payloads.
  pub fn actor_eligibility(
    actor_id: ActorId,
  ) -> Result<ActorEligibility<BlockNumberFor<T>, T::Balance>, ActorClassificationError> {
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
      LoadedActorStateOf::NotRegistered => return Ok(ActorEligibility::NotRegistered),
      LoadedActorStateOf::Dormant(_) => return Ok(ActorEligibility::Dormant),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(ActorClassificationError::ActorInvariant),
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    // The strict state loader has already authenticated generation and every residence carrier.
    let process =
      ActorProcesses::<T>::get(actor_id).ok_or(ActorClassificationError::ActorInvariant)?;
    let residence = match (process.status, process.residence) {
      (
        ProcessStatus::Serving | ProcessStatus::AwaitingFunding,
        Some(ProcessResidence::Service(kind)),
      ) => ActorActivationResidence::Service(kind),
      (
        ProcessStatus::Serving | ProcessStatus::AwaitingFunding,
        Some(ProcessResidence::Deadline { key, .. }),
      ) => ActorActivationResidence::Deadline(key),
      (ProcessStatus::Serving, Some(ProcessResidence::Parked(evidence))) => {
        ActorActivationResidence::Parked {
          reason: evidence.reason,
          review_at: evidence.review_at,
        }
      }
      (ProcessStatus::Disabled(_), None) => ActorActivationResidence::Disabled,
      _ => return Err(ActorClassificationError::ActorInvariant),
    };
    let financial_wait = match (
      process.status,
      crate::ActorFinancialWaits::<T>::get(actor_id),
    ) {
      (ProcessStatus::AwaitingFunding, Some(episode)) => {
        let unpaid_liability = episode.record().unpaid_liability();
        let current_service_spendability = T::AssetOps::balance(
          &state.identity.sovereign_account,
          T::FeeNativeAssetId::get(),
        )
        .saturating_sub(T::MinUserBalance::get());
        Some(crate::FinancialWaitProjection {
          unpaid_service: episode.record().unpaid_service(),
          unpaid_liability,
          current_service_spendability,
          funding_shortfall: unpaid_liability.saturating_sub(current_service_spendability),
          entered_at_millis: episode.record().entered_at_millis(),
          expiry_tick: episode.record().expiry_tick(),
        })
      }
      (ProcessStatus::Serving | ProcessStatus::Disabled(_), None) => None,
      _ => return Err(ActorClassificationError::ActorInvariant),
    };
    let trigger = match &state.contract.trigger {
      Trigger::Manual => ActorTriggerActivation::Manual,
      Trigger::AddressEvent { .. } => ActorTriggerActivation::AddressEvent,
      Trigger::AtTime { after_ticks } => {
        let TriggerRuntimeState::AtTime { consumed, .. } = state.hot.trigger_runtime_state else {
          return Err(ActorClassificationError::ActorInvariant);
        };
        ActorTriggerActivation::AtTime {
          after_ticks: *after_ticks,
          consumed,
        }
      }
      Trigger::Cadenced { every_ticks } => ActorTriggerActivation::Cadenced {
        every_ticks: *every_ticks,
      },
    };
    Ok(ActorEligibility::Active(ActiveActorActivation {
      trigger,
      pending_signal: state.hot.pending_signal,
      residence,
      financial_wait,
      behavior_episode: Self::project_behavior_episode(actor_id, &state.contract)?,
      eligibility: Self::classify_actor_lifecycle_loaded(
        actor_id,
        &instance,
        state.run_state.as_ref(),
      )?,
    }))
  }

  fn source_matches_filter(
    filter: &SourceFilterOf<T>,
    owner: &T::AccountId,
    source: Option<&T::AccountId>,
  ) -> bool {
    match (filter, source) {
      (SourceFilter::Any, _) => true,
      (SourceFilter::OwnerOnly, Some(who)) => who == owner,
      (SourceFilter::OwnerOnly, None) => false,
      (SourceFilter::Whitelist(list), Some(who)) => list.contains(who),
      (SourceFilter::Whitelist(_), None) => false,
    }
  }

  fn asset_matches_filter(filter: &AssetFilterOf<T>, asset: T::AssetId) -> bool {
    match filter {
      AssetFilter::Any => true,
      AssetFilter::Whitelist(list) => list.contains(&asset),
    }
  }

  pub fn notify_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Signed;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
      TriggerCauseProvenance::ExternalPhase,
    )
  }

  pub fn notify_internal_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::InternalProtocol;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
      TriggerCauseProvenance::Deferred,
    )
  }

  pub fn notify_xcm_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Xcm;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
      TriggerCauseProvenance::Deferred,
    )
  }

  pub fn notify_address_event_without_source(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      None,
      None,
      TriggerCauseProvenance::Deferred,
    )
  }

  pub fn preflight_funding_event(
    actor_id: ActorId,
    _asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    _provenance: Option<&FundingProvenance>,
  ) -> DispatchResult {
    if amount.is_zero() {
      return Ok(());
    }
    if Self::load_financial_ingress_state(actor_id, source)?.is_some() {
      return Ok(());
    }
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => return Ok(()),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    if source == Some(&state.identity.sovereign_account) {
      return Ok(());
    }
    Self::ensure_machine_unfrozen()?;
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) || amount.is_zero() {
      return Ok(());
    }
    Ok(())
  }

  /// Typed certified-ingress preflight (spec 5.3, 6.2). Read-only and covers
  /// lifecycle, funding, trigger, and required placement. An absent or Dormant
  /// destination, a zero amount and self movement are balance-only. Positive
  /// Active ingress refuses temporarily during freeze, including an expired window.
  pub fn preflight_ingress(
    event: &AddressEvent<T::AccountId, T::AssetId, T::Balance>,
  ) -> Result<(), IngressFailure> {
    if event.amount.is_zero() || event.source.as_ref() == Some(&event.destination) {
      return Ok(());
    }
    let Some(actor_id) = Self::sovereign_index(&event.destination) else {
      return Ok(());
    };
    Self::preflight_funding_event(
      actor_id,
      event.asset,
      event.amount,
      event.source.as_ref(),
      event.provenance.as_ref(),
    )
    .map_err(Self::classify_ingress_error)?;
    if T::AssetOps::balance(&event.destination, event.asset)
      .checked_add(&event.amount)
      .is_none()
    {
      return Err(IngressFailure::permanent(Error::<T>::ComputationOverflow));
    }
    Ok(())
  }

  fn trigger_cause_provenance(provenance: Option<&FundingProvenance>) -> TriggerCauseProvenance {
    match provenance {
      Some(FundingProvenance::Signed) => TriggerCauseProvenance::ExternalPhase,
      Some(FundingProvenance::InternalProtocol | FundingProvenance::Xcm) | None => {
        TriggerCauseProvenance::Deferred
      }
    }
  }

  #[cfg(test)]
  pub(crate) fn test_trigger_cause_provenance(
    provenance: Option<&FundingProvenance>,
  ) -> TriggerCauseProvenance {
    Self::trigger_cause_provenance(provenance)
  }

  /// Typed certified-ingress consequence (spec 5.3, 6.2). Executes exactly once at
  /// the host protocol's declared notify or transactional-precommit phase and preserves
  /// the placement classification: recoverable queue/wakeup capacity or placement
  /// unavailability or machine freeze is Temporary; monotonic
  /// ticket/index exhaustion, topology corruption, and invariant failure are
  /// Permanent.
  pub fn notify_ingress(
    event: &AddressEvent<T::AccountId, T::AssetId, T::Balance>,
  ) -> Result<(), IngressFailure> {
    if event.amount.is_zero() || event.source.as_ref() == Some(&event.destination) {
      return Ok(());
    }
    let Some(actor_id) = Self::sovereign_index(&event.destination) else {
      return Ok(());
    };
    Self::notify_address_event_with_context(
      actor_id,
      event.asset,
      event.amount,
      event.source.as_ref(),
      event.provenance.as_ref(),
      Self::trigger_cause_provenance(event.provenance.as_ref()),
    )
    .map_err(Self::classify_ingress_error)
  }

  /// Maps one certified-ingress error to its closed retry class.
  ///
  /// Recoverable queue/wakeup capacity or placement unavailability surfaces as
  /// `QueueCapacityUnavailable` (queue saturation and failed wakeup placement) and
  /// `StateHoldUnavailable` (owner may fund the positive geometry delta) and machine freeze are Temporary.
  /// Monotonic ticket/index exhaustion, topology corruption, and invariant failure are Permanent.
  fn classify_ingress_error(error: DispatchError) -> IngressFailure {
    if error == Error::<T>::QueueCapacityUnavailable.into()
      || error == Error::<T>::StateHoldUnavailable.into()
      || error == Error::<T>::GlobalCircuitBreakerActive.into()
      || error == Error::<T>::AwaitFundingUnavailable.into()
    {
      IngressFailure::temporary(error)
    } else {
      IngressFailure::permanent(error)
    }
  }

  fn notify_address_event_with_context(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
    cause_provenance: TriggerCauseProvenance,
  ) -> DispatchResult {
    // Zero or self/no-op movement creates no Actors ingress.
    if amount.is_zero() {
      return Ok(());
    }
    Self::preflight_funding_event(actor_id, asset, amount, source, provenance)?;
    Self::with_reused_transaction(|| {
      Self::apply_address_event_parts(
        actor_id,
        asset,
        amount,
        source,
        provenance,
        cause_provenance,
      )
    })
  }

  fn apply_address_event_parts(
    actor_id: ActorId,
    asset: T::AssetId,
    _amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
    cause_provenance: TriggerCauseProvenance,
  ) -> DispatchResult {
    if let Some(waiting) = Self::load_financial_ingress_state(actor_id, source)? {
      let FinancialIngressState::Waiting {
        actor,
        state,
        process,
        episode,
        terminal_reason,
        cleanup,
      } = waiting
      else {
        return Ok(());
      };
      if let Some(reason) = terminal_reason {
        if !Self::close_control_weight_for_contract(&state.contract)
          .is_some_and(|weight| weight.all_lte(cleanup))
        {
          return Err(Error::<T>::AwaitFundingUnavailable.into());
        }
        let run = state.run_state.clone();
        return Self::remove_actor_publication_and_finalize(actor, state, run.as_ref(), reason)
          .map_err(|_| Error::<T>::ActorInvariant.into());
      }
      if asset != T::FeeNativeAssetId::get()
        || !Self::financial_ingress_authorized(actor_id, &state, source, provenance)
        || matches!(process.residence, Some(ProcessResidence::Service(_)))
      {
        return Ok(());
      }
      let custody = T::AssetOps::balance(
        &state.identity.sovereign_account,
        T::FeeNativeAssetId::get(),
      );
      let spendable = custody.saturating_sub(T::MinUserBalance::get());
      if spendable < episode.record().unpaid_liability() {
        return Ok(());
      }
      T::FinancialWaitBacking::secure_funded_ingress_return(
        source,
        provenance,
        crate::FinancialWaitReceipt {
          actor,
          episode_identity: episode.original_identity(),
          unpaid_service: episode.record().unpaid_service(),
        },
        Self::financial_wait_return_fee(&state.contract)?,
      )
      .map_err(|_| Error::<T>::AwaitFundingUnavailable)?;
      ensure!(
        T::AssetOps::balance(
          &state.identity.sovereign_account,
          T::FeeNativeAssetId::get()
        ) == custody,
        Error::<T>::ActorInvariant
      );
      return Self::return_financial_wait_to_service(
        actor,
        process,
        episode.record().unpaid_service(),
      )
      .map_err(|error| match error {
        crate::FinancialWaitDueError::Service(ServiceRingMutationError::CapacityExceeded) => {
          Error::<T>::QueueCapacityUnavailable.into()
        }
        _ => Error::<T>::ActorInvariant.into(),
      });
    }
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => return Ok(()),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    if source == Some(&state.identity.sovereign_account) {
      return Ok(());
    }
    Self::ensure_machine_unfrozen()?;
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) {
      return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
    }
    // A busy Actor may still account authorized funding, but the open Cycle owns all
    // process authority: the occurrence cannot pay for or latch a future Cycle.
    let signal_matched = if instance.cycle_state == CycleState::Idle
      && !instance.pending_signal
      && let Trigger::AddressEvent {
        source_filter,
        asset_filter,
      } = &instance.trigger
    {
      Self::source_matches_filter(source_filter, &instance.owner, source)
        && Self::asset_matches_filter(asset_filter, asset)
    } else {
      false
    };
    if signal_matched {
      let actor_type = instance.actor_class.actor_type();
      let breakdown = Self::trigger_fee_for_actor(
        actor_id,
        actor_type,
        instance.on_unfunded,
        TriggerFamily::AddressEvent,
        T::WeightInfo::address_event_trigger_occurrence(),
      )?;
      let _ = Self::try_commit_frame_automatic_trigger_occurrence(
        actor_id,
        actor_type,
        &instance.sovereign_account,
        breakdown,
        cause_provenance,
      )?;
    }
    Ok(())
  }

  /// The common process selector precedes optional cold reads. Concrete certified producers must
  /// admit the complete additional resource owner and pay ingress work; destruction keeps its
  /// independent Creation backing.
  fn load_financial_ingress_state(
    actor_id: ActorId,
    source: Option<&T::AccountId>,
  ) -> Result<Option<FinancialIngressState<T>>, DispatchError> {
    let Some(process) = ActorProcesses::<T>::get(actor_id)
      .filter(|process| process.status == ProcessStatus::AwaitingFunding)
    else {
      return Ok(None);
    };
    let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<T>::get(actor_id) else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    if process.generation != semantic.generation {
      return Err(Error::<T>::ActorInvariant.into());
    }
    if source == Some(&semantic.identity.sovereign_account) {
      return Ok(Some(FinancialIngressState::BalanceOnly));
    }
    Self::ensure_machine_unfrozen()?;
    if !Self::admission_matches_current_authority(&semantic.admission) {
      return Err(Error::<T>::ActorInvariant.into());
    }
    let (_, cleanup) =
      Self::financial_wait_ingress_weights().ok_or(Error::<T>::AwaitFundingUnavailable)?;
    let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    let actor = ActorRef {
      actor_id,
      generation: process.generation,
    };
    let episode = ActorFinancialWaits::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
    if episode.record().scope().actor != actor
      || <T::Time as polkadot_sdk::frame_support::traits::Time>::now()
        < episode.record().entered_at_millis()
    {
      return Err(Error::<T>::ActorInvariant.into());
    }
    let unpaid_trigger_due = match episode.record().unpaid_service() {
      crate::UnpaidActorService::AtTimeTrigger { due_tick } => Some(due_tick),
      _ => None,
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let terminal_reason = Self::classify_actor_loaded_with_unpaid_trigger(
      &instance,
      state.run_state.as_ref(),
      unpaid_trigger_due,
    )
    .map_err(Self::classification_dispatch_error)?
    .terminal_reason;
    Ok(Some(FinancialIngressState::Waiting {
      actor,
      state,
      process,
      episode,
      terminal_reason,
      cleanup,
    }))
  }

  fn financial_ingress_authorized(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> bool {
    let signed = provenance == Some(&FundingProvenance::Signed);
    match &state.contract.funding {
      FundingSourcePolicy::OwnerOnly => signed && source == Some(&state.identity.owner),
      FundingSourcePolicy::SignedAllowlist(sources) => {
        signed && source.is_some_and(|source| sources.contains(source))
      }
      FundingSourcePolicy::RuntimePolicy => {
        (source.is_some() || provenance.is_some())
          && T::FundingAuthority::permits(actor_id, &state.identity.owner, source, provenance)
      }
      FundingSourcePolicy::AnyVerifiedIngress => source.is_some() || provenance.is_some(),
    }
  }

  pub(crate) fn evaluate_actor_liveness(actor_id: ActorId) -> DispatchResult {
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(Error::<T>::ActorNotFound.into());
      }
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    if let Some(reason) =
      Self::classify_actor_lifecycle_loaded(actor_id, &instance, run_state.as_ref())
        .map_err(Self::classification_dispatch_error)?
        .terminal_reason
    {
      return Self::finalize_actor(actor_id, &instance, reason);
    }
    Ok(())
  }
}
