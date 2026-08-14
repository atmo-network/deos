/*
Domain: Actors reactive scenario-corpus validation
Owns: Bounded bucket fixtures, partial reaction cores, non-price scalar evidence, and explicit data/failure ownership.
Excludes: Runtime bucket policy, market viability, invented oracle meanings, signing, submission, and live execution.
Zone: Web-client validation entrypoint; composes canonical automation contracts only.
*/
import { encodeAddress } from '@polkadot/util-crypto';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { analyzeActorContract } from '../src/lib/automation/analysis.ts';
import {
  ACTORS_AUTHORING_CONDITION_TYPES,
  createActorArtifactFromAuthoring,
  validateActorAuthoringContract,
} from '../src/lib/automation/authoring.ts';
import { inspectActorContractArtifact } from '../src/lib/automation/contract-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const recipient = encodeAddress(new Uint8Array(32).fill(1), 42);
const native = { type: 'Native' };
const quoteAsset = { type: 'Local', id: 7 };
const priceFeed = (assetIn, assetOut) => ({
  assetIn,
  assetOut,
  method: 'PreExecutionSpot',
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
  scale: 12,
});
const weightModel = {
  identity: 'reactive-scenario-corpus-weights',
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

function activeContract({
  trigger,
  predicates,
  task,
  completionPolicy,
  onError,
}) {
  return {
    actorType: 'User',
    mutability: 'Mutable',
    completionPolicy,
    trigger,
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps: [
      {
        key: 'reaction',
        preconditions:
          predicates.length === 0
            ? { type: 'Unconditional' }
            : {
                type: 'AnyOf',
                clauses: [
                  predicates.map((predicate) => ({
                    timing: 'Current',
                    predicate,
                  })),
                ],
              },
        task,
        errorPolicy: onError,
      },
    ],
  };
}

function priceBucket({ direction, threshold }) {
  const buying = direction === 'Buy';
  const assetIn = buying ? native : quoteAsset;
  const assetOut = buying ? quoteAsset : native;
  const feed = priceFeed(assetIn, assetOut);
  return {
    name: `${direction} bucket ${threshold}`,
    classification: 'Expressible',
    completeStrategy: true,
    data: {
      sampleOwner: 'DeosRouter',
      truthOwner: 'pallet-oracle',
      reactionOwner: 'pallet-deos-actors',
      provenance: 'DeosRouterPreExecutionReserves',
      meaning: 'DirectionalLocalPoolPrice',
    },
    failure:
      'Non-fresh observation skips; Temporary swap failure retries at one cursor; retry exhaustion closes.',
    contract: activeContract({
      trigger: {
        type: 'Immediate',
        sources: [{ type: 'OnObservationChange', feed }],
      },
      predicates: [
        {
          type: buying ? 'ObservationBelow' : 'ObservationAbove',
          feed,
          threshold,
          maxAgeBlocks: 12,
        },
        { type: 'BalanceAbove', asset: assetIn, threshold: '99' },
      ],
      task: {
        type: 'SwapIn',
        assetIn,
        amountIn: { type: 'Fixed', value: '100' },
        assetOut,
        slippageParts: 10_000_000,
      },
      onError: { type: 'RetryLater', maxAttempts: 3 },
      completionPolicy: 'CloseAfterProductiveCycle',
    }),
  };
}

const descendingBuyBuckets = [
  '900000000000',
  '800000000000',
  '700000000000',
].map((threshold) => priceBucket({ direction: 'Buy', threshold }));
const ascendingSellBuckets = [
  '1100000000000',
  '1200000000000',
  '1300000000000',
].map((threshold) => priceBucket({ direction: 'Sell', threshold }));

const partialScenarios = [
  {
    name: 'Treasury reserve-ratio reaction core',
    classification: 'Partial',
    completeStrategy: false,
    missingSurface: 'TreasuryReserveRatioObservation',
    data: {
      sampleOwner: 'Unassigned',
      truthOwner: 'NoCurrentTypedFeed',
      reactionOwner: 'pallet-deos-actors',
      provenance: 'Unavailable',
      meaning: 'NotDirectionalLocalPoolPrice',
    },
    failure:
      'The core must not run automatically until a typed treasury-ratio producer and feed meaning exist.',
    contract: activeContract({
      trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
      predicates: [],
      task: {
        type: 'SplitTransfer',
        asset: native,
        amount: { type: 'AllAvailable' },
        legs: [
          { to: recipient, shareParts: 500_000_000 },
          {
            to: encodeAddress(new Uint8Array(32).fill(2), 42),
            shareParts: 500_000_000,
          },
        ],
      },
      onError: { type: 'AbortCycle' },
      completionPolicy: 'Persistent',
    }),
  },
  {
    name: 'Liquidity-depth reaction core',
    classification: 'Partial',
    completeStrategy: false,
    missingSurface: 'AbsolutePoolLiquidityDepthObservation',
    data: {
      sampleOwner: 'Unassigned',
      truthOwner: 'NoCurrentTypedFeed',
      reactionOwner: 'pallet-deos-actors',
      provenance: 'Unavailable',
      meaning: 'NotDirectionalLocalPoolPrice',
    },
    failure:
      'The core must not run automatically until a typed depth producer and feed meaning exist.',
    contract: activeContract({
      trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
      predicates: [],
      task: {
        type: 'AddLiquidity',
        assetA: native,
        assetB: quoteAsset,
        amountA: { type: 'Fixed', value: '100' },
        amountB: { type: 'Fixed', value: '100' },
        minLpOut: '1',
      },
      onError: { type: 'RetryLater', maxAttempts: 3 },
      completionPolicy: 'Persistent',
    }),
  },
];

const nonPriceScalar = {
  name: 'Block-height treasury release',
  classification: 'Expressible',
  completeStrategy: true,
  data: {
    sampleOwner: 'FRAME System',
    truthOwner: 'Runtime block number',
    reactionOwner: 'pallet-deos-actors',
    provenance: 'CanonicalChainCurrentBlock',
    meaning: 'BlockHeight',
  },
  failure:
    'Before the authored block the condition skips; transfer failure follows AbortCycle without retry state.',
  contract: activeContract({
    trigger: {
      type: 'Cadenced',
      everyBlocks: 10,
      mode: { type: 'Always' },
    },
    predicates: [{ type: 'BlockNumberAbove', threshold: 100 }],
    task: {
      type: 'Transfer',
      to: recipient,
      asset: native,
      amount: { type: 'Fixed', value: '100' },
    },
    onError: { type: 'AbortCycle' },
    completionPolicy: 'CloseAfterProductiveCycle',
  }),
};

const scenarios = [
  ...descendingBuyBuckets,
  ...ascendingSellBuckets,
  ...partialScenarios,
  nonPriceScalar,
];

function artifact(contract) {
  return createActorArtifactFromAuthoring({ contract, metadataBytes, runtime });
}

test('descending buys and ascending sells lower as independent bounded one-shot actors', () => {
  assert.deepEqual(
    descendingBuyBuckets.map(
      (scenario) =>
        scenario.contract.steps[0].preconditions.clauses[0][0].predicate
          .threshold,
    ),
    ['900000000000', '800000000000', '700000000000'],
  );
  assert.deepEqual(
    ascendingSellBuckets.map(
      (scenario) =>
        scenario.contract.steps[0].preconditions.clauses[0][0].predicate
          .threshold,
    ),
    ['1100000000000', '1200000000000', '1300000000000'],
  );
  for (const scenario of [...descendingBuyBuckets, ...ascendingSellBuckets]) {
    const validation = validateActorAuthoringContract(scenario.contract);
    assert.equal(validation.valid, true, scenario.name);
    const inspection = inspectActorContractArtifact(
      artifact(scenario.contract),
      metadataBytes,
      runtime,
    );
    assert.equal(inspection.valid, true, scenario.name);
    if (!inspection.valid) continue;
    assert.equal(inspection.projection.value.steps.length, 1, scenario.name);
    assert.equal(
      inspection.projection.value.completion.type,
      'CloseAfterProductiveCycle',
      scenario.name,
    );
    assert.equal(
      inspection.projection.value.schedule.trigger.value.sources[0].type,
      'OnObservationChange',
      scenario.name,
    );
  }
});

test('partial reaction cores lower without inventing reserve-ratio or depth predicates', () => {
  assert.equal(
    ACTORS_AUTHORING_CONDITION_TYPES.includes('TreasuryReserveRatio'),
    false,
  );
  assert.equal(
    ACTORS_AUTHORING_CONDITION_TYPES.includes('LiquidityDepth'),
    false,
  );
  for (const scenario of partialScenarios) {
    assert.equal(scenario.completeStrategy, false, scenario.name);
    assert.equal(scenario.data.truthOwner, 'NoCurrentTypedFeed', scenario.name);
    assert.equal(scenario.data.provenance, 'Unavailable', scenario.name);
    assert.equal(
      validateActorAuthoringContract(scenario.contract).valid,
      true,
      scenario.name,
    );
    const analysis = analyzeActorContract({
      artifact: artifact(scenario.contract),
      metadataBytes,
      runtime: { ...runtime, modelIdentity: 'reactive-scenario-corpus' },
      weightModel,
    });
    assert.equal(
      analysis.steps[0].preconditions.mode,
      'Unconditional',
      scenario.name,
    );
    assert(BigInt(analysis.steps[0].costs.totalUpper.refTime) > 0n);
  }
});

test('non-price scalar strategy uses runtime block truth rather than a mislabeled price feed', () => {
  assert.equal(nonPriceScalar.data.truthOwner, 'Runtime block number');
  assert.equal(nonPriceScalar.data.provenance, 'CanonicalChainCurrentBlock');
  assert.equal(
    validateActorAuthoringContract(nonPriceScalar.contract).valid,
    true,
  );
  const analysis = analyzeActorContract({
    artifact: artifact(nonPriceScalar.contract),
    metadataBytes,
    runtime: { ...runtime, modelIdentity: 'reactive-scenario-corpus' },
    weightModel,
  });
  assert.equal(analysis.trigger.admission, 'CadencedAlways');
  assert.equal(analysis.steps[0].predicates[0].type, 'BlockNumberAbove');
  assert.equal(analysis.steps[0].predicates[0].observation, 'block-number');
  assert.equal(analysis.steps[0].task, 'Transfer');
});

test('every scenario declares availability, data ownership, provenance, and failure behavior', () => {
  assert.equal(scenarios.length, 9);
  for (const scenario of scenarios) {
    assert(['Expressible', 'Partial'].includes(scenario.classification));
    assert.equal(typeof scenario.completeStrategy, 'boolean');
    assert(scenario.data.sampleOwner.length > 0, scenario.name);
    assert(scenario.data.truthOwner.length > 0, scenario.name);
    assert.equal(
      scenario.data.reactionOwner,
      'pallet-deos-actors',
      scenario.name,
    );
    assert(scenario.data.provenance.length > 0, scenario.name);
    assert(scenario.data.meaning.length > 0, scenario.name);
    assert(scenario.failure.length > 0, scenario.name);
    assert.equal(validateActorAuthoringContract(scenario.contract).valid, true);
  }
});
