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
import {
  AAA_SEMANTIC_MANIFEST,
  parseAaaSemanticManifest,
} from '../src/lib/automation/semantic-manifest.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const stepEditorSource = await readFile(
  new URL('../src/lib/automation/AutomationStepEditor.svelte', import.meta.url),
  'utf8',
);
const taskEditorSource = await readFile(
  new URL('../src/lib/automation/AutomationTaskEditor.svelte', import.meta.url),
  'utf8',
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
const observationFeed = {
  asset_in: native,
  asset_out: local,
  method: variant('PreExecutionSpot'),
  aggregation: { type: 'Ema', value: { half_life_blocks: 100 } },
  scale: 12,
};
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
const conditionNames = [
  'BalanceAbove',
  'BalanceBelow',
  'BalanceEquals',
  'BalanceNotEquals',
  'BlockNumberAbove',
  'BlockNumberBelow',
  'ObservationAbove',
  'ObservationBelow',
  'ObservationEquals',
  'ObservationNotEquals',
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
    LiquidityOps: 'supported',
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
    case 'SwapIn':
      return {
        asset_in: native,
        amount_in: amount,
        asset_out: local,
        slippage_tolerance: 10_000_000,
      };
    case 'SwapOut':
      return {
        asset_out: local,
        amount_out: amount,
        asset_in: native,
        input_limit: { type: 'Absolute', value: 100n },
        slippage_tolerance: 10_000_000,
      };
    case 'AddLiquidity':
      return {
        asset_a: native,
        asset_b: local,
        amount_a: amount,
        amount_b: fixed(20n),
        min_lp_out: 1n,
      };
    case 'RemoveLiquidity':
      return {
        lp_asset: local,
        amount,
        min_amount_a: 1n,
        min_amount_b: 1n,
      };
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
    on_error:
      onError === 'RetryLater'
        ? { type: onError, value: { max_attempts: 3 } }
        : variant(onError),
  };
}

function condition(name) {
  if (name.startsWith('Balance')) {
    return { type: name, value: { asset: native, threshold: 1n } };
  }
  if (name.startsWith('Observation')) {
    return {
      type: name,
      value: { feed: observationFeed, threshold: 1n, max_age_blocks: 12 },
    };
  }
  return { type: name, value: { threshold: 1 } };
}

function activeProgram(steps) {
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
      execution_plan: steps,
      completion_policy: variant('Persistent'),
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

function manifestValues(value, path) {
  let values = [value];
  for (const segment of path.slice(1).split('/')) {
    values = values.flatMap((current) => {
      if (segment === '*') return current;
      if (Array.isArray(current)) return [current[Number(segment)]];
      return [current[segment]];
    });
  }
  return values;
}

function manifestRecipients(contract, parameters) {
  return contract.recipients.flatMap((recipient) => {
    if (recipient.kind !== 'Explicit') return [{ kind: recipient.kind }];
    return manifestValues(parameters, recipient.path).map((value) => ({
      kind: 'Explicit',
      value,
    }));
  });
}

test('analysis is deterministic, exactly bound, and produces every cursor envelope', () => {
  const artifact = artifactFor({
    steps: [
      step({ task: 'SwapIn' }),
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
      ['advance', 'complete-cycle'].includes(current.successfulControl),
    ),
  );
  assert(
    first.steps.every((current) =>
      current.failureControls.every((control) =>
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

test('every Task analysis row equals the complete Rust-generated contract', () => {
  for (const task of taskNames) {
    const artifact = artifactFor({
      steps: [step({ task })],
      aaaType: task === 'Mint' ? 'System' : 'User',
    });
    const projected = analyze(artifact).steps[0];
    const contract = AAA_SEMANTIC_MANIFEST.tasks.find(
      (candidate) => candidate.task === task,
    );
    assert(contract);
    assert.equal(projected.task, task);
    assert.deepEqual(
      projected.requiredAdapters,
      contract.requiredAdapter === 'None' ? [] : [contract.requiredAdapter],
    );
    assert.deepEqual(
      projected.economicSurface.assetsRead,
      contract.assetsRead.flatMap((path) =>
        manifestValues(projected.parameters, path),
      ),
    );
    assert.deepEqual(
      projected.economicSurface.assetsWritten,
      contract.assetsWritten.flatMap((path) =>
        manifestValues(projected.parameters, path),
      ),
    );
    assert.equal(
      projected.economicSurface.adapterDerivedAssetsRead,
      contract.readsAdapterDerivedAssets,
    );
    assert.equal(
      projected.economicSurface.adapterDerivedAssetsWritten,
      contract.writesAdapterDerivedAssets,
    );
    assert.deepEqual(
      projected.economicSurface.recipients,
      manifestRecipients(contract, projected.parameters),
    );
    assert.equal(
      projected.economicSurface.transferExposure,
      contract.effects.includes('Transfer'),
    );
    assert.equal(
      projected.economicSurface.mintExposure,
      contract.effects.includes('SupplyMint'),
    );
    assert.equal(
      projected.economicSurface.burnExposure,
      contract.effects.includes('SupplyBurn'),
    );
    assert.equal(
      projected.economicSurface.liquidityMutation,
      contract.effects.includes('LiquidityMutation'),
    );
    assert.equal(
      projected.economicSurface.stakingMutation,
      contract.effects.includes('StakingMutation'),
    );
    assert.equal(
      projected.economicSurface.committedNonCompensatedEffects,
      contract.committedNonCompensatedEffects,
    );
    assert.equal(projected.availability, contract.availability);
    assert.equal(
      projected.successfulControl,
      contract.successfulControl === 'CompleteCycle'
        ? 'complete-cycle'
        : 'advance',
    );
    assert.equal(projected.weightOwner, contract.weightOwner);
    assert.equal(
      projected.boundedInternalAlgorithm,
      contract.boundedInternalAlgorithm,
    );
    assert.deepEqual(
      projected.amounts.map((amount) => amount.path),
      contract.amountSurfaces.map((amount) => amount.path),
    );
    assert(BigInt(projected.costs.totalUpper.refTime) > 0n);
    assert(BigInt(projected.costs.totalUpper.proofSize) > 0n);
    assert.equal(projected.failureSurface.temporaryFailureReachability, 'no');
  }
});

test('generated manifest preserves recipient kinds and rejects identity drift', () => {
  const analyzeTask = (task) =>
    analyze(artifactFor({ steps: [step({ task })] })).steps[0];
  assert.deepEqual(analyzeTask('Transfer').economicSurface.recipients, [
    { kind: 'Explicit', value: account },
  ]);
  assert.deepEqual(analyzeTask('SplitTransfer').economicSurface.recipients, [
    { kind: 'Explicit', value: account },
    { kind: 'Explicit', value: account },
    { kind: 'ActorSovereign' },
  ]);
  assert.deepEqual(analyzeTask('SwapIn').economicSurface.recipients, [
    { kind: 'ActorSovereign' },
  ]);
  assert.deepEqual(analyzeTask('DonateLiquidity').economicSurface.recipients, [
    { kind: 'AdapterDerived' },
  ]);
  const wrongVersion = structuredClone(AAA_SEMANTIC_MANIFEST);
  wrongVersion.formatVersion = 3;
  assert.throws(
    () => parseAaaSemanticManifest(wrongVersion),
    /Unsupported AAA semantic manifest version/,
  );
  const unknownTask = structuredClone(AAA_SEMANTIC_MANIFEST);
  unknownTask.tasks[0].task = 'UnknownTask';
  assert.throws(
    () => parseAaaSemanticManifest(unknownTask),
    /Task variants are unknown/,
  );
  const reorderedCondition = structuredClone(AAA_SEMANTIC_MANIFEST);
  reorderedCondition.conditions[0].scaleIndex = 1;
  assert.throws(
    () => parseAaaSemanticManifest(reorderedCondition),
    /Condition SCALE indices are unknown/,
  );
});

test('split transfer analysis exposes atomic temporary deposit preflight', () => {
  const result = analyze(
    artifactFor({ steps: [step({ task: 'SplitTransfer' })] }),
  );
  assert.deepEqual(
    result.findings.filter(
      (finding) => finding.kind === 'SplitTransferDepositPreflight',
    ),
    [
      {
        kind: 'SplitTransferDepositPreflight',
        step: 0,
        failureClass: 'Temporary',
        atomic: true,
      },
    ],
  );
});

test('System swap analysis exposes local reference limits without fair-price claims', () => {
  const steps = [step({ task: 'SwapIn' }), step({ task: 'SwapOut' })];
  const system = analyze(artifactFor({ aaaType: 'System', steps }));
  assert.deepEqual(
    system.findings.filter(
      (finding) => finding.kind === 'SystemReferenceDeviationGuard',
    ),
    [0, 1].map((stepIndex) => ({
      kind: 'SystemReferenceDeviationGuard',
      step: stepIndex,
      reference: 'FreshEmaOrDirectReserve',
      localExecutionGuard: true,
      fairPriceProof: false,
      orderingProtection: false,
    })),
  );
  const user = analyze(artifactFor({ steps }));
  assert.equal(
    user.findings.some(
      (finding) => finding.kind === 'SystemReferenceDeviationGuard',
    ),
    false,
  );
  for (const disclosure of [
    'fresh EMA or direct-pool reserve reference',
    'manipulated pool state',
    'fair price nor transaction-order protection',
  ]) {
    assert(taskEditorSource.includes(disclosure));
  }
});

test('fixed split warnings require provenance-bound minimum and zero-balance evidence', () => {
  const nativeArtifact = artifactFor({
    steps: [step({ task: 'SplitTransfer', amount: fixed(20n) })],
  });
  const baseline = analyze(nativeArtifact);
  const nativeParameters = baseline.steps[0].parameters;
  const nativeEvidence = {
    provenance: 'FinalizedStateProjection',
    identity: 'finalized-minimums@native-1',
    blockHash: `0x${'22'.repeat(32)}`,
    entries: [
      {
        asset: nativeParameters.asset,
        minimumBalance: '9',
        recipientBalances: [
          { recipient: nativeParameters.legs[0].to, balance: '0' },
        ],
      },
    ],
  };
  const below = analyze(nativeArtifact, {
    minimumBalanceEvidence: nativeEvidence,
  });
  assert.deepEqual(
    below.findings.filter(
      (finding) => finding.kind === 'SplitTransferLegBelowKnownMinimum',
    ),
    [
      {
        kind: 'SplitTransferLegBelowKnownMinimum',
        step: 0,
        leg: 0,
        asset: nativeParameters.asset,
        recipient: nativeParameters.legs[0].to,
        amount: '8',
        minimumBalance: '9',
        evidenceIdentity: nativeEvidence.identity,
        evidenceBlockHash: nativeEvidence.blockHash,
      },
    ],
  );
  assert.equal(
    below.identity.minimumBalanceEvidenceIdentity,
    nativeEvidence.identity,
  );
  assert.equal(
    below.identity.minimumBalanceEvidenceBlockHash,
    nativeEvidence.blockHash,
  );

  for (const minimumBalance of ['8', '7']) {
    const result = analyze(nativeArtifact, {
      minimumBalanceEvidence: {
        ...nativeEvidence,
        entries: [{ ...nativeEvidence.entries[0], minimumBalance }],
      },
    });
    assert.equal(
      result.findings.some(
        (finding) => finding.kind === 'SplitTransferLegBelowKnownMinimum',
      ),
      false,
    );
  }
  assert.equal(
    baseline.findings.some(
      (finding) => finding.kind === 'SplitTransferLegBelowKnownMinimum',
    ),
    false,
  );
  assert.equal(baseline.identity.minimumBalanceEvidenceIdentity, null);
  assert.equal(baseline.identity.minimumBalanceEvidenceBlockHash, null);
  const funded = analyze(nativeArtifact, {
    minimumBalanceEvidence: {
      ...nativeEvidence,
      entries: [
        {
          ...nativeEvidence.entries[0],
          recipientBalances: [
            { recipient: nativeParameters.legs[0].to, balance: '1' },
          ],
        },
      ],
    },
  });
  assert.equal(
    funded.findings.some(
      (finding) => finding.kind === 'SplitTransferLegBelowKnownMinimum',
    ),
    false,
  );

  const localStep = step({ task: 'SplitTransfer', amount: fixed(20n) });
  localStep.task.value.asset = local;
  const localArtifact = artifactFor({ steps: [localStep] });
  const localParameters = analyze(localArtifact).steps[0].parameters;
  const localResult = analyze(localArtifact, {
    minimumBalanceEvidence: {
      provenance: 'FinalizedStateProjection',
      identity: 'finalized-minimums@local-7',
      blockHash: `0x${'33'.repeat(32)}`,
      entries: [
        {
          asset: localParameters.asset,
          minimumBalance: '9',
          recipientBalances: [
            { recipient: localParameters.legs[0].to, balance: '0' },
          ],
        },
      ],
    },
  });
  assert.equal(
    localResult.findings.filter(
      (finding) => finding.kind === 'SplitTransferLegBelowKnownMinimum',
    ).length,
    1,
  );
  assert.throws(
    () =>
      analyze(nativeArtifact, {
        minimumBalanceEvidence: { ...nativeEvidence, identity: '' },
      }),
    /evidence identity is required/,
  );
});

test('authoring copy separates skips, funding, and task failure classes', () => {
  for (const label of [
    'Condition false:',
    'Resolution skipped:',
    'Funding unavailable:',
    'Temporary task failure:',
    'Permanent task failure:',
    'Abort on task failure',
  ]) {
    assert(stepEditorSource.includes(label), `missing outcome label: ${label}`);
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
    assert.equal(result.steps[0].successfulControl, 'advance');
    assert.deepEqual(result.steps[0].failureControls, ['advance', 'terminate']);
  }
  const always = analyze(artifactFor({ steps: [step()] })).steps[0];
  assert.equal(always.conditionSet.mode, 'Always');
  assert.equal(always.conditionSet.atomicCount, 0);
});

test('StopCycle separates successful cycle completion from failure fall-through', () => {
  const result = analyze(
    artifactFor({
      steps: [
        step({ task: 'StopCycle', onError: 'ContinueNextStep' }),
        step({ task: 'Transfer' }),
      ],
    }),
  );
  assert.equal(result.steps[0].successfulControl, 'complete-cycle');
  assert.deepEqual(result.steps[0].failureControls, ['advance']);
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
    if (name.startsWith('Observation')) {
      assert.equal(projected.observation, 'scalar-observation');
      assert.deepEqual(projected.readSurface, {
        feed: {
          aggregation: {
            type: 'Ema',
            value: {
              half_life_blocks: {
                $runtimeType: 'number',
                $integer: '100',
              },
            },
          },
          asset_in: { type: 'Native', value: { $none: true } },
          asset_out: {
            type: 'Local',
            value: { $runtimeType: 'number', $integer: '7' },
          },
          method: { type: 'PreExecutionSpot', value: { $none: true } },
          scale: { $runtimeType: 'number', $integer: '12' },
        },
        maxAgeBlocks: 12,
        freshness: 'fresh-only',
        nonFreshResult: 'false',
      });
    }
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
    const contract = AAA_SEMANTIC_MANIFEST.amountResolutions.find(
      (candidate) => candidate.resolution === name,
    );
    assert(contract);
    assert.equal(projected.resolution, name);
    assert.deepEqual(
      projected.dataDependencies,
      contract.dataDependencies.map(
        (dependency) =>
          ({
            ArtifactValue: 'artifact-value',
            CurrentBalanceOrShares: 'current-balance-or-shares',
            TriggerSnapshot: 'trigger-snapshot',
            LastFundingSnapshot: 'last-funding-snapshot',
            TaskPolicyCapacity: 'task-policy-capacity',
          })[dependency],
      ),
    );
    assert.equal(projected.minimumBalanceDependency, 'task-policy');
    assert.equal(projected.feeReserveDependency, 'task-policy');
    assert.equal(
      projected.valueObservation,
      {
        ArtifactTime: 'artifact-time',
        LogicalRunStart: 'logical-run-start',
        StepAttemptTime: 'step-attempt-time',
      }[contract.valueObservationWindow],
    );
    assert.equal(
      projected.retryObservation,
      contract.retryObservation === 'ReobserveLiveValue'
        ? 'reobserve-live'
        : 'reuse-frozen-with-live-capacity',
    );
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
        assert.equal(
          projected.retryMaxAttempts,
          onError === 'RetryLater' ? 3 : null,
        );
        assert.equal(projected.successfulControl, 'advance');
        const expectedFailureControls =
          onError === 'ContinueNextStep'
            ? ['advance']
            : onError === 'AbortCycle'
              ? ['advance', 'terminate']
              : mutability === 'Mutable'
                ? ['terminate', 'stutter-current']
                : ['terminate'];
        assert.deepEqual(projected.failureControls, expectedFailureControls);
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
      assert.equal(dormantResult.completionPolicy, null);
      assert.equal(dormantResult.trigger, null);
      assert.deepEqual(dormantResult.steps, []);
      assert.equal(dormantResult.suffixEnvelopes.length, 1);
      const active = artifactFor({ steps: [step()], aaaType, mutability });
      assert.equal(analyze(active).program, 'Active');
    }
  }
});

test('trigger analysis separates readiness sources from admission and runtime proof', () => {
  const addressEvent = {
    type: 'OnAddressEvent',
    value: {
      source_filter: variant('Any'),
      asset_filter: variant('Any'),
    },
  };
  const observationChange = {
    type: 'OnObservationChange',
    value: { feed: observationFeed },
  };
  const programWithTrigger = (trigger) => {
    const program = activeProgram([step()]);
    program.value.schedule.trigger = trigger;
    return program;
  };
  const immediate = analyze(
    artifactFor({
      program: programWithTrigger({
        type: 'Immediate',
        value: {
          sources: [variant('Manual'), addressEvent, observationChange],
        },
      }),
    }),
  );
  assert.equal(immediate.completionPolicy, 'Persistent');
  assert.deepEqual(immediate.trigger, {
    admission: 'Immediate',
    everyBlocks: null,
    sourceCount: 3,
    sourceKinds: ['Manual', 'AddressEvent', 'ObservationChange'],
    observationFeeds: [
      {
        aggregation: {
          type: 'Ema',
          value: {
            half_life_blocks: { $runtimeType: 'number', $integer: '100' },
          },
        },
        asset_in: { type: 'Native', value: { $none: true } },
        asset_out: {
          type: 'Local',
          value: { $runtimeType: 'number', $integer: '7' },
        },
        method: { type: 'PreExecutionSpot', value: { $none: true } },
        scale: { $runtimeType: 'number', $integer: '12' },
      },
    ],
  });
  assert(
    immediate.findings.some(
      (finding) =>
        finding.kind === 'ExternallySignalledAdmission' &&
        finding.gate === 'Immediate',
    ),
  );
  const triggerAmountProgram = programWithTrigger({
    type: 'Immediate',
    value: { sources: [observationChange] },
  });
  triggerAmountProgram.value.execution_plan = [
    step({
      amount: { type: 'PercentageOfTrigger', value: 500_000_000 },
    }),
  ];
  const triggerAmountAnalysis = analyze(
    artifactFor({ program: triggerAmountProgram }),
  );
  assert(
    triggerAmountAnalysis.findings.some(
      (finding) =>
        finding.kind === 'TriggerAmountCompatibilityViolation' &&
        finding.reason === 'AddressEventOnlyRequired' &&
        finding.steps[0] === 0 &&
        finding.sourceKinds[0] === 'ObservationChange',
    ),
  );

  const periodic = analyze(
    artifactFor({
      program: programWithTrigger({
        type: 'Cadenced',
        value: {
          every_blocks: 10,
          mode: variant('Always'),
        },
      }),
    }),
  );
  assert.deepEqual(periodic.trigger, {
    admission: 'CadencedAlways',
    everyBlocks: 10,
    sourceCount: 0,
    sourceKinds: [],
    observationFeeds: [],
  });
  assert(
    periodic.findings.some(
      (finding) =>
        finding.kind === 'PeriodicAdmission' && finding.everyBlocks === 10,
    ),
  );

  const signalled = analyze(
    artifactFor({
      program: programWithTrigger({
        type: 'Cadenced',
        value: {
          every_blocks: 20,
          mode: {
            type: 'WhenSignalled',
            value: [addressEvent],
          },
        },
      }),
    }),
  );
  assert.deepEqual(signalled.trigger, {
    admission: 'CadencedWhenSignalled',
    everyBlocks: 20,
    sourceCount: 1,
    sourceKinds: ['AddressEvent'],
    observationFeeds: [],
  });
  assert(
    signalled.findings.some(
      (finding) =>
        finding.kind === 'ExternallySignalledAdmission' &&
        finding.gate === 'Cadenced',
    ),
  );
  assert(!('conditions' in signalled.trigger));
  assert(!('steps' in signalled.trigger));
  assert(!JSON.stringify(signalled.trigger).includes('runtime execution'));
});

test('unknown capabilities remain factual and no state-specific claim appears', () => {
  const artifact = artifactFor({
    steps: [step({ task: 'SwapOut', onError: 'RetryLater' })],
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
