/*
Domain: Prepared Actors eligibility contract
Owns: Source-only canonical residence projection and refusal fixtures before the binding cohort.
Excludes: SCALE metadata conformance, generated descriptors, runtime execution, and chain access.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { projectActorEligibility } from '../src/lib/automation/eligibility.ts';

function active(residence, executionPhase = { type: 'Ready' }) {
  return {
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Manual' },
        pending_signal: true,
        residence,
        eligibility: {
          terminal_reason: undefined,
          execution_phase: executionPhase,
        },
      },
    },
  };
}

test('prepared eligibility retains both Service kinds and Disabled', () => {
  for (const kind of ['Pending', 'Live']) {
    const view = projectActorEligibility(
      active({ type: 'Service', value: { type: kind } }),
    );
    assert.deepEqual(view.residence, { type: 'Service', kind });
    assert.equal(view.pendingSignal, true);
  }
  assert.deepEqual(
    projectActorEligibility(active({ type: 'Disabled' })).residence,
    { type: 'Disabled' },
  );
});

test('prepared eligibility keeps Deadline payloads distinct and exact', () => {
  const tick = 18_446_744_073_709_551_615n;
  assert.deepEqual(
    projectActorEligibility(
      active({ type: 'Deadline', value: { type: 'Tick', value: tick } }),
    ).residence,
    { type: 'DeadlineTick', tick },
  );
  const view = projectActorEligibility(
    active(
      { type: 'Deadline', value: { type: 'Block', value: 31 } },
      { type: 'WaitingRetry', value: 31 },
    ),
  );
  assert.deepEqual(view.residence, { type: 'DeadlineBlock', block: 31 });
  assert.deepEqual(view.executionPhase, { type: 'WaitingRetry', block: 31 });
});

test('prepared eligibility retains typed Park reasons and optional review block', () => {
  for (const reason of ['ParkedBalanceBelowThreshold']) {
    for (const reviewAt of [undefined, 104]) {
      const view = projectActorEligibility(
        active({
          type: 'Parked',
          value: { reason: { type: reason }, review_at: reviewAt },
        }),
      );
      assert.deepEqual(view.residence, {
        type: 'Parked',
        reason,
        reviewAt: reviewAt ?? null,
      });
    }
  }
  for (const reason of [
    'PredicateFalse',
    'SourceUnavailable',
    'MonotonicBoundaryPassed',
  ]) {
    assert.throws(() =>
      projectActorEligibility(
        active({
          type: 'Parked',
          value: { reason: { type: reason }, review_at: 104 },
        }),
      ),
    );
  }
});

test('prepared eligibility refuses obsolete placement and malformed residences', () => {
  const obsolete = active(undefined);
  delete obsolete.value.value.residence;
  obsolete.value.value.placement = { type: 'Queue', value: 9n };
  assert.throws(
    () => projectActorEligibility(obsolete),
    /residence must be a runtime object/,
  );
  for (const residence of [
    { type: 'Unplaced' },
    { type: 'Queue', value: 9n },
    { type: 'Wakeup', value: { type: 'Tick', value: 21n } },
    { type: 'Service', value: { type: 'Unknown' } },
    { type: 'Deadline', value: { type: 'Unknown', value: 1 } },
    { type: 'Parked', value: { reason: { type: 'Unknown' } } },
    {
      type: 'Parked',
      value: { reason: { type: 'ParkedBalanceBelowThreshold' }, review_at: -1 },
    },
  ]) {
    assert.throws(() => projectActorEligibility(active(residence)));
  }
});

test('prepared eligibility preserves absence, dormancy and typed runtime refusals', () => {
  for (const type of ['NotRegistered', 'Dormant']) {
    assert.deepEqual(
      projectActorEligibility({ success: true, value: { type } }),
      { type },
    );
  }
  assert.throws(
    () =>
      projectActorEligibility({
        success: false,
        value: { type: 'ActorInvariant' },
      }),
    /ActorInvariant/,
  );
});

test('prepared eligibility keeps the unpaid boundary and original grace independent of residence', () => {
  const fixtures = [
    [
      { type: 'AtTimeTrigger', value: { due_tick: 21n } },
      { type: 'AtTimeTrigger', dueTick: 21n },
    ],
    [
      { type: 'PipelineOpening', value: { cycle_nonce: 1n } },
      { type: 'PipelineOpening', cycleNonce: 1n },
    ],
    [
      { type: 'RunningAction', value: { cycle_nonce: 2n, cursor: 3 } },
      { type: 'RunningAction', cycleNonce: 2n, cursor: 3 },
    ],
    [
      {
        type: 'SuspendedAction',
        value: {
          cycle_nonce: 2n,
          cursor: 3,
          unsuccessful_attempts_at_cursor: 4,
        },
      },
      {
        type: 'SuspendedAction',
        cycleNonce: 2n,
        cursor: 3,
        unsuccessfulAttemptsAtCursor: 4,
      },
    ],
  ];
  for (const [unpaidService, expected] of fixtures) {
    for (const residence of [
      { type: 'Deadline', value: { type: 'Tick', value: 4_838_421n } },
      { type: 'Service', value: { type: 'Live' } },
    ]) {
      const result = active(residence);
      result.value.value.financial_wait = {
        unpaid_service: unpaidService,
        unpaid_liability: 300n,
        current_service_spendability: 299n,
        funding_shortfall: 1n,
        entered_at_millis: 10_500n,
        expiry_tick: 4_838_421n,
      };
      assert.deepEqual(projectActorEligibility(result).financialWait, {
        unpaidService: expected,
        unpaidLiability: 300n,
        currentServiceSpendability: 299n,
        fundingShortfall: 1n,
        enteredAtMillis: 10_500n,
        expiryTick: 4_838_421n,
      });
    }
  }
  assert.equal(
    projectActorEligibility(active({ type: 'Disabled' })).financialWait,
    null,
  );
});

test('prepared eligibility refuses malformed financial status', () => {
  for (const unpaidService of [
    { type: 'Unknown', value: {} },
    { type: 'PipelineOpening', value: { cycle_nonce: 0n } },
    { type: 'RunningAction', value: { cycle_nonce: 1n, cursor: -1 } },
    {
      type: 'RunningAction',
      value: { cycle_nonce: 1n, cursor: 0x1_0000_0000 },
    },
  ]) {
    const result = active({ type: 'Service', value: { type: 'Live' } });
    result.value.value.financial_wait = {
      unpaid_service: unpaidService,
      unpaid_liability: 300n,
      current_service_spendability: 299n,
      funding_shortfall: 1n,
      entered_at_millis: 10_500n,
      expiry_tick: 4_838_421n,
    };
    assert.throws(() => projectActorEligibility(result));
  }
});
