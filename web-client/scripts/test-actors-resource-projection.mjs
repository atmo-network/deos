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
  assert.equal(projection.machineControl, null);
  assert.match(
    projection.machineControlUnavailableReason,
    /metadata is unavailable/,
  );
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

test('resource transport projects independent creation and freeze flags at the same hash', async () => {
  const calls = [];
  const typedApi = api(undefined, undefined, calls);
  typedApi.apis.ActorResourceApi.machine_control_flags = async (options) => {
    calls.push(['flags', options]);
    return { user_creation_enabled: true, machine_frozen: true };
  };
  const projection = await readActorResourceProjection(typedApi, '0x42');
  assert.deepEqual(projection.machineControl, {
    userCreationEnabled: true,
    machineFrozen: true,
  });
  assert.equal(projection.machineControlUnavailableReason, null);
  assert.deepEqual(
    calls.find(([name]) => name === 'flags'),
    ['flags', { at: '0x42' }],
  );
});

test('resource flags remain distinct from retained block halt', async () => {
  const typedApi = api(
    {
      block_number: 12,
      phase: { type: 'ExternalPhase' },
      budget: budget(200n),
      usage,
      outstanding_reservations: 0,
      optional_actor_work_halted: true,
    },
    undefined,
  );
  for (const userCreationEnabled of [false, true]) {
    for (const machineFrozen of [false, true]) {
      typedApi.apis.ActorResourceApi.machine_control_flags = async () => ({
        user_creation_enabled: userCreationEnabled,
        machine_frozen: machineFrozen,
      });
      const projection = await readActorResourceProjection(typedApi, '0x42');
      assert.deepEqual(projection.machineControl, {
        userCreationEnabled,
        machineFrozen,
      });
      assert.equal(projection.current.optionalActorWorkHalted, true);
    }
  }
});

test('unavailable or malformed flags preserve the independent budget projection', async () => {
  for (const response of [
    null,
    {},
    { user_creation_enabled: true, machine_frozen: 0 },
    'rpc-failure',
  ]) {
    const typedApi = api(undefined, undefined);
    typedApi.apis.ActorResourceApi.machine_control_flags = async () => {
      if (response === 'rpc-failure') throw new Error('flag RPC failed');
      return response;
    };
    const projection = await readActorResourceProjection(typedApi, '0x42');
    assert.equal(projection.machineControl, null);
    assert.equal(typeof projection.machineControlUnavailableReason, 'string');
    assert.equal(projection.budget.maximumBlock.refTime, 100n);
    assert.equal(projection.current, null);
  }
});
