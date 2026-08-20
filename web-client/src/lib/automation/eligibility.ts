/*
Domain: Actors eligibility projection
Owns: Canonical mapping of the read-only runtime `ActorEligibilityApi` result into the browser domain.
Excludes: Chain transport, storage reads, scheduler execution, and plan authoring.
Zone: Automation public contract; adapters and widgets import it, never reimplementing cadence,
cooldown, window, retry backoff, breaker, or latch arithmetic.
*/
export const ACTORS_ELIGIBILITY_RUNTIME_API =
  'ActorEligibilityApi_actor_eligibility' as const;
export const ACTORS_ELIGIBILITY_RUNTIME_API_VERSION = 1 as const;

export type ActorEligibilityFailure =
  | 'ActorInvariant'
  | 'ContinuationInvariant'
  | 'ComputationOverflow';

export type ActorExecutionPhase =
  | { type: 'Ready' }
  | { type: 'Paused' }
  | { type: 'GlobalCircuitBreaker' }
  | { type: 'WaitingSignal' }
  | { type: 'WaitingRetry'; block: number }
  | { type: 'WaitingBlock'; block: number }
  | { type: 'WaitingCadenceTick'; tick: number };

export type ActorEligibilityView =
  | { type: 'NotRegistered' }
  | { type: 'Dormant' }
  | {
      type: 'Active';
      terminalReason: string | null;
      executionPhase: ActorExecutionPhase;
    };

const ELIGIBILITY_FAILURES: ReadonlySet<string> = new Set([
  'ActorInvariant',
  'ContinuationInvariant',
  'ComputationOverflow',
]);

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
  const classification = asRecord(
    eligibility.value,
    'ActorEligibility.Active classification',
  );
  return {
    type: 'Active',
    terminalReason:
      classification.terminal_reason === undefined
        ? null
        : asVariant(
            classification.terminal_reason,
            'ActorClassification.terminal_reason',
          ).type,
    executionPhase: projectExecutionPhase(classification.execution_phase),
  };
}
