/*
Domain: Actors risk and composition warnings validation
Owns: Typed projection of runtime facts into composition warnings.
Excludes: Signing, submission, runtime mutation, and protocol policy.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeActorContract } from '../src/lib/automation/analysis.ts';
import {
  createActorContractArtifact,
  encodeActorContractValue,
} from '../src/lib/automation/contract-artifact.ts';
import {
  ACTORS_COMPOSITION_WARNING_KINDS,
  projectActorCompositionWarnings,
} from '../src/lib/automation/risk-warnings.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
  modelIdentity: 'deos-runtime@0.7.4-test',
};
const account = '5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT';
const native = { type: 'Native', value: undefined };
const local = { type: 'Local', value: 7 };
const variant = (type) => ({ type, value: undefined });
const fixed = (value = 10n) => ({ type: 'Fixed', value });

const taskNames = [
  'Transfer',
  'SplitTransfer',
  'SwapIn',
  'SwapOut',
  'AddLiquidity',
  'RemoveLiquidity',
  'Burn',
  'Mint',
  'Stake',
  'DonateLiquidity',
  'Unstake',
  'StopCycle',
];

const weightModel = {
  identity: 'deos-test-weights',
  version: '1',
  evaluationFeeUpper: (conditionCount) => 2n + BigInt(conditionCount),
  evaluationWeight: (conditionCount) => ({
    refTime: 10n + BigInt(conditionCount),
    proofSize: 2n + BigInt(conditionCount),
  }),
  taskUpper: ({ splitLegs }) => ({
    weight: { refTime: 100n + BigInt(splitLegs), proofSize: 5n },
    executionFeeUpper: 7n + BigInt(splitLegs),
  }),
  lifecycleOverhead: {
    weight: { refTime: 20n, proofSize: 3n },
    fee: 4n,
  },
  fundingPromotionOverhead: {
    weight: { refTime: 30n, proofSize: 4n },
    fee: 5n,
  },
  referenceBudget: { refTime: 1_000n, proofSize: 100n },
};

const adapterCapabilities = {
  identity: 'all-test-adapters@1',
  adapters: {
    AssetOps: 'supported',
    DexOps: 'supported',
    StakingOps: 'supported',
    LiquidityOps: 'supported',
  },
  temporaryFailures: Object.fromEntries(taskNames.map((name) => [name, 'no'])),
};

function taskValue(name, amount = fixed()) {
  switch (name) {
    case 'Transfer':
      return { to: account, asset: native, amount };
    case 'StopCycle':
      return undefined;
    default:
      return { asset: native, amount };
  }
}

function step({
  task = 'Transfer',
  amount = fixed(),
  onError = 'AbortCycle',
} = {}) {
  return {
    precondition: undefined,
    task: { type: task, value: taskValue(task, amount) },
    on_error:
      onError === 'RetryLater'
        ? { type: onError, value: { max_attempts: 3 } }
        : variant(onError),
  };
}

function activeContract(steps) {
  return {
    type: 'Active',
    value: {
      schedule: {
        trigger: {
          type: 'Immediate',
          value: {
            sources: [{ type: 'Manual', value: undefined }],
          },
        },
        cooldown_blocks: 5,
      },
      schedule_window: undefined,
      steps: steps,
      completion: variant('Persistent'),
      funding: variant('OwnerOnly'),
    },
  };
}

function artifactFor({ steps, mutability = 'Mutable' } = {}) {
  const contractScale = encodeActorContractValue(
    metadataBytes,
    activeContract(steps),
  );
  return createActorContractArtifact({
    metadataBytes,
    runtime,
    actorType: 'User',
    mutability,
    contractScale,
  });
}

function analyze(artifact) {
  return analyzeActorContract({
    artifact,
    metadataBytes,
    runtime,
    weightModel,
    adapterCapabilities,
  });
}

test('Immutable actor without a terminal step produces a critical warning', () => {
  const artifact = artifactFor({
    steps: [step({ onError: 'ContinueNextStep' })],
    mutability: 'Immutable',
  });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({ artifact, analysis });
  const hit = warnings.find(
    (w) => w.kind === 'ImmutableWithoutReachableTerminal',
  );
  assert.ok(hit, 'immutable-without-terminal warning expected');
  assert.equal(hit.severity, 'critical');
  assert.match(hit.message, /custody permanently/);
});

test('Mutable actor with a terminal step does not warn about permanence', () => {
  const artifact = artifactFor({ steps: [step()], mutability: 'Mutable' });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({ artifact, analysis });
  assert.ok(
    !warnings.some((w) => w.kind === 'ImmutableWithoutReachableTerminal'),
    'mutable actor must not warn about permanent custody',
  );
});

test('stop-fallthrough custody finding projects residual-custody warning', () => {
  const artifact = artifactFor({
    steps: [step({ task: 'StopCycle', onError: 'ContinueNextStep' })],
  });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({ artifact, analysis });
  const hit = warnings.find(
    (w) => w.kind === 'ResidualCustodyThroughLocatorReuse',
  );
  assert.ok(hit, 'residual-custody warning expected');
  assert.equal(hit.severity, 'warning');
});

test('deep step graph projects amplification warning', () => {
  const artifact = artifactFor({
    steps: [step(), step(), step()],
  });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({ artifact, analysis });
  assert.ok(
    warnings.some((w) => w.kind === 'DeepActorGraphAmplification'),
    'deep-graph warning expected for 3+ steps',
  );
});

test('shared single FIFO projects strict head-of-line warning', () => {
  const artifact = artifactFor({ steps: [step()] });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({
    artifact,
    analysis,
    strictFifoHeadOfLine: true,
  });
  assert.ok(
    warnings.some((w) => w.kind === 'StrictFifoHeadOfLine'),
    'strict-FIFO warning expected when sharing the single FIFO',
  );
});

test('Completed with partial task success projects the completed-vs-success warning', () => {
  const artifact = artifactFor({ steps: [step()] });
  const analysis = analyze(artifact);
  const warnings = projectActorCompositionWarnings({
    artifact,
    analysis,
    simulatorStatus: 'Completed',
    successfulTaskCount: 2,
    totalTaskCount: 5,
  });
  assert.ok(
    warnings.some((w) => w.kind === 'CompletedDoesNotImplyAllTasksSuccess'),
    'completed-vs-success warning expected for partial success',
  );
});

test('all warning kinds are declared in the stable enumeration', () => {
  const artifact = artifactFor({ steps: [step()] });
  const analysis = analyze(artifact);
  projectActorCompositionWarnings({ artifact, analysis });
  assert.ok(ACTORS_COMPOSITION_WARNING_KINDS.length >= 6);
  assert.ok(
    ACTORS_COMPOSITION_WARNING_KINDS.includes(
      'ImmutableWithoutReachableTerminal',
    ),
  );
  assert.ok(
    ACTORS_COMPOSITION_WARNING_KINDS.includes(
      'CompletedDoesNotImplyAllTasksSuccess',
    ),
  );
});
