import assert from 'node:assert/strict';
import test from 'node:test';

import { readActorResourceProjection } from '../src/lib/adapters/blockchain/actor-resource.ts';

const weight = (value) => ({ ref_time: value, proof_size: value + 1n });
const budget = (value) => ({
  maximum_block: weight(value),
  fixed_envelope: weight(value + 2n),
  limits: {
    actor_control: weight(value + 4n),
    shared_economic: weight(value + 6n),
    actor_base_turn: weight(value + 8n),
    user_base_turn: weight(value + 10n),
  },
});
const usage = {
  actor_control: weight(1n),
  actor_effect: weight(3n),
  user_dispatch: weight(5n),
};

function api(current, finalized, calls = []) {
  const read = (name, value) => async (options) => {
    calls.push([name, options]);
    return value;
  };
  return {
    apis: {
      ActorResourceApi: {
        block_resource_budget: read('budget', budget(100n)),
        current_block_resource_state: read('current', current),
        finalized_block_resource_snapshot: read('finalized', finalized),
      },
    },
  };
}

test('resource transport preserves each block budget instead of substituting configured limits', async () => {
  const calls = [];
  const projection = await readActorResourceProjection(
    api(
      {
        block_number: 12,
        phase: { type: 'ExternalPhase' },
        budget: budget(200n),
        usage,
        outstanding_reservations: 1,
        optional_actor_work_halted: false,
      },
      {
        block_number: 11,
        budget: budget(300n),
        usage,
        optional_actor_work_halted: true,
      },
      calls,
    ),
    '0xabcd',
  );
  assert.deepEqual(calls, [
    ['budget', { at: '0xabcd' }],
    ['current', { at: '0xabcd' }],
    ['finalized', { at: '0xabcd' }],
  ]);
  assert.equal(projection.budget.maximumBlock.refTime, 100n);
  assert.equal(projection.current.budget.maximumBlock.refTime, 200n);
  assert.equal(projection.current.budget.limits.actorControl.proofSize, 205n);
  assert.equal(projection.finalized.budget.fixedEnvelope.refTime, 302n);
  assert.equal(projection.current.blockNumber, 12);
  assert.equal(projection.finalized.blockNumber, 11);
  assert.deepEqual(projection.finalized.usage.actorEffect, {
    refTime: 3n,
    proofSize: 4n,
  });
  assert.equal(projection.finalized.optionalActorWorkHalted, true);
});

test('resource transport preserves absent state and an unfrozen current budget', async () => {
  const absent = await readActorResourceProjection(
    api(undefined, undefined),
    '0x1',
  );
  assert.equal(absent.current, null);
  assert.equal(absent.finalized, null);
  const unfrozen = await readActorResourceProjection(
    api(
      {
        block_number: 12,
        phase: { type: 'ContextIncomplete' },
        budget: undefined,
        usage,
        outstanding_reservations: 0,
        optional_actor_work_halted: false,
      },
      undefined,
    ),
    '0x1',
  );
  assert.equal(unfrozen.current.phase, 'ContextIncomplete');
  assert.equal(unfrozen.current.budget, null);
});
