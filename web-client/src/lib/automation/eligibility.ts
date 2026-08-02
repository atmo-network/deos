/*
Domain: AAA eligibility projection
Owns: Canonical mapping of the read-only runtime `AaaEligibilityApi` result into the browser domain.
Excludes: Chain transport, storage reads, scheduler execution, and plan authoring.
Zone: Automation public contract; adapters and widgets import it, never reimplementing cadence,
cooldown, window, retry backoff, breaker, or latch arithmetic.
*/
export const AAA_ELIGIBILITY_RUNTIME_API =
  'AaaEligibilityApi_aaa_eligibility' as const;
export const AAA_ELIGIBILITY_RUNTIME_API_VERSION = 1 as const;

export type AaaEligibilityPhase =
  | 'NotRegistered'
  | 'Dormant'
  | 'Ready'
  | 'Paused'
  | 'GlobalCircuitBreaker'
  | 'WindowExpired'
  | 'CycleNonceExhausted'
  | 'ConsecutiveFailureLimit'
  | 'AutoCloseDue'
  | 'WaitingSignal'
  | 'WaitingRetry'
  | 'WaitingTemporal';

export type AaaEligibilityFailure =
  | 'ComputationOverflow'
  | 'ContinuationInvariant';

export type AaaEligibilityProjection = {
  /** Scheduler readiness verdict at the read block; admission capacity is not guaranteed. */
  ready: boolean;
  /** Scheduler-owned reason for the verdict. */
  phase: AaaEligibilityPhase;
  /** Next block at which temporal eligibility opens, or null when none is computable. */
  nextEligibleBlock: number | null;
};

const ELIGIBILITY_PHASES: ReadonlySet<string> = new Set([
  'NotRegistered',
  'Dormant',
  'Ready',
  'Paused',
  'GlobalCircuitBreaker',
  'WindowExpired',
  'CycleNonceExhausted',
  'ConsecutiveFailureLimit',
  'AutoCloseDue',
  'WaitingSignal',
  'WaitingRetry',
  'WaitingTemporal',
]);

const ELIGIBILITY_FAILURES: ReadonlySet<string> = new Set([
  'ComputationOverflow',
  'ContinuationInvariant',
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
  return variant.type;
}

function asOptionalBlock(value: unknown, field: string): number | null {
  if (value === undefined) {
    return null;
  }
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
  return value;
}

export function projectAaaEligibility(
  value: unknown,
): AaaEligibilityProjection {
  const result = asRecord(value, 'runtime Result');
  if (result.success === false) {
    const failure = asVariant(result.value, 'eligibility error');
    if (!ELIGIBILITY_FAILURES.has(failure)) {
      throw new Error(`Unsupported runtime eligibility error ${failure}`);
    }
    throw new Error(
      `Runtime eligibility projection rejected: ${failure as AaaEligibilityFailure}`,
    );
  }
  if (result.success !== true) {
    throw new Error('Runtime eligibility output must be a SCALE Result');
  }
  const projection = asRecord(result.value, 'eligibility projection');
  if (typeof projection.ready !== 'boolean') {
    throw new Error('ready must be a runtime boolean');
  }
  const phase = asVariant(projection.phase, 'eligibility phase');
  if (!ELIGIBILITY_PHASES.has(phase)) {
    throw new Error(`Unsupported runtime eligibility phase ${phase}`);
  }
  return {
    ready: projection.ready,
    phase: phase as AaaEligibilityPhase,
    nextEligibleBlock: asOptionalBlock(
      projection.next_eligible_block,
      'next_eligible_block',
    ),
  };
}
