/*
Domain: Actors materialization projection validation
Owns: Named capacity/fault lowering, absent-state behavior, and finalized transport call evidence.
Excludes: Runtime execution, fault repair, historical indexing, and UI rendering.
Zone: Web-client validation entrypoint for ActorEligibilityApi v6 materialization projections.
*/
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  projectActorMaterialization,
  readActorMaterializationProjection,
} from '../src/lib/adapters/blockchain/actor-materialization.ts';

const capacity = {
  user_limit: 10_000,
  total_limit: 10_000,
  user_memberships: 128,
  total_memberships: 129,
};

const noFaults = {
  crossing: undefined,
  fanout: undefined,
  wakeup: undefined,
};

test('named Crossing capacity and absent current faults remain distinct', () => {
  assert.deepEqual(projectActorMaterialization(capacity, noFaults), {
    crossingCapacity: {
      userLimit: 10_000,
      totalLimit: 10_000,
      userMemberships: 128,
      totalMemberships: 129,
    },
    faults: { crossing: null, fanout: null, wakeup: null },
  });
});

test('all materialization fault families preserve canonical coordinates', () => {
  const hash = new Uint8Array(32).fill(7);
  const projection = projectActorMaterialization(capacity, {
    crossing: {
      feed: { id: 'crossing' },
      revision: 4n,
      threshold: 99n,
      class: { type: 'Capacity' },
    },
    fanout: {
      feed: { id: 'fanout' },
      revision: 8n,
      subscriber_page: 2,
      subscriber_position: 3,
      actor_id: 11n,
      semantic_contract_id: hash,
      body_commitment: undefined,
      admission_identity: { asBytes: () => hash },
      branch: { type: 'Placed' },
      class: { type: 'SchedulerExhausted' },
    },
    wakeup: {
      key: { type: 'Tick', value: 42n },
      page: 5,
      class: { type: 'Invariant' },
    },
  });

  assert.deepEqual(projection.faults.crossing, {
    feed: { id: 'crossing' },
    revision: 4n,
    threshold: 99n,
    class: 'Capacity',
  });
  assert.deepEqual(projection.faults.fanout, {
    feed: { id: 'fanout' },
    revision: 8n,
    subscriberPage: 2,
    subscriberPosition: 3,
    actorId: 11n,
    semanticContractId: hash,
    bodyCommitment: null,
    admissionIdentity: hash,
    branch: 'Placed',
    class: 'SchedulerExhausted',
  });
  assert.deepEqual(projection.faults.wakeup, {
    key: { type: 'Tick', tick: 42n },
    page: 5,
    class: 'Invariant',
  });
});

test('materialization projection fails closed on unknown fault semantics', () => {
  assert.throws(
    () =>
      projectActorMaterialization(capacity, {
        ...noFaults,
        crossing: {
          feed: {},
          revision: undefined,
          threshold: undefined,
          class: { type: 'FutureClass' },
        },
      }),
    /Unsupported materialization fault class FutureClass/,
  );
});

test('materialization transport pins both API reads to one finalized block', async () => {
  const calls = [];
  const at = Symbol('finalized block');
  const feed = { runtime: 'feed' };
  const typedApi = {
    apis: {
      ActorEligibilityApi: {
        crossing_capacity: async (requestedFeed, options) => {
          calls.push(['capacity', requestedFeed, options.at]);
          return capacity;
        },
        materialization_faults: async (options) => {
          calls.push(['faults', options.at]);
          return noFaults;
        },
      },
    },
  };

  const projection = await readActorMaterializationProjection(
    typedApi,
    at,
    feed,
  );
  assert.equal(projection.crossingCapacity.totalMemberships, 129);
  assert.deepEqual(calls, [
    ['capacity', feed, at],
    ['faults', at],
  ]);
});
