use frame::prelude::*;
use polkadot_sdk::frame_support::weights::Weight;

/// Explicit independent resource ceilings for one rollback-only Actor preview.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct SimulationBudget {
  pub actor_control: Weight,
  pub shared_economic: Weight,
}

impl SimulationBudget {
  pub fn checked_limits(self) -> Result<BlockResourceLimits, BlockResourceError> {
    let actor_base = weight_floor_div(self.shared_economic, 2);
    let user_base = self
      .shared_economic
      .checked_sub(&actor_base)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    BlockResourceLimits::new(
      self.actor_control,
      self.shared_economic,
      actor_base,
      user_base,
    )
  }
}

/// One authoritative block-resource domain.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum BlockResourceDomain {
  ActorControl,
  ActorBaseEffect,
  UserDispatch,
  ActorDrainEffect,
}

/// One-way consensus phase for the current block's Economic Zipper.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum BlockResourcePhase {
  ContextIncomplete,
  PrepassExecuting,
  ExternalPhase,
  FreshDrain,
  Finalizable,
}

/// Transient authoritative state for exactly one block.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BlockResourceState<BlockNumber> {
  block_number: BlockNumber,
  phase: BlockResourcePhase,
  usage: BlockResourceUsage,
  outstanding_reservations: u32,
  finalized_fixed_reserved: Option<Weight>,
  optional_actor_work_halted: bool,
}

/// Checked reference-runtime limits for Actor control and shared economic work.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BlockResourceLimits {
  actor_control: Weight,
  shared_economic: Weight,
  actor_base_turn: Weight,
  user_base_turn: Weight,
}

/// Immutable relationship between FRAME maximum, fixed/context reserve, and schedulable limits.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct BlockResourceBudget {
  maximum_block: Weight,
  fixed_envelope: Weight,
  limits: BlockResourceLimits,
}

/// Independently evidenced owners of the fixed/context envelope. The first component owns FRAME
/// overhead, every non-Actor initialization/finalization hook not already owned by the four
/// explicit components, and bounded non-economic maintenance such as XCMP lazy-migration `on_idle`.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct FixedBlockWeightComponents {
  other_runtime_hooks_and_frame_base: Weight,
  timestamp: Weight,
  parachain_validation: Weight,
  downward_messages: Weight,
  horizontal_messages: Weight,
}

/// Finite pre-execution bounds for variable-size parachain context messages.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ContextMessageLimits {
  downward_messages: u32,
  horizontal_messages: u32,
  horizontal_channels: u32,
}

/// Count-only witness extracted from shared parachain inherent data.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ContextMessageGeometry {
  downward_full: u32,
  downward_hashed: u32,
  horizontal_full: u32,
  horizontal_hashed: u32,
  horizontal_channels: u32,
}

/// Finalized read-only observation; it is never resource authority.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct FinalizedBlockResourceSnapshot<BlockNumber> {
  block_number: BlockNumber,
  fixed_reserved: Weight,
  usage: BlockResourceUsage,
  optional_actor_work_halted: bool,
}

/// Current authoritative usage. Actor base and Drain effects share one counter.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  MaxEncodedLen,
  PartialEq,
  TypeInfo,
)]
pub struct BlockResourceUsage {
  actor_control: Weight,
  actor_effect: Weight,
  user_dispatch: Weight,
}

/// A pre-mutation maximum reservation that can be settled only in its owning domain.
#[derive(Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct BlockResourceReservation {
  domain: BlockResourceDomain,
  maximum: Weight,
  phase: BlockResourcePhase,
  settled: bool,
}

/// Atomic paired reservation for one Actor Step's control and Task-effect maxima.
#[derive(Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct ActorStepResourceReservation {
  control: BlockResourceReservation,
  effect: BlockResourceReservation,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum BlockResourceError {
  InvalidLimits,
  ArithmeticOverflow,
  LimitExceeded,
  ActualExceedsReserved,
  ReservationAlreadySettled,
  ReservationOutstanding,
  InconsistentReservation,
  FixedEnvelopeExceeded,
  ReconciliationMismatch,
  WrongBlock,
  InvalidPhase,
  ContextGeometryExceeded,
  OptionalActorWorkHalted,
}

impl ContextMessageLimits {
  pub fn new(downward_messages: u32, horizontal_messages: u32, horizontal_channels: u32) -> Self {
    Self {
      downward_messages,
      horizontal_messages,
      horizontal_channels,
    }
  }

  pub fn validate(&self, geometry: ContextMessageGeometry) -> Result<(), BlockResourceError> {
    let downward = geometry
      .downward_full
      .checked_add(geometry.downward_hashed)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    let horizontal = geometry
      .horizontal_full
      .checked_add(geometry.horizontal_hashed)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    if downward > self.downward_messages
      || horizontal > self.horizontal_messages
      || geometry.horizontal_channels > self.horizontal_channels
    {
      return Err(BlockResourceError::ContextGeometryExceeded);
    }
    Ok(())
  }
}

impl ContextMessageGeometry {
  pub fn new(
    downward_full: u32,
    downward_hashed: u32,
    horizontal_full: u32,
    horizontal_hashed: u32,
    horizontal_channels: u32,
  ) -> Self {
    Self {
      downward_full,
      downward_hashed,
      horizontal_full,
      horizontal_hashed,
      horizontal_channels,
    }
  }
}

impl<BlockNumber: Copy> FinalizedBlockResourceSnapshot<BlockNumber> {
  pub fn block_number(&self) -> BlockNumber {
    self.block_number
  }

  pub fn fixed_reserved(&self) -> Weight {
    self.fixed_reserved
  }

  pub fn usage(&self) -> BlockResourceUsage {
    self.usage
  }

  pub fn optional_actor_work_halted(&self) -> bool {
    self.optional_actor_work_halted
  }
}

impl<BlockNumber: Copy + PartialEq> BlockResourceState<BlockNumber> {
  pub fn new(block_number: BlockNumber) -> Self {
    Self {
      block_number,
      phase: BlockResourcePhase::ContextIncomplete,
      usage: BlockResourceUsage::default(),
      outstanding_reservations: 0,
      finalized_fixed_reserved: None,
      optional_actor_work_halted: false,
    }
  }

  pub fn block_number(&self) -> BlockNumber {
    self.block_number
  }

  pub fn phase(&self) -> BlockResourcePhase {
    self.phase
  }

  pub fn usage(&self) -> BlockResourceUsage {
    self.usage
  }

  pub fn outstanding_reservations(&self) -> u32 {
    self.outstanding_reservations
  }

  pub fn optional_actor_work_halted(&self) -> bool {
    self.optional_actor_work_halted
  }

  pub fn ensure_block(&self, block_number: BlockNumber) -> Result<(), BlockResourceError> {
    if self.block_number != block_number {
      return Err(BlockResourceError::WrongBlock);
    }
    Ok(())
  }

  pub fn begin_prepass(&mut self) -> Result<(), BlockResourceError> {
    self.advance(
      BlockResourcePhase::ContextIncomplete,
      BlockResourcePhase::PrepassExecuting,
    )
  }

  pub fn open_external_phase(&mut self) -> Result<(), BlockResourceError> {
    self.advance(
      BlockResourcePhase::PrepassExecuting,
      BlockResourcePhase::ExternalPhase,
    )
  }

  pub fn begin_drain(&mut self) -> Result<(), BlockResourceError> {
    self.advance(
      BlockResourcePhase::ExternalPhase,
      BlockResourcePhase::FreshDrain,
    )
  }

  pub fn finish_drain(
    &mut self,
    budget: BlockResourceBudget,
    fixed_reserved: Weight,
  ) -> Result<(), BlockResourceError> {
    if self.phase != BlockResourcePhase::FreshDrain {
      return Err(BlockResourceError::InvalidPhase);
    }
    if self.outstanding_reservations != 0 {
      return Err(BlockResourceError::ReservationOutstanding);
    }
    self.usage.reconcile_with_block(budget, fixed_reserved)?;
    self.finalized_fixed_reserved = Some(fixed_reserved);
    self.phase = BlockResourcePhase::Finalizable;
    Ok(())
  }

  pub fn finalized_snapshot(
    &self,
  ) -> Result<FinalizedBlockResourceSnapshot<BlockNumber>, BlockResourceError> {
    if self.phase != BlockResourcePhase::Finalizable {
      return Err(BlockResourceError::InvalidPhase);
    }
    let fixed_reserved = self
      .finalized_fixed_reserved
      .ok_or(BlockResourceError::ReconciliationMismatch)?;
    Ok(FinalizedBlockResourceSnapshot {
      block_number: self.block_number,
      fixed_reserved,
      usage: self.usage,
      optional_actor_work_halted: self.optional_actor_work_halted,
    })
  }

  pub fn halt_optional_actor_work(&mut self) {
    self.optional_actor_work_halted = true;
  }

  pub fn reserve(
    &mut self,
    limits: BlockResourceLimits,
    domain: BlockResourceDomain,
    maximum: Weight,
  ) -> Result<BlockResourceReservation, BlockResourceError> {
    let actor_domain = domain != BlockResourceDomain::UserDispatch;
    if actor_domain && self.optional_actor_work_halted {
      return Err(BlockResourceError::OptionalActorWorkHalted);
    }
    let phase_allows = match domain {
      BlockResourceDomain::ActorControl => matches!(
        self.phase,
        BlockResourcePhase::PrepassExecuting
          | BlockResourcePhase::ExternalPhase
          | BlockResourcePhase::FreshDrain
      ),
      BlockResourceDomain::ActorBaseEffect => self.phase == BlockResourcePhase::PrepassExecuting,
      BlockResourceDomain::UserDispatch => self.phase == BlockResourcePhase::ExternalPhase,
      BlockResourceDomain::ActorDrainEffect => self.phase == BlockResourcePhase::FreshDrain,
    };
    if !phase_allows {
      return Err(BlockResourceError::InvalidPhase);
    }
    let outstanding = match self.outstanding_reservations.checked_add(1) {
      Some(outstanding) => outstanding,
      None => {
        if actor_domain {
          self.optional_actor_work_halted = true;
        }
        return Err(BlockResourceError::ArithmeticOverflow);
      }
    };
    let mut reservation = match self.usage.reserve(limits, domain, maximum) {
      Ok(reservation) => reservation,
      Err(error) => {
        if actor_domain && error == BlockResourceError::ArithmeticOverflow {
          self.optional_actor_work_halted = true;
        }
        return Err(error);
      }
    };
    reservation.phase = self.phase;
    self.outstanding_reservations = outstanding;
    Ok(reservation)
  }

  /// Negative capacity probe only; real reservation retains phase and overflow handling.
  pub(crate) fn capacity_exceeded(
    &self,
    limits: BlockResourceLimits,
    domain: BlockResourceDomain,
    maximum: Weight,
  ) -> bool {
    let mut probe = *self;
    matches!(
      probe.reserve(limits, domain, maximum),
      Err(BlockResourceError::LimitExceeded)
    )
  }

  pub fn reserve_actor_step(
    &mut self,
    limits: BlockResourceLimits,
    effect_domain: BlockResourceDomain,
    control_maximum: Weight,
    effect_maximum: Weight,
  ) -> Result<ActorStepResourceReservation, BlockResourceError> {
    if !matches!(
      effect_domain,
      BlockResourceDomain::ActorBaseEffect | BlockResourceDomain::ActorDrainEffect
    ) {
      return Err(BlockResourceError::InvalidPhase);
    }
    let before = *self;
    let control = match self.reserve(limits, BlockResourceDomain::ActorControl, control_maximum) {
      Ok(reservation) => reservation,
      Err(error) => return Err(error),
    };
    let effect = match self.reserve(limits, effect_domain, effect_maximum) {
      Ok(reservation) => reservation,
      Err(error) => {
        *self = before;
        return Err(error);
      }
    };
    Ok(ActorStepResourceReservation { control, effect })
  }

  pub fn settle_actor_step(
    &mut self,
    reservation: &mut ActorStepResourceReservation,
    actual_control: Weight,
    actual_effect: Weight,
  ) -> Result<(), BlockResourceError> {
    let before = *self;
    let control_settled = reservation.control.settled;
    let effect_settled = reservation.effect.settled;
    if let Err(error) = self
      .settle(&mut reservation.control, actual_control)
      .and_then(|()| self.settle(&mut reservation.effect, actual_effect))
    {
      *self = before;
      reservation.control.settled = control_settled;
      reservation.effect.settled = effect_settled;
      return Err(error);
    }
    Ok(())
  }

  pub fn settle(
    &mut self,
    reservation: &mut BlockResourceReservation,
    actual: Weight,
  ) -> Result<(), BlockResourceError> {
    let actor_domain = reservation.domain != BlockResourceDomain::UserDispatch;
    if reservation.phase != self.phase {
      if actor_domain {
        self.optional_actor_work_halted = true;
      }
      return Err(BlockResourceError::InvalidPhase);
    }
    let outstanding = match self.outstanding_reservations.checked_sub(1) {
      Some(outstanding) => outstanding,
      None => {
        if actor_domain {
          self.optional_actor_work_halted = true;
        }
        return Err(BlockResourceError::InconsistentReservation);
      }
    };
    if let Err(error) = self.usage.settle(reservation, actual) {
      if actor_domain {
        self.optional_actor_work_halted = true;
      }
      return Err(error);
    }
    self.outstanding_reservations = outstanding;
    Ok(())
  }

  fn advance(
    &mut self,
    expected: BlockResourcePhase,
    next: BlockResourcePhase,
  ) -> Result<(), BlockResourceError> {
    if self.phase != expected {
      return Err(BlockResourceError::InvalidPhase);
    }
    if self.outstanding_reservations != 0 {
      return Err(BlockResourceError::ReservationOutstanding);
    }
    self.phase = next;
    Ok(())
  }
}

impl FixedBlockWeightComponents {
  pub fn new(
    other_runtime_hooks_and_frame_base: Weight,
    timestamp: Weight,
    parachain_validation: Weight,
    downward_messages: Weight,
    horizontal_messages: Weight,
  ) -> Self {
    Self {
      other_runtime_hooks_and_frame_base,
      timestamp,
      parachain_validation,
      downward_messages,
      horizontal_messages,
    }
  }

  pub fn total(&self) -> Result<Weight, BlockResourceError> {
    [
      self.other_runtime_hooks_and_frame_base,
      self.timestamp,
      self.parachain_validation,
      self.downward_messages,
      self.horizontal_messages,
    ]
    .into_iter()
    .try_fold(Weight::zero(), |total, component| {
      total
        .checked_add(&component)
        .ok_or(BlockResourceError::ArithmeticOverflow)
    })
  }
}

impl BlockResourceBudget {
  /// Produces a zero-schedulable budget when runtime composition cannot be trusted.
  pub fn fail_closed(maximum_block: Weight) -> Self {
    Self {
      maximum_block,
      fixed_envelope: maximum_block,
      limits: BlockResourceLimits {
        actor_control: Weight::zero(),
        shared_economic: Weight::zero(),
        actor_base_turn: Weight::zero(),
        user_base_turn: Weight::zero(),
      },
    }
  }

  pub fn from_fixed_components(
    maximum_block: Weight,
    components: FixedBlockWeightComponents,
  ) -> Result<Self, BlockResourceError> {
    Self::new(maximum_block, components.total()?)
  }

  pub fn new(maximum_block: Weight, fixed_envelope: Weight) -> Result<Self, BlockResourceError> {
    Self::new_with_control_ratio(maximum_block, fixed_envelope, 1, 5)
  }

  /// Constructs one fixed candidate allocation without changing resource-domain semantics.
  pub fn new_with_control_ratio(
    maximum_block: Weight,
    fixed_envelope: Weight,
    control_numerator: u64,
    control_denominator: u64,
  ) -> Result<Self, BlockResourceError> {
    let schedulable = maximum_block
      .checked_sub(&fixed_envelope)
      .ok_or(BlockResourceError::InvalidLimits)?;
    Ok(Self {
      maximum_block,
      fixed_envelope,
      limits: BlockResourceLimits::from_schedulable_ratio(
        schedulable,
        control_numerator,
        control_denominator,
      )?,
    })
  }

  pub fn maximum_block(&self) -> Weight {
    self.maximum_block
  }

  pub fn fixed_envelope(&self) -> Weight {
    self.fixed_envelope
  }

  pub fn limits(&self) -> BlockResourceLimits {
    self.limits
  }
}

impl BlockResourceLimits {
  fn from_schedulable_ratio(
    schedulable: Weight,
    control_numerator: u64,
    control_denominator: u64,
  ) -> Result<Self, BlockResourceError> {
    if control_denominator == 0 || control_numerator > control_denominator {
      return Err(BlockResourceError::InvalidLimits);
    }
    let actor_control = weight_floor_mul_div(schedulable, control_numerator, control_denominator)?;
    let shared_economic = schedulable
      .checked_sub(&actor_control)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    let actor_base_turn = weight_floor_div(shared_economic, 2);
    let user_base_turn = shared_economic
      .checked_sub(&actor_base_turn)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    Ok(Self {
      actor_control,
      shared_economic,
      actor_base_turn,
      user_base_turn,
    })
  }

  pub fn new(
    actor_control: Weight,
    shared_economic: Weight,
    actor_base_turn: Weight,
    user_base_turn: Weight,
  ) -> Result<Self, BlockResourceError> {
    actor_control
      .checked_add(&shared_economic)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    let expected_actor_base = weight_floor_div(shared_economic, 2);
    let expected_user_base = shared_economic
      .checked_sub(&expected_actor_base)
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    if actor_base_turn != expected_actor_base || user_base_turn != expected_user_base {
      return Err(BlockResourceError::InvalidLimits);
    }
    Ok(Self {
      actor_control,
      shared_economic,
      actor_base_turn,
      user_base_turn,
    })
  }

  pub fn actor_control(&self) -> Weight {
    self.actor_control
  }

  pub fn shared_economic(&self) -> Weight {
    self.shared_economic
  }

  pub fn actor_base_turn(&self) -> Weight {
    self.actor_base_turn
  }

  pub fn user_base_turn(&self) -> Weight {
    self.user_base_turn
  }
}

fn weight_floor_div(value: Weight, denominator: u64) -> Weight {
  Weight::from_parts(
    value.ref_time() / denominator,
    value.proof_size() / denominator,
  )
}

fn weight_floor_mul_div(
  value: Weight,
  numerator: u64,
  denominator: u64,
) -> Result<Weight, BlockResourceError> {
  let component = |value: u64| {
    let quotient = value / denominator;
    let remainder = value % denominator;
    quotient
      .checked_mul(numerator)
      .and_then(|scaled| {
        remainder
          .checked_mul(numerator)
          .map(|tail| (scaled, tail / denominator))
      })
      .and_then(|(scaled, tail)| scaled.checked_add(tail))
      .ok_or(BlockResourceError::ArithmeticOverflow)
  };
  Ok(Weight::from_parts(
    component(value.ref_time())?,
    component(value.proof_size())?,
  ))
}

fn checked_add_with_limit(
  current: Weight,
  added: Weight,
  limit: Weight,
) -> Result<Weight, BlockResourceError> {
  let next = current
    .checked_add(&added)
    .ok_or(BlockResourceError::ArithmeticOverflow)?;
  if !next.all_lte(limit) {
    return Err(BlockResourceError::LimitExceeded);
  }
  Ok(next)
}

impl BlockResourceUsage {
  fn reserve(
    &mut self,
    limits: BlockResourceLimits,
    domain: BlockResourceDomain,
    maximum: Weight,
  ) -> Result<BlockResourceReservation, BlockResourceError> {
    match domain {
      BlockResourceDomain::ActorControl => {
        self.actor_control =
          checked_add_with_limit(self.actor_control, maximum, limits.actor_control)?;
      }
      BlockResourceDomain::ActorBaseEffect => {
        let next = checked_add_with_limit(self.actor_effect, maximum, limits.actor_base_turn)?;
        checked_add_with_limit(next, self.user_dispatch, limits.shared_economic)?;
        self.actor_effect = next;
      }
      BlockResourceDomain::UserDispatch => {
        let next = self
          .user_dispatch
          .checked_add(&maximum)
          .ok_or(BlockResourceError::ArithmeticOverflow)?;
        checked_add_with_limit(next, self.actor_effect, limits.shared_economic)?;
        self.user_dispatch = next;
      }
      BlockResourceDomain::ActorDrainEffect => {
        let next = self
          .actor_effect
          .checked_add(&maximum)
          .ok_or(BlockResourceError::ArithmeticOverflow)?;
        checked_add_with_limit(next, self.user_dispatch, limits.shared_economic)?;
        self.actor_effect = next;
      }
    }
    Ok(BlockResourceReservation {
      domain,
      maximum,
      phase: BlockResourcePhase::ContextIncomplete,
      settled: false,
    })
  }

  fn settle(
    &mut self,
    reservation: &mut BlockResourceReservation,
    actual: Weight,
  ) -> Result<(), BlockResourceError> {
    if reservation.settled {
      return Err(BlockResourceError::ReservationAlreadySettled);
    }
    if !actual.all_lte(reservation.maximum) {
      return Err(BlockResourceError::ActualExceedsReserved);
    }
    let target = match reservation.domain {
      BlockResourceDomain::ActorControl => &mut self.actor_control,
      BlockResourceDomain::ActorBaseEffect | BlockResourceDomain::ActorDrainEffect => {
        &mut self.actor_effect
      }
      BlockResourceDomain::UserDispatch => &mut self.user_dispatch,
    };
    *target = target
      .checked_sub(&reservation.maximum)
      .and_then(|released| released.checked_add(&actual))
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    reservation.settled = true;
    Ok(())
  }

  pub fn actor_control_used(&self) -> Weight {
    self.actor_control
  }

  pub fn actor_effect_used(&self) -> Weight {
    self.actor_effect
  }

  pub fn user_dispatch_used(&self) -> Weight {
    self.user_dispatch
  }

  pub fn shared_used(&self) -> Result<Weight, BlockResourceError> {
    self
      .actor_effect
      .checked_add(&self.user_dispatch)
      .ok_or(BlockResourceError::ArithmeticOverflow)
  }

  pub fn reconcile_with_block(
    &self,
    budget: BlockResourceBudget,
    fixed_reserved: Weight,
  ) -> Result<(), BlockResourceError> {
    if !fixed_reserved.all_lte(budget.fixed_envelope) {
      return Err(BlockResourceError::FixedEnvelopeExceeded);
    }
    let shared = self.shared_used()?;
    if !self.actor_control.all_lte(budget.limits.actor_control)
      || !shared.all_lte(budget.limits.shared_economic)
    {
      return Err(BlockResourceError::ReconciliationMismatch);
    }
    let total = fixed_reserved
      .checked_add(&self.actor_control)
      .and_then(|used| used.checked_add(&shared))
      .ok_or(BlockResourceError::ArithmeticOverflow)?;
    if !total.all_lte(budget.maximum_block) {
      return Err(BlockResourceError::ReconciliationMismatch);
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn simulation_budget_preserves_domain_ceilings_and_checked_sum() -> Result<(), BlockResourceError>
  {
    for (actor_control, shared_economic) in [
      (Weight::zero(), Weight::zero()),
      (Weight::MAX, Weight::zero()),
      (Weight::from_parts(7, 11), Weight::from_parts(13, 17)),
      (
        Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
        Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
      ),
    ] {
      let limits = SimulationBudget {
        actor_control,
        shared_economic,
      }
      .checked_limits()?;
      assert_eq!(limits.actor_control(), actor_control);
      assert_eq!(limits.shared_economic(), shared_economic);
      assert_eq!(
        limits.actor_base_turn(),
        weight_floor_div(shared_economic, 2)
      );
      assert_eq!(
        limits
          .actor_base_turn()
          .checked_add(&limits.user_base_turn()),
        Some(shared_economic),
      );
    }
    for shared_economic in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      assert_eq!(
        SimulationBudget {
          actor_control: Weight::MAX,
          shared_economic
        }
        .checked_limits(),
        Err(BlockResourceError::ArithmeticOverflow),
      );
    }
    Ok(())
  }

  fn weight(ref_time: u64, proof_size: u64) -> Weight {
    Weight::from_parts(ref_time, proof_size)
  }

  fn limits() -> BlockResourceLimits {
    BlockResourceLimits {
      actor_control: weight(20, 20),
      shared_economic: weight(80, 80),
      actor_base_turn: weight(40, 40),
      user_base_turn: weight(40, 40),
    }
  }

  fn retained_reservation(
    result: Result<BlockResourceReservation, BlockResourceError>,
    domain: BlockResourceDomain,
    maximum: Weight,
  ) -> BlockResourceReservation {
    assert!(result.is_ok());
    result.unwrap_or(BlockResourceReservation {
      domain,
      maximum,
      phase: BlockResourcePhase::ContextIncomplete,
      settled: false,
    })
  }

  #[test]
  fn actor_step_pair_reserves_and_settles_atomically_in_base_phase() {
    let mut state = BlockResourceState::new(1u32);
    assert_eq!(state.begin_prepass(), Ok(()));
    let mut reservation = state
      .reserve_actor_step(
        limits(),
        BlockResourceDomain::ActorBaseEffect,
        weight(10, 10),
        weight(30, 30),
      )
      .expect("paired maxima fit their base domains"); // deos-bypass: panic-owner — explicit test limits dominate both maxima component-wise.
    assert_eq!(state.outstanding_reservations(), 2);
    assert_eq!(state.usage().actor_control_used(), weight(10, 10));
    assert_eq!(state.usage().actor_effect_used(), weight(30, 30));
    assert_eq!(
      state.settle_actor_step(&mut reservation, weight(5, 5), weight(20, 20)),
      Ok(())
    );
    assert_eq!(state.outstanding_reservations(), 0);
    assert_eq!(state.usage().actor_control_used(), weight(5, 5));
    assert_eq!(state.usage().actor_effect_used(), weight(20, 20));
  }

  #[test]
  fn actor_step_pair_rolls_back_partial_reserve_and_failed_settlement() {
    let mut state = BlockResourceState::new(1u32);
    assert_eq!(state.begin_prepass(), Ok(()));
    let before = state;
    assert_eq!(
      state.reserve_actor_step(
        limits(),
        BlockResourceDomain::ActorBaseEffect,
        weight(10, 10),
        weight(41, 41),
      ),
      Err(BlockResourceError::LimitExceeded)
    );
    assert_eq!(state, before);

    let mut reservation = state
      .reserve_actor_step(
        limits(),
        BlockResourceDomain::ActorBaseEffect,
        weight(10, 10),
        weight(30, 30),
      )
      .expect("paired maxima fit"); // deos-bypass: panic-owner — explicit test limits dominate both maxima component-wise.
    let reserved = state;
    assert_eq!(
      state.settle_actor_step(&mut reservation, weight(5, 5), weight(31, 31)),
      Err(BlockResourceError::ActualExceedsReserved)
    );
    assert_eq!(state, reserved);
    assert_eq!(state.outstanding_reservations(), 2);
    assert_eq!(
      state.settle_actor_step(&mut reservation, weight(5, 5), weight(20, 20)),
      Ok(())
    );
  }

  #[test]
  fn actor_step_pair_uses_drain_effect_only_in_fresh_drain() {
    let mut state = BlockResourceState::new(1u32);
    assert_eq!(state.begin_prepass(), Ok(()));
    assert_eq!(state.open_external_phase(), Ok(()));
    assert_eq!(state.begin_drain(), Ok(()));
    assert!(
      state
        .reserve_actor_step(
          limits(),
          BlockResourceDomain::ActorDrainEffect,
          weight(1, 1),
          weight(1, 1),
        )
        .is_ok()
    );
  }

  #[test]
  fn context_geometry_checks_full_hashed_and_channel_dimensions() {
    let limits = ContextMessageLimits::new(512, 1_000, 128);
    assert_eq!(
      limits.validate(ContextMessageGeometry::new(256, 256, 500, 500, 128)),
      Ok(())
    );
    for geometry in [
      ContextMessageGeometry::new(256, 257, 0, 0, 0),
      ContextMessageGeometry::new(0, 0, 500, 501, 128),
      ContextMessageGeometry::new(0, 0, 0, 0, 129),
    ] {
      assert_eq!(
        limits.validate(geometry),
        Err(BlockResourceError::ContextGeometryExceeded)
      );
    }
    assert_eq!(
      limits.validate(ContextMessageGeometry::new(u32::MAX, 1, 0, 0, 0)),
      Err(BlockResourceError::ArithmeticOverflow)
    );
  }

  #[test]
  fn block_phase_is_block_tagged_one_way_and_duplicate_safe() {
    let mut state = BlockResourceState::new(7u64);
    assert_eq!(state.block_number(), 7);
    assert_eq!(state.phase(), BlockResourcePhase::ContextIncomplete);
    assert_eq!(state.ensure_block(8), Err(BlockResourceError::WrongBlock));
    assert_eq!(state.begin_prepass(), Ok(()));
    assert_eq!(state.begin_prepass(), Err(BlockResourceError::InvalidPhase));
    assert_eq!(state.begin_drain(), Err(BlockResourceError::InvalidPhase));
    assert_eq!(state.open_external_phase(), Ok(()));
    assert_eq!(state.begin_drain(), Ok(()));
    state.halt_optional_actor_work();
    assert!(state.optional_actor_work_halted());
    let budget = BlockResourceBudget::new(weight(110, 110), weight(10, 10));
    assert!(budget.is_ok());
    let budget = budget.unwrap_or(BlockResourceBudget {
      maximum_block: Weight::zero(),
      fixed_envelope: Weight::zero(),
      limits: limits(),
    });
    assert_eq!(
      state.finalized_snapshot(),
      Err(BlockResourceError::InvalidPhase)
    );
    assert_eq!(state.finish_drain(budget, weight(10, 10)), Ok(()));
    assert_eq!(state.phase(), BlockResourcePhase::Finalizable);
    let snapshot = state.finalized_snapshot();
    assert!(snapshot.is_ok());
    let snapshot = snapshot.unwrap_or(FinalizedBlockResourceSnapshot {
      block_number: 0,
      fixed_reserved: Weight::zero(),
      usage: BlockResourceUsage::default(),
      optional_actor_work_halted: false,
    });
    assert_eq!(snapshot.block_number(), 7);
    assert_eq!(snapshot.fixed_reserved(), weight(10, 10));
    assert!(snapshot.optional_actor_work_halted());
    assert_eq!(
      state.finish_drain(budget, weight(10, 10)),
      Err(BlockResourceError::InvalidPhase)
    );
  }

  #[test]
  fn finalization_reconciliation_fails_closed_without_advancing_phase() {
    let budget = BlockResourceBudget::new(weight(110, 110), weight(10, 10));
    assert!(budget.is_ok());
    let budget = budget.unwrap_or(BlockResourceBudget {
      maximum_block: Weight::zero(),
      fixed_envelope: Weight::zero(),
      limits: limits(),
    });
    let mut state = BlockResourceState::new(1u64);
    state.phase = BlockResourcePhase::FreshDrain;
    state.usage.actor_control = weight(21, 1);
    assert_eq!(
      state.finish_drain(budget, weight(1, 1)),
      Err(BlockResourceError::ReconciliationMismatch)
    );
    assert_eq!(state.phase(), BlockResourcePhase::FreshDrain);
  }

  #[test]
  fn phase_owner_admits_only_its_domains_and_halt_preserves_user_dispatch() {
    let mut state = BlockResourceState::new(1u64);
    assert_eq!(
      state.reserve(limits(), BlockResourceDomain::UserDispatch, weight(1, 1)),
      Err(BlockResourceError::InvalidPhase)
    );
    assert_eq!(state.begin_prepass(), Ok(()));
    let base = state.reserve(limits(), BlockResourceDomain::ActorBaseEffect, weight(1, 1));
    assert!(base.is_ok());
    let mut base = base.unwrap_or(BlockResourceReservation {
      domain: BlockResourceDomain::ActorBaseEffect,
      maximum: Weight::zero(),
      phase: BlockResourcePhase::PrepassExecuting,
      settled: false,
    });
    assert_eq!(state.outstanding_reservations(), 1);
    assert_eq!(
      state.open_external_phase(),
      Err(BlockResourceError::ReservationOutstanding)
    );
    assert_eq!(state.settle(&mut base, weight(1, 1)), Ok(()));
    assert_eq!(state.outstanding_reservations(), 0);
    assert_eq!(state.open_external_phase(), Ok(()));
    assert_eq!(
      state.reserve(limits(), BlockResourceDomain::ActorBaseEffect, weight(1, 1),),
      Err(BlockResourceError::InvalidPhase)
    );
    state.halt_optional_actor_work();
    assert_eq!(
      state.reserve(limits(), BlockResourceDomain::ActorControl, weight(1, 1)),
      Err(BlockResourceError::OptionalActorWorkHalted)
    );
    assert!(
      state
        .reserve(limits(), BlockResourceDomain::UserDispatch, weight(1, 1))
        .is_ok()
    );
  }

  #[test]
  fn candidate_control_ratios_preserve_exact_component_partitions() {
    for (numerator, denominator, expected_control, expected_shared) in [
      (1, 5, 20, 80),
      (1, 4, 25, 75),
      (3, 10, 30, 70),
      (1, 3, 33, 67),
    ] {
      let candidate = BlockResourceBudget::new_with_control_ratio(
        weight(100, 100),
        Weight::zero(),
        numerator,
        denominator,
      );
      assert!(candidate.is_ok());
      let budget = candidate.unwrap_or_else(|_| BlockResourceBudget::fail_closed(weight(100, 100)));
      assert_eq!(
        budget.limits().actor_control(),
        weight(expected_control, expected_control)
      );
      assert_eq!(
        budget.limits().shared_economic(),
        weight(expected_shared, expected_shared)
      );
      assert_eq!(
        budget
          .limits()
          .actor_base_turn()
          .checked_add(&budget.limits().user_base_turn()),
        Some(budget.limits().shared_economic())
      );
    }
    for (numerator, denominator) in [(1, 0), (2, 1)] {
      assert_eq!(
        BlockResourceBudget::new_with_control_ratio(
          weight(100, 100),
          Weight::zero(),
          numerator,
          denominator,
        ),
        Err(BlockResourceError::InvalidLimits)
      );
    }
  }

  #[test]
  fn candidate_ratios_preserve_saturated_base_turns_and_forward_borrowing() {
    for (numerator, denominator) in [(1, 5), (1, 4), (3, 10), (1, 3)] {
      let budget = BlockResourceBudget::new_with_control_ratio(
        weight(100, 100),
        Weight::zero(),
        numerator,
        denominator,
      )
      .unwrap_or_else(|_| BlockResourceBudget::fail_closed(weight(100, 100)));
      let limits = budget.limits();
      let mut saturated = BlockResourceUsage::default();
      assert!(
        saturated
          .reserve(
            limits,
            BlockResourceDomain::ActorBaseEffect,
            limits.actor_base_turn(),
          )
          .is_ok()
      );
      assert!(
        saturated
          .reserve(
            limits,
            BlockResourceDomain::UserDispatch,
            limits.user_base_turn(),
          )
          .is_ok()
      );
      assert_eq!(saturated.shared_used(), Ok(limits.shared_economic()));

      let mut borrowed = BlockResourceUsage::default();
      let actor_actual = weight_floor_div(limits.actor_base_turn(), 2);
      let mut actor = borrowed
        .reserve(
          limits,
          BlockResourceDomain::ActorBaseEffect,
          limits.actor_base_turn(),
        )
        .unwrap_or(BlockResourceReservation {
          domain: BlockResourceDomain::ActorBaseEffect,
          maximum: Weight::zero(),
          phase: BlockResourcePhase::ContextIncomplete,
          settled: true,
        });
      assert_eq!(borrowed.settle(&mut actor, actor_actual), Ok(()));
      let user_remainder = limits.shared_economic().saturating_sub(actor_actual);
      assert!(
        borrowed
          .reserve(limits, BlockResourceDomain::UserDispatch, user_remainder,)
          .is_ok()
      );
      assert_eq!(borrowed.shared_used(), Ok(limits.shared_economic()));
    }
  }

  #[test]
  fn limits_follow_floor_and_remainder_ownership_exactly() {
    assert_eq!(
      BlockResourceLimits::new(
        weight(20, 20),
        weight(80, 80),
        weight(40, 40),
        weight(40, 40),
      ),
      Ok(limits())
    );
    assert_eq!(
      BlockResourceLimits::new(
        weight(20, 21),
        weight(83, 86),
        weight(41, 43),
        weight(42, 43),
      ),
      Ok(BlockResourceLimits {
        actor_control: weight(20, 21),
        shared_economic: weight(83, 86),
        actor_base_turn: weight(41, 43),
        user_base_turn: weight(42, 43),
      })
    );
    assert_eq!(
      BlockResourceLimits::new(
        weight(30, 30),
        weight(70, 70),
        weight(35, 35),
        weight(35, 35),
      ),
      Ok(BlockResourceLimits {
        actor_control: weight(30, 30),
        shared_economic: weight(70, 70),
        actor_base_turn: weight(35, 35),
        user_base_turn: weight(35, 35),
      })
    );
    assert_eq!(
      BlockResourceLimits::new(
        weight(20, 20),
        weight(80, 80),
        weight(39, 40),
        weight(41, 40),
      ),
      Err(BlockResourceError::InvalidLimits)
    );
  }

  #[test]
  fn actor_base_cannot_borrow_user_turn_in_either_dimension() {
    for maximum in [weight(41, 1), weight(1, 41)] {
      let mut usage = BlockResourceUsage::default();
      assert_eq!(
        usage.reserve(limits(), BlockResourceDomain::ActorBaseEffect, maximum),
        Err(BlockResourceError::LimitExceeded)
      );
      assert_eq!(usage, BlockResourceUsage::default());
    }
  }

  #[test]
  fn user_and_drain_borrow_only_actual_shared_remainder() {
    let mut usage = BlockResourceUsage::default();
    let mut actor = retained_reservation(
      usage.reserve(
        limits(),
        BlockResourceDomain::ActorBaseEffect,
        weight(10, 10),
      ),
      BlockResourceDomain::ActorBaseEffect,
      weight(10, 10),
    );
    assert_eq!(usage.settle(&mut actor, weight(8, 8)), Ok(()));
    let _user = retained_reservation(
      usage.reserve(limits(), BlockResourceDomain::UserDispatch, weight(60, 60)),
      BlockResourceDomain::UserDispatch,
      weight(60, 60),
    );
    assert_eq!(
      usage.reserve(
        limits(),
        BlockResourceDomain::ActorDrainEffect,
        weight(13, 1)
      ),
      Err(BlockResourceError::LimitExceeded)
    );
    let _drain = retained_reservation(
      usage.reserve(
        limits(),
        BlockResourceDomain::ActorDrainEffect,
        weight(12, 12),
      ),
      BlockResourceDomain::ActorDrainEffect,
      weight(12, 12),
    );
    assert_eq!(usage.shared_used(), Ok(weight(80, 80)));
  }

  #[test]
  fn reservation_settles_only_valid_component_wise_actual() {
    let mut usage = BlockResourceUsage::default();
    let mut reservation = retained_reservation(
      usage.reserve(limits(), BlockResourceDomain::ActorControl, weight(10, 10)),
      BlockResourceDomain::ActorControl,
      weight(10, 10),
    );
    assert_eq!(
      usage.settle(&mut reservation, weight(11, 9)),
      Err(BlockResourceError::ActualExceedsReserved)
    );
    assert_eq!(usage.actor_control, weight(10, 10));
    assert_eq!(usage.settle(&mut reservation, weight(7, 8)), Ok(()));
    assert_eq!(usage.actor_control, weight(7, 8));
    assert_eq!(
      usage.settle(&mut reservation, weight(7, 8)),
      Err(BlockResourceError::ReservationAlreadySettled)
    );
  }

  #[test]
  fn small_weight_grid_preserves_partition_and_component_admission() {
    for ref_time in 0..=12 {
      for proof_size in 0..=12 {
        let schedulable = weight(ref_time, proof_size);
        let budget = BlockResourceBudget::new(schedulable, Weight::zero());
        assert!(budget.is_ok());
        let budget = budget.unwrap_or(BlockResourceBudget {
          maximum_block: Weight::zero(),
          fixed_envelope: Weight::zero(),
          limits: BlockResourceLimits::from_schedulable_ratio(Weight::zero(), 1, 5)
            .unwrap_or(limits()),
        });
        let limits = budget.limits();
        assert_eq!(
          limits
            .actor_control()
            .checked_add(&limits.shared_economic()),
          Some(schedulable)
        );
        assert_eq!(
          limits
            .actor_base_turn()
            .checked_add(&limits.user_base_turn()),
          Some(limits.shared_economic())
        );

        for candidate_ref in 0..=ref_time.saturating_add(1) {
          for candidate_proof in 0..=proof_size.saturating_add(1) {
            let candidate = weight(candidate_ref, candidate_proof);
            let mut usage = BlockResourceUsage::default();
            assert_eq!(
              usage
                .reserve(limits, BlockResourceDomain::ActorBaseEffect, candidate)
                .is_ok(),
              candidate.all_lte(limits.actor_base_turn())
            );
          }
        }
      }
    }
  }

  #[test]
  fn shared_envelope_is_work_conserving_after_actor_and_user_turns() {
    let limits = limits();
    for actor_ref in (0..=40).step_by(10) {
      for actor_proof in (0..=40).step_by(10) {
        let actor = weight(actor_ref, actor_proof);
        for user_ref in (0..=80).step_by(10) {
          for user_proof in (0..=80).step_by(10) {
            let user = weight(user_ref, user_proof);
            let mut usage = BlockResourceUsage::default();
            assert!(
              usage
                .reserve(limits, BlockResourceDomain::ActorBaseEffect, actor)
                .is_ok()
            );
            let expected_user = actor
              .checked_add(&user)
              .map(|total| total.all_lte(limits.shared_economic()))
              .unwrap_or(false);
            let user_result = usage.reserve(limits, BlockResourceDomain::UserDispatch, user);
            assert_eq!(user_result.is_ok(), expected_user);
            if !expected_user {
              continue;
            }
            let remainder = limits
              .shared_economic()
              .checked_sub(&actor)
              .and_then(|available| available.checked_sub(&user));
            assert!(remainder.is_some());
            let remainder = remainder.unwrap_or(Weight::zero());
            let before_drain = usage;
            assert!(
              usage
                .reserve(limits, BlockResourceDomain::ActorDrainEffect, remainder,)
                .is_ok()
            );
            assert_eq!(usage.shared_used(), Ok(limits.shared_economic()));

            let mut ref_overrun = before_drain;
            assert_eq!(
              ref_overrun.reserve(
                limits,
                BlockResourceDomain::ActorDrainEffect,
                remainder.saturating_add(weight(1, 0)),
              ),
              Err(BlockResourceError::LimitExceeded)
            );
            let mut proof_overrun = before_drain;
            assert_eq!(
              proof_overrun.reserve(
                limits,
                BlockResourceDomain::ActorDrainEffect,
                remainder.saturating_add(weight(0, 1)),
              ),
              Err(BlockResourceError::LimitExceeded)
            );
          }
        }
      }
    }
  }

  #[test]
  fn untrustworthy_actor_actual_halts_actor_work_but_user_failure_does_not() {
    let mut actor_state = BlockResourceState::new(1u64);
    assert_eq!(actor_state.begin_prepass(), Ok(()));
    let actor = actor_state.reserve(limits(), BlockResourceDomain::ActorControl, weight(1, 1));
    assert!(actor.is_ok());
    let mut actor = actor.unwrap_or(BlockResourceReservation {
      domain: BlockResourceDomain::ActorControl,
      maximum: Weight::zero(),
      phase: BlockResourcePhase::PrepassExecuting,
      settled: false,
    });
    assert_eq!(
      actor_state.settle(&mut actor, weight(2, 1)),
      Err(BlockResourceError::ActualExceedsReserved)
    );
    assert!(actor_state.optional_actor_work_halted());

    let mut user_state = BlockResourceState::new(1u64);
    assert_eq!(user_state.begin_prepass(), Ok(()));
    assert_eq!(user_state.open_external_phase(), Ok(()));
    let user = user_state.reserve(limits(), BlockResourceDomain::UserDispatch, weight(1, 1));
    assert!(user.is_ok());
    let mut user = user.unwrap_or(BlockResourceReservation {
      domain: BlockResourceDomain::UserDispatch,
      maximum: Weight::zero(),
      phase: BlockResourcePhase::ExternalPhase,
      settled: false,
    });
    assert_eq!(
      user_state.settle(&mut user, weight(1, 2)),
      Err(BlockResourceError::ActualExceedsReserved)
    );
    assert!(!user_state.optional_actor_work_halted());
  }

  #[test]
  fn settlement_grid_accepts_exactly_component_wise_actuals() {
    for maximum_ref in 0..=4 {
      for maximum_proof in 0..=4 {
        for actual_ref in 0..=5 {
          for actual_proof in 0..=5 {
            let maximum = weight(maximum_ref, maximum_proof);
            let actual = weight(actual_ref, actual_proof);
            let mut state = BlockResourceState::new(1u64);
            assert_eq!(state.begin_prepass(), Ok(()));
            let reservation = state.reserve(limits(), BlockResourceDomain::ActorControl, maximum);
            assert!(reservation.is_ok());
            let mut reservation = reservation.unwrap_or(BlockResourceReservation {
              domain: BlockResourceDomain::ActorControl,
              maximum,
              phase: BlockResourcePhase::PrepassExecuting,
              settled: false,
            });
            assert_eq!(
              state.settle(&mut reservation, actual).is_ok(),
              actual.all_lte(maximum)
            );
          }
        }
      }
    }
  }

  #[test]
  fn arithmetic_overflow_is_not_reported_as_capacity_pressure() {
    let mut usage = BlockResourceUsage {
      actor_control: Weight::MAX,
      ..Default::default()
    };
    assert_eq!(
      usage.reserve(limits(), BlockResourceDomain::ActorControl, weight(1, 0)),
      Err(BlockResourceError::ArithmeticOverflow)
    );
  }

  #[test]
  fn capacity_probe_is_read_only_and_distinguishes_owner_errors() {
    let domain = BlockResourceDomain::ActorDrainEffect;
    let initial = BlockResourceState::new(1u64);
    let mut active = initial;
    active.phase = BlockResourcePhase::FreshDrain;
    let mut halted = active;
    halted.halt_optional_actor_work();
    let mut overflow = active;
    overflow.usage.actor_effect = Weight::MAX;
    let mut reservation_overflow = active;
    reservation_overflow.outstanding_reservations = u32::MAX;
    for (state, maximum, exceeded) in [
      (initial, Weight::MAX, false),
      (active, Weight::zero(), false),
      (active, weight(80, 80), false),
      (active, weight(81, 0), true),
      (active, weight(0, 81), true),
      (halted, Weight::MAX, false),
      (overflow, weight(1, 0), false),
      (reservation_overflow, weight(1, 0), false),
    ] {
      let before = state;
      assert_eq!(state.capacity_exceeded(limits(), domain, maximum), exceeded);
      assert_eq!(state, before);
    }
  }

  #[test]
  fn fixed_components_are_complete_checked_and_feed_the_budget() {
    let components = FixedBlockWeightComponents::new(
      weight(1, 2),
      weight(3, 4),
      weight(5, 6),
      weight(7, 8),
      weight(9, 10),
    );
    assert_eq!(components.total(), Ok(weight(25, 30)));
    assert_eq!(
      BlockResourceBudget::from_fixed_components(weight(125, 130), components),
      BlockResourceBudget::new(weight(125, 130), weight(25, 30))
    );
    let overflowing = FixedBlockWeightComponents::new(
      Weight::MAX,
      weight(1, 0),
      Weight::zero(),
      Weight::zero(),
      Weight::zero(),
    );
    assert_eq!(
      overflowing.total(),
      Err(BlockResourceError::ArithmeticOverflow)
    );
  }

  #[test]
  fn whole_block_fixed_envelope_cannot_admit_a_mandatory_prepass() {
    let budget = BlockResourceBudget::new(weight(100, 100), weight(100, 100));
    assert!(budget.is_ok());
    let budget = budget.unwrap_or(BlockResourceBudget {
      maximum_block: Weight::zero(),
      fixed_envelope: Weight::zero(),
      limits: limits(),
    });
    assert_eq!(budget.limits().actor_control(), Weight::zero());
    let mut state = BlockResourceState::new(1u64);
    assert_eq!(state.begin_prepass(), Ok(()));
    assert_eq!(
      state.reserve(
        budget.limits(),
        BlockResourceDomain::ActorControl,
        weight(1, 1),
      ),
      Err(BlockResourceError::LimitExceeded)
    );
  }

  #[test]
  fn budget_derives_schedulable_limits_and_rejects_component_underflow() {
    let budget = BlockResourceBudget::new(weight(110, 210), weight(10, 10));
    assert_eq!(
      budget,
      Ok(BlockResourceBudget {
        maximum_block: weight(110, 210),
        fixed_envelope: weight(10, 10),
        limits: BlockResourceLimits {
          actor_control: weight(20, 40),
          shared_economic: weight(80, 160),
          actor_base_turn: weight(40, 80),
          user_base_turn: weight(40, 80),
        },
      })
    );
    assert_eq!(
      BlockResourceBudget::new(weight(110, 9), weight(10, 10)),
      Err(BlockResourceError::InvalidLimits)
    );
  }

  #[test]
  fn block_reconciliation_rejects_fixed_overrun_and_corrupt_usage() {
    let budget = BlockResourceBudget::new(weight(110, 110), weight(10, 10));
    assert!(budget.is_ok());
    let budget = budget.unwrap_or(BlockResourceBudget {
      maximum_block: Weight::zero(),
      fixed_envelope: Weight::zero(),
      limits: limits(),
    });
    let usage = BlockResourceUsage {
      actor_control: weight(20, 10),
      actor_effect: weight(30, 40),
      user_dispatch: weight(40, 30),
    };
    assert_eq!(usage.reconcile_with_block(budget, weight(10, 10)), Ok(()));
    assert_eq!(
      usage.reconcile_with_block(budget, weight(11, 10)),
      Err(BlockResourceError::FixedEnvelopeExceeded)
    );
    let corrupt = BlockResourceUsage {
      actor_control: weight(21, 10),
      ..usage
    };
    assert_eq!(
      corrupt.reconcile_with_block(budget, weight(1, 1)),
      Err(BlockResourceError::ReconciliationMismatch)
    );
  }
}
