/*
Domain: AAA static-analysis validation
Owns: Exhaustive primitive, identity, suffix-envelope, dependency, finding, and determinism fixtures.
Excludes: Runtime queries, adapter execution, simulation, signing, submission, and browser rendering.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeAaaProgram } from '../src/lib/automation/analysis.ts';
import {
  createAaaPlanArtifact,
  encodeAaaProgramValue,
} from '../src/lib/automation/plan-artifact.ts';

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
const fixed = (value = 10n) => ({ type: 'Fixed', value });
const variant = (type) => ({ type, value: undefined });

const taskNames = [
  'Transfer',
  'SplitTransfer',
  'SwapExactIn',
  'SwapExactOut',
  'AddLiquidity',
  'RemoveLiquidity',
  'Burn',
  'Mint',
  'Stake',
  'DonateLiquidity',
  'Unstake',
  'StopCycle',
];
const conditionNames = [
  'BalanceAbove',
  'BalanceBelow',
  'BalanceEquals',
  'BalanceNotEquals',
  'BlockNumberAbove',
  'BlockNumberBelow',
];
const amountNames = [
  'Fixed',
  'PercentageOfCurrent',
  'PercentageOfTrigger',
  'PercentageOfLastFunding',
  'AllBalance',
];
const errorPolicies = ['AbortCycle', 'ContinueNextStep', 'RetryLater'];

const weightModel = {
  identity: 'deos-test-weights',
  version: '1',
  stepBaseFee: 2n,
  conditionReadFee: 1n,
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
    LiquidityDonationOps: 'supported',
  },
  temporaryFailures: Object.fromEntries(taskNames.map((name) => [name, 'no'])),
};

function taskValue(name, amount = fixed()) {
  switch (name) {
    case 'Transfer':
      return { to: account, asset: native, amount };
    case 'SplitTransfer':
      return {
        asset: native,
        amount,
        legs: [
          { to: account, share: 400_000_000 },
          { to: account, share: 600_000_000 },
        ],
      };
    case 'SwapExactIn':
      return {
        asset_in: native,
        asset_out: local,
        amount_in: amount,
        slippage_tolerance: 10_000_000,
      };
    case 'SwapExactOut':
      return {
        asset_in: native,
        asset_out: local,
        amount_out: amount,
        slippage_tolerance: 10_000_000,
      };
    case 'AddLiquidity':
      return {
        asset_a: native,
        asset_b: local,
        amount_a: amount,
        amount_b: fixed(20n),
      };
    case 'RemoveLiquidity':
      return { lp_asset: local, amount };
    case 'Burn':
    case 'Mint':
    case 'Stake':
      return { asset: native, amount };
    case 'DonateLiquidity':
      return {
        asset_a: native,
        asset_b: local,
        amount,
        max_ratio_error: 10_000_000,
      };
    case 'Unstake':
      return { asset: native, shares: amount };
    case 'StopCycle':
      return undefined;
    default:
      throw new Error(`Unknown task fixture: ${name}`);
  }
}

function step({
  task = 'Transfer',
  amount = fixed(),
  conditions = [],
  conditionMode = conditions.length === 0 ? 'Always' : 'All',
  onError = 'AbortCycle',
} = {}) {
  return {
    conditions: {
      type: conditionMode,
      value: conditionMode === 'Always' ? undefined : conditions,
    },
    task: { type: task, value: taskValue(task, amount) },
    on_error: variant(onError),
  };
}

function condition(name) {
  return name.startsWith('Balance')
    ? { type: name, value: { asset: native, threshold: 1n } }
    : { type: name, value: { threshold: 1 } };
}

function activeProgram(steps) {
  return {
    type: 'Active',
    value: {
      schedule: {
        trigger: { type: 'Manual', value: undefined },
        cooldown_blocks: 5,
      },
      schedule_window: undefined,
      execution_plan: steps,
      funding_source_policy: variant('OwnerOnly'),
    },
  };
}

function artifactFor({
  steps,
  program = activeProgram(steps),
  aaaType = 'User',
  mutability = 'Mutable',
} = {}) {
  const programScale = encodeAaaProgramValue(metadataBytes, program);
  return createAaaPlanArtifact({
    metadataBytes,
    runtime,
    aaaType,
    mutability,
    programScale,
  });
}

function analyze(artifact, overrides = {}) {
  return analyzeAaaProgram({
    artifact,
    metadataBytes,
    runtime,
    weightModel,
    adapterCapabilities,
    ...overrides,
  });
}

test('analysis is deterministic, exactly bound, and produces every cursor envelope', () => {
  const artifact = artifactFor({
    steps: [
      step({ task: 'SwapExactIn' }),
      step({
        task: 'Transfer',
        amount: { type: 'AllBalance', value: undefined },
        conditions: [condition('BalanceAbove')],
        onError: 'RetryLater',
      }),
      step({ task: 'Burn' }),
    ],
    aaaType: 'System',
  });
  const first = analyze(artifact);
  const second = analyze(artifact);
  assert.deepEqual(first, second);
  assert.equal(JSON.stringify(first), JSON.stringify(second));
  assert.equal(first.provenance, 'StaticStructuralProjection');
  assert.equal(first.identity.planId, artifact.planId);
  assert.equal(first.identity.genesisHash, artifact.genesisHash);
  assert.equal(first.identity.metadataHash, artifact.metadataHash);
  assert.equal(first.suffixEnvelopes.length, first.steps.length + 1);
  assert.equal(first.suffixEnvelopes[0].remainingSteps, first.steps.length);
  assert.equal(first.suffixEnvelopes.at(-1).remainingSteps, 0);
  for (let index = 1; index < first.suffixEnvelopes.length; index += 1) {
    assert.equal(
      first.suffixEnvelopes[index].remainingSteps,
      first.suffixEnvelopes[index - 1].remainingSteps - 1,
    );
    assert(
      BigInt(first.suffixEnvelopes[index].maximumRefTime) <=
        BigInt(first.suffixEnvelopes[index - 1].maximumRefTime),
    );
    assert(
      BigInt(first.suffixEnvelopes[index].maximumProofSize) <=
        BigInt(first.suffixEnvelopes[index - 1].maximumProofSize),
    );
  }
  assert(first.dataDependencies.some((edge) => edge.fromStep === 0));
  assert(
    first.findings.some(
      (finding) => finding.kind === 'CommittedEffectBeforeRetryableStep',
    ),
  );
  assert(
    first.findings.some(
      (finding) =>
        finding.kind === 'AdministrativeInvalidationSurface' &&
        finding.conditional,
    ),
  );
  assert(
    first.steps.every((current) =>
      current.possibleControls.every((control) =>
        ['advance', 'terminate', 'stutter-current'].includes(control),
      ),
    ),
  );
});

test('identity drift rejects cross-genesis and cross-metadata analysis', () => {
  const artifact = artifactFor({ steps: [step()] });
  assert.throws(
    () =>
      analyze(artifact, {
        runtime: { ...runtime, genesisHash: `0x${'22'.repeat(32)}` },
      }),
    /genesisHash does not match/,
  );
  const changedMetadata = metadataBytes.slice();
  changedMetadata[changedMetadata.length - 1] ^= 1;
  assert.throws(
    () => analyze(artifact, { metadataBytes: changedMetadata }),
    /metadataHash does not match metadata/,
  );
});

test('every current Task has deterministic semantic and weight classification', () => {
  for (const task of taskNames) {
    const artifact = artifactFor({
      steps: [step({ task })],
      aaaType: task === 'Mint' ? 'System' : 'User',
    });
    const result = analyze(artifact);
    assert.equal(result.steps[0].task, task);
    assert.equal(
      result.steps[0].requiredAdapters.length,
      task === 'StopCycle' ? 0 : 1,
    );
    assert(BigInt(result.steps[0].costs.totalUpper.refTime) > 0n);
    assert(BigInt(result.steps[0].costs.totalUpper.proofSize) > 0n);
    assert.equal(
      result.steps[0].failureSurface.temporaryFailureReachability,
      'no',
    );
  }
});

test('condition aggregate mode and atomic count remain explicit without graph control', () => {
  const atoms = [condition('BalanceAbove'), condition('BlockNumberBelow')];
  for (const mode of ['All', 'Any']) {
    const result = analyze(
      artifactFor({
        steps: [step({ conditions: atoms, conditionMode: mode })],
      }),
    );
    assert.deepEqual(result.steps[0].conditionSet, {
      mode,
      atomicCount: 2,
      evaluation: 'all-atoms-no-short-circuit',
      admission: mode === 'All' ? 'all-true' : 'at-least-one-true',
      falseControl: 'advance-fixed-successor',
      atomicError: 'fail-whole-group',
    });
    assert.deepEqual(result.steps[0].possibleControls, [
      'advance',
      'terminate',
    ]);
  }
  const always = analyze(artifactFor({ steps: [step()] })).steps[0];
  assert.equal(always.conditionSet.mode, 'Always');
  assert.equal(always.conditionSet.atomicCount, 0);
});

test('StopCycle ContinueNextStep exposes pre-execution fall-through and suffix effects', () => {
  const result = analyze(
    artifactFor({
      steps: [
        step({ task: 'StopCycle', onError: 'ContinueNextStep' }),
        step({ task: 'Transfer' }),
      ],
    }),
  );
  assert(
    result.findings.some(
      (finding) =>
        finding.kind === 'StopCycleFailureMayFallThrough' &&
        finding.step === 0 &&
        finding.suffixHasEconomicEffects,
    ),
  );
});

test('every current Condition is pure, bounded, and attempt-observed', () => {
  for (const name of conditionNames) {
    const artifact = artifactFor({
      steps: [step({ conditions: [condition(name)] })],
    });
    const projected = analyze(artifact).steps[0].conditions[0];
    assert.equal(projected.type, name);
    assert.equal(projected.pure, true);
    assert.equal(projected.boundedReadCount, 1);
    assert.equal(projected.observationWindow, 'step-attempt-time');
  }
});

test('every current AmountResolution reports frozen or live retry semantics', () => {
  for (const name of amountNames) {
    const amount =
      name === 'Fixed'
        ? fixed()
        : name === 'AllBalance'
          ? { type: name, value: undefined }
          : { type: name, value: 500_000_000 };
    const artifact = artifactFor({ steps: [step({ amount })] });
    const projected = analyze(artifact).steps[0].amounts[0];
    assert.equal(projected.resolution, name);
    assert(projected.dataDependencies.includes('task-policy-capacity'));
    assert.equal(projected.minimumBalanceDependency, 'task-policy');
    assert.equal(projected.feeReserveDependency, 'task-policy');
    if (name === 'PercentageOfCurrent' || name === 'AllBalance') {
      assert.equal(projected.retryObservation, 'reobserve-live');
    } else {
      assert.equal(
        projected.retryObservation,
        'reuse-frozen-with-live-capacity',
      );
    }
  }
});

test('every error policy, actor type, and mutability has only linear controls', () => {
  for (const aaaType of ['User', 'System']) {
    for (const mutability of ['Mutable', 'Immutable']) {
      for (const onError of errorPolicies) {
        const artifact = artifactFor({
          steps: [step({ onError })],
          aaaType,
          mutability,
        });
        const projected = analyze(artifact).steps[0];
        assert.equal(projected.errorPolicy, onError);
        assert(!projected.possibleControls.includes('branch'));
        assert(!projected.possibleControls.includes('jump'));
        assert.equal(
          projected.failureSurface.continuationEligible,
          mutability === 'Mutable' && onError === 'RetryLater',
        );
      }
    }
  }
});

test('Dormant and active programs both produce complete bounded analysis', () => {
  for (const aaaType of ['User', 'System']) {
    for (const mutability of ['Mutable', 'Immutable']) {
      const dormant = artifactFor({
        program: { type: 'Dormant', value: undefined },
        aaaType,
        mutability,
      });
      const dormantResult = analyze(dormant);
      assert.equal(dormantResult.program, 'Dormant');
      assert.deepEqual(dormantResult.steps, []);
      assert.equal(dormantResult.suffixEnvelopes.length, 1);
      const active = artifactFor({ steps: [step()], aaaType, mutability });
      assert.equal(analyze(active).program, 'Active');
    }
  }
});

test('unknown capabilities remain factual and no state-specific claim appears', () => {
  const artifact = artifactFor({
    steps: [step({ task: 'SwapExactOut', onError: 'RetryLater' })],
  });
  const result = analyze(artifact, {
    adapterCapabilities: { identity: 'unknown-profile' },
  });
  assert(
    result.findings.some(
      (finding) =>
        finding.kind === 'AdapterCapability' && finding.status === 'unknown',
    ),
  );
  assert(
    result.findings.some(
      (finding) => finding.kind === 'UnknownTemporaryFailureClassification',
    ),
  );
  assert(
    !JSON.stringify(result).includes('Continuation is active'),
    'static output must not invent runtime state',
  );
});
