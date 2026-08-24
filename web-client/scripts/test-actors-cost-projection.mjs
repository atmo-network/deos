/*
Domain: Actors cost projection validation
Owns: Fail-closed fixtures for independent runtime cost owners and actual Action receipts.
Excludes: Metadata generation, chain transport, fee policy, and historical event indexing.
Zone: Web-client validation entrypoint; imports only the automation cost contract.
*/
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { readActorCost } from '../src/lib/adapters/blockchain/actor-cost.ts';
import {
  ACTORS_COST_VECTORS,
  parseActorCostVectors,
} from '../src/lib/automation/cost-vectors.ts';
import {
  ACTORS_COST_RUNTIME_API,
  ACTORS_COST_RUNTIME_API_VERSION,
  projectActorActionFeeReceipt,
  projectActorCostQuote,
} from '../src/lib/automation/cost.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const costVectorArtifact = JSON.parse(
  await readFile(
    new URL('../src/lib/automation/actors-cost-vectors.json', import.meta.url),
    'utf8',
  ),
);

const hash = (byte) => new Uint8Array(32).fill(byte);
const weight = (refTime, proofSize) => ({
  ref_time: BigInt(refTime),
  proof_size: BigInt(proofSize),
});

function activeUserQuote() {
  return {
    success: true,
    value: {
      actor_type: { type: 'User', value: undefined },
      creation_fee: 20n,
      prospective_trigger_fee: {
        trigger_family: { type: 'Manual', value: undefined },
        maximum_weight: weight(113_494_000, 9_635),
        fee: 114n,
        production_weight_identity: hash(1),
      },
      prospective_pipeline_fee: {
        pipeline_machine_fee: 1_000n,
        cleanup_fee: 600n,
        total_fee: 1_600n,
        strategy: { type: 'UpfrontBounded', value: undefined },
        admission_identity: hash(2),
        production_weight_identity: hash(3),
      },
      maximum_next_action_fee: {
        maximum_effect_weight: weight(280_000_000, 13_000),
        maximum_effect_fee: 280n,
        production_weight_identity: hash(4),
      },
      actor_state_hold: {
        exempt: false,
        base_per_component: 10n,
        per_encoded_byte: 1n,
        breakdown: {
          identity: 11n,
          contract_head: 12n,
          contract_body: 0n,
          detector: 13n,
          funding: 14n,
          run: 0n,
        },
        total: 50n,
      },
    },
  };
}

test('metadata exposes one versioned Actor cost quote with canonical input', () => {
  assert.equal(ACTORS_COST_RUNTIME_API, 'ActorCostApi_actor_cost_quote');
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const apis = metadata.apis.filter(
    (candidate) => candidate.name === 'ActorCostApi',
  );
  assert.equal(apis.length, 1);
  assert.equal(apis[0].version, ACTORS_COST_RUNTIME_API_VERSION);
  const methods = apis[0].methods.filter(
    (candidate) => candidate.name === 'actor_cost_quote',
  );
  assert.equal(methods.length, 1);
  assert.deepEqual(
    methods[0].inputs.map((input) => input.name),
    ['actor_id'],
  );
});

test('cost projection keeps every economic owner and provenance separate', () => {
  const quote = projectActorCostQuote(activeUserQuote());
  assert.equal(quote.actorType, 'User');
  assert.equal(quote.creationFee, 20n);
  assert.deepEqual(quote.prospectiveTriggerFee, {
    family: 'Manual',
    maximumWeight: { refTime: 113_494_000n, proofSize: 9_635n },
    fee: 114n,
    productionWeightIdentity: `0x${'01'.repeat(32)}`,
  });
  assert.deepEqual(quote.prospectivePipelineFee, {
    machineFee: 1_000n,
    cleanupFee: 600n,
    totalFee: 1_600n,
    strategy: 'UpfrontBounded',
    admissionIdentity: `0x${'02'.repeat(32)}`,
    productionWeightIdentity: `0x${'03'.repeat(32)}`,
  });
  assert.equal(quote.maximumNextActionFee.maximumEffectFee, 280n);
  assert.equal(quote.stateHold.total, 50n);
  assert.equal('activationTotal' in quote, false);
  assert.equal('remainingMachineBudget' in quote, false);
});

test('dormant and System quotes preserve absence and explicit exemption', () => {
  const dormant = activeUserQuote();
  dormant.value.prospective_trigger_fee = undefined;
  dormant.value.prospective_pipeline_fee = undefined;
  dormant.value.maximum_next_action_fee.maximum_effect_weight = weight(0, 0);
  dormant.value.maximum_next_action_fee.maximum_effect_fee = 0n;
  const dormantView = projectActorCostQuote(dormant);
  assert.equal(dormantView.prospectiveTriggerFee, null);
  assert.equal(dormantView.prospectivePipelineFee, null);
  assert.equal(dormantView.maximumNextActionFee.maximumEffectFee, 0n);

  const system = activeUserQuote();
  system.value.actor_type = { type: 'System', value: undefined };
  system.value.creation_fee = 0n;
  system.value.prospective_trigger_fee.fee = 0n;
  system.value.prospective_pipeline_fee.pipeline_machine_fee = 0n;
  system.value.prospective_pipeline_fee.cleanup_fee = 0n;
  system.value.prospective_pipeline_fee.total_fee = 0n;
  system.value.maximum_next_action_fee.maximum_effect_fee = 0n;
  system.value.actor_state_hold.exempt = true;
  for (const component of Object.keys(
    system.value.actor_state_hold.breakdown,
  )) {
    system.value.actor_state_hold.breakdown[component] = 0n;
  }
  system.value.actor_state_hold.total = 0n;
  const systemView = projectActorCostQuote(system);
  assert.equal(systemView.actorType, 'System');
  assert.equal(systemView.stateHold.exempt, true);
  assert.equal(systemView.stateHold.total, 0n);
});

test('cost projection rejects unknown variants and inconsistent named totals', () => {
  assert.throws(
    () =>
      projectActorCostQuote({
        success: false,
        value: { type: 'FutureFailure', value: undefined },
      }),
    /Unsupported runtime Actor cost error FutureFailure/,
  );
  const unknownStrategy = activeUserQuote();
  unknownStrategy.value.prospective_pipeline_fee.strategy = {
    type: 'RefundAfterRun',
    value: undefined,
  };
  assert.throws(
    () => projectActorCostQuote(unknownStrategy),
    /Unsupported runtime Pipeline strategy RefundAfterRun/,
  );
  const badPipeline = activeUserQuote();
  badPipeline.value.prospective_pipeline_fee.total_fee = 1_601n;
  assert.throws(
    () => projectActorCostQuote(badPipeline),
    /Pipeline total must equal Machine plus cleanup fees/,
  );
  const badHold = activeUserQuote();
  badHold.value.actor_state_hold.total = 51n;
  assert.throws(
    () => projectActorCostQuote(badHold),
    /state hold total must equal its named components/,
  );
});

test('finalized cost transport invokes the typed API once at the requested block', async () => {
  const calls = [];
  const typedApi = {
    apis: {
      ActorCostApi: {
        actor_cost_quote: async (...args) => {
          calls.push(args);
          return activeUserQuote();
        },
      },
    },
  };
  const result = await readActorCost(typedApi, '0x1234', 7);
  assert.deepEqual(calls, [[7n, { at: '0x1234' }]]);
  assert.equal(result.projection?.actorType, 'User');
  assert.equal(result.unavailableReason, null);

  typedApi.apis.ActorCostApi.actor_cost_quote = async () => {
    throw new Error('unavailable');
  };
  assert.deepEqual(await readActorCost(typedApi, '0x1234', 7), {
    projection: null,
    unavailableReason: 'unavailable',
  });
});

test('ActionFeeCharged projects exact coordinates, actual Weight, and zero fee', () => {
  assert.deepEqual(
    projectActorActionFeeReceipt({
      actor_id: 7n,
      cycle_nonce: 9n,
      step_index: 3,
      actual_effect_weight: weight(0, 0),
      fee: 0n,
    }),
    {
      actorId: 7n,
      cycleNonce: 9n,
      stepIndex: 3,
      actualEffectWeight: { refTime: 0n, proofSize: 0n },
      fee: 0n,
    },
  );
});

test('runtime-generated cost vectors bind metadata, Weight, geometry, and Trigger families', async () => {
  const metadataSha256 = createHash('sha256')
    .update(metadataBytes)
    .digest('hex');
  const weights = await readFile(
    new URL(
      '../../template/runtime/src/weights/pallet_deos_actors.rs',
      import.meta.url,
    ),
  );
  assert.equal(ACTORS_COST_VECTORS.metadataSha256, metadataSha256);
  assert.equal(
    ACTORS_COST_VECTORS.weightSha256,
    createHash('sha256').update(weights).digest('hex'),
  );
  assert.equal(ACTORS_COST_VECTORS.runtimeApiVersion, 1);
  assert.equal(ACTORS_COST_VECTORS.vectors.length, 12);

  const manual = ACTORS_COST_VECTORS.vectors
    .filter(
      (vector) =>
        vector.quote.actorType === 'User' && vector.triggerFamily === 'Manual',
    )
    .sort((left, right) => left.contractStepCount - right.contractStepCount);
  assert.deepEqual(
    manual.map((vector) => vector.contractStepCount),
    [0, 1, 4, 8, 32],
  );
  assert.equal(manual[0].quote.maximumNextActionFee.maximumEffectFee, 0n);
  assert.equal(manual[1].quote.stateHold.components.contractBody, 0n);
  assert.ok(manual[2].quote.stateHold.components.contractBody > 0n);
  assert.deepEqual(
    manual
      .slice(1)
      .map((vector) => vector.quote.maximumNextActionFee.maximumEffectFee),
    Array(4).fill(manual[1].quote.maximumNextActionFee.maximumEffectFee),
    'unreachable tails must not inflate the current Action maximum',
  );
  assert.ok(
    manual[4].quote.prospectivePipelineFee.totalFee >
      manual[1].quote.prospectivePipelineFee.totalFee,
    'complete upfront Pipeline Machine authority grows with admitted geometry',
  );

  const families = new Set(
    ACTORS_COST_VECTORS.vectors
      .filter(
        (vector) =>
          vector.quote.actorType === 'User' && vector.contractStepCount === 1,
      )
      .map((vector) => vector.triggerFamily),
  );
  assert.deepEqual(
    families,
    new Set([
      'Manual',
      'AddressEvent',
      'ObservationChange',
      'ObservationCrossing',
      'AtTime',
      'Cadenced',
    ]),
  );
});

test('generated cost vectors preserve explicit System exemption and dormant absence', () => {
  const system = ACTORS_COST_VECTORS.vectors.find(
    (vector) => vector.name === 'system-manual-1',
  ).quote;
  assert.equal(system.creationFee, 0n);
  assert.equal(system.prospectiveTriggerFee.fee, 0n);
  assert.equal(system.prospectivePipelineFee.totalFee, 0n);
  assert.equal(system.maximumNextActionFee.maximumEffectFee, 0n);
  assert.equal(system.stateHold.exempt, true);
  assert.equal(system.stateHold.total, 0n);

  const dormant = ACTORS_COST_VECTORS.vectors.find(
    (vector) => vector.name === 'user-dormant',
  ).quote;
  assert.equal(dormant.prospectiveTriggerFee, null);
  assert.equal(dormant.prospectivePipelineFee, null);
  assert.equal(dormant.maximumNextActionFee.maximumEffectFee, 0n);
  assert.ok(dormant.stateHold.total > 0n);
});

test('generated cost vector parser fails closed on drift and malformed ownership', () => {
  const unsupported = structuredClone(costVectorArtifact);
  unsupported.formatVersion = 2;
  assert.throws(() => parseActorCostVectors(unsupported), /unsupported/);

  const malformedTotal = structuredClone(costVectorArtifact);
  malformedTotal.vectors.find(
    (vector) => vector.name === 'user-manual-1',
  ).quote.prospectivePipelineFee.totalFee = '1';
  assert.throws(
    () => parseActorCostVectors(malformedTotal),
    /total is inconsistent/,
  );

  const missingGeometry = structuredClone(costVectorArtifact);
  missingGeometry.vectors = missingGeometry.vectors.filter(
    (vector) => vector.name !== 'user-manual-32',
  );
  assert.throws(
    () => parseActorCostVectors(missingGeometry),
    /0\/1\/4\/8\/32 geometry/,
  );
});
