/*
Domain: AAA forecast validation
Owns: Amount-resolution parity, separated Weight/fee aggregation, and staleness regression fixtures.
Excludes: Chain queries, adapter execution, simulation, signing, and browser rendering.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  forecastAaaCosts,
  isAaaForecastStale,
  resolveAaaAmount,
} from '../src/lib/automation/forecast.ts';

const hash = (byte) => `0x${byte.repeat(64)}`;

const artifact = {
  format: 'deos.aaa.plan',
  formatVersion: 1,
  genesisHash: hash('1'),
  specVersion: 1,
  transactionVersion: 1,
  metadataHash: hash('2'),
  aaaType: 'User',
  mutability: 'Mutable',
  programScale: '0x00',
  planId: hash('3'),
};

function observation(overrides) {
  return {
    resolution: { type: 'Fixed', value: 1n },
    policy: 'PreserveSpend',
    current: 1_000n,
    minimumBalance: 100n,
    reservedFee: 50n,
    isFeeNative: true,
    ...overrides,
  };
}

test('asset amount resolution preserves fee reserve and minimum balance', () => {
  assert.deepEqual(
    resolveAaaAmount(
      observation({ resolution: { type: 'Fixed', value: 851n } }),
    ),
    {
      status: 'FundingUnavailable',
      amount: null,
      basis: null,
      spendLimit: 850n,
    },
  );
  assert.deepEqual(
    resolveAaaAmount(
      observation({
        resolution: { type: 'PercentageOfCurrent', parts: 500_000_000 },
      }),
    ),
    {
      status: 'Resolved',
      amount: 425n,
      basis: 850n,
      spendLimit: 850n,
    },
  );
});

test('snapshot, funding, rounding, mint, and staking-share outcomes remain distinct', () => {
  assert.equal(
    resolveAaaAmount(
      observation({
        resolution: { type: 'PercentageOfTrigger', parts: 1_000_000_000 },
      }),
    ).status,
    'SnapshotUnavailable',
  );
  assert.equal(
    resolveAaaAmount(
      observation({
        resolution: {
          type: 'PercentageOfLastFunding',
          parts: 1_000_000_000,
        },
        lastFunding: 0n,
      }),
    ).status,
    'FundingUnavailable',
  );
  assert.equal(
    resolveAaaAmount(
      observation({
        resolution: { type: 'PercentageOfCurrent', parts: 1 },
        current: 1n,
        minimumBalance: 0n,
        reservedFee: 0n,
      }),
    ).status,
    'Skipped',
  );
  assert.deepEqual(
    resolveAaaAmount(
      observation({
        resolution: { type: 'Fixed', value: 10_000n },
        policy: 'Mint',
      }),
    ).amount,
    10_000n,
  );
  assert.equal(
    resolveAaaAmount(
      observation({
        resolution: { type: 'Fixed', value: 11n },
        policy: 'UnstakeShares',
        current: 10n,
      }),
    ).status,
    'FundingUnavailable',
  );
});

test('cost forecast keeps RefTime, ProofSize, fee classes, and provenance separate', () => {
  const forecast = forecastAaaCosts({
    artifact,
    blockHash: hash('4'),
    blockNumber: 42,
    model: 'deos-runtime-weights',
    modelVersion: '0.7.3',
    actorType: 'User',
    stepBaseFee: 2n,
    conditionReadFee: 3n,
    steps: [
      {
        stepIndex: 0,
        conditionCount: 2,
        conditionOutcome: 'Pass',
        executionDisposition: 'Execute',
        evaluationWeight: { refTime: 10n, proofSize: 2n },
        executionWeightUpper: { refTime: 100n, proofSize: 5n },
        executionFeeUpper: 7n,
      },
      {
        stepIndex: 1,
        conditionCount: 0,
        conditionOutcome: 'Fail',
        executionDisposition: 'Skip',
        evaluationWeight: { refTime: 20n, proofSize: 3n },
        executionWeightUpper: { refTime: 200n, proofSize: 6n },
        executionFeeUpper: 8n,
      },
      {
        stepIndex: 2,
        conditionCount: 0,
        conditionOutcome: 'Unknown',
        executionDisposition: 'Unknown',
        evaluationWeight: { refTime: 30n, proofSize: 4n },
        executionWeightUpper: { refTime: 300n, proofSize: 7n },
        executionFeeUpper: 9n,
      },
    ],
    lifecycle: { weight: { refTime: 5n, proofSize: 1n }, fee: 1n },
  });

  assert.equal(forecast.scope, 'StaticAllStepsReached');
  assert.deepEqual(forecast.evaluation, {
    weight: { refTime: 60n, proofSize: 9n },
    fee: 12n,
  });
  assert.deepEqual(forecast.totalMinimum, {
    weight: { refTime: 165n, proofSize: 15n },
    fee: 20n,
  });
  assert.deepEqual(forecast.totalUpper, {
    weight: { refTime: 465n, proofSize: 22n },
    fee: 29n,
  });
  assert.equal(
    isAaaForecastStale(forecast, {
      blockHash: hash('4'),
      metadataHash: hash('2'),
      model: 'deos-runtime-weights',
      modelVersion: '0.7.3',
    }),
    false,
  );
  assert.equal(
    isAaaForecastStale(forecast, {
      blockHash: hash('5'),
      metadataHash: hash('2'),
      model: 'deos-runtime-weights',
      modelVersion: '0.7.3',
    }),
    true,
  );
});
