/*
Domain: AAA deferred-event projection validation
Owns: Client regressions for fresh and Continuation CycleDeferred candidates.
Excludes: Runtime candidate derivation, transport decoding, and UI rendering.
Zone: Automation validation entrypoint.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { projectAaaCycleDeferred } from '../src/lib/automation/cycle-deferred.ts';

test('fresh deferral projects the checked next cycle with opening attempt and cursor', () => {
  assert.deepEqual(
    projectAaaCycleDeferred({
      candidate_cycle_nonce: 1n,
      candidate_attempt: 0,
      cursor: 0,
      reason: { type: 'RefTime' },
    }),
    {
      candidateCycleNonce: '1',
      candidateAttempt: '0',
      cursor: '0',
      reason: 'RefTime',
    },
  );
});

test('Continuation deferral retains cycle and cursor while projecting next attempt', () => {
  assert.deepEqual(
    projectAaaCycleDeferred({
      candidate_cycle_nonce: 7n,
      candidate_attempt: 3,
      cursor: 4,
      reason: { type: 'ProofSize' },
    }),
    {
      candidateCycleNonce: '7',
      candidateAttempt: '3',
      cursor: '4',
      reason: 'ProofSize',
    },
  );
});

test('malformed candidate fields remain visibly unknown instead of inferred', () => {
  assert.deepEqual(projectAaaCycleDeferred({}), {
    candidateCycleNonce: '?',
    candidateAttempt: '?',
    cursor: '?',
    reason: 'unknown',
  });
});
