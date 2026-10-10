use super::{
  ActorRef, ActorRunAuthority, ActorType, DeadlineMutationError, FinancialWaitClockError,
  SchedulerTick, ServiceRingMutationError, financial_wait_expiry_tick, scheduler_tick_floor,
};
use frame::prelude::*;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum UnpaidActorService {
  AtTimeTrigger {
    due_tick: SchedulerTick,
  },
  PipelineOpening {
    cycle_nonce: u64,
  },
  RunningAction {
    cycle_nonce: u64,
    cursor: u32,
  },
  SuspendedAction {
    cycle_nonce: u64,
    cursor: u32,
    unsuccessful_attempts_at_cursor: u32,
  },
}

impl UnpaidActorService {
  fn is_valid(self) -> bool {
    match self {
      Self::AtTimeTrigger { due_tick } => due_tick > 0,
      Self::PipelineOpening { cycle_nonce }
      | Self::RunningAction { cycle_nonce, .. }
      | Self::SuspendedAction { cycle_nonce, .. } => cycle_nonce > 0,
    }
  }
}

/// Source inputs authenticated by the lifecycle/Contract owner, not by an author or notification.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FinancialWaitScope {
  pub actor: ActorRef,
  pub actor_type: ActorType,
  pub contract_authority: ActorRunAuthority<[u8; 32]>,
  pub behavior_episode: Option<[u8; 32]>,
}

impl FinancialWaitScope {
  fn validate(self) -> Result<(), FinancialWaitError> {
    if self.actor_type != ActorType::User {
      return Err(FinancialWaitError::UserRequired);
    }
    if self.actor.generation == 0 {
      return Err(FinancialWaitError::InvalidScope);
    }
    Ok(())
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialWaitError {
  UserRequired,
  InvalidScope,
  InvalidBoundary,
  ZeroLiability,
  BindingMismatch,
  Clock(FinancialWaitClockError),
  InvalidExpiry,
  ClockReversed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialWaitDecision {
  Awaiting,
  Funded,
  GraceExpired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialWaitPublicationError {
  Unqualified,
  InsufficientWeight,
  MachineFrozen,
  InvalidSource,
  AlreadyWaiting,
  Solvent,
  Record(FinancialWaitError),
  Deadline(DeadlineMutationError),
  Service(ServiceRingMutationError),
  StateHold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinancialWaitDueError {
  Unqualified,
  InsufficientWeight,
  MachineFrozen,
  InvalidSource,
  NotDue,
  Record(FinancialWaitError),
  Deadline(DeadlineMutationError),
  Service(ServiceRingMutationError),
  TerminalCleanup,
}

/// Transaction-local result identity, retained even when cleanup removes the financial companion.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FinancialWaitReceipt {
  pub actor: ActorRef,
  pub episode_identity: [u8; 32],
  pub unpaid_service: UnpaidActorService,
}

/// Original debt and current protected spendability; neither certifies execution admission.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FinancialWaitProjection<Balance> {
  pub unpaid_service: UnpaidActorService,
  pub unpaid_liability: Balance,
  pub current_service_spendability: Balance,
  pub funding_shortfall: Balance,
  pub entered_at_millis: u64,
  pub expiry_tick: SchedulerTick,
}

/// Optional process-owned companion. Its original identity is never renewed by a solvency hint.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FinancialWaitEpisode<Balance> {
  record: FinancialWaitRecord<Balance>,
  original_admission_identity: [u8; 32],
  original_pipeline_service_identity: [u8; 32],
  original_identity: [u8; 32],
}

impl<Balance: Copy + Eq + Ord + Zero + Encode> FinancialWaitEpisode<Balance> {
  pub fn new(record: FinancialWaitRecord<Balance>) -> Self {
    Self {
      original_admission_identity: record.scope.contract_authority.admission_identity,
      original_pipeline_service_identity: record.scope.contract_authority.pipeline_service_identity,
      original_identity: record.identity(),
      record,
    }
  }

  pub fn record(&self) -> &FinancialWaitRecord<Balance> {
    &self.record
  }

  pub fn original_identity(&self) -> [u8; 32] {
    self.original_identity
  }

  /// Authenticates current derived authority and the immutable original episode independently.
  pub fn bind(
    &self,
    scope: FinancialWaitScope,
    unpaid_service: UnpaidActorService,
    unpaid_liability: Balance,
    tick_millis: u64,
    max_delay_ticks: SchedulerTick,
  ) -> Result<BoundFinancialWait<'_, Balance>, FinancialWaitError> {
    let mut original = self.record;
    original.scope.contract_authority.admission_identity = self.original_admission_identity;
    original.scope.contract_authority.pipeline_service_identity =
      self.original_pipeline_service_identity;
    if original.identity() != self.original_identity {
      return Err(FinancialWaitError::BindingMismatch);
    }
    self.record.bind(
      scope,
      unpaid_service,
      unpaid_liability,
      self.record.identity(),
      tick_millis,
      max_delay_ticks,
    )
  }

  /// Rebinds compatible certificates without renewing debt, expiry, generation or episode.
  pub fn rebind(
    &self,
    scope: FinancialWaitScope,
    tick_millis: u64,
    max_delay_ticks: SchedulerTick,
  ) -> Result<Self, FinancialWaitError> {
    self.bind(
      self.record.scope,
      self.record.unpaid_service,
      self.record.unpaid_liability,
      tick_millis,
      max_delay_ticks,
    )?;
    let mut expected = self.record.scope;
    expected.contract_authority.admission_identity = scope.contract_authority.admission_identity;
    expected.contract_authority.pipeline_service_identity =
      scope.contract_authority.pipeline_service_identity;
    if expected != scope {
      return Err(FinancialWaitError::BindingMismatch);
    }
    let mut rebound = *self;
    rebound.record.scope = scope;
    Ok(rebound)
  }
}

/// Storage-free episode proposal; publication still requires independent economic/physical admission.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct FinancialWaitRecord<Balance> {
  scope: FinancialWaitScope,
  unpaid_service: UnpaidActorService,
  unpaid_liability: Balance,
  entered_at_millis: u64,
  tick_millis: u64,
  expiry_tick: SchedulerTick,
}

impl<Balance: Copy + Eq + Ord + Zero + Encode> FinancialWaitRecord<Balance> {
  pub fn new(
    scope: FinancialWaitScope,
    unpaid_service: UnpaidActorService,
    unpaid_liability: Balance,
    timestamp_millis: u64,
    tick_millis: u64,
    max_delay_ticks: SchedulerTick,
  ) -> Result<Self, FinancialWaitError> {
    scope.validate()?;
    if !unpaid_service.is_valid() {
      return Err(FinancialWaitError::InvalidBoundary);
    }
    if unpaid_liability.is_zero() {
      return Err(FinancialWaitError::ZeroLiability);
    }
    let expiry_tick = financial_wait_expiry_tick(timestamp_millis, tick_millis, max_delay_ticks)
      .map_err(FinancialWaitError::Clock)?;
    if let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid_service {
      let now_tick = scheduler_tick_floor(timestamp_millis, tick_millis).ok_or(
        FinancialWaitError::Clock(FinancialWaitClockError::InvalidTickDuration),
      )?;
      if due_tick > now_tick {
        return Err(FinancialWaitError::InvalidBoundary);
      }
    }
    Ok(Self {
      scope,
      unpaid_service,
      unpaid_liability,
      entered_at_millis: timestamp_millis,
      tick_millis,
      expiry_tick,
    })
  }

  pub fn scope(&self) -> FinancialWaitScope {
    self.scope
  }

  pub fn unpaid_service(&self) -> UnpaidActorService {
    self.unpaid_service
  }

  pub fn unpaid_liability(&self) -> Balance {
    self.unpaid_liability
  }

  pub fn entered_at_millis(&self) -> u64 {
    self.entered_at_millis
  }

  pub fn expiry_tick(&self) -> SchedulerTick {
    self.expiry_tick
  }

  pub fn identity(&self) -> [u8; 32] {
    (b"DEOS_ACTOR_FINANCIAL_WAIT_EPISODE", self).using_encoded(frame::hashing::blake2_256)
  }

  /// Verifies a decoded record against current source inputs and its original absolute endpoint.
  pub fn bind(
    &self,
    scope: FinancialWaitScope,
    unpaid_service: UnpaidActorService,
    unpaid_liability: Balance,
    episode_identity: [u8; 32],
    tick_millis: u64,
    max_delay_ticks: SchedulerTick,
  ) -> Result<BoundFinancialWait<'_, Balance>, FinancialWaitError> {
    let expected = Self::new(
      scope,
      unpaid_service,
      unpaid_liability,
      self.entered_at_millis,
      tick_millis,
      max_delay_ticks,
    )?;
    if self.scope != scope
      || self.unpaid_service != unpaid_service
      || self.unpaid_liability != unpaid_liability
    {
      return Err(FinancialWaitError::BindingMismatch);
    }
    if self.tick_millis != tick_millis || self.expiry_tick != expected.expiry_tick {
      return Err(FinancialWaitError::InvalidExpiry);
    }
    if self.identity() != episode_identity {
      return Err(FinancialWaitError::BindingMismatch);
    }
    Ok(BoundFinancialWait { record: self })
  }
}

/// Borrowed source witness. Solvency checks never replace it or extend the financial grace.
pub struct BoundFinancialWait<'a, Balance> {
  record: &'a FinancialWaitRecord<Balance>,
}

impl<Balance: Copy + Ord> BoundFinancialWait<'_, Balance> {
  pub fn classify(
    &self,
    current_service_spendability: Balance,
    timestamp_millis: u64,
  ) -> Result<FinancialWaitDecision, FinancialWaitError> {
    if timestamp_millis < self.record.entered_at_millis {
      return Err(FinancialWaitError::ClockReversed);
    }
    if current_service_spendability >= self.record.unpaid_liability {
      return Ok(FinancialWaitDecision::Funded);
    }
    let now_tick = scheduler_tick_floor(timestamp_millis, self.record.tick_millis).ok_or(
      FinancialWaitError::Clock(FinancialWaitClockError::InvalidTickDuration),
    )?;
    Ok(if now_tick >= self.record.expiry_tick {
      FinancialWaitDecision::GraceExpired
    } else {
      FinancialWaitDecision::Awaiting
    })
  }
}
