/*
Domain: Actors eligibility projection
Owns: Canonical mapping of the read-only runtime `ActorEligibilityApi` result into the browser domain.
Excludes: Chain transport, storage reads, scheduler execution, and plan authoring.
Zone: Automation public contract; adapters and widgets import it, never reimplementing cadence,
cooldown, window, retry backoff, breaker, or latch arithmetic.
*/
import {
  type ActorBehaviorEpisodeView,
  projectActorBehaviorEpisode,
} from './behavior-projection.ts';

export const ACTORS_ELIGIBILITY_RUNTIME_API =
  'ActorEligibilityApi_actor_eligibility' as const;
export const ACTORS_ELIGIBILITY_RUNTIME_API_VERSION = 8 as const;

export type ActorEligibilityFailure =
  | 'ActorInvariant'
  | 'RunInvariant'
  | 'ComputationOverflow';

export const ACTOR_CLOSE_REASONS = [
  'OwnerInitiated',
  'CycleAdmissionInsufficient',
  'TriggerAdmissionInsufficient',
  'ConsecutiveFailures',
  'WindowExpired',
  'CycleNonceExhausted',
  'AutoCloseNonceReached',
  'RetryAttemptsExhausted',
  'BehaviorPolicySatisfied',
  'FundingGraceExpired',
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

export type ActorActivationResidence =
  | { type: 'Service'; kind: 'Live' | 'Pending' }
  | { type: 'DeadlineBlock'; block: number }
  | { type: 'DeadlineTick'; tick: bigint }
  | { type: 'Parked'; reason: ActorParkReason; reviewAt: number | null }
  | { type: 'Disabled' };

export type ActorParkReason = 'ParkedBalanceBelowThreshold';

export type ActorTriggerActivation =
  | { type: 'Manual' }
  | { type: 'AddressEvent' }
  | { type: 'AtTime'; afterTicks: bigint; consumed: boolean }
  | { type: 'Cadenced'; everyTicks: bigint };

export type ActorUnpaidService =
  | { type: 'AtTimeTrigger'; dueTick: bigint }
  | { type: 'PipelineOpening'; cycleNonce: bigint }
  | {
      type: 'RunningAction';
      cycleNonce: bigint;
      cursor: number;
    }
  | {
      type: 'SuspendedAction';
      cycleNonce: bigint;
      cursor: number;
      unsuccessfulAttemptsAtCursor: number;
    };

export type ActorFinancialWaitView = {
  unpaidService: ActorUnpaidService;
  unpaidLiability: bigint;
  currentServiceSpendability: bigint;
  fundingShortfall: bigint;
  enteredAtMillis: bigint;
  expiryTick: bigint;
};

export type ActorEligibilityView =
  | { type: 'NotRegistered' }
  | { type: 'Dormant' }
  | {
      type: 'Active';
      trigger: ActorTriggerActivation;
      pendingSignal: boolean;
      residence: ActorActivationResidence;
      financialWait: ActorFinancialWaitView | null;
      behaviorEpisode: ActorBehaviorEpisodeView | null;
      terminalReason: ActorCloseReason | null;
      executionPhase: ActorExecutionPhase;
    };

const ELIGIBILITY_FAILURES: ReadonlySet<string> = new Set([
  'ActorInvariant',
  'RunInvariant',
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

function asUnsigned(value: unknown, field: string): bigint {
  if (typeof value === 'bigint' && value >= 0n) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  throw new Error(`${field} must be an unsigned runtime integer`);
}

function projectResidence(value: unknown): ActorActivationResidence {
  const residence = asVariant(value, 'ActiveActorActivation.residence');
  if (residence.type === 'Disabled') return { type: 'Disabled' };
  if (residence.type === 'Service') {
    const kind = asVariant(residence.value, 'Service residence kind').type;
    if (kind !== 'Live' && kind !== 'Pending') {
      throw new Error(`Unsupported runtime Service residence ${kind}`);
    }
    return { type: 'Service', kind };
  }
  if (residence.type === 'Deadline') {
    const key = asVariant(residence.value, 'activation Deadline');
    if (key.type === 'Block') {
      return {
        type: 'DeadlineBlock',
        block: asBlock(key.value, 'Deadline block'),
      };
    }
    if (key.type === 'Tick') {
      return {
        type: 'DeadlineTick',
        tick: asUnsigned(key.value, 'Deadline tick'),
      };
    }
    throw new Error(`Unsupported runtime Deadline key ${key.type}`);
  }
  if (residence.type === 'Parked') {
    const fields = asRecord(residence.value, 'Parked residence');
    const reason = asVariant(fields.reason, 'Parked reason').type;
    if (!['ParkedBalanceBelowThreshold'].includes(reason)) {
      throw new Error(`Unsupported runtime Parked reason ${reason}`);
    }
    return {
      type: 'Parked',
      reason: reason as ActorParkReason,
      reviewAt:
        fields.review_at === undefined
          ? null
          : asBlock(fields.review_at, 'Parked review block'),
    };
  }
  throw new Error(`Unsupported runtime activation residence ${residence.type}`);
}

function projectFinancialWait(value: unknown): ActorFinancialWaitView | null {
  if (value === undefined) return null;
  const wait = asRecord(value, 'ActiveActorActivation.financial_wait');
  const service = asVariant(wait.unpaid_service, 'financial unpaid service');
  const fields = asRecord(service.value, 'unpaid service fields');
  let unpaidService: ActorUnpaidService;
  if (service.type === 'AtTimeTrigger') {
    unpaidService = {
      type: service.type,
      dueTick: asUnsigned(fields.due_tick, 'owed AtTime tick'),
    };
  } else if (
    service.type === 'PipelineOpening' ||
    service.type === 'RunningAction' ||
    service.type === 'SuspendedAction'
  ) {
    const cycleNonce = asUnsigned(fields.cycle_nonce, 'unpaid Cycle nonce');
    if (cycleNonce === 0n)
      throw new Error('Unpaid Cycle nonce must be positive');
    if (service.type === 'PipelineOpening') {
      unpaidService = { type: service.type, cycleNonce };
    } else {
      const cursor = asBlock(fields.cursor, 'unpaid Action cursor');
      if (cursor > 0xffff_ffff)
        throw new Error('Unpaid Action cursor must fit u32');
      if (service.type === 'SuspendedAction') {
        const unsuccessfulAttemptsAtCursor = asBlock(
          fields.unsuccessful_attempts_at_cursor,
          'unpaid retry attempts',
        );
        if (unsuccessfulAttemptsAtCursor > 0xffff_ffff)
          throw new Error('Unpaid retry attempts must fit u32');
        unpaidService = {
          type: service.type,
          cycleNonce,
          cursor,
          unsuccessfulAttemptsAtCursor,
        };
      } else {
        unpaidService = { type: service.type, cycleNonce, cursor };
      }
    }
  } else {
    throw new Error(`Unsupported runtime unpaid service ${service.type}`);
  }
  const unpaidLiability = asUnsigned(
    wait.unpaid_liability,
    'unpaid service liability',
  );
  const currentServiceSpendability = asUnsigned(
    wait.current_service_spendability,
    'current service spendability',
  );
  const fundingShortfall = asUnsigned(
    wait.funding_shortfall,
    'funding shortfall',
  );
  if (
    unpaidLiability === 0n ||
    unpaidLiability >= 1n << 128n ||
    currentServiceSpendability >= 1n << 128n ||
    fundingShortfall >= 1n << 128n ||
    fundingShortfall !==
      (unpaidLiability > currentServiceSpendability
        ? unpaidLiability - currentServiceSpendability
        : 0n)
  ) {
    throw new Error(
      'Financial wait amounts must describe a valid u128 debt snapshot',
    );
  }
  return {
    unpaidService,
    unpaidLiability,
    currentServiceSpendability,
    fundingShortfall,
    enteredAtMillis: asUnsigned(
      wait.entered_at_millis,
      'financial wait entry time',
    ),
    expiryTick: asUnsigned(wait.expiry_tick, 'financial wait expiry tick'),
  };
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
  if (trigger.type === 'AtTime') {
    if (typeof fields.consumed !== 'boolean') {
      throw new Error('AtTime consumed state must be boolean');
    }
    return {
      type: trigger.type,
      afterTicks: asUnsigned(fields.after_ticks, 'AtTime delay ticks'),
      consumed: fields.consumed,
    };
  }
  if (trigger.type === 'Cadenced') {
    return {
      type: trigger.type,
      everyTicks: asUnsigned(fields.every_ticks, 'cadence ticks'),
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
    residence: projectResidence(activation.residence),
    financialWait: projectFinancialWait(activation.financial_wait),
    behaviorEpisode: projectActorBehaviorEpisode(activation.behavior_episode),
    terminalReason:
      classification.terminal_reason === undefined
        ? null
        : projectCloseReason(classification.terminal_reason),
    executionPhase: projectExecutionPhase(classification.execution_phase),
  };
}
