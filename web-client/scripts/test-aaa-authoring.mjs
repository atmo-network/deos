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
    errorPolicy: { type: 'AbortCycle' },
    ...overrides,
  };
}

function program(steps = [authoringStep('step-0')], overrides = {}) {
  return {
    aaaType: 'User',
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
  assert.equal(AAA_AUTHORING_CONDITION_TYPES.length, 10);
});

test('one metadata-aligned eight-step baseline applies to both actor classes', () => {
  const steps = Array.from({ length: 9 }, (_, index) =>
    authoringStep(`step-${index}`),
  );
  for (const aaaType of ['User', 'System']) {
    assert.equal(
      validateAaaAuthoringProgram(program(steps.slice(0, 8), { aaaType }))
        .valid,
      true,
    );
    const tooLong = validateAaaAuthoringProgram(program(steps, { aaaType }));
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
  const persistent = lowerAaaAuthoringProgram(program());
  assert.deepEqual(persistent.value.completion_policy, {
    type: 'Persistent',
    value: undefined,
  });
  const oneShot = lowerAaaAuthoringProgram(
    program(undefined, { completionPolicy: 'CloseAfterProductiveRun' }),
  );
  assert.deepEqual(oneShot.value.completion_policy, {
    type: 'CloseAfterProductiveRun',
    value: undefined,
  });
  assert.equal(
    validateAaaAuthoringProgram(
      program(undefined, { completionPolicy: 'UnknownLifecycle' }),
    ).valid,
    false,
  );
  assert.match(automationWidgetSource, /Close after productive run/);
  assert.match(automationWidgetSource, /committed effectful task/);
});

test('optional auto-close target lowers exactly and rejects invalid u64 values', () => {
  const target = lowerAaaAuthoringProgram(
    program(undefined, { autoCloseAtCycleNonce: 7n }),
  );
  assert.equal(target.value.auto_close_at_cycle_nonce, 7n);
  assert.match(automationWidgetSource, /Auto-close run \(optional\)/);
  assert.match(automationWidgetSource, /logical-run nonce completes/);
  for (const autoCloseAtCycleNonce of [0n, -1n, 1n << 64n]) {
    const result = validateAaaAuthoringProgram(
      program(undefined, { autoCloseAtCycleNonce }),
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
  const invalid = createAaaAuthoringCondition('ObservationBelow');
  invalid.maxAgeBlocks = 0;
  const result = validateAaaAuthoringProgram(
    program([
      authoringStep('observation', transferTask(), {
        conditionSet: { type: 'All', conditions: [invalid] },
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

test('observation sources lower exactly and cannot supply PercentageOfTrigger', () => {
  const observationOnly = program([authoringStep('only', transferTask())], {
    trigger: observationTrigger,
  });
  assert.equal(validateAaaAuthoringProgram(observationOnly).valid, true);
  const lowered = lowerAaaAuthoringProgram(observationOnly);
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

  const triggerAmountStep = authoringStep(
    'trigger-amount',
    transferTask({ type: 'PercentageOfTrigger', parts: 500_000_000 }),
  );
  const observationAmount = program([triggerAmountStep], {
    trigger: observationTrigger,
  });
  const observationValidation = validateAaaAuthoringProgram(observationAmount);
  assert.equal(observationValidation.valid, false);
  assert(
    observationValidation.issues.some((issue) =>
      issue.message.includes('provide no trigger amount'),
    ),
  );
  const mixed = program([triggerAmountStep], {
    trigger: {
      type: 'Immediate',
      sources: [addressTrigger.sources[0], observationTrigger.sources[0]],
    },
  });
  assert.equal(validateAaaAuthoringProgram(mixed).valid, false);
  assert.equal(
    validateAaaAuthoringProgram(
      program([triggerAmountStep], { trigger: addressTrigger }),
    ).valid,
    true,
  );
  assert.equal(
    validateAaaAuthoringProgram(
      program([triggerAmountStep], {
        trigger: {
          type: 'Immediate',
          sources: [
            {
              type: 'OnAddressEvent',
              sourceFilter: { type: 'Any' },
              assetFilter: { type: 'Whitelist', assets: [local] },
            },
          ],
        },
      }),
    ).valid,
    false,
  );
});

test('typed authoring lowers to one deterministic exact canonical artifact', () => {
  const draft = program(
    [
      authoringStep('swap', {
        type: 'SwapOut',
        assetOut: local,
        amountOut: fixed('25'),
        assetIn: native,
        inputLimit: { type: 'Absolute', amount: '100' },
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
        errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
      }),
    ],
    { completionPolicy: 'CloseAfterProductiveRun' },
  );
  const first = artifact(draft);
  const second = artifact(structuredClone(draft));
  assert.deepEqual(first, second);
  const inspection = inspectAaaPlanArtifact(first, metadataBytes, runtime);
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.equal(inspection.projection.value.execution_plan.length, 2);
    assert.equal(
      inspection.projection.value.completion_policy.type,
      'CloseAfterProductiveRun',
    );
    assert.deepEqual(
      inspection.projection.value.execution_plan[0].task.value.input_limit,
      {
        type: 'Absolute',
        value: { $runtimeType: 'bigint', $integer: '100' },
      },
    );
    assert.equal(
      inspection.projection.value.execution_plan[1].on_error.type,
      'RetryLater',
    );
    assert.equal(
      inspection.projection.value.execution_plan[1].on_error.value.max_attempts
        .$integer,
      '3',
    );
  }
  const analysis = analyzeAaaProgram({
    artifact: first,
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'authoring-test-runtime' },
    weightModel,
  });
  assert.equal(analysis.identity.planId, first.planId);
  assert.equal(analysis.completionPolicy, 'CloseAfterProductiveRun');
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
    if (current.type.startsWith('Observation')) {
      assert.deepEqual(
        lowered.value.execution_plan[0].conditions.value[0].value,
        {
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
        },
      );
    }
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
      program(
        [authoringStep('only', transferTask(amount))],
        amount.type === 'PercentageOfTrigger'
          ? { trigger: addressTrigger }
          : {},
      ),
    );
    assert.equal(
      lowered.value.execution_plan[0].task.value.amount.type,
      amount.type,
    );
  }
});

test('typed validation rejects control-flow-adjacent and runtime-invalid drafts', () => {
  const immutableRetry = program(
    [
      authoringStep('only', transferTask(), {
        errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
      }),
    ],
    { mutability: 'Immutable' },
  );
  assert.equal(validateAaaAuthoringProgram(immutableRetry).valid, false);
  for (const maxAttempts of [0, 11, 4_294_967_296, 1.5]) {
    const invalidRetryLimit = program([
      authoringStep('only', transferTask(), {
        errorPolicy: { type: 'RetryLater', maxAttempts },
      }),
    ]);
    assert.equal(validateAaaAuthoringProgram(invalidRetryLimit).valid, false);
  }
  assert.equal(
    validateAaaAuthoringProgram(
      program([
        authoringStep('retry-ten', transferTask(), {
          errorPolicy: { type: 'RetryLater', maxAttempts: 10 },
        }),
      ]),
    ).valid,
    true,
  );
  const userMint = program([
    authoringStep('only', { type: 'Mint', asset: native, amount: fixed() }),
  ]);
  assert.equal(validateAaaAuthoringProgram(userMint).valid, false);
  for (const amount of [
    { type: 'Fixed', value: '0' },
    { type: 'PercentageOfCurrent', parts: 0 },
    { type: 'PercentageOfTrigger', parts: 0 },
    { type: 'PercentageOfLastFunding', parts: 0 },
  ]) {
    assert.equal(
      validateAaaAuthoringProgram(
        program([authoringStep('zero', transferTask(amount))]),
      ).valid,
      false,
    );
  }
  for (const task of [
    { ...createAaaAuthoringTask('SwapIn'), assetIn: native, assetOut: native },
    { ...createAaaAuthoringTask('SwapOut'), assetIn: native, assetOut: native },
    {
      ...createAaaAuthoringTask('AddLiquidity'),
      assetA: native,
      assetB: native,
    },
    {
      ...createAaaAuthoringTask('DonateLiquidity'),
      assetA: native,
      assetB: native,
    },
  ]) {
    assert.equal(
      validateAaaAuthoringProgram(
        program([authoringStep('identical-assets', task)]),
      ).valid,
      false,
    );
  }
  const zeroAbsoluteInputLimit = program([
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
    validateAaaAuthoringProgram(zeroAbsoluteInputLimit).valid,
    false,
  );
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
  assert.equal(
    validateAaaAuthoringProgram(
      program(undefined, {
        trigger: {
          type: 'Immediate',
          sources: [{ type: 'Manual' }, { type: 'Manual' }],
        },
      }),
    ).valid,
    false,
  );
  assert.equal(
    validateAaaAuthoringProgram(
      program(undefined, {
        trigger: { type: 'Immediate', sources: [] },
      }),
    ).valid,
    false,
  );
  for (const task of [
    { ...createAaaAuthoringTask('AddLiquidity'), minLpOut: '0' },
    { ...createAaaAuthoringTask('RemoveLiquidity'), minAmountA: '0' },
    { ...createAaaAuthoringTask('RemoveLiquidity'), minAmountB: '0' },
  ]) {
    const validation = validateAaaAuthoringProgram(
      program([authoringStep('bounded-liquidity', task)]),
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
    type: 'SwapIn',
    assetIn: local,
    amountIn: { type: 'AllBalance' },
    assetOut: native,
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
      tasks: ['SwapIn', 'Burn'],
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
          minLpOut: '1',
        }),
      ]),
      tasks: ['AddLiquidity'],
    },
    {
      name: 'Periodic DCA',
      program: program([authoringStep('dca', swap)], {
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

test('trigger, completion, and funding policy variants lower as typed ProgramInput fields', () => {
  const drafts = [
    program(undefined, {
      trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
      fundingPolicy: { type: 'OwnerOnly' },
    }),
    program(undefined, {
      trigger: {
        type: 'Cadenced',
        everyBlocks: 10,
        mode: { type: 'Always' },
      },
      fundingPolicy: { type: 'RuntimePolicy' },
      aaaType: 'System',
    }),
    program(undefined, {
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
    program(undefined, {
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
      inspectAaaPlanArtifact(canonical, metadataBytes, runtime).valid,
      true,
    );
  }
  const lowered = lowerAaaAuthoringProgram(drafts[2]);
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
