/*
Domain: AAA observation inspection validation
Owns: Exact formatting, four-state freshness classification, provenance, and UI disclosure fixtures.
Excludes: Live chain access, observation history, trading advice, and runtime mutation.
Zone: Web-client validation entrypoint for the observation control plane.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  canonicalObservationReadModel,
  formatObservationFeed,
  formatScaledObservation,
  projectObservationActorDeliveryInspection,
  projectObservationDeliveryInspection,
  projectObservationInspection,
} from '../src/lib/observation/inspection.ts';

const feed = {
  assetIn: { type: 'Native' },
  assetOut: { type: 'Local', id: 7 },
  method: 'PreExecutionSpot',
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
  scale: 12,
};
const config = {
  producer: '5FProducer',
  provenance: 'AxialRouterPreExecutionReserves',
  lifecycle: 'Active',
  scale: 12,
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
};
const observation = {
  value: 1_234_500_000_000n,
  updatedAt: 90,
  revision: 7n,
};

function inspect(overrides = {}) {
  return projectObservationInspection({
    feed,
    config,
    observation,
    finalizedBlock: 100,
    maxAgeBlocks: 10,
    ...overrides,
  });
}

test('typed observation formatting preserves scale and directional identity', () => {
  assert.equal(formatScaledObservation(1_234_500_000_000n, 12), '1.2345');
  assert.equal(formatScaledObservation(1_000_000_000_000n, 12), '1');
  assert.equal(formatScaledObservation(7n, 0), '7');
  assert.match(formatObservationFeed(feed), /Native → Local\(7\)/);
  assert.match(formatObservationFeed(feed), /EMA\/100/);
  assert.match(formatObservationFeed(feed), /10\^12/);
});

test('observation inspection distinguishes Fresh, Stale, Uninitialized, and Unavailable', () => {
  const fresh = inspect();
  assert.equal(fresh.status, 'Fresh');
  assert.equal(fresh.ageBlocks, 10);
  assert.equal(fresh.authoredMaxAgeBlocks, 10);
  assert.equal(fresh.formattedValue, '1.2345');
  assert.equal(fresh.revision, 7n);
  assert.equal(fresh.latestStateCoalescing, true);
  assert.equal(fresh.fairPriceProof, false);
  assert.equal(fresh.delivery, null);

  assert.equal(inspect({ finalizedBlock: 101 }).status, 'Stale');
  assert.equal(inspect({ observation: null }).status, 'Uninitialized');
  const unavailable = inspect({
    config: { ...config, lifecycle: 'Deactivated' },
  });
  assert.equal(unavailable.status, 'Unavailable');
  assert.equal(unavailable.value, null);
  assert.equal(inspect({ config: null }).status, 'Unavailable');
  assert.throws(() => inspect({ maxAgeBlocks: 0 }), /nonzero u32/);
});

test('selected actor projection exposes one exact queue or wakeup admission path', () => {
  const queued = projectObservationActorDeliveryInspection({
    aaaId: 7n,
    hot: {
      actorClass: 'System',
      pendingSignal: true,
      queueTicket: 42n,
      wakeup: null,
    },
  });
  assert.equal(queued.queueLane, 'System');
  assert.equal(queued.queueAdmissionStatus, 'Queued');
  assert.equal(queued.queueTicket, 42n);

  const wakeup = projectObservationActorDeliveryInspection({
    aaaId: 8n,
    hot: {
      actorClass: 'User',
      pendingSignal: true,
      queueTicket: null,
      wakeup: { block: 120, pageId: 3n, slot: 4 },
    },
  });
  assert.equal(wakeup.queueAdmissionStatus, 'WakeupScheduled');
  assert.deepEqual(wakeup.wakeup, { block: 120, pageId: 3n, slot: 4 });

  const blocked = projectObservationActorDeliveryInspection({
    aaaId: 9n,
    hot: {
      actorClass: 'System',
      pendingSignal: true,
      queueTicket: null,
      wakeup: null,
    },
  });
  assert.equal(blocked.queueAdmissionStatus, 'PendingQueueAdmission');

  const idle = projectObservationActorDeliveryInspection({
    aaaId: 10n,
    hot: {
      actorClass: 'User',
      pendingSignal: false,
      queueTicket: null,
      wakeup: null,
    },
  });
  assert.equal(idle.queueAdmissionStatus, 'NoPendingSignal');

  const missing = projectObservationActorDeliveryInspection({
    aaaId: 11n,
    hot: null,
  });
  assert.equal(missing.queueAdmissionStatus, 'ActorMissing');
  assert.equal(missing.pendingSignal, null);
  assert.throws(
    () => projectObservationActorDeliveryInspection({
      aaaId: 12n,
      hot: {
        actorClass: 'System',
        pendingSignal: true,
        queueTicket: 1n,
        wakeup: { block: 120, pageId: 3n, slot: 4 },
      },
    }),
    /cannot own queue and wakeup/,
  );
});

test('reactive delivery projection derives exact dirty age and bounded estimates', () => {
  const budget = {
    runtimeIdentity: 'deos-runtime@spec-1',
    weightIdentity: 'aaa-weights@6688fe06',
    maxPagesPerBlock: 5,
    maxActiveDirtyFeeds: 40_000,
    maxSubscriberPagesPerFeed: 157,
  };
  const activeList = {
    head: feed,
    tail: feed,
    cursor: feed,
    count: 1,
    selectedPosition: 0,
  };
  const pending = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 8n,
      dirtySince: 90,
      nextSubscriberPage: 4,
    },
    activeList,
    occupiedPageCount: 12,
    remainingPageCount: 12,
    finalizedBlock: 100,
    budget,
  });
  assert.equal(pending.status, 'PendingFanout');
  assert.equal(pending.dirtyAgeBlocks, 10);
  assert.equal(pending.estimatedRemainingFanoutPages, 12);
  assert.equal(pending.estimatedRemainingBlocks, 3);
  assert.match(pending.estimateAssumptions.at(-1), /not actor queue admission/);

  const inProgress = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 9n,
      dirtySince: 90,
      nextSubscriberPage: 7,
    },
    activeList,
    occupiedPageCount: 12,
    remainingPageCount: 4,
    finalizedBlock: 100,
    budget,
  });
  assert.equal(inProgress.status, 'FanoutInProgress');
  assert.equal(inProgress.estimatedRemainingBlocks, 1);

  const awaitingCleanup = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 9n,
      dirtySince: 90,
      nextSubscriberPage: null,
    },
    activeList,
    occupiedPageCount: 12,
    remainingPageCount: 0,
    finalizedBlock: 100,
    budget,
  });
  assert.equal(awaitingCleanup.status, 'AwaitingCleanup');

  const clean = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: null,
    activeList: { ...activeList, selectedPosition: null },
    occupiedPageCount: 12,
    remainingPageCount: 0,
    finalizedBlock: 100,
    budget,
  });
  assert.equal(clean.status, 'Clean');
  assert.equal(clean.dirtyAgeBlocks, null);
  assert.equal(clean.estimatedRemainingBlocks, 0);
  assert.deepEqual(clean.estimateAssumptions, []);
});

test('reactive delivery projection fails closed on mixed snapshots and impossible topology', () => {
  const base = {
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 8n,
      dirtySince: 90,
      nextSubscriberPage: null,
    },
    activeList: {
      head: feed,
      tail: feed,
      cursor: feed,
      count: 1,
      selectedPosition: 0,
    },
    occupiedPageCount: 2,
    remainingPageCount: 2,
    finalizedBlock: 100,
    budget: {
      runtimeIdentity: 'deos-runtime@spec-1',
      weightIdentity: 'aaa-weights@6688fe06',
      maxPagesPerBlock: 5,
      maxActiveDirtyFeeds: 40_000,
      maxSubscriberPagesPerFeed: 157,
    },
  };
  assert.throws(
    () => projectObservationDeliveryInspection({ ...base, oracleRevision: 10n }),
    /must match/,
  );
  assert.throws(
    () => projectObservationDeliveryInspection({ ...base, remainingPageCount: 1 }),
    /restart from every occupied/,
  );
  assert.throws(
    () => projectObservationDeliveryInspection({
      ...base,
      dirty: { ...base.dirty, dirtySince: 101 },
    }),
    /cannot exceed/,
  );
  assert.throws(
    () => projectObservationDeliveryInspection({
      ...base,
      activeList: { ...base.activeList, selectedPosition: null },
    }),
    /active-list position/,
  );
});

test('observation read model remains bounded direct canonical-chain truth', () => {
  const value = canonicalObservationReadModel(
    inspect(),
    'Oracle.Feeds + Oracle.Observations',
    { asOfBlock: 100, asOfHash: `0x${'11'.repeat(32)}` },
  );
  assert.deepEqual(value.provenance, {
    contractClass: 'canonical-chain',
    realization: 'direct',
    sourceKind: 'storage',
    scope: 'live',
    bounded: true,
    sourceRef: 'Oracle.Feeds + Oracle.Observations',
  });
  assert.equal(value.asOfBlock, 100);
});

test('typed provider reads the bounded registry and selected exact storage keys', async () => {
  const source = await readFile(
    new URL('../src/lib/adapters/blockchain/observations.ts', import.meta.url),
    'utf8',
  );
  assert.match(source, /Oracle\.FeedIds\.getValue/);
  assert.match(source, /Oracle\.Feeds\.getValue\(key/);
  assert.match(source, /Oracle\.Observations\.getValue\(key/);
  assert.match(source, /AAA\.ActorHot\.getValue/);
  assert.match(source, /projectObservationActorDeliveryInspection/);
  assert.match(source, /AAA\.DirtyObservationFeeds\.getValue/);
  assert.match(source, /AAA\.DirtyObservationListState\.getValue/);
  assert.match(source, /AAA\.ObservationSubscriberPageLists\.getValue/);
  assert.match(source, /AAA\.ObservationSubscriberPages\.getValue/);
  assert.match(source, /dirty\.dirty_since/);
  assert.match(source, /maxPagesPerBlock: 5/);
  assert.match(source, /6688fe062147259f/);
  assert.doesNotMatch(source, /getEntries/);
  assert.match(source, /canonicalObservationReadModel/);
});

test('inspection UI discloses current-state and non-fair-price boundaries', async () => {
  const source = await readFile(
    new URL(
      '../src/lib/observation/ObservationInspector.svelte',
      import.meta.url,
    ),
    'utf8',
  );
  for (const copy of [
    'Feed identity',
    'Authored maximum age',
    'Producer',
    'Provenance',
    'Revision',
    'Current age',
    'Reactive delivery',
    'Exact dirty age',
    'Active-list position',
    'Fair cursor',
    'Occupied / remaining pages',
    'Estimated fanout blocks',
    'Budget evidence',
    'Selected actor delivery',
    'Selected actor admission',
    'Queue-admission status',
    'Pending signal',
    'Queue ticket',
    'Wakeup page / slot',
    'Estimate assumptions',
    'latest-state reconsideration',
    'not fair-price',
    'intermediate revisions',
  ]) {
    assert.match(source, new RegExp(copy, 'i'));
  }
});
