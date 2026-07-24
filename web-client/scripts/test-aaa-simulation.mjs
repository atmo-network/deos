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
    steps: [
      { stepIndex: 0, onError: 'AbortCycle' },
      { stepIndex: 1, onError: 'ContinueNextStep' },
      { stepIndex: 2, onError: 'AbortCycle' },
    ],
    runStep(step, state) {
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
    steps: [
      { stepIndex: 0, onError: 'AbortCycle' },
      { stepIndex: 1, onError: 'RetryLater' },
      { stepIndex: 2, onError: 'AbortCycle' },
    ],
    runStep(step, state) {
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
  assert.equal(suspended.finalizedThrough, 0);

  const resumed = simulateAaaLocally({
    ...provenance,
    attempt: 1,
    startCursor: suspended.continuationCursor,
    initialState: suspended.state,
    initialCounts: suspended.cumulative,
    steps: [
      { stepIndex: 0, onError: 'AbortCycle' },
      { stepIndex: 1, onError: 'RetryLater' },
      { stepIndex: 2, onError: 'AbortCycle' },
    ],
    runStep(step, state) {
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

test('permanent RetryLater aborts, and Immutable plans reject retry policy', () => {
  const aborted = simulateAaaLocally({
    ...provenance,
    initialState: { balance: 1n },
    steps: [{ stepIndex: 0, onError: 'RetryLater' }],
    runStep(_step, state) {
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
        steps: [{ stepIndex: 0, onError: 'RetryLater' }],
        runStep() {
          return { kind: 'Executed' };
        },
      }),
    /Mutable-only/,
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
