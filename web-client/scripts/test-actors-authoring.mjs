/*
Domain: Actors authoring validation
Owns: Typed builder operations, structural guards, exact lowering, canonical artifact, and analyzer handoff fixtures.
Excludes: Runtime queries, signing, submission, simulation, recipes, and browser rendering.
Zone: Web-client validation entrypoint; composes automation domain contracts only.
*/
import { encodeAddress } from '@polkadot/util-crypto';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeActorContract } from '../src/lib/automation/analysis.ts';
import {
  ACTORS_AUTHORING_CONDITION_TYPES,
  ACTORS_AUTHORING_TASK_TYPES,
  appendActorStep,
  createActorArtifactFromAuthoring,
  createActorAuthoringPredicate,
  createActorAuthoringTask,
  lowerActorAuthoringContract,
  moveActorStep,
  removeActorStep,
  replaceActorStep,
  validateActorAuthoringContract,
} from '../src/lib/automation/authoring.ts';
import { inspectActorContractArtifact } from '../src/lib/automation/contract-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const triggerEditorSource = await readFile(
  new URL(
    '../src/lib/automation/AutomationTriggerEditor.svelte',
    import.meta.url,
  ),
  'utf8',
);
const conditionEditorSource = await readFile(
  new URL(
    '../src/lib/automation/AutomationConditionEditor.svelte',
    import.meta.url,
  ),
  'utf8',
);
const automationWidgetSource = await readFile(
  new URL('../src/lib/widgets/AutomationWidget.svelte', import.meta.url),
  'utf8',
);
const taskEditorSource = await readFile(
  new URL('../src/lib/automation/AutomationTaskEditor.svelte', import.meta.url),
  'utf8',
);
const stepEditorSource = await readFile(
  new URL('../src/lib/automation/AutomationStepEditor.svelte', import.meta.url),
  'utf8',
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
const observationFeed = {
  assetIn: native,
  assetOut: local,
  method: 'PreExecutionSpot',
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
  scale: 12,
};
const fixed = (value = '10') => ({ type: 'Fixed', value });
const addressTrigger = {
  type: 'Immediate',
  sources: [
    {
      type: 'OnAddressEvent',
      sourceFilter: { type: 'Any' },
      assetFilter: { type: 'Any' },
    },
  ],
};
const observationTrigger = {
  type: 'Immediate',
  sources: [{ type: 'OnObservationChange', feed: observationFeed }],
};

const weightModel = {
  identity: 'authoring-test-weights',
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
};

function transferTask(amount = fixed()) {
  return { type: 'Transfer', to: accountA, asset: native, amount };
}

function authoringStep(key, task = transferTask(), overrides = {}) {
  return {
    key,
    preconditions: { type: 'Unconditional' },
    task,
    errorPolicy: { type: 'AbortCycle' },
    ...overrides,
  };
}

function contract(steps = [authoringStep('step-0')], overrides = {}) {
  return {
    actorType: 'User',
    mutability: 'Mutable',
    completionPolicy: 'Persistent',
    trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps,
    ...overrides,
  };
}

function artifact(value) {
  return createActorArtifactFromAuthoring({
    contract: value,
    metadataBytes,
    runtime,
  });
}

test('authoring controls cover every current task and predicate variant', () => {
  assert.deepEqual(
    ACTORS_AUTHORING_TASK_TYPES.map(
      (type) => createActorAuthoringTask(type).type,
    ),
    ACTORS_AUTHORING_TASK_TYPES,
  );
  assert.deepEqual(
    ACTORS_AUTHORING_CONDITION_TYPES.map(
      (type) => createActorAuthoringPredicate(type).type,
    ),
    ACTORS_AUTHORING_CONDITION_TYPES,
  );
  assert.equal(ACTORS_AUTHORING_TASK_TYPES.length, 12);
  assert.equal(ACTORS_AUTHORING_CONDITION_TYPES.length, 10);
});

test('one metadata-aligned eight-step baseline applies to both actor classes', () => {
  const steps = Array.from({ length: 9 }, (_, index) =>
    authoringStep(`step-${index}`),
  );
  for (const actorType of ['User', 'System']) {
    assert.equal(
      validateActorAuthoringContract(contract(steps.slice(0, 8), { actorType }))
        .valid,
      true,
    );
    const tooLong = validateActorAuthoringContract(
      contract(steps, { actorType }),
    );
    assert.equal(tooLong.valid, false);
    if (!tooLong.valid) {
      assert(
        tooLong.issues.some(
          (issue) =>
            issue.path === 'steps' && /1\.\.8 steps/.test(issue.message),
        ),
      );
    }
  }
});

test('completion policy lowers exactly and rejects unknown lifecycle values', () => {
  const persistent = lowerActorAuthoringContract(contract());
  assert.deepEqual(persistent.value.completion, {
    type: 'Persistent',
    value: undefined,
  });
  const oneShot = lowerActorAuthoringContract(
    contract(undefined, { completionPolicy: 'CloseAfterProductiveCycle' }),
  );
  assert.deepEqual(oneShot.value.completion, {
    type: 'CloseAfterProductiveCycle',
    value: undefined,
  });
  assert.equal(
    validateActorAuthoringContract(
      contract(undefined, { completionPolicy: 'UnknownLifecycle' }),
    ).valid,
    false,
  );
  assert.match(automationWidgetSource, /Close after productive cycle/);
  assert.match(automationWidgetSource, /committed effectful task/);
});

test('optional auto-close target lowers exactly and rejects invalid u64 values', () => {
  const target = lowerActorAuthoringContract(
    contract(undefined, { autoCloseAtCycleNonce: 7n }),
  );
  assert.equal(target.value.auto_close_at_cycle_nonce, 7n);
  assert.match(automationWidgetSource, /Auto-close cycle \(optional\)/);
  assert.match(automationWidgetSource, /logical-cycle nonce completes/);
  for (const autoCloseAtCycleNonce of [0n, -1n, 1n << 64n]) {
    const result = validateActorAuthoringContract(
      contract(undefined, { autoCloseAtCycleNonce }),
    );
    assert.equal(result.valid, false);
    assert(
      result.issues.some((issue) => issue.path === 'autoCloseAtCycleNonce'),
    );
  }
});

test('observation authoring exposes freshness and validates bounded identity', () => {
  for (const disclosure of [
    'Observation feed identity',
    'Maximum age (blocks)',
    'Only a fresh typed observation compares true',
  ]) {
    assert(
      conditionEditorSource.includes(disclosure),
      `${disclosure} is missing`,
    );
  }
  const invalid = createActorAuthoringPredicate('ObservationBelow');
  invalid.maxAgeBlocks = 0;
  const result = validateActorAuthoringContract(
    contract([
      authoringStep('observation', transferTask(), {
        preconditions: {
          type: 'AnyOf',
          clauses: [[{ timing: 'Current', predicate: invalid }]],
        },
      }),
    ]),
  );
  assert.equal(result.valid, false);
  assert(result.issues.some((issue) => issue.path.endsWith('.maxAgeBlocks')));
});

test('SwapOut editor requires explicit live-market or absolute-ceiling intent', () => {
  for (const disclosure of [
    'LiveQuote',
    'Absolute',
    'may execute at any future market price',
    'will not spend above this declared maximum input',
  ]) {
    assert(taskEditorSource.includes(disclosure), `${disclosure} is missing`);
  }
});

test('RetryLater editor exposes explicit bounded unsuccessful-attempt semantics', () => {
  for (const disclosure of [
    'Maximum unsuccessful attempts',
    'Includes the initial unsuccessful attempt',
    'A value of 1 closes immediately without suspension',
  ]) {
    assert(stepEditorSource.includes(disclosure), `${disclosure} is missing`);
  }
});

test('trigger editor exposes admission and bounded source controls without graph vocabulary', () => {
  for (const control of [
    'Immediate',
    'Cadenced',
    'Always',
    'WhenSignalled',
    'Manual',
    'OnAddressEvent',
    'OnObservationChange',
    'OwnerOnly',
    'Whitelist',
  ]) {
    assert(
      triggerEditorSource.includes(control),
      `${control} control is missing`,
    );
  }
  assert(triggerEditorSource.includes('maxTriggerSources'));
  assert(triggerEditorSource.includes('this source carries no amount'));
  for (const rejected of ['successor', 'callback', 'branch target']) {
    assert(!triggerEditorSource.toLowerCase().includes(rejected));
  }
});

test('observation sources lower exactly and PercentageAtOpening is trigger-independent', () => {
  const openingAmountStep = authoringStep(
    'opening-amount',
    transferTask({ type: 'PercentageAtOpening', parts: 500_000_000 }),
  );
  const observationOnly = contract([openingAmountStep], {
    trigger: observationTrigger,
  });
  assert.equal(validateActorAuthoringContract(observationOnly).valid, true);
  const lowered = lowerActorAuthoringContract(observationOnly);
  assert.deepEqual(lowered.value.schedule.trigger.value.sources[0], {
    type: 'OnObservationChange',
    value: {
      feed: {
        asset_in: { type: 'Native', value: undefined },
        asset_out: { type: 'Local', value: 7 },
        method: { type: 'PreExecutionSpot', value: undefined },
        aggregation: { type: 'Ema', value: { half_life_blocks: 100 } },
        scale: 12,
      },
    },
  });
  const mixed = contract([openingAmountStep], {
    trigger: {
      type: 'Immediate',
      sources: [addressTrigger.sources[0], observationTrigger.sources[0]],
    },
  });
  assert.equal(validateActorAuthoringContract(mixed).valid, true);
});

test('typed authoring lowers to one deterministic exact canonical artifact', () => {
  const draft = contract(
    [
      authoringStep('swap', {
        type: 'SwapOut',
        assetOut: local,
        amountOut: fixed('25'),
        assetIn: native,
        inputLimit: { type: 'Absolute', amount: '100' },
        slippageParts: 10_000_000,
      }),
      authoringStep('transfer', transferTask({ type: 'AllAvailable' }), {
        preconditions: {
          type: 'AnyOf',
          clauses: [
            [
              {
                timing: 'Opening',
                predicate: {
                  type: 'BalanceAbove',
                  asset: local,
                  threshold: '0',
                },
              },
            ],
          ],
        },
        errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
      }),
    ],
    { completionPolicy: 'CloseAfterProductiveCycle' },
  );
  const first = artifact(draft);
  const second = artifact(structuredClone(draft));
  assert.deepEqual(first, second);
  const inspection = inspectActorContractArtifact(
    first,
    metadataBytes,
    runtime,
  );
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.equal(inspection.projection.value.steps.length, 2);
    assert.equal(
      inspection.projection.value.completion.type,
      'CloseAfterProductiveCycle',
    );
    assert.deepEqual(
      inspection.projection.value.steps[0].task.value.input_limit,
      {
        type: 'Absolute',
        value: { $runtimeType: 'bigint', $integer: '100' },
      },
    );
    assert.equal(
      inspection.projection.value.steps[1].on_error.type,
      'RetryLater',
    );
    assert.equal(
      inspection.projection.value.steps[1].on_error.value.max_attempts.$integer,
      '3',
    );
  }
  const analysis = analyzeActorContract({
    artifact: first,
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'authoring-test-runtime' },
    weightModel,
  });
  assert.equal(analysis.identity.contractId, first.contractId);
  assert.equal(analysis.completionPolicy, 'CloseAfterProductiveCycle');
  assert.deepEqual(analysis.steps[0].parameters.input_limit, {
    type: 'Absolute',
    value: { $runtimeType: 'bigint', $integer: '100' },
  });
  assert.deepEqual(
    analysis.steps.map((current) => current.task),
    ['SwapOut', 'Transfer'],
  );
});

test('ordered builder operations change only explicit Step order or content', () => {
  const initial = contract([
    authoringStep('a', transferTask(fixed('1'))),
    authoringStep('b', { type: 'Burn', asset: native, amount: fixed('2') }),
  ]);
  const appended = appendActorStep(
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
  const moved = moveActorStep(appended, 2, 0);
  assert.deepEqual(
    moved.steps.map((current) => current.key),
    ['c', 'a', 'b'],
  );
  const replaced = replaceActorStep(
    moved,
    'a',
    authoringStep('a', transferTask(fixed('9'))),
  );
  assert.equal(replaced.steps[1].task.amount.value, '9');
  const removed = removeActorStep(replaced, 'b');
  assert.deepEqual(
    removed.steps.map((current) => current.key),
    ['c', 'a'],
  );
  assert.notEqual(artifact(initial).contractId, artifact(removed).contractId);
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
      type: 'SwapIn',
      assetIn: native,
      amountIn: fixed(),
      assetOut: local,
      slippageParts: 0,
    },
    {
      type: 'SwapOut',
      assetOut: local,
      amountOut: fixed(),
      assetIn: native,
      inputLimit: { type: 'Absolute', amount: '100' },
      slippageParts: 0,
    },
    {
      type: 'AddLiquidity',
      assetA: native,
      assetB: local,
      amountA: fixed(),
      amountB: fixed('20'),
      minLpOut: '1',
    },
    {
      type: 'RemoveLiquidity',
      lpAsset: local,
      assetA: native,
      assetB: local,
      lpAmount: fixed(),
      minAmountA: '1',
      minAmountB: '1',
    },
    { type: 'Burn', asset: native, amount: fixed() },
    { type: 'Mint', asset: native, amount: fixed() },
    { type: 'Stake', asset: native, amount: fixed() },
    {
      type: 'DonateLiquidity',
      assetA: native,
      assetB: local,
      maxAmountA: fixed(),
      maxRatioErrorParts: 0,
    },
    { type: 'Unstake', asset: native, shares: fixed() },
    { type: 'StopCycle' },
  ];
  for (const task of tasks) {
    const draft = contract([authoringStep('only', task)], {
      actorType: task.type === 'Mint' ? 'System' : 'User',
      fundingPolicy:
        task.type === 'Mint'
          ? { type: 'RuntimePolicy' }
          : { type: 'OwnerOnly' },
    });
    const result = analyzeActorContract({
      artifact: artifact(draft),
      metadataBytes,
      runtime: { ...runtime, modelIdentity: 'authoring-test-runtime' },
      weightModel,
    });
    assert.equal(result.steps[0].task, task.type);
  }
  assert.equal(tasks.length, 12);
});

test('Unconditional and bounded DNF lower exactly and empty forms fail before encoding', () => {
  const atom = { type: 'BlockNumberAbove', threshold: 1 };
  for (const preconditions of [
    { type: 'Unconditional' },
    {
      type: 'AnyOf',
      clauses: [[{ timing: 'Opening', predicate: atom }]],
    },
  ]) {
    const lowered = lowerActorAuthoringContract(
      contract([authoringStep('only', transferTask(), { preconditions })]),
    );
    assert.equal(lowered.value.steps[0].preconditions.type, preconditions.type);
  }
  for (const clauses of [[], [[]]]) {
    const invalid = contract([
      authoringStep('only', transferTask(), {
        preconditions: { type: 'AnyOf', clauses },
      }),
    ]);
    assert.equal(validateActorAuthoringContract(invalid).valid, false);
    assert.throws(
      () => lowerActorAuthoringContract(invalid),
      /must not be empty|at least one/,
    );
  }
});

test('every Predicate and AmountResolution lowers without changing step topology', () => {
  const predicates = [
    { type: 'BalanceAbove', asset: native, threshold: '1' },
    { type: 'BalanceBelow', asset: native, threshold: '1' },
    { type: 'BalanceEquals', asset: native, threshold: '1' },
    { type: 'BalanceNotEquals', asset: native, threshold: '1' },
    { type: 'BlockNumberAbove', threshold: 1 },
    { type: 'BlockNumberBelow', threshold: 1 },
    {
      type: 'ObservationAbove',
      feed: observationFeed,
      threshold: '1',
      maxAgeBlocks: 12,
    },
    {
      type: 'ObservationBelow',
      feed: observationFeed,
      threshold: '1',
      maxAgeBlocks: 12,
    },
    {
      type: 'ObservationEquals',
      feed: observationFeed,
      threshold: '1',
      maxAgeBlocks: 12,
    },
    {
      type: 'ObservationNotEquals',
      feed: observationFeed,
      threshold: '1',
      maxAgeBlocks: 12,
    },
  ];
  for (const current of predicates) {
    const lowered = lowerActorAuthoringContract(
      contract([
        authoringStep('only', transferTask(), {
          preconditions: {
            type: 'AnyOf',
            clauses: [[{ timing: 'Current', predicate: current }]],
          },
        }),
      ]),
    );
    assert.equal(lowered.value.steps.length, 1);
    assert.equal(lowered.value.steps[0].preconditions.type, 'AnyOf');
    const loweredPredicate = lowered.value.steps[0].preconditions.value[0][0];
    assert.equal(loweredPredicate.timing.type, 'Current');
    assert.equal(loweredPredicate.predicate.type, current.type);
    if (current.type.startsWith('Observation')) {
      assert.deepEqual(loweredPredicate.predicate.value, {
        feed: {
          asset_in: { type: 'Native', value: undefined },
          asset_out: { type: 'Local', value: 7 },
          method: { type: 'PreExecutionSpot', value: undefined },
          aggregation: {
            type: 'Ema',
            value: { half_life_blocks: 100 },
          },
          scale: 12,
        },
        threshold: 1n,
        max_age_blocks: 12,
      });
    }
  }
  const amounts = [
    fixed(),
    { type: 'PercentageOfCurrent', parts: 500_000_000 },
    { type: 'PercentageAtOpening', parts: 500_000_000 },
    { type: 'PercentageOfLastFunding', parts: 500_000_000 },
    { type: 'AllAvailable' },
  ];
  for (const amount of amounts) {
    const lowered = lowerActorAuthoringContract(
      contract([authoringStep('only', transferTask(amount))]),
    );
    assert.equal(lowered.value.steps[0].task.value.amount.type, amount.type);
  }
});

test('typed validation rejects control-flow-adjacent and runtime-invalid drafts', () => {
  const immutableRetry = contract(
    [
      authoringStep('only', transferTask(), {
        errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
      }),
    ],
    { mutability: 'Immutable' },
  );
  assert.equal(validateActorAuthoringContract(immutableRetry).valid, false);
  for (const maxAttempts of [0, 11, 4_294_967_296, 1.5]) {
    const invalidRetryLimit = contract([
      authoringStep('only', transferTask(), {
        errorPolicy: { type: 'RetryLater', maxAttempts },
      }),
    ]);
    assert.equal(
      validateActorAuthoringContract(invalidRetryLimit).valid,
      false,
    );
  }
  assert.equal(
    validateActorAuthoringContract(
      contract([
        authoringStep('retry-ten', transferTask(), {
          errorPolicy: { type: 'RetryLater', maxAttempts: 10 },
        }),
      ]),
    ).valid,
    true,
  );
  const userMint = contract([
    authoringStep('only', { type: 'Mint', asset: native, amount: fixed() }),
  ]);
  assert.equal(validateActorAuthoringContract(userMint).valid, false);
  for (const amount of [
    { type: 'Fixed', value: '0' },
    { type: 'PercentageOfCurrent', parts: 0 },
    { type: 'PercentageAtOpening', parts: 0 },
    { type: 'PercentageOfLastFunding', parts: 0 },
  ]) {
    assert.equal(
      validateActorAuthoringContract(
        contract([authoringStep('zero', transferTask(amount))]),
      ).valid,
      false,
    );
  }
  for (const task of [
    {
      ...createActorAuthoringTask('SwapIn'),
      assetIn: native,
      assetOut: native,
    },
    {
      ...createActorAuthoringTask('SwapOut'),
      assetIn: native,
      assetOut: native,
    },
    {
      ...createActorAuthoringTask('AddLiquidity'),
      assetA: native,
      assetB: native,
    },
    {
      ...createActorAuthoringTask('DonateLiquidity'),
      assetA: native,
      assetB: native,
    },
  ]) {
    assert.equal(
      validateActorAuthoringContract(
        contract([authoringStep('identical-assets', task)]),
      ).valid,
      false,
    );
  }
  const zeroAbsoluteInputLimit = contract([
    authoringStep('only', {
      type: 'SwapOut',
      assetOut: local,
      amountOut: fixed(),
      assetIn: native,
      inputLimit: { type: 'Absolute', amount: '0' },
      slippageParts: 0,
    }),
  ]);
  assert.equal(
    validateActorAuthoringContract(zeroAbsoluteInputLimit).valid,
    false,
  );
  const duplicateSplit = contract([
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
  const splitValidation = validateActorAuthoringContract(duplicateSplit);
  assert.equal(splitValidation.valid, false);
  if (!splitValidation.valid) {
    assert(
      splitValidation.issues.some((issue) => /unique/.test(issue.message)),
    );
    assert(
      splitValidation.issues.some((issue) => /exceed/.test(issue.message)),
    );
  }
  assert.equal(validateActorAuthoringContract(contract([])).valid, false);
  assert.equal(
    validateActorAuthoringContract(
      contract(undefined, {
        trigger: {
          type: 'Immediate',
          sources: [{ type: 'Manual' }, { type: 'Manual' }],
        },
      }),
    ).valid,
    false,
  );
  assert.equal(
    validateActorAuthoringContract(
      contract(undefined, {
        trigger: { type: 'Immediate', sources: [] },
      }),
    ).valid,
    false,
  );
  for (const task of [
    { ...createActorAuthoringTask('AddLiquidity'), minLpOut: '0' },
    { ...createActorAuthoringTask('RemoveLiquidity'), minAmountA: '0' },
    { ...createActorAuthoringTask('RemoveLiquidity'), minAmountB: '0' },
  ]) {
    const validation = validateActorAuthoringContract(
      contract([authoringStep('bounded-liquidity', task)]),
    );
    assert.equal(validation.valid, false);
    if (!validation.valid) {
      assert(
        validation.issues.some((issue) =>
          /must be greater than zero/.test(issue.message),
        ),
      );
    }
  }
  assert.throws(
    () => lowerActorAuthoringContract(immutableRetry),
    /RetryLater/,
  );
});

test('scenario corpus lowers every expressible or partial execution core without inventing missing predicates', () => {
  const all = (...predicates) => ({
    type: 'AnyOf',
    clauses: [
      predicates.map((predicate) => ({ timing: 'Current', predicate })),
    ],
  });
  const balanceAbove = (asset = native) => ({
    type: 'BalanceAbove',
    asset,
    threshold: '1',
  });
  const split = (asset = native) => ({
    type: 'SplitTransfer',
    asset,
    amount: { type: 'AllAvailable' },
    legs: [
      { to: accountA, shareParts: 500_000_000 },
      { to: accountB, shareParts: 500_000_000 },
    ],
  });
  const swap = {
    type: 'SwapIn',
    assetIn: local,
    amountIn: { type: 'AllAvailable' },
    assetOut: native,
    slippageParts: 10_000_000,
  };
  const scenarios = [
    {
      name: 'DEOS Burn Actor',
      contract: contract(
        [
          authoringStep('swap', swap, {
            preconditions: all(balanceAbove(local)),
          }),
          authoringStep('burn', {
            type: 'Burn',
            asset: native,
            amount: { type: 'AllAvailable' },
          }),
        ],
        { actorType: 'System', fundingPolicy: { type: 'RuntimePolicy' } },
      ),
      tasks: ['SwapIn', 'Burn'],
    },
    {
      name: 'DEOS Fee Sink',
      contract: contract([authoringStep('split', split())]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'DEOS BLDR splitter',
      contract: contract([
        authoringStep('split', split(local), {
          preconditions: all(balanceAbove(local)),
        }),
      ]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'DEOS Liquidity Actor core',
      contract: contract([
        authoringStep('liquidity', {
          type: 'AddLiquidity',
          assetA: native,
          assetB: local,
          amountA: fixed(),
          amountB: fixed(),
          minLpOut: '1',
        }),
      ]),
      tasks: ['AddLiquidity'],
    },
    {
      name: 'Periodic DCA',
      contract: contract([authoringStep('dca', swap)], {
        trigger: {
          type: 'Cadenced',
          everyBlocks: 10,
          mode: { type: 'Always' },
        },
      }),
      tasks: ['SwapIn'],
    },
    {
      name: 'Threshold payroll',
      contract: contract([
        authoringStep('payroll', split(), {
          preconditions: all(balanceAbove(), {
            type: 'BlockNumberAbove',
            threshold: 1,
          }),
        }),
      ]),
      tasks: ['SplitTransfer'],
    },
    {
      name: 'Resilient swap-to-liquidity pipeline',
      contract: contract([
        authoringStep('swap', swap),
        authoringStep(
          'liquidity',
          {
            type: 'AddLiquidity',
            assetA: native,
            assetB: local,
            amountA: fixed(),
            amountB: fixed(),
            minLpOut: '1',
          },
          { errorPolicy: { type: 'RetryLater', maxAttempts: 3 } },
        ),
        authoringStep('remainder', transferTask()),
      ]),
      tasks: ['SwapIn', 'AddLiquidity', 'Transfer'],
    },
    {
      name: 'Stake and unstake execution core',
      contract: contract([
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
    const analysis = analyzeActorContract({
      artifact: artifact(scenario.contract),
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
  assert.equal(ACTORS_AUTHORING_CONDITION_TYPES.includes('PriceAbove'), false);
  assert.equal(
    ACTORS_AUTHORING_CONDITION_TYPES.includes('PortfolioRatio'),
    false,
  );
  assert.equal(ACTORS_AUTHORING_TASK_TYPES.includes('Rebalance'), false);
});

test('trigger, completion, and funding policy variants lower as typed ContractInput fields', () => {
  const drafts = [
    contract(undefined, {
      trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
      fundingPolicy: { type: 'OwnerOnly' },
    }),
    contract(undefined, {
      trigger: {
        type: 'Cadenced',
        everyBlocks: 10,
        mode: { type: 'Always' },
      },
      fundingPolicy: { type: 'RuntimePolicy' },
      actorType: 'System',
    }),
    contract(undefined, {
      trigger: {
        type: 'Immediate',
        sources: [
          {
            type: 'OnAddressEvent',
            sourceFilter: { type: 'Whitelist', accounts: [accountA] },
            assetFilter: { type: 'Whitelist', assets: [local, native] },
          },
          { type: 'Manual' },
        ],
      },
      fundingPolicy: { type: 'SignedAllowlist', accounts: [accountA] },
    }),
    contract(undefined, {
      trigger: {
        type: 'Cadenced',
        everyBlocks: 10,
        mode: {
          type: 'WhenSignalled',
          sources: [
            {
              type: 'OnAddressEvent',
              sourceFilter: { type: 'Any' },
              assetFilter: { type: 'Any' },
            },
          ],
        },
      },
      fundingPolicy: { type: 'AnyVerifiedIngress' },
    }),
  ];
  for (const draft of drafts) {
    const canonical = artifact(draft);
    assert.equal(
      inspectActorContractArtifact(canonical, metadataBytes, runtime).valid,
      true,
    );
  }
  const lowered = lowerActorAuthoringContract(drafts[2]);
  const sources = lowered.value.schedule.trigger.value.sources;
  assert.deepEqual(
    sources.map((source) => source.type),
    ['Manual', 'OnAddressEvent'],
  );
  assert.deepEqual(
    sources[1].value.asset_filter.value.map((asset) => asset.type),
    ['Native', 'Local'],
  );
});
