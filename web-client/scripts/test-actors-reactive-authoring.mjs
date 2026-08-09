/*
Domain: Actors reactive-authoring validation
Owns: One canonical latest-observation strategy across exact artifact, analysis, composition, local projection, matching-Wasm, and UI evidence.
Excludes: Runtime policy defaults, live chain execution, signing, submission, and market-viability claims.
Zone: Web-client validation entrypoint; composes automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeActorProgram } from '../src/lib/automation/analysis.ts';
import { createActorArtifactFromAuthoring } from '../src/lib/automation/authoring.ts';
import { composeActorRuntimeCall } from '../src/lib/automation/governance-composition.ts';
import { runActorMatchingWasmSimulation } from '../src/lib/automation/matching-wasm.ts';
import { inspectActorPlanArtifact } from '../src/lib/automation/plan-artifact.ts';
import { encodeActorRuntimeSimulationResult } from '../src/lib/automation/runtime-simulation-codec.ts';
import { simulateActorLocally } from '../src/lib/automation/simulation.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const native = { type: 'Native' };
const quoteAsset = { type: 'Local', id: 7 };
const feed = {
  assetIn: native,
  assetOut: quoteAsset,
  method: 'PreExecutionSpot',
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
  scale: 12,
};
const canonicalProgram = {
  actorType: 'User',
  mutability: 'Mutable',
  completionPolicy: 'CloseAfterProductiveCycle',
  trigger: {
    type: 'Immediate',
    sources: [{ type: 'OnObservationChange', feed }],
  },
  cooldownBlocks: 0,
  scheduleWindow: null,
  fundingPolicy: { type: 'OwnerOnly' },
  steps: [
    {
      key: 'one-shot-buy-bucket',
      conditionSet: {
        type: 'All',
        conditions: [
          {
            type: 'ObservationBelow',
            feed,
            threshold: '1000000000000',
            maxAgeBlocks: 12,
          },
          {
            type: 'BalanceAbove',
            asset: native,
            threshold: '99',
          },
        ],
      },
      task: {
        type: 'SwapIn',
        assetIn: native,
        amountIn: { type: 'Fixed', value: '100' },
        assetOut: quoteAsset,
        slippageParts: 10_000_000,
      },
      errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
    },
  ],
};
const artifact = createActorArtifactFromAuthoring({
  program: canonicalProgram,
  metadataBytes,
  runtime,
});
const weightModel = {
  identity: 'reactive-authoring-fixture-weights',
  version: '1',
  evaluationFeeUpper: (conditionCount) => 2n + BigInt(conditionCount),
  evaluationWeight: (conditionCount) => ({
    refTime: 10n + BigInt(conditionCount),
    proofSize: 2n + BigInt(conditionCount),
  }),
  taskUpper: () => ({
    weight: { refTime: 100n, proofSize: 5n },
    executionFeeUpper: 7n,
  }),
  lifecycleOverhead: {
    weight: { refTime: 20n, proofSize: 3n },
    fee: 4n,
  },
  fundingPromotionOverhead: {
    weight: { refTime: 30n, proofSize: 4n },
    fee: 5n,
  },
};

function localStep() {
  return {
    stepIndex: 0,
    conditionSet: canonicalProgram.steps[0].conditionSet,
    taskControl: 'Execute',
    onError: canonicalProgram.steps[0].errorPolicy,
  };
}

function matchingOutcome(resultScale) {
  return {
    status: 'Closed',
    closeReason: 'ProductiveCycleCompleted',
    cycleNonce: 1n,
    attempt: 1,
    startCursor: 0,
    continuationCursor: null,
    unsuccessfulAttemptsAtCursor: null,
    cumulativeOutcomes: {
      executedSteps: 1,
      committedEffectfulTasks: 1,
      skippedConditions: 0,
      skippedResolution: 0,
      skippedFundingUnavailable: 0,
      failedSteps: 0,
    },
    steps: [{ stepIndex: 0, outcome: { type: 'Executed' } }],
    resultScale,
  };
}

test('canonical reactive one-shot strategy round-trips and projects exact semantics', () => {
  assert.deepEqual(
    artifact,
    createActorArtifactFromAuthoring({
      program: structuredClone(canonicalProgram),
      metadataBytes,
      runtime,
    }),
  );
  const inspection = inspectActorPlanArtifact(artifact, metadataBytes, runtime);
  assert.equal(inspection.valid, true);
  if (!inspection.valid) return;
  assert.equal(
    inspection.projection.value.schedule.trigger.value.sources[0].type,
    'OnObservationChange',
  );
  assert.deepEqual(
    inspection.projection.value.execution_plan[0].conditions.value.map(
      (condition) => condition.type,
    ),
    ['ObservationBelow', 'BalanceAbove'],
  );
  assert.equal(
    inspection.projection.value.execution_plan[0].task.type,
    'SwapIn',
  );
  assert.equal(
    inspection.projection.value.execution_plan[0].on_error.type,
    'RetryLater',
  );
  assert.equal(
    inspection.projection.value.completion_policy.type,
    'CloseAfterProductiveCycle',
  );

  const analysis = analyzeActorProgram({
    artifact,
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'reactive-authoring-fixture' },
    weightModel,
  });
  assert.equal(analysis.identity.planId, artifact.planId);
  assert.equal(analysis.trigger.sourceKinds[0], 'ObservationChange');
  assert.equal(analysis.steps[0].conditionSet.mode, 'All');
  assert.equal(analysis.steps[0].conditionSet.atomicCount, 2);
  assert.equal(analysis.steps[0].task, 'SwapIn');
  assert.equal(analysis.steps[0].errorPolicy, 'RetryLater');
  assert.equal(analysis.steps[0].retryMaxAttempts, 3);
  assert.equal(analysis.completionPolicy, 'CloseAfterProductiveCycle');
});

test('reactive strategy preserves topology under persistent lifecycle policy', () => {
  const persistentProgram = structuredClone(canonicalProgram);
  persistentProgram.completionPolicy = 'Persistent';
  const persistentArtifact = createActorArtifactFromAuthoring({
    program: persistentProgram,
    metadataBytes,
    runtime,
  });
  const inspection = inspectActorPlanArtifact(
    persistentArtifact,
    metadataBytes,
    runtime,
  );
  assert.equal(inspection.valid, true);
  if (!inspection.valid) return;
  assert.equal(
    inspection.projection.value.completion_policy.type,
    'Persistent',
  );
  assert.equal(
    inspection.projection.value.schedule.trigger.value.sources[0].type,
    'OnObservationChange',
  );
  assert.equal(
    inspection.projection.value.execution_plan[0].task.type,
    'SwapIn',
  );
  assert.notEqual(persistentArtifact.planId, artifact.planId);
});

test('canonical reactive artifact composes without runtime bucket policy', () => {
  const composition = composeActorRuntimeCall({
    artifact,
    metadataBytes,
    runtime,
    target: { type: 'Create' },
  });
  assert.equal(composition.planId, artifact.planId);
  assert.equal(composition.authority.requiredOrigin, 'OwnerSigned');
  assert.equal(composition.preimage.governanceAdmission, 'DirectCallOnly');
  assert.equal(composition.call.method, 'create_user_actor');
});

test('local projection preserves one-shot readiness, retry, and productive closure', () => {
  const base = {
    artifact,
    blockHash: `0x${'22'.repeat(32)}`,
    model: 'reactive-authoring-fixture-adapters',
    modelVersion: '1',
    cycleNonce: 1n,
    attempt: 0,
    startCursor: 0,
    completionPolicy: 'CloseAfterProductiveCycle',
    initialState: { nativeBalance: 100n, quoteBalance: 0n },
    steps: [localStep()],
  };
  const notReady = simulateActorLocally({
    ...base,
    evaluateCondition(condition, state) {
      if (condition.type === 'ObservationBelow') {
        return { kind: 'Value', value: false };
      }
      return {
        kind: 'Value',
        value: state.nativeBalance > BigInt(condition.threshold),
      };
    },
    runTask() {
      throw new Error('false latest-state condition must not execute');
    },
  });
  assert.equal(notReady.status, 'Completed');
  assert.equal(notReady.closeReason, null);

  const suspended = simulateActorLocally({
    ...base,
    evaluateCondition() {
      return { kind: 'Value', value: true };
    },
    runTask(_step, state) {
      state.nativeBalance -= 100n;
      return { kind: 'Failed', retry: 'Temporary', error: 'quote unavailable' };
    },
  });
  assert.equal(suspended.status, 'Suspended');
  assert.equal(suspended.state.nativeBalance, 100n);
  assert.equal(suspended.continuationCursor, 0);
  assert.equal(suspended.unsuccessfulAttemptsAtCursor, 1);

  const closed = simulateActorLocally({
    ...base,
    attempt: 1,
    unsuccessfulAttemptsAtCursor: suspended.unsuccessfulAttemptsAtCursor,
    initialState: suspended.state,
    initialCounts: suspended.cumulative,
    evaluateCondition() {
      return { kind: 'Value', value: true };
    },
    runTask(_step, state) {
      state.nativeBalance -= 100n;
      state.quoteBalance += 90n;
      return { kind: 'Executed' };
    },
  });
  assert.equal(closed.status, 'Closed');
  assert.equal(closed.closeReason, 'ProductiveCycleCompleted');
  assert.equal(closed.cumulative.committedEffectfulTasks, 1);
  assert.deepEqual(closed.state, { nativeBalance: 0n, quoteBalance: 90n });
});

test('matching-Wasm contract accepts canonical productive closure for the fixture', async () => {
  const resultScale = encodeActorRuntimeSimulationResult(metadataBytes, {
    success: true,
    value: {
      status: {
        type: 'Closed',
        value: { type: 'ProductiveCycleCompleted', value: undefined },
      },
      cycle_nonce: 1n,
      attempt: 1,
      start_cursor: 0,
      continuation_cursor: undefined,
      unsuccessful_attempts_at_cursor: undefined,
      cumulative_outcomes: {
        executed_steps: 1,
        committed_effectful_tasks: 1,
        skipped_conditions: 0,
        skipped_resolution: 0,
        skipped_funding_unavailable: 0,
        failed_steps: 0,
      },
      steps: [
        {
          step_index: 0,
          outcome: { type: 'Executed', value: undefined },
        },
      ],
    },
  });
  let requestedPlanId = null;
  const response = await runActorMatchingWasmSimulation({
    artifact,
    actorId: 9n,
    mode: 'CurrentContinuation',
    metadataBytes,
    runtime,
    runtimeCodeBytes: Uint8Array.of(1, 2, 3),
    snapshot: {
      blockHash: `0x${'22'.repeat(32)}`,
      blockNumber: 42,
      stateRoot: `0x${'33'.repeat(32)}`,
      stateSource: 'FinalizedBlock',
    },
    runtimeApi: 'ActorSimulationApi_simulate_current_program',
    runtimeApiVersion: 1,
    provider: {
      async simulate(request) {
        requestedPlanId = request.pin.planId;
        return {
          engine: 'RuntimeWasm',
          pin: request.pin,
          outcome: matchingOutcome(resultScale),
        };
      },
    },
  });
  assert.equal(requestedPlanId, artifact.planId);
  assert.equal(response.outcome.status, 'Closed');
  assert.equal(response.outcome.closeReason, 'ProductiveCycleCompleted');
});

test('reactive authoring UI exposes every canonical fixture control', async () => {
  const sources = await Promise.all(
    [
      '../src/lib/automation/AutomationTriggerEditor.svelte',
      '../src/lib/automation/AutomationConditionEditor.svelte',
      '../src/lib/automation/AutomationTaskEditor.svelte',
      '../src/lib/automation/AutomationStepEditor.svelte',
      '../src/lib/widgets/AutomationWidget.svelte',
    ].map((path) => readFile(new URL(path, import.meta.url), 'utf8')),
  );
  const source = sources.join('\n');
  assert(sources[1].includes('ACTORS_AUTHORING_CONDITION_TYPES'));
  for (const control of [
    'OnObservationChange',
    'SwapIn',
    'RetryLater',
    'Close after productive cycle',
    'Persistent',
  ]) {
    assert(source.includes(control), `${control} control is missing`);
  }
});
