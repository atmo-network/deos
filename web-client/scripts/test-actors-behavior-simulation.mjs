/*
Domain: Actors outcome-policy local projection validation
Owns: Canonical grammar and committed-history boundary witnesses for the adapter-local model.
Excludes: Runtime admission, resource prices, financial clocks, chain state, and Wasm equivalence.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ACTOR_BEHAVIOR_METRICS,
  validateActorCloseWhen,
} from '../src/lib/automation/behavior-policy.ts';
import { simulateActorLocally } from '../src/lib/automation/simulation.ts';

const behaviorLimits = { maxClauses: 4, maxAtomsPerClause: 4 };
const atom = (metric, threshold = 1) => ({ metric, threshold });
const policy = (...clauses) => ({ clauses });
const artifact = {
  format: 'deos.actor.contract',
  formatVersion: 1,
  genesisHash: `0x${'11'.repeat(32)}`,
  metadataHash: `0x${'22'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
  actorType: 'User',
  mutability: 'Mutable',
  contractScale: '0x00',
  contractId: `0x${'33'.repeat(32)}`,
};
const step = (stepIndex, extra = {}) => ({
  stepIndex,
  precondition: null,
  taskControl: 'Execute',
  onError: { type: 'ContinueNextStep' },
  ...extra,
});
const base = {
  artifact,
  blockHash: `0x${'44'.repeat(32)}`,
  model: 'outcome-fixture-adapters',
  modelVersion: '2',
  cycleNonce: 1n,
  startCursor: 0,
  behaviorLimits,
  initialState: { credit: 0 },
  steps: [step(0)],
  runTask(_step, state) {
    state.credit += 1;
    return { kind: 'Executed' };
  },
};
const next = (result) => ({
  initialState: result.state,
  initialCounts: result.cumulative,
  initialBehaviorState: result.behavior,
});

test('closed metric vocabulary accepts positive u32 thresholds and rejects noncanonical grammar', () => {
  assert.equal(ACTOR_BEHAVIOR_METRICS.length, 10);
  for (const metric of ACTOR_BEHAVIOR_METRICS) {
    for (const threshold of [1, 0xffff_ffff]) {
      const value = policy([atom(metric, threshold)]);
      assert.deepEqual(validateActorCloseWhen(value, behaviorLimits), value);
    }
  }
  const invalid = [
    null,
    {},
    policy(),
    policy([]),
    { clauses: Array(1) },
    policy(Array(1)),
    policy([atom('Unknown')]),
    policy([atom('ActionSuccessTotal', 0)]),
    policy([atom('ActionSuccessTotal', -1)]),
    policy([atom('ActionSuccessTotal', 1.5)]),
    policy([atom('ActionSuccessTotal', 0x1_0000_0000)]),
    policy([atom('ActionSuccessTotal'), atom('ActionSuccessTotal', 2)]),
    policy([atom('CycleCleanTotal'), atom('ActionSuccessTotal')]),
    policy([atom('ActionSuccessTotal')], [atom('ActionSuccessTotal')]),
    policy([atom('ActionSuccessTotal', 1)], [atom('ActionSuccessTotal', 2)]),
    policy([atom('CycleCleanTotal')], [atom('ActionSuccessTotal')]),
    policy(
      [atom('ActionSuccessTotal'), atom('CycleCleanTotal')],
      [atom('CycleCleanTotal')],
    ),
    policy([{ ...atom('ActionSuccessTotal'), destination: 'Run' }]),
    { ...policy([atom('ActionSuccessTotal')]), not: true },
  ];
  for (const value of invalid)
    assert.throws(() => validateActorCloseWhen(value, behaviorLimits));
  assert.throws(() =>
    validateActorCloseWhen(policy([atom('ActionSuccessTotal')]), {
      maxClauses: 0,
      maxAtomsPerClause: 4,
    }),
  );
  assert.throws(() =>
    validateActorCloseWhen(policy([atom('ActionSuccessTotal')]), {
      maxClauses: 0xffff_ffff,
      maxAtomsPerClause: 2,
    }),
  );
  assert.throws(() =>
    validateActorCloseWhen(
      policy([atom('ActionSuccessTotal'), atom('CycleCleanTotal')]),
      { maxClauses: 1, maxAtomsPerClause: 1 },
    ),
  );
});

test('each metric closes from its own qualified observation', () => {
  for (const metric of ACTOR_BEHAVIOR_METRICS) {
    const rejecting = metric.startsWith('PreconditionReject');
    const failing = metric.startsWith('ActionFailure');
    const result = simulateActorLocally({
      ...base,
      closeWhen: policy([atom(metric)]),
      steps: [
        step(0, {
          precondition: metric.startsWith('Precondition')
            ? { clauses: [[rejecting ? false : true]] }
            : null,
        }),
      ],
      evaluatePredicate(value) {
        return { kind: 'Value', value };
      },
      runTask(_step, state) {
        state.credit += 1;
        return failing
          ? { kind: 'Failed', retry: 'Permanent', error: 'refused' }
          : { kind: 'Executed' };
      },
    });
    assert.equal(result.closeReason, 'BehaviorPolicySatisfied', metric);
    assert.equal(result.behavior.cappedMetrics[metric], 1, metric);
    assert.equal(result.state.credit, rejecting || failing ? 0 : 1, metric);
    assert.equal(result.provenance.truth, 'AdapterLocalProjection');
  }
});

test('a first Action terminal commits its effect and never evaluates the remaining tail', () => {
  const visited = [];
  const result = simulateActorLocally({
    ...base,
    closeWhen: policy([atom('ActionSuccessTotal')]),
    steps: [step(0), step(1)],
    runTask(current, state) {
      visited.push(current.stepIndex);
      state.credit += 1;
      return { kind: 'Executed' };
    },
  });
  assert.deepEqual(visited, [0]);
  assert.equal(result.state.credit, 1);
  assert.equal(result.journal.length, 1);
});

test('a passed aggregate still permits its Task before policy Close, including a rolled-back Task failure', () => {
  const result = simulateActorLocally({
    ...base,
    closeWhen: policy([atom('PreconditionPassTotal')]),
    steps: [
      step(0, {
        precondition: { clauses: [[true]] },
        onError: { type: 'AbortCycle' },
      }),
    ],
    evaluatePredicate(value) {
      return { kind: 'Value', value };
    },
    runTask(_step, state) {
      state.credit = 100;
      return { kind: 'Failed', retry: 'Permanent', error: 'postcondition' };
    },
  });
  assert.equal(result.closeReason, 'BehaviorPolicySatisfied');
  assert.equal(result.state.credit, 0);
  assert.equal(result.cumulative.failedSteps, 1);
});

test('success streak crosses unclean Cycles and skips without proving a clean Cycle', () => {
  const closeWhen = policy([atom('ActionSuccessStreak', 2)]);
  const steps = [step(0), step(1), step(2)];
  const first = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    runTask(current, state) {
      state.credit += 1;
      return current.stepIndex === 1
        ? { kind: 'Failed', retry: 'Permanent', error: 'failed' }
        : { kind: 'Executed' };
    },
  });
  assert.equal(first.status, 'Completed');
  assert.equal(first.behavior.cappedMetrics.ActionSuccessStreak, 1);
  const skipped = simulateActorLocally({
    ...base,
    closeWhen,
    ...next(first),
    steps: [step(0, { precondition: { clauses: [[false]] } })],
    evaluatePredicate(value) {
      return { kind: 'Value', value };
    },
  });
  assert.equal(skipped.behavior.cappedMetrics.ActionSuccessStreak, 1);
  const closed = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    ...next(skipped),
  });
  assert.equal(closed.closeReason, 'BehaviorPolicySatisfied');
  assert.equal(closed.journal.length, 1);
  assert.equal(closed.state.credit, 3);
});

test('opposite outcomes reset streaks while absent observations leave them unchanged', () => {
  const closeWhen = policy([
    atom('ActionSuccessStreak', 2),
    atom('CycleCompletedTotal', 10),
  ]);
  const success = simulateActorLocally({ ...base, closeWhen });
  const failed = simulateActorLocally({
    ...base,
    closeWhen,
    ...next(success),
    runTask() {
      return { kind: 'Failed', retry: 'Permanent', error: 'failed' };
    },
  });
  assert.equal(failed.behavior.cappedMetrics.ActionSuccessStreak, 0);
  const empty = simulateActorLocally({
    ...base,
    closeWhen,
    ...next(failed),
    steps: [],
  });
  assert.equal(empty.behavior.cappedMetrics.ActionSuccessStreak, 0);
  assert.equal(empty.behavior.cappedMetrics.CycleCompletedTotal, 3);
});

test('a failed-then-successful retry stays unclean; the next clean Cycle closes', () => {
  const closeWhen = policy([atom('CycleCleanTotal')]);
  const steps = [
    step(0),
    step(1, { onError: { type: 'RetryLater', maxAttempts: 3 } }),
  ];
  const suspended = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    runTask(current, state) {
      state.credit += 1;
      return current.stepIndex === 0
        ? { kind: 'Executed' }
        : { kind: 'Failed', retry: 'Temporary', error: 'market' };
    },
  });
  assert.equal(suspended.status, 'Suspended');
  assert.equal(suspended.state.credit, 1);
  const resumed = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    ...next(suspended),
    startCursor: 1,
    unsuccessfulAttemptsAtCursor: suspended.unsuccessfulAttemptsAtCursor,
  });
  assert.equal(resumed.status, 'Completed');
  assert.equal(resumed.behavior.cappedMetrics.CycleCleanTotal, 0);
  assert.equal(resumed.state.credit, 2);
  const clean = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    ...next(resumed),
  });
  assert.equal(clean.closeReason, 'BehaviorPolicySatisfied');
  assert.equal(clean.state.credit, 4);
});

test('conditional StopCycle completes normally, but an unvisited tail cannot qualify clean', () => {
  const closeWhen = policy([
    atom('CycleCompletedTotal', 2),
    atom('CycleCleanTotal'),
  ]);
  const steps = [
    step(0),
    step(1, {
      taskControl: 'StopCycle',
      precondition: { clauses: [['return']] },
    }),
    step(2),
  ];
  const returned = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    evaluatePredicate() {
      return { kind: 'Value', value: true };
    },
  });
  assert.equal(returned.behavior.cappedMetrics.CycleCompletedTotal, 1);
  assert.equal(returned.behavior.cappedMetrics.CycleCleanTotal, 0);
  assert.equal(returned.journal.length, 2);
  const full = simulateActorLocally({
    ...base,
    closeWhen,
    steps,
    ...next(returned),
    evaluatePredicate() {
      return { kind: 'Value', value: false };
    },
  });
  assert.equal(full.closeReason, 'BehaviorPolicySatisfied');
  assert.equal(full.journal.length, 3);
  assert.equal(full.state.credit, 3);
});

test('zero-Step and control-only Cycles complete without becoming clean', () => {
  for (const steps of [[], [step(0, { taskControl: 'StopCycle' })]]) {
    const completed = simulateActorLocally({
      ...base,
      steps,
      closeWhen: policy([atom('CycleCompletedTotal')]),
    });
    assert.equal(completed.closeReason, 'BehaviorPolicySatisfied');
    const clean = simulateActorLocally({
      ...base,
      steps,
      closeWhen: policy([atom('CycleCleanTotal')]),
    });
    assert.equal(clean.status, 'Completed');
    assert.equal(clean.behavior.cappedMetrics.CycleCleanTotal, 0);
  }
});

test('predicate errors are neither boolean observations nor invoked Action failures', () => {
  const result = simulateActorLocally({
    ...base,
    closeWhen: policy(
      [atom('PreconditionPassTotal')],
      [atom('ActionFailureTotal')],
    ),
    steps: [
      step(0, {
        precondition: { clauses: [['error']] },
        onError: { type: 'RetryLater', maxAttempts: 3 },
      }),
    ],
    evaluatePredicate() {
      return { kind: 'Error', retry: 'Temporary', error: 'unavailable' };
    },
    runTask() {
      throw new Error('must not invoke');
    },
  });
  assert.equal(result.status, 'Suspended');
  assert.deepEqual(result.behavior.cappedMetrics, {
    PreconditionPassTotal: 0,
    ActionFailureTotal: 0,
  });
});

test('AND atoms mature independently and values cap at authored thresholds', () => {
  const closeWhen = policy(
    [atom('PreconditionPassTotal', 2), atom('ActionSuccessTotal', 5)],
    [atom('CycleCleanTotal', 5)],
  );
  let result;
  for (let cycle = 0; cycle < 3; cycle += 1) {
    result = simulateActorLocally({
      ...base,
      closeWhen,
      ...(result ? next(result) : {}),
      steps: [step(0, { precondition: { clauses: [[true]] } })],
      evaluatePredicate(value) {
        return { kind: 'Value', value };
      },
    });
    assert.equal(result.status, 'Completed');
  }
  assert.deepEqual(result.behavior.cappedMetrics, {
    PreconditionPassTotal: 2,
    ActionSuccessTotal: 3,
    CycleCleanTotal: 3,
  });
});

test('retry exhaustion precedes a simultaneously satisfied behavior policy', () => {
  const result = simulateActorLocally({
    ...base,
    closeWhen: policy([atom('ActionFailureTotal')]),
    steps: [step(0, { onError: { type: 'RetryLater', maxAttempts: 1 } })],
    runTask() {
      return { kind: 'Failed', retry: 'Temporary', error: 'market' };
    },
  });
  assert.equal(result.closeReason, 'RetryAttemptsExhausted');
  assert.equal(result.behavior.cappedMetrics.ActionFailureTotal, 1);
});

test('missing or mismatched history refuses before execution instead of silently resetting', () => {
  const closeWhen = policy([atom('CycleCleanTotal', 2)]);
  const saved = simulateActorLocally({ ...base, closeWhen });
  for (const extra of [
    { startCursor: 1, steps: [step(0), step(1)] },
    { unsuccessfulAttemptsAtCursor: 1 },
    {
      initialBehaviorState: {
        ...saved.behavior,
        contractId: `0x${'55'.repeat(32)}`,
      },
    },
    {
      initialBehaviorState: {
        ...saved.behavior,
        cappedMetrics: { CycleCleanTotal: 3 },
      },
    },
    { initialBehaviorState: { ...saved.behavior, cappedMetrics: {} } },
    {
      initialBehaviorState: {
        ...saved.behavior,
        cappedMetrics: Object.assign(Object.create({ CycleCleanTotal: 1 }), {
          wrong: 1,
        }),
      },
    },
    { initialBehaviorState: saved.behavior, closeWhen: null },
    { behaviorLimits: undefined },
    { completionPolicy: 'Persistent' },
  ]) {
    assert.throws(() => simulateActorLocally({ ...base, closeWhen, ...extra }));
  }
});

test('an empty or control-only plan cannot import an economic prefix', () => {
  for (const steps of [[], [step(0, { taskControl: 'StopCycle' })]]) {
    const closeWhen = policy([atom('CycleCleanTotal', 2)]);
    const saved = simulateActorLocally({ ...base, closeWhen });
    saved.behavior.cycle.economicSuccess = true;
    assert.throws(
      () => simulateActorLocally({ ...base, closeWhen, steps, ...next(saved) }),
      /economic Step/,
    );
  }
});

test('fee insolvency remains unavailable to the local model and cannot fabricate an observation', () => {
  const initialState = { credit: 0 };
  assert.throws(
    () =>
      simulateActorLocally({
        ...base,
        initialState,
        closeWhen: policy([atom('ActionFailureTotal')]),
        runTask(_step, state) {
          state.credit = 100;
          return { kind: 'FundingUnavailable', reason: 'ServiceFee' };
        },
      }),
    /cannot qualify service-fee insolvency/,
  );
  assert.deepEqual(initialState, { credit: 0 });
});
