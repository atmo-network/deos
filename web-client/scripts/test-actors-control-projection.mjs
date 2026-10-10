/*
Domain: Actors canonical control projection validation
Owns: Finalized semantic/process residence and absent/dormant/unavailable/corrupt fixtures.
Excludes: Live RPC, scheduler mutation, full Contract reconstruction and historical indexing.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { readActorControlProjection } from '../src/lib/adapters/blockchain/actor-control.ts';
import { automationResidenceLabel } from '../src/lib/automation/types.ts';

const at = `0x${'11'.repeat(32)}`;
const actorId = 65n;
const identity = {
  sovereign_account: 'actor-custody',
  actor_class: { type: 'System', value: { sovereign_id: 4n } },
  cycle_nonce: 7n,
};
const hot = {
  lifecycle: { type: 'Active' },
  cycle_state: { type: 'Running' },
  pending_signal: false,
  last_cycle_block: 123,
};
const active = {
  type: 'Active',
  value: {
    generation: 3n,
    identity,
    hot,
    admission: { admission_identity: 'certificate' },
  },
};
const service = { type: 'Service', value: { type: 'Live' } };
const serving = {
  generation: 3n,
  status: { type: 'Serving' },
  residence: service,
};

function api({ semantic, process } = {}) {
  function reader(value) {
    return {
      async getValue(id, options) {
        assert.equal(id, actorId);
        assert.deepEqual(options, { at });
        return value;
      },
    };
  }
  return {
    query: {
      Actors: {
        ActorSemanticStates: reader(semantic),
        ActorProcesses: reader(process),
        ActorControlLocators: {
          getValue() {
            throw new Error('Legacy locator read');
          },
        },
      },
    },
  };
}

test('canonical control distinguishes absent, dormant and missing metadata', async () => {
  assert.deepEqual(await readActorControlProjection(api(), at, actorId), {
    status: 'NotRegistered',
  });
  const dormant = { type: 'Dormant', value: { identity, generation: 0n } };
  assert.deepEqual(
    await readActorControlProjection(api({ semantic: dormant }), at, actorId),
    { status: 'Dormant', identity, generation: 0n },
  );
  assert.deepEqual(
    await readActorControlProjection({ query: { Actors: {} } }, at, actorId),
    {
      status: 'Unavailable',
      reason: 'Canonical Actors metadata is unavailable',
    },
  );
});

test('canonical Active is discovered without a legacy locator and preserves semantic evidence', async () => {
  const result = await readActorControlProjection(
    api({ semantic: active, process: serving }),
    at,
    actorId,
  );
  assert.equal(result.status, 'Active');
  assert.deepEqual(result.record, active.value);
  assert.deepEqual(result.residence, { type: 'Service', kind: 'Live' });
  assert.equal(result.awaitingFunding, false);
});

test('current residences project without scheduling arithmetic or FIFO tickets', async () => {
  for (const [residence, expected, label] of [
    [service, { type: 'Service', kind: 'Live' }, 'Service · Live'],
    [
      { type: 'Service', value: { type: 'Pending' } },
      { type: 'Service', kind: 'Pending' },
      'Service · Pending',
    ],
    [
      {
        type: 'Deadline',
        value: { key: { type: 'Block', value: 120 }, page: 3n, slot: 1 },
      },
      { type: 'Deadline', key: { type: 'Block', value: 120 } },
      'Block 120',
    ],
    [
      {
        type: 'Deadline',
        value: { key: { type: 'Tick', value: 4_838_400n }, page: 3n, slot: 1 },
      },
      { type: 'Deadline', key: { type: 'Tick', value: 4_838_400n } },
      'Tick 4838400',
    ],
    [
      {
        type: 'Parked',
        value: {
          reason: { type: 'ParkedBalanceBelowThreshold' },
          review_at: null,
        },
      },
      { type: 'Parked', reason: 'ParkedBalanceBelowThreshold' },
      'Parked · ParkedBalanceBelowThreshold',
    ],
  ]) {
    const result = await readActorControlProjection(
      api({ semantic: active, process: { ...serving, residence } }),
      at,
      actorId,
    );
    assert.deepEqual(result.residence, expected);
    assert.equal(automationResidenceLabel(result.residence), label);
  }
  const disabled = await readActorControlProjection(
    api({
      semantic: active,
      process: {
        ...serving,
        status: { type: 'Disabled' },
        residence: undefined,
      },
    }),
    at,
    actorId,
  );
  assert.deepEqual(disabled.residence, { type: 'Disabled' });
});

test('User waiting retains current residence while System waiting refuses', async () => {
  const process = { ...serving, status: { type: 'AwaitingFunding' } };
  const user = {
    ...active,
    value: {
      ...active.value,
      identity: {
        ...identity,
        actor_class: { type: 'User', value: { owner_slot: 2 } },
      },
    },
  };
  const result = await readActorControlProjection(
    api({ semantic: user, process }),
    at,
    actorId,
  );
  assert.equal(result.awaitingFunding, true);
  assert.deepEqual(result.residence, { type: 'Service', kind: 'Live' });
  await assert.rejects(
    readActorControlProjection(api({ semantic: active, process }), at, actorId),
    /System Actor cannot await fee funding/,
  );
});

test('orphan, missing, stale generation and unsupported process authorities fail closed', async () => {
  for (const [semantic, process, message] of [
    [undefined, serving, /Unregistered Actor retains/],
    [
      { type: 'Dormant', value: { identity, generation: 0n } },
      serving,
      /Dormant Actor retains/,
    ],
    [active, undefined, /Active Actor process is malformed/],
    [active, { ...serving, generation: 4n }, /generation mismatch/],
    [
      active,
      { ...serving, status: { type: 'Retired' } },
      /status is unsupported/,
    ],
    [
      active,
      { ...serving, status: { type: 'Disabled' } },
      /retains executable residence/,
    ],
    [
      active,
      { ...serving, residence: { type: 'Ready', value: { ticket: 1n } } },
      /residence is unsupported/,
    ],
  ]) {
    await assert.rejects(
      readActorControlProjection(api({ semantic, process }), at, actorId),
      message,
    );
  }
});
