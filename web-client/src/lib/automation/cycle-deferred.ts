/*
Domain: AAA deferred-event projection
Owns: Exact client projection of runtime CycleDeferred candidate identity.
Excludes: Candidate derivation, scheduler admission, event transport, and UI state.
Zone: Automation runtime projection; preserves metadata field meaning without inference.
*/
export type AaaCycleDeferredProjection = {
  candidateCycleNonce: string;
  candidateAttempt: string;
  cursor: string;
  reason: string;
};

function property(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null
    ? Reflect.get(value, key)
    : undefined;
}

function projectedScalar(value: unknown): string {
  return value === undefined || value === null ? '?' : String(value);
}

export function projectAaaCycleDeferred(
  payload: unknown,
): AaaCycleDeferredProjection {
  const reason = property(payload, 'reason');
  const reasonType = property(reason, 'type') ?? reason;
  return {
    candidateCycleNonce: projectedScalar(
      property(payload, 'candidate_cycle_nonce'),
    ),
    candidateAttempt: projectedScalar(property(payload, 'candidate_attempt')),
    cursor: projectedScalar(property(payload, 'cursor')),
    reason: typeof reasonType === 'string' ? reasonType : 'unknown',
  };
}
