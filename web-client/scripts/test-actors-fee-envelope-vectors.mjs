/*
Domain: Actors fee-envelope vector validation
Owns: Generated package-vector and browser forecast conformance.
Excludes: Runtime fee collection, metadata generation, and task execution.
Zone: Automation validation entrypoint; prevents browser fee-policy drift.
*/
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  ACTORS_FEE_ENVELOPE_VECTORS,
  actorFeeEnvelopeCharges,
  actorFeeNativeProtectedMinimum,
  actorFeeStepCharge,
  actorUserFeeBudgetAdmits,
  parseActorFeeEnvelopeVectors,
  settleActorFeeStep,
} from '../src/lib/automation/fee-envelope-vectors.ts';
import { forecastActorCosts } from '../src/lib/automation/forecast.ts';

const hash = (byte) => `0x${byte.repeat(64)}`;

const artifact = (actorType) => ({
  format: 'deos.actor.plan',
  formatVersion: 1,
  genesisHash: hash('1'),
  specVersion: 1,
  transactionVersion: 1,
  metadataHash: hash('2'),
  actorType,
  mutability: 'Mutable',
  programScale: '0x00',
  planId: hash('3'),
});

function forecast(actorType) {
  return forecastActorCosts({
    artifact: artifact(actorType),
    blockHash: hash('4'),
    blockNumber: 1,
    model: 'fixture',
    modelVersion: '1',
    actorType: actorType,
    steps: [
      {
        stepIndex: 0,
        conditionCount: 2,
        conditionOutcome: 'Pass',
        executionDisposition: 'Execute',
        evaluationWeight: { refTime: 1n, proofSize: 1n },
        evaluationFeeUpper: 8n,
        executionWeightUpper: { refTime: 2n, proofSize: 2n },
        executionFeeUpper: 7n,
      },
    ],
    lifecycle: { weight: { refTime: 0n, proofSize: 0n }, fee: 0n },
  });
}

test('fee-envelope vectors bind the final metadata and Actors weights identities', async () => {
  const metadata = await readFile(
    new URL('../.papi/metadata/deos.scale', import.meta.url),
  );
  const weights = await readFile(
    new URL(
      '../../template/runtime/src/weights/pallet_deos_actors.rs',
      import.meta.url,
    ),
  );
  const metadataSha256 = createHash('sha256').update(metadata).digest('hex');
  const weightSha256 = createHash('sha256').update(weights).digest('hex');
  assert.equal(ACTORS_FEE_ENVELOPE_VECTORS.metadataSha256, metadataSha256);
  assert.equal(ACTORS_FEE_ENVELOPE_VECTORS.weightSha256, weightSha256);
  assert.match(ACTORS_FEE_ENVELOPE_VECTORS.metadataSha256, /^[0-9a-f]{64}$/);
  assert.match(ACTORS_FEE_ENVELOPE_VECTORS.weightSha256, /^[0-9a-f]{64}$/);
});

test('fee-envelope vector parser rejects a missing or malformed identity binding', () => {
  const missing = structuredClone(ACTORS_FEE_ENVELOPE_VECTORS);
  delete missing.metadataSha256;
  assert.throws(
    () => parseActorFeeEnvelopeVectors(missing),
    /metadata identity/,
  );
  const malformed = structuredClone(ACTORS_FEE_ENVELOPE_VECTORS);
  malformed.weightSha256 = 'deadbeef';
  assert.throws(
    () => parseActorFeeEnvelopeVectors(malformed),
    /weight identity/,
  );
});

test('package-generated fee-envelope vectors cover User and System suffixes', () => {
  assert.equal(ACTORS_FEE_ENVELOPE_VECTORS.formatVersion, 2);
  assert.equal(ACTORS_FEE_ENVELOPE_VECTORS.vectors.length, 4);
  assert.equal(actorFeeEnvelopeCharges('User'), true);
  assert.equal(actorFeeEnvelopeCharges('System'), false);
  const userSuffix = ACTORS_FEE_ENVELOPE_VECTORS.vectors.find(
    (vector) => vector.actorType === 'User' && vector.cursor === 1,
  );
  assert.equal(userSuffix?.total, '45');
});

test('generated cases bind release, rollback pricing, and protected floors', () => {
  const release = ACTORS_FEE_ENVELOPE_VECTORS.settlementCases.find(
    (candidate) => candidate.name === 'releaseToZero',
  );
  assert.deepEqual(release?.charges, ['2', '12', '0']);
  assert.deepEqual(release?.reservationRemaining, ['45', '33', '0']);
  const rollback = ACTORS_FEE_ENVELOPE_VECTORS.settlementCases.find(
    (candidate) => candidate.name === 'attemptPricedRollback',
  );
  assert.deepEqual(rollback?.charges, ['102']);
  assert.deepEqual(rollback?.reservationRemaining, ['0']);
  assert.equal(actorFeeStepCharge('User', 2n, 100n, 'Attempted'), 102n);
  assert.equal(
    settleActorFeeStep(
      'User',
      102n,
      { evaluation: 2n, execution: 100n },
      'EvaluationOnly',
    ).reservationRemaining,
    0n,
  );
  assert.equal(actorFeeNativeProtectedMinimum('User', true, 1n, 50n), 50n);
  assert.equal(actorFeeNativeProtectedMinimum('System', true, 1n, 50n), 1n);
  assert.equal(
    actorUserFeeBudgetAdmits(102n, 50n, 102n),
    false,
    'raw balance may cover the envelope while protected available budget does not',
  );
  assert.equal(actorUserFeeBudgetAdmits(152n, 50n, 102n), true);
});

test('forecast fee policy consumes generated User/System envelope semantics', () => {
  assert.equal(forecast('User').totalUpper.fee, 15n);
  assert.equal(forecast('System').totalUpper.fee, 0n);
});

test('fee-envelope vector parser rejects altered package semantics', () => {
  const malformedEnvelope = structuredClone(ACTORS_FEE_ENVELOPE_VECTORS);
  malformedEnvelope.vectors[0].steps[0].total = '101';
  assert.throws(
    () => parseActorFeeEnvelopeVectors(malformedEnvelope),
    /disagrees with package envelope semantics/,
  );
  const malformedSettlement = structuredClone(ACTORS_FEE_ENVELOPE_VECTORS);
  malformedSettlement.settlementCases[1].charges[0] = '2';
  assert.throws(
    () => parseActorFeeEnvelopeVectors(malformedSettlement),
    /disagrees with package settlement semantics/,
  );
  const malformedFloor = structuredClone(ACTORS_FEE_ENVELOPE_VECTORS);
  malformedFloor.floorCases[0].protectedMinimum = '1';
  assert.throws(
    () => parseActorFeeEnvelopeVectors(malformedFloor),
    /disagrees with package protected-minimum semantics/,
  );
});
