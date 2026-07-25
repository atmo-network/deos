/*
Domain: AAA authoring validation
Owns: Typed builder operations, structural guards, exact lowering, canonical artifact, and analyzer handoff fixtures.
Excludes: Runtime queries, signing, submission, simulation, recipes, and browser rendering.
Zone: Web-client validation entrypoint; composes automation domain contracts only.
*/
import { encodeAddress } from '@polkadot/util-crypto';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeAaaProgram } from '../src/lib/automation/analysis.ts';
import {
  AAA_AUTHORING_CONDITION_TYPES,
  AAA_AUTHORING_TASK_TYPES,
  appendAaaStep,
  createAaaArtifactFromAuthoring,
  createAaaAuthoringCondition,
  createAaaAuthoringTask,
  lowerAaaAuthoringProgram,
  moveAaaStep,
  removeAaaStep,
  replaceAaaStep,
  validateAaaAuthoringProgram,
} from '../src/lib/automation/authoring.ts';
import { inspectAaaPlanArtifact } from '../src/lib/automation/plan-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const accountA = encodeAddress(new Uint8Array(32).fill(1), 42);
const accountB = encodeAddress(new Uint8Array(32).fill(2), 42);
const native = { type: 'Native' };
const local = { type: 'Local', id: 7 };
const fixed = (value = '10') => ({ type: 'Fixed', value });

const weightModel = {
  identity: 'authoring-test-weights',
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
};

function transferTask(amount = fixed()) {
  return { type: 'Transfer', to: accountA, asset: native, amount };
}

function authoringStep(key, task = transferTask(), overrides = {}) {
  return {
    key,
    conditionSet: { type: 'Always' },
    task,
    errorPolicy: 'AbortCycle',
    ...overrides,
  };
}

function program(steps = [authoringStep('step-0')], overrides = {}) {
  return {
    aaaType: 'User',
    mutability: 'Mutable',
    trigger: { type: 'Manual' },
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps,
    ...overrides,
  };
}

function artifact(value) {
  return createAaaArtifactFromAuthoring({
    program: value,
    metadataBytes,
    runtime,
  });
}

test('authoring controls cover every current task and condition variant', () => {
  assert.deepEqual(
    AAA_AUTHORING_TASK_TYPES.map((type) => createAaaAuthoringTask(type).type),
    AAA_AUTHORING_TASK_TYPES,
  );
  assert.deepEqual(
    AAA_AUTHORING_CONDITION_TYPES.map(
      (type) => createAaaAuthoringCondition(type).type,
    ),
    AAA_AUTHORING_CONDITION_TYPES,
  );
  assert.equal(AAA_AUTHORING_TASK_TYPES.length, 12);
  assert.equal(AAA_AUTHORING_CONDITION_TYPES.length, 6);
});

test('typed authoring lowers to one deterministic exact canonical artifact', () => {
  const draft = program([
    authoringStep('swap', {
      type: 'SwapExactOut',
      assetIn: native,
      assetOut: local,
      amountOut: fixed('25'),
      slippageParts: 10_000_000,
    }),
    authoringStep('transfer', transferTask({ type: 'AllBalance' }), {
      conditionSet: {
        type: 'All',
        conditions: [
          {
            type: 'BalanceAbove',
            asset: local,
            threshold: '0',
          },
        ],
      },
      errorPolicy: 'RetryLater',
    }),
  ]);
  const first = artifact(draft);
  const second = artifact(structuredClone(draft));
  assert.deepEqual(first, second);
  const inspection = inspectAaaPlanArtifact(first, metadataBytes, runtime);
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.equal(inspection.projection.value.execution_plan.length, 2);
    assert.equal(
      inspection.projection.value.execution_plan[1].on_error.type,
      'RetryLater',
    );
  }
  const analysis = analyzeAaaProgram({
    artifact: first,
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'authoring-test-runtime' },
    weightModel,
  });
  assert.equal(analysis.identity.planId, first.planId);
  assert.deepEqual(
    analysis.steps.map((current) => current.task),
    ['SwapExactOut', 'Transfer'],
  );
});

test('ordered builder operations change only explicit Step order or content', () => {
  const initial = program([
    authoringStep('a', transferTask(fixed('1'))),
    authoringStep('b', { type: 'Burn', asset: native, amount: fixed('2') }),
  ]);
  const appended = appendAaaStep(
    initial,
    authoringStep('c', { type: 'Stake', asset: native, amount: fixed('3') }),
  );
  assert.deepEqual(
    initial.steps.map((current) => current.key),
    ['a', 'b'],
  );
  assert.deepEqual(
    appended.steps.map((current) => current.key),
    ['a', 'b', 'c'],
  );
  const moved = moveAaaStep(appended, 2, 0);
  assert.deepEqual(
    moved.steps.map((current) => current.key),
    ['c', 'a', 'b'],
  );
  const replaced = replaceAaaStep(
    moved,
    'a',
    authoringStep('a', transferTask(fixed('9'))),
  );
  assert.equal(replaced.steps[1].task.amount.value, '9');
  const removed = removeAaaStep(replaced, 'b');
  assert.deepEqual(
    removed.steps.map((current) => current.key),
    ['c', 'a'],
  );
  assert.notEqual(artifact(initial).planId, artifact(removed).planId);
});

test('every current Task lowers through metadata and remains analyzer-visible', () => {
  const tasks = [
    transferTask(),
    {
      type: 'SplitTransfer',
      asset: native,
      amount: fixed(),
      legs: [
        { to: accountA, shareParts: 400_000_000 },
        { to: accountB, shareParts: 600_000_000 },
      ],
    },
    {
      type: 'SwapExactIn',
      assetIn: native,
      assetOut: local,
      amountIn: fixed(),
      slippageParts: 0,
    },
    {
      type: 'SwapExactOut',
      assetIn: native,
      assetOut: local,
      amountOut: fixed(),
      slippageParts: 0,
    },
    {
      type: 'AddLiquidity',
      assetA: native,
      assetB: local,
      amountA: fixed(),
      amountB: fixed('20'),
    },
    { type: 'RemoveLiquidity', lpAsset: local, amount: fixed() },
    { type: 'Burn', asset: native, amount: fixed() },
    { type: 'Mint', asset: native, amount: fixed() },
    { type: 'Stake', asset: native, amount: fixed() },
    {
      type: 'DonateLiquidity',
      assetA: native,
      assetB: local,
      amount: fixed(),
      maxRatioErrorParts: 0,
    },
    { type: 'Unstake', asset: native, shares: fixed() },
    { type: 'StopCycle' },
  ];
  for (const task of tasks) {
    const draft = program([authoringStep('only', task)], {
      aaaType: task.type === 'Mint' ? 'System' : 'User',
      fundingPolicy:
        task.type === 'Mint'
          ? { type: 'RuntimePolicy' }
          : { type: 'OwnerOnly' },
    });
    const result = analyzeAaaProgram({
      artifact: artifact(draft),
      metadataBytes,
      runtime: { ...runtime, modelIdentity: 'authoring-test-runtime' },
      weightModel,
    });
    assert.equal(result.steps[0].task, task.type);
  }
  assert.equal(tasks.length, 12);
});

test('Always, All, and Any lower exactly and empty groups fail before encoding', () => {
  const atom = { type: 'BlockNumberAbove', threshold: 1 };
  const modes = [
    { type: 'Always' },
    { type: 'All', conditions: [atom] },
    { type: 'Any', conditions: [atom] },
  ];
  for (const conditionSet of modes) {
    const lowered = lowerAaaAuthoringProgram(
      program([authoringStep('only', transferTask(), { conditionSet })]),
    );
    assert.equal(
      lowered.value.execution_plan[0].conditions.type,
      conditionSet.type,
    );
  }
  for (const type of ['All', 'Any']) {
    const invalid = program([
      authoringStep('only', transferTask(), {
        conditionSet: { type, conditions: [] },
      }),
    ]);
    assert.equal(validateAaaAuthoringProgram(invalid).valid, false);
    assert.throws(() => lowerAaaAuthoringProgram(invalid), /at least one/);
  }
});

test('every Condition and AmountResolution lowers without changing step topology', () => {
  const conditions = [
    { type: 'BalanceAbove', asset: native, threshold: '1' },
    { type: 'BalanceBelow', asset: native, threshold: '1' },
    { type: 'BalanceEquals', asset: native, threshold: '1' },
    { type: 'BalanceNotEquals', asset: native, threshold: '1' },
    { type: 'BlockNumberAbove', threshold: 1 },
    { type: 'BlockNumberBelow', threshold: 1 },
  ];
  for (const current of conditions) {
    const lowered = lowerAaaAuthoringProgram(
      program([
        authoringStep('only', transferTask(), {
          conditionSet: { type: 'Any', conditions: [current] },
        }),
      ]),
    );
    assert.equal(lowered.value.execution_plan.length, 1);
    assert.equal(lowered.value.execution_plan[0].conditions.type, 'Any');
    assert.equal(
      lowered.value.execution_plan[0].conditions.value[0].type,
      current.type,
    );
  }
  const amounts = [
    fixed(),
    { type: 'PercentageOfCurrent', parts: 500_000_000 },
    { type: 'PercentageOfTrigger', parts: 500_000_000 },
    { type: 'PercentageOfLastFunding', parts: 500_000_000 },
    { type: 'AllBalance' },
  ];
  for (const amount of amounts) {
    const lowered = lowerAaaAuthoringProgram(
      program([authoringStep('only', transferTask(amount))]),
    );
    assert.equal(
      lowered.value.execution_plan[0].task.value.amount.type,
      amount.type,
    );
  }
});

test('typed validation rejects control-flow-adjacent and runtime-invalid drafts', () => {
  const immutableRetry = program(
    [authoringStep('only', transferTask(), { errorPolicy: 'RetryLater' })],
    { mutability: 'Immutable' },
  );
  assert.equal(validateAaaAuthoringProgram(immutableRetry).valid, false);
  const userMint = program([
    authoringStep('only', { type: 'Mint', asset: native, amount: fixed() }),
  ]);
  assert.equal(validateAaaAuthoringProgram(userMint).valid, false);
  const duplicateSplit = program([
    authoringStep('only', {
      type: 'SplitTransfer',
      asset: native,
      amount: fixed(),
      legs: [
        { to: accountA, shareParts: 600_000_000 },
        { to: accountA, shareParts: 600_000_000 },
      ],
    }),
  ]);
  const splitValidation = validateAaaAuthoringProgram(duplicateSplit);
  assert.equal(splitValidation.valid, false);
  if (!splitValidation.valid) {
    assert(
      splitValidation.issues.some((issue) => /unique/.test(issue.message)),
    );
    assert(
      splitValidation.issues.some((issue) => /exceed/.test(issue.message)),
    );
  }
  assert.equal(validateAaaAuthoringProgram(program([])).valid, false);
  assert.throws(() => lowerAaaAuthoringProgram(immutableRetry), /RetryLater/);
});

test('scenario corpus lowers every expressible or partial execution core without inventing missing predicates', () => {
  const all = (...conditions) => ({ type: 'All', conditions });
  const balanceAbove = (asset = native) => ({
    type: 'BalanceAbove',
    asset,
    threshold: '1',
  });
  const split = (asset = native) => ({
    type: 'SplitTransfer',
    asset,
    amount: { type: 'AllBalance' },
    legs: [
      { to: accountA, shareParts: 500_000_000 },
      { to: accountB, shareParts: 500_000_000 },
    ],
  });
  const swap = {
    type: 'SwapExactIn',
    assetIn: local,
    assetOut: native,
    amountIn: { type: 'AllBalance' },
    slippageParts: 10_000_000,
  };
  const scenarios = [
    {
      name: 'DEOS Burn Actor',
      program: program(
        [
          authoringStep('swap', swap, {
            conditionSet: all(balanceAbove(local)),
          }),
          authoringStep('burn', {
            type: 'Burn',
            asset: native,
            amount: { type: 'AllBalance' },
          }),
        ],
        { aaaType: 'System', fundingPolicy: { type: 'RuntimePolicy' } },
      ),
      tasks: ['SwapExactIn', 'Burn'],
    },
    {
      name: 'DEOS Fee Sink',
      program: program([authoringStep('split', split())]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'DEOS BLDR splitter',
      program: program([
        authoringStep('split', split(local), {
          conditionSet: all(balanceAbove(local)),
        }),
      ]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'DEOS Liquidity Actor core',
      program: program([
        authoringStep('liquidity', {
          type: 'AddLiquidity',
          assetA: native,
          assetB: local,
          amountA: fixed(),
          amountB: fixed(),
        }),
      ]),
      tasks: ['AddLiquidity'],
    },
    {
      name: 'Periodic DCA',
      program: program([authoringStep('dca', swap)], {
        trigger: { type: 'Timer', everyBlocks: 10 },
      }),
      tasks: ['SwapExactIn'],
    },
    {
      name: 'Threshold payroll',
      program: program([
        authoringStep('payroll', split(), {
          conditionSet: all(balanceAbove(), {
            type: 'BlockNumberAbove',
            threshold: 1,
          }),
        }),
      ]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'Resilient swap-to-liquidity pipeline',
      program: program([
        authoringStep('swap', swap),
        authoringStep(
          'liquidity',
          {
            type: 'AddLiquidity',
            assetA: native,
            assetB: local,
            amountA: fixed(),
            amountB: fixed(),
          },
          { errorPolicy: 'RetryLater' },
        ),
        authoringStep('remainder', transferTask()),
      ]),
      tasks: ['SwapExactIn', 'AddLiquidity', 'Transfer'],
    },
    {
      name: 'Stake and unstake execution core',
      program: program([
        authoringStep('stake', {
          type: 'Stake',
          asset: native,
          amount: fixed(),
        }),
        authoringStep('unstake', {
          type: 'Unstake',
          asset: native,
          shares: fixed(),
        }),
      ]),
      tasks: ['Stake', 'Unstake'],
    },
  ];
  for (const scenario of scenarios) {
    const analysis = analyzeAaaProgram({
      artifact: artifact(scenario.program),
      metadataBytes,
      runtime: { ...runtime, modelIdentity: 'scenario-corpus-runtime' },
      weightModel,
    });
    assert.deepEqual(
      analysis.steps.map((step) => step.task),
      scenario.tasks,
      scenario.name,
    );
    assert(
      analysis.steps.every(
        (step) => BigInt(step.costs.totalUpper.refTime) > 0n,
      ),
      scenario.name,
    );
  }
  assert.equal(scenarios.length, 8);
  assert.equal(AAA_AUTHORING_CONDITION_TYPES.includes('PriceAbove'), false);
  assert.equal(AAA_AUTHORING_CONDITION_TYPES.includes('PortfolioRatio'), false);
  assert.equal(AAA_AUTHORING_TASK_TYPES.includes('Rebalance'), false);
});

test('trigger and funding policy variants lower as typed ProgramInput fields', () => {
  const drafts = [
    program(undefined, {
      trigger: { type: 'Manual' },
      fundingPolicy: { type: 'OwnerOnly' },
    }),
    program(undefined, {
      trigger: { type: 'Timer', everyBlocks: 10 },
      fundingPolicy: { type: 'RuntimePolicy' },
      aaaType: 'System',
    }),
    program(undefined, {
      trigger: {
        type: 'OnAddressEvent',
        sourceFilter: { type: 'Whitelist', accounts: [accountA] },
        assetFilter: { type: 'Whitelist', assets: [native, local] },
      },
      fundingPolicy: { type: 'SignedAllowlist', accounts: [accountA] },
    }),
    program(undefined, {
      trigger: {
        type: 'OnAddressEvent',
        sourceFilter: { type: 'Any' },
        assetFilter: { type: 'Any' },
      },
      fundingPolicy: { type: 'AnySource' },
    }),
  ];
  for (const draft of drafts) {
    const canonical = artifact(draft);
    assert.equal(
      inspectAaaPlanArtifact(canonical, metadataBytes, runtime).valid,
      true,
    );
  }
});
