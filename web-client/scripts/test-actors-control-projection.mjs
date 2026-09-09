/*
Domain: Actors canonical control projection validation
Owns: Bounded locator resolution and absence/dormancy/corruption classification fixtures.
Excludes: Live RPC, scheduler mutation, Contract reconstruction, and presentation.
Zone: Web-client validation entrypoint; exercises the canonical browser control reader.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import { readActorControlProjection } from '../src/lib/adapters/blockchain/actor-control.ts';

const at = `0x${'11'.repeat(32)}`;
const actorId = 65n;
const identity = {
  actor_class: { type: 'System', value: { sovereign_id: 4n } },
};
const hot = { pending_signal: false };
const cell = { actor_id: actorId, identity, hot };

function api({ location, dormantIdentity, unsignaled, ready, waiting }) {
  return {
    query: {
      Actors: {
        ActorControlLocators: {
          async getValue(id, options) {
            assert.equal(id, actorId);
            assert.deepEqual(options, { at });
            return location;
          },
        },
        ActorIdentities: {
          async getValue(id, options) {
            assert.equal(id, actorId);
            assert.deepEqual(options, { at });
            return dormantIdentity;
          },
        },
        ActorUnsignaledControlCells: {
          async getValue() {
            return unsignaled;
          },
        },
        ActorReadyFrameChunks: {
          async getValue(page, options) {
            assert.equal(page, 2n);
            assert.deepEqual(options, { at });
            return ready;
          },
        },
        ActorWaitingFrameChunks: {
          async getValue(key, options) {
            assert.deepEqual(key, [{ type: 'Block', value: 120 }, 3n]);
            assert.deepEqual(options, { at });
            return waiting;
          },
        },
      },
    },
  };
}

test('canonical control projection distinguishes absence and dormant identity', async () => {
  assert.deepEqual(await readActorControlProjection(api({}), at, actorId), {
    status: 'NotRegistered',
  });
  assert.deepEqual(
    await readActorControlProjection(
      api({ dormantIdentity: identity }),
      at,
      actorId,
    ),
    { status: 'Dormant', identity },
  );
});

test('canonical control projection resolves every bounded active owner', async () => {
  const unsignaledLocation = { type: 'Unsignaled', value: undefined };
  assert.deepEqual(
    await readActorControlProjection(
      api({ location: unsignaledLocation, unsignaled: cell }),
      at,
      actorId,
    ),
    { status: 'Active', location: unsignaledLocation, cell },
  );

  const readyLocation = { type: 'Ready', value: { ticket: 65n } };
  const ready = [undefined, cell];
  assert.deepEqual(
    await readActorControlProjection(
      api({ location: readyLocation, ready }),
      at,
      actorId,
    ),
    { status: 'Active', location: readyLocation, cell },
  );

  const waitingLocation = {
    type: 'Waiting',
    value: { key: { type: 'Block', value: 120 }, page: 3n, slot: 1 },
  };
  const waiting = {
    entries: [undefined, { type: 'Primary', value: cell }],
  };
  assert.deepEqual(
    await readActorControlProjection(
      api({ location: waitingLocation, waiting }),
      at,
      actorId,
    ),
    { status: 'Active', location: waitingLocation, cell },
  );
});

test('canonical control projection fails closed on split or mismatched authority', async () => {
  await assert.rejects(
    readActorControlProjection(
      api({
        location: { type: 'Unsignaled', value: undefined },
        dormantIdentity: identity,
        unsignaled: cell,
      }),
      at,
      actorId,
    ),
    /both dormant and active control owners/,
  );
  await assert.rejects(
    readActorControlProjection(
      api({
        location: { type: 'Unsignaled', value: undefined },
        unsignaled: { ...cell, actor_id: actorId + 1n },
      }),
      at,
      actorId,
    ),
    /does not resolve to its Actor/,
  );
});
