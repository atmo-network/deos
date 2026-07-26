/*
Domain: AAA local-simulation validation
Owns: Task rollback, committed-prefix, Continuation cursor, retry-class, and donation-sensitivity fixtures.
Excludes: Runtime-Wasm execution, chain access, signing, submission, and UI rendering.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  classifyAaaDonationSensitivity,
  simulateAaaLocally,
} from '../src/lib/automation/simulation.ts';

const hash = (byte) => `0x${byte.repeat(64)}`;

function artifact(mutability = 'Mutable') {
  return {
    format: 'deos.aaa.plan',
    formatVersion: 1,
    genesisHash: hash('1'),
    specVersion: 1,
    transactionVersion: 1,
    metadataHash: hash('2'),
    aaaType: 'User',
    mutability,
    programScale: '0x00',
    planId: hash('3'),
  };
}

const blockHash = hash('4');
const localStep = (stepIndex, onError = 'AbortCycle', overrides = {}) => ({
  stepIndex,
  conditionSet: { type: 'Always' },
  taskControl: 'Execute',
  onError:
    onError === 'RetryLater'
      ? { type: onError, maxAttempts: 3 }
      : { type: onError },
  ...overrides,
});

const provenance = {
  artifact: artifact(),
  blockHash,
  model: 'fixture-adapters',
  modelVersion: '1',
  cycleNonce: 7n,
  attempt: 0,
  startCursor: 0,
};

test('local projection commits successful tasks and rolls back one failed task', () => {
  const result = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 100n },
    steps: [localStep(0), localStep(1, 'ContinueNextStep'), localStep(2)],
    runTask(step, state) {
      if (step.stepIndex === 0) {
        state.balance -= 10n;
        return { kind: 'Executed' };
      }
      if (step.stepIndex === 1) {
        state.balance -= 50n;
        return { kind: 'Failed', retry: 'Permanent', error: 'fixture' };
      }
      state.balance += 5n;
      return { kind: 'Executed' };
    },
  });

  assert.equal(result.provenance.truth, 'AdapterLocalProjection');
  assert.equal(result.status, 'Completed');
  assert.equal(result.state.balance, 95n);
  assert.equal(result.continuationCursor, null);
  assert.equal(result.finalizedThrough, 2);
  assert.deepEqual(
    result.journal.map(({ outcome, stateCommitted }) => [
      outcome.kind,
      stateCommitted,
    ]),
    [
      ['Executed', true],
      ['Failed', false],
      ['Executed', true],
    ],
  );
});

test('temporary RetryLater preserves the prefix and resumes from one scalar cursor', () => {
  const suspended = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 100n },
    steps: [localStep(0), localStep(1, 'RetryLater'), localStep(2)],
    runTask(step, state) {
      if (step.stepIndex === 0) {
        state.balance -= 10n;
        return { kind: 'Executed' };
      }
      state.balance -= 50n;
      return { kind: 'Failed', retry: 'Temporary', error: 'unavailable' };
    },
  });

  assert.equal(suspended.status, 'Suspended');
  assert.equal(suspended.state.balance, 90n);
  assert.equal(suspended.continuationCursor, 1);
  assert.equal(suspended.unsuccessfulAttemptsAtCursor, 1);
  assert.equal(suspended.finalizedThrough, 0);

  const resumed = simulateAaaLocally({
    ...provenance,
    attempt: 1,
    startCursor: suspended.continuationCursor,
    unsuccessfulAttemptsAtCursor: suspended.unsuccessfulAttemptsAtCursor,
    initialState: suspended.state,
    initialCounts: suspended.cumulative,
    steps: [localStep(0), localStep(1, 'RetryLater'), localStep(2)],
    runTask(step, state) {
      if (step.stepIndex === 1) state.balance -= 20n;
      else state.balance += 5n;
      return { kind: 'Executed' };
    },
  });

  assert.equal(resumed.status, 'Completed');
  assert.equal(resumed.cycleNonce, suspended.cycleNonce);
  assert.equal(resumed.state.balance, 75n);
  assert.equal(resumed.cumulative.executedSteps, 3);
  assert.deepEqual(
    resumed.journal.map(({ stepIndex }) => stepIndex),
    [1, 2],
  );
});

test('RetryLater closes exactly on its local bound and counts funding unavailability', () => {
  const closedImmediately = simulateAaaLocally({
    ...provenance,
    initialState: {},
    steps: [
      localStep(0, 'RetryLater', {
        onError: { type: 'RetryLater', maxAttempts: 1 },
      }),
    ],
    runTask() {
      return { kind: 'Failed', retry: 'Temporary', error: 'unavailable' };
    },
  });
  assert.equal(closedImmediately.status, 'Closed');
  assert.equal(closedImmediately.closeReason, 'RetryAttemptsExhausted');
  assert.equal(closedImmediately.continuationCursor, null);

  const suspended = simulateAaaLocally({
    ...provenance,
    initialState: {},
    steps: [
      localStep(0, 'RetryLater', {
        onError: { type: 'RetryLater', maxAttempts: 2 },
      }),
    ],
    runTask() {
      return { kind: 'FundingUnavailable' };
    },
  });
  assert.equal(suspended.status, 'Suspended');
  assert.equal(suspended.unsuccessfulAttemptsAtCursor, 1);

  const exhausted = simulateAaaLocally({
    ...provenance,
    attempt: 1,
    startCursor: 0,
    unsuccessfulAttemptsAtCursor: 1,
    initialState: suspended.state,
    initialCounts: suspended.cumulative,
    steps: [
      localStep(0, 'RetryLater', {
        onError: { type: 'RetryLater', maxAttempts: 2 },
      }),
    ],
    runTask() {
      return { kind: 'FundingUnavailable' };
    },
  });
  assert.equal(exhausted.status, 'Closed');
  assert.equal(exhausted.closeReason, 'RetryAttemptsExhausted');
  assert.equal(exhausted.cumulative.skippedFundingUnavailable, 2);
});

test('cursor advancement resets the local unsuccessful-attempt count', () => {
  const result = simulateAaaLocally({
    ...provenance,
    unsuccessfulAttemptsAtCursor: 2,
    initialState: {},
    steps: [
      localStep(0),
      localStep(1, 'RetryLater', {
        onError: { type: 'RetryLater', maxAttempts: 2 },
      }),
    ],
    runTask(step) {
      return step.stepIndex === 0
        ? { kind: 'Executed' }
        : { kind: 'Failed', retry: 'Temporary', error: 'later-cursor' };
    },
  });
  assert.equal(result.status, 'Suspended');
  assert.equal(result.continuationCursor, 1);
  assert.equal(result.unsuccessfulAttemptsAtCursor, 1);
});

test('permanent RetryLater aborts, and Immutable plans reject retry policy', () => {
  const aborted = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 1n },
    steps: [localStep(0, 'RetryLater')],
    runTask(_step, state) {
      state.balance = 0n;
      return { kind: 'Failed', retry: 'Permanent', error: 'invalid' };
    },
  });
  assert.equal(aborted.status, 'Aborted');
  assert.equal(aborted.continuationCursor, null);
  assert.equal(aborted.state.balance, 1n);

  assert.throws(
    () =>
      simulateAaaLocally({
        ...provenance,
        artifact: artifact('Immutable'),
        initialState: {},
        steps: [localStep(0, 'RetryLater')],
        runTask() {
          return { kind: 'Executed' };
        },
      }),
    /Mutable-only/,
  );
});

test('condition groups evaluate every atom and any atomic error fails the whole group', () => {
  const observations = [];
  const result = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 10n },
    steps: [
      localStep(0, 'ContinueNextStep', {
        conditionSet: {
          type: 'Any',
          conditions: ['true', 'error', 'false'],
        },
      }),
      localStep(1),
    ],
    evaluateCondition(condition) {
      observations.push(condition);
      return condition === 'error'
        ? { kind: 'Error', retry: 'Permanent', error: 'observation-failed' }
        : { kind: 'Value', value: condition === 'true' };
    },
    runTask(step, state) {
      state.balance += BigInt(step.stepIndex + 1);
      return { kind: 'Executed' };
    },
  });
  assert.deepEqual(observations, ['true', 'error', 'false']);
  assert.deepEqual(
    result.journal.map(({ outcome }) => outcome.kind),
    ['Failed', 'Executed'],
  );
  assert.equal(result.state.balance, 12n);

  let reads = 0;
  const skipped = simulateAaaLocally({
    ...provenance,
    initialState: {},
    steps: [
      localStep(0, 'AbortCycle', {
        conditionSet: { type: 'All', conditions: [true, false, true] },
      }),
    ],
    evaluateCondition(_condition) {
      const value = [true, false, true][reads++];
      return { kind: 'Value', value };
    },
    runTask() {
      throw new Error('false aggregate must not execute its task');
    },
  });
  assert.equal(reads, 3);
  assert.equal(skipped.journal[0].outcome.kind, 'SkippedCondition');
});

test('StopCycle completes after its committed prefix and leaves the suffix unreachable', () => {
  const executed = [];
  const result = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 10n },
    steps: [
      localStep(0),
      localStep(1, 'RetryLater', { taskControl: 'StopCycle' }),
      localStep(2),
    ],
    runTask(step, state) {
      executed.push(step.stepIndex);
      state.balance += 1n;
      return { kind: 'Executed' };
    },
  });
  assert.equal(result.status, 'Completed');
  assert.equal(result.finalizedThrough, 1);
  assert.equal(result.state.balance, 11n);
  assert.deepEqual(executed, [0]);
  assert.deepEqual(
    result.journal.map(({ outcome }) => outcome.kind),
    ['Executed', 'Stopped'],
  );
});

test('donation classification identifies observation window and amount surface', () => {
  assert.deepEqual(
    classifyAaaDonationSensitivity([
      {
        stepIndex: 0,
        surface: 'asset:1:amountIn',
        resolution: 'Fixed',
        observation: 'ActorBalance',
      },
      {
        stepIndex: 1,
        surface: 'asset:2:amount',
        resolution: 'AllBalance',
        observation: 'ActorBalance',
      },
      {
        stepIndex: 2,
        surface: 'asset:3:trigger',
        resolution: 'PercentageOfTrigger',
        observation: 'ActorBalance',
      },
      {
        stepIndex: 3,
        surface: 'asset:4:funding',
        resolution: 'PercentageOfLastFunding',
        observation: 'ActorFunding',
      },
      {
        stepIndex: 4,
        surface: 'pool:1:quote',
        resolution: 'AllBalance',
        observation: 'AdapterState',
      },
    ]).map(({ sensitivity }) => sensitivity),
    [
      'InsensitiveFixedAmount',
      'BeforeStepResolution',
      'BeforeTriggerSnapshot',
      'BeforeFundingSnapshot',
      'BeforeAdapterObservation',
    ],
  );
});
