/*
Domain: Actors eligibility projection
Owns: Canonical mapping of the read-only runtime `ActorEligibilityApi` result into the browser domain.
Excludes: Chain transport, storage reads, scheduler execution, and plan authoring.
Zone: Automation public contract; adapters and widgets import it, never reimplementing cadence,
cooldown, window, retry backoff, breaker, or latch arithmetic.
*/
export const ACTORS_ELIGIBILITY_RUNTIME_API =
  'ActorEligibilityApi_actor_eligibility' as const;
export const ACTORS_ELIGIBILITY_RUNTIME_API_VERSION = 4 as const;
export const ACTORS_TRIGGER_STATE_BOND_RUNTIME_API =
  'ActorEligibilityApi_trigger_state_bond' as const;

export type ActorEligibilityFailure =
  | 'ActorInvariant'
  | 'ContinuationInvariant'
  | 'ComputationOverflow';

export const ACTOR_CLOSE_REASONS = [
  'OwnerInitiated',
  'BalanceExhausted',
  'ConsecutiveFailures',
  'WindowExpired',
  'CycleNonceExhausted',
  'FeeBudgetExhausted',
  'AutoCloseNonceReached',
  'RetryAttemptsExhausted',
  'ProductiveCycleCompleted',
  'SchedulerIndexExhausted',
] as const;

export type ActorCloseReason = (typeof ACTOR_CLOSE_REASONS)[number];

export type ActorExecutionPhase =
  | { type: 'Ready' }
  | { type: 'Paused' }
  | { type: 'GlobalCircuitBreaker' }
  | { type: 'WaitingSignal' }
  | { type: 'WaitingRetry'; block: number }
  | { type: 'WaitingBlock'; block: number }
  | { type: 'WaitingCadenceTick'; tick: number };

export type ActorActivationPlacement =
  | { type: 'Unplaced' }
  | { type: 'Queue'; ticket: bigint }
  | { type: 'WakeupBlock'; block: number }
  | { type: 'WakeupTick'; tick: bigint };

export type ActorTriggerActivation =
  | { type: 'Manual' }
  | { type: 'AddressEvent' }
  | {
      type: 'ObservationChange';
      feed: unknown;
      subscriberCount: number;
      pendingRevision: bigint | null;
    }
  | {
      type: 'ObservationCrossing';
      feed: unknown;
      direction: 'Rising' | 'Falling';
      threshold: bigint;
      rearmThreshold: bigint;
      phase: 'Armed' | 'WaitingForRearm';
      installedAtRevision: bigint;
      pendingRevisions: number;
      processingRevision: bigint | null;
    }
  | { type: 'Cadenced'; everyTicks: bigint };

export type ActorEligibilityView =
  | { type: 'NotRegistered' }
  | { type: 'Dormant' }
  | {
      type: 'Active';
      trigger: ActorTriggerActivation;
      pendingSignal: boolean;
      placement: ActorActivationPlacement;
      terminalReason: ActorCloseReason | null;
      executionPhase: ActorExecutionPhase;
    };

const ELIGIBILITY_FAILURES: ReadonlySet<string> = new Set([
  'ActorInvariant',
  'ContinuationInvariant',
  'ComputationOverflow',
]);

const CLOSE_REASONS: ReadonlySet<string> = new Set(ACTOR_CLOSE_REASONS);

const SCALAR_EXECUTION_PHASES: ReadonlySet<string> = new Set([
  'Ready',
  'Paused',
  'GlobalCircuitBreaker',
  'WaitingSignal',
]);

function asRecord(value: unknown, field: string): Record<string, unknown> {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field} must be a runtime object`);
  }
  return value as Record<string, unknown>;
}

function asVariant(value: unknown, field: string) {
  const variant = asRecord(value, field);
  if (typeof variant.type !== 'string' || variant.type.length === 0) {
    throw new Error(`${field} must carry a runtime variant type`);
  }
  return variant as Record<string, unknown> & {
    type: string;
    value?: unknown;
  };
}

function asBlock(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
  return value;
}

function asTick(value: unknown): number {
  if (typeof value === 'bigint') {
    const tick = Number(value);
    if (value >= 0n && Number.isSafeInteger(tick)) return tick;
  }
  return asBlock(value, 'WaitingCadenceTick tick');
}

function asU64(value: unknown, field: string): bigint {
  if (typeof value === 'bigint' && value >= 0n) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  throw new Error(`${field} must be an unsigned runtime integer`);
}

function asU128(value: unknown, field: string): bigint {
  const parsed = asU64(value, field);
  if (parsed >= 1n << 128n) throw new Error(`${field} must fit u128`);
  return parsed;
}

function asCount(value: unknown, field: string): number {
  return asBlock(value, field);
}

function optionalU64(value: unknown, field: string): bigint | null {
  return value === undefined ? null : asU64(value, field);
}

function projectPlacement(value: unknown): ActorActivationPlacement {
  const placement = asVariant(value, 'ActiveActorActivation.placement');
  if (placement.type === 'Unplaced') return { type: 'Unplaced' };
  if (placement.type === 'Queue') {
    return { type: 'Queue', ticket: asU64(placement.value, 'queue ticket') };
  }
  if (placement.type === 'Wakeup') {
    const wakeup = asVariant(placement.value, 'activation wakeup');
    if (wakeup.type === 'Block') {
      return {
        type: 'WakeupBlock',
        block: asBlock(wakeup.value, 'wakeup block'),
      };
    }
    if (wakeup.type === 'Tick') {
      return { type: 'WakeupTick', tick: asU64(wakeup.value, 'wakeup tick') };
    }
    throw new Error(`Unsupported runtime wakeup placement ${wakeup.type}`);
  }
  throw new Error(`Unsupported runtime activation placement ${placement.type}`);
}

function projectTriggerActivation(value: unknown): ActorTriggerActivation {
  const trigger = asVariant(value, 'ActiveActorActivation.trigger');
  if (trigger.type === 'Manual' || trigger.type === 'AddressEvent') {
    return { type: trigger.type };
  }
  const fields = asRecord(
    trigger.value,
    `ActorTriggerActivation.${trigger.type}`,
  );
  if (trigger.type === 'ObservationChange') {
    return {
      type: trigger.type,
      feed: fields.feed,
      subscriberCount: asCount(fields.subscriber_count, 'subscriber count'),
      pendingRevision: optionalU64(fields.pending_revision, 'pending revision'),
    };
  }
  if (trigger.type === 'ObservationCrossing') {
    const direction = asVariant(fields.direction, 'Crossing direction').type;
    if (direction !== 'Rising' && direction !== 'Falling') {
      throw new Error(`Unsupported runtime Crossing direction ${direction}`);
    }
    const phase = asVariant(fields.phase, 'Crossing phase').type;
    if (phase !== 'Armed' && phase !== 'WaitingForRearm') {
      throw new Error(`Unsupported runtime Crossing phase ${phase}`);
    }
    return {
      type: trigger.type,
      feed: fields.feed,
      direction,
      threshold: asU128(fields.threshold, 'Crossing fire threshold'),
      rearmThreshold: asU128(
        fields.rearm_threshold,
        'Crossing rearm threshold',
      ),
      phase,
      installedAtRevision: asU64(
        fields.installed_at_revision,
        'Crossing installation revision',
      ),
      pendingRevisions: asCount(
        fields.pending_revisions,
        'pending Crossing revisions',
      ),
      processingRevision: optionalU64(
        fields.processing_revision,
        'processing Crossing revision',
      ),
    };
  }
  if (trigger.type === 'Cadenced') {
    return {
      type: trigger.type,
      everyTicks: asU64(fields.every_ticks, 'cadence ticks'),
    };
  }
  throw new Error(`Unsupported runtime trigger activation ${trigger.type}`);
}

function projectCloseReason(value: unknown): ActorCloseReason {
  const reason = asVariant(value, 'ActorClassification.terminal_reason').type;
  if (!CLOSE_REASONS.has(reason)) {
    throw new Error(`Unsupported runtime close reason ${reason}`);
  }
  return reason as ActorCloseReason;
}

function projectExecutionPhase(value: unknown): ActorExecutionPhase {
  const phase = asVariant(value, 'ActorClassification.execution_phase');
  if (SCALAR_EXECUTION_PHASES.has(phase.type)) {
    return { type: phase.type } as ActorExecutionPhase;
  }
  if (phase.type === 'WaitingRetry' || phase.type === 'WaitingBlock') {
    return {
      type: phase.type,
      block: asBlock(phase.value, `${phase.type} block`),
    };
  }
  if (phase.type === 'WaitingCadenceTick') {
    return {
      type: phase.type,
      tick: asTick(phase.value),
    };
  }
  throw new Error(`Unsupported runtime execution phase ${phase.type}`);
}

export function projectActorEligibility(value: unknown): ActorEligibilityView {
  const result = asRecord(value, 'runtime Result');
  if (result.success === false) {
    const failure = asVariant(result.value, 'eligibility error').type;
    if (!ELIGIBILITY_FAILURES.has(failure)) {
      throw new Error(`Unsupported runtime eligibility error ${failure}`);
    }
    throw new Error(
      `Runtime eligibility projection rejected: ${failure as ActorEligibilityFailure}`,
    );
  }
  if (result.success !== true) {
    throw new Error('Runtime eligibility output must be a SCALE Result');
  }
  const eligibility = asVariant(result.value, 'ActorEligibility');
  if (eligibility.type === 'NotRegistered' || eligibility.type === 'Dormant') {
    return { type: eligibility.type };
  }
  if (eligibility.type !== 'Active') {
    throw new Error(`Unsupported runtime eligibility ${eligibility.type}`);
  }
  const activation = asRecord(
    eligibility.value,
    'ActorEligibility.Active activation',
  );
  if (typeof activation.pending_signal !== 'boolean') {
    throw new Error('ActiveActorActivation.pending_signal must be boolean');
  }
  const classification = asRecord(
    activation.eligibility,
    'ActiveActorActivation.eligibility',
  );
  return {
    type: 'Active',
    trigger: projectTriggerActivation(activation.trigger),
    pendingSignal: activation.pending_signal,
    placement: projectPlacement(activation.placement),
    terminalReason:
      classification.terminal_reason === undefined
        ? null
        : projectCloseReason(classification.terminal_reason),
    executionPhase: projectExecutionPhase(classification.execution_phase),
  };
}
