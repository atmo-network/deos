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
  projectObservationFanoutServiceTopology,
  projectObservationInspection,
} from '../src/lib/observation/inspection.ts';
import { DEOS_OBSERVATION_RUNTIME_EVIDENCE } from '../src/lib/observation/runtime-evidence.generated.ts';
import { compareObservationRuntimeEvidenceIdentity } from '../src/lib/observation/runtime-evidence.ts';

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
    () =>
      projectObservationActorDeliveryInspection({
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

const verifiedEvidence = {
  status: 'Verified',
  observedIdentity: 'deos-runtime@spec-1',
};
const fanoutBudget = {
  runtimeIdentity: 'deos-runtime@spec-1',
  weightIdentity: 'aaa-weights@6688fe06',
  maxServiceUnitsPerBlock: 5,
  maxActiveDirtyFeeds: 40_000,
  maxSubscriberPagesPerFeed: 157,
};
const singleActiveFeed = {
  head: feed,
  tail: feed,
  cursor: feed,
  count: 1,
  selectedPosition: 0,
  cursorPosition: 0,
};

test('reactive delivery projection counts page visits, revision restart, and cleanup service', () => {
  const pending = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 8n,
      dirtySince: 90,
      nextSubscriberPage: 4,
    },
    activeList: singleActiveFeed,
    occupiedPageCount: 12,
    remainingCurrentRevisionPages: 4,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  });
  assert.equal(pending.status, 'PendingFanout');
  assert.equal(pending.dirtyAgeBlocks, 10);
  assert.equal(pending.remainingCurrentRevisionPages, 4);
  assert.equal(pending.remainingFanoutServiceUnits, 16);
  assert.equal(pending.exclusiveBudgetLowerBoundBlocks, 4);
  assert.equal(pending.fairServiceCeilingBlocks, 4);
  assert.notEqual(pending.estimateContextIdentity, null);
  assert.match(pending.estimateAssumptions.at(-1), /condition evaluation/);
  assert.match(
    pending.estimateAssumptions.join(' '),
    /queue blocking invalidates/,
  );

  const inProgress = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 9n,
      dirtySince: 90,
      nextSubscriberPage: 7,
    },
    activeList: singleActiveFeed,
    occupiedPageCount: 12,
    remainingCurrentRevisionPages: 4,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  });
  assert.equal(inProgress.status, 'FanoutInProgress');
  assert.equal(inProgress.remainingFanoutServiceUnits, 4);
  assert.equal(inProgress.exclusiveBudgetLowerBoundBlocks, 1);

  const awaitingCleanup = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 9n,
      dirtySince: 90,
      nextSubscriberPage: null,
    },
    activeList: singleActiveFeed,
    occupiedPageCount: 12,
    remainingCurrentRevisionPages: 0,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  });
  assert.equal(awaitingCleanup.status, 'AwaitingCleanup');
  assert.equal(awaitingCleanup.remainingFanoutServiceUnits, 1);

  const pendingAtBoundary = projectObservationDeliveryInspection({
    oracleRevision: 10n,
    dirty: {
      latestRevision: 10n,
      fanoutRevision: 9n,
      dirtySince: 90,
      nextSubscriberPage: null,
    },
    activeList: singleActiveFeed,
    occupiedPageCount: 12,
    remainingCurrentRevisionPages: 0,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  });
  assert.equal(pendingAtBoundary.remainingFanoutServiceUnits, 13);

  const clean = projectObservationDeliveryInspection({
    oracleRevision: 9n,
    dirty: null,
    activeList: {
      head: null,
      tail: null,
      cursor: null,
      count: 0,
      selectedPosition: null,
      cursorPosition: null,
    },
    occupiedPageCount: 12,
    remainingCurrentRevisionPages: 0,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  });
  assert.equal(clean.status, 'Clean');
  assert.equal(clean.dirtyAgeBlocks, null);
  assert.equal(clean.remainingFanoutServiceUnits, null);
  assert.equal(clean.fairServiceCeilingBlocks, null);
  assert.equal(clean.estimateStatus, 'NotApplicable');
  assert.deepEqual(clean.estimateAssumptions, []);
});

test('finalized runtime evidence verifies exact generated identity and classifies drift', () => {
  const expected = DEOS_OBSERVATION_RUNTIME_EVIDENCE;
  const identity = {
    runtime: structuredClone(expected.runtime),
    runtimeCodeHash: expected.runtimeCodeHash,
    metadataHash: expected.metadataHash,
    fanout: {
      configuredServiceUnitsPerBlock:
        expected.fanout.configuredServiceUnitsPerBlock,
      fanoutWeightLimit: {
        refTime: BigInt(expected.fanout.fanoutWeightLimit.refTime),
        proofSize: BigInt(expected.fanout.fanoutWeightLimit.proofSize),
      },
      maxActiveActors: 10_000,
      maxTriggerSources: 4,
      queuePageSize: 64,
    },
  };
  assert.equal(
    compareObservationRuntimeEvidenceIdentity(identity).status,
    'Verified',
  );
  for (const [mutate, reason] of [
    [(value) => (value.runtime.specVersion += 1), 'spec version mismatch'],
    [
      (value) => (value.runtimeCodeHash = `0x${'11'.repeat(32)}`),
      'runtime code mismatch',
    ],
    [
      (value) => (value.metadataHash = `0x${'22'.repeat(32)}`),
      'V16 metadata mismatch',
    ],
    [
      (value) => (value.fanout.configuredServiceUnitsPerBlock += 1),
      'configured fanout service-unit bound mismatch',
    ],
    [
      (value) => (value.fanout.fanoutWeightLimit.proofSize += 1n),
      'fanout Weight limit mismatch',
    ],
    [
      (value) => (value.fanout.maxActiveActors += 1),
      'active dirty-feed bound mismatch',
    ],
    [
      (value) => (value.fanout.queuePageSize = 63),
      'subscriber-page bound mismatch',
    ],
  ]) {
    const changed = structuredClone(identity);
    mutate(changed);
    const result = compareObservationRuntimeEvidenceIdentity(changed);
    assert.equal(result.status, 'EvidenceMismatch');
    assert.ok(result.reasons.includes(reason));
  }
});

test('fair-service ceiling follows selected position before, at, and after the cursor', () => {
  const topology = (selectedPosition) =>
    projectObservationFanoutServiceTopology({
      latestRevision: 9n,
      fanoutRevision: 9n,
      nextSubscriberPage: 7,
      occupiedPageCount: 3,
      remainingCurrentRevisionPages: 3,
      activeDirtyFeedCount: 4,
      selectedPosition,
      cursorPosition: 2,
      maxServiceUnitsPerBlock: 5,
    });
  assert.deepEqual(topology(2), {
    remainingFanoutServiceUnits: 3,
    exclusiveBudgetLowerBoundBlocks: 1,
    fairServiceCeilingBlocks: 2,
  });
  assert.equal(topology(3).fairServiceCeilingBlocks, 2);
  assert.equal(topology(1).fairServiceCeilingBlocks, 3);
});

test('active-set changes invalidate context identity and maximum production bounds remain safe', () => {
  const projection = (count, selectedPosition, cursorPosition) =>
    projectObservationDeliveryInspection({
      oracleRevision: 9n,
      dirty: {
        latestRevision: 9n,
        fanoutRevision: 9n,
        dirtySince: 90,
        nextSubscriberPage: 0,
      },
      activeList: {
        head: feed,
        tail: feed,
        cursor: feed,
        count,
        selectedPosition,
        cursorPosition,
      },
      occupiedPageCount: 3,
      remainingCurrentRevisionPages: 3,
      finalizedBlock: 100,
      budget: fanoutBudget,
      evidence: verifiedEvidence,
    });
  const original = projection(4, 1, 2);
  const added = projection(5, 1, 2);
  const removed = projection(3, 1, 2);
  assert.notEqual(
    original.estimateContextIdentity,
    added.estimateContextIdentity,
  );
  assert.notEqual(
    original.estimateContextIdentity,
    removed.estimateContextIdentity,
  );

  const maximum = projectObservationFanoutServiceTopology({
    latestRevision: 2n,
    fanoutRevision: 1n,
    nextSubscriberPage: 0,
    occupiedPageCount: 157,
    remainingCurrentRevisionPages: 157,
    activeDirtyFeedCount: 40_000,
    selectedPosition: 39_999,
    cursorPosition: 0,
    maxServiceUnitsPerBlock: 5,
  });
  assert.equal(maximum.remainingFanoutServiceUnits, 314);
  assert.equal(maximum.exclusiveBudgetLowerBoundBlocks, 63);
  assert.equal(maximum.fairServiceCeilingBlocks, 2_512_000);
});

test('reactive delivery projection fails closed on mixed snapshots and impossible topology', () => {
  const base = {
    oracleRevision: 9n,
    dirty: {
      latestRevision: 9n,
      fanoutRevision: 8n,
      dirtySince: 90,
      nextSubscriberPage: 0,
    },
    activeList: singleActiveFeed,
    occupiedPageCount: 2,
    remainingCurrentRevisionPages: 2,
    finalizedBlock: 100,
    budget: fanoutBudget,
    evidence: verifiedEvidence,
  };
  assert.throws(
    () =>
      projectObservationDeliveryInspection({ ...base, oracleRevision: 10n }),
    /must match/,
  );
  assert.throws(
    () =>
      projectObservationDeliveryInspection({
        ...base,
        remainingCurrentRevisionPages: 3,
      }),
    /exceed occupied/,
  );
  assert.throws(
    () =>
      projectObservationDeliveryInspection({
        ...base,
        dirty: { ...base.dirty, dirtySince: 101 },
      }),
    /cannot exceed/,
  );
  assert.throws(
    () =>
      projectObservationDeliveryInspection({
        ...base,
        activeList: { ...base.activeList, selectedPosition: null },
      }),
    /active-list position/,
  );
  assert.throws(
    () =>
      projectObservationDeliveryInspection({
        ...base,
        activeList: { ...base.activeList, cursorPosition: null },
      }),
    /fair cursor/,
  );
  assert.throws(
    () =>
      projectObservationDeliveryInspection({
        ...base,
        occupiedPageCount: 0,
        remainingCurrentRevisionPages: 0,
      }),
    /occupied subscriber page/,
  );
  const retainedPage = projectObservationDeliveryInspection(base);
  assert.equal(retainedPage.remainingCurrentRevisionPages, 2);
  assert.match(retainedPage.estimateAssumptions.join(' '), /queue blocking/);

  const mismatch = projectObservationDeliveryInspection({
    ...base,
    evidence: {
      status: 'EvidenceMismatch',
      observedIdentity: 'other-runtime@spec-2',
      reasons: ['runtime code mismatch'],
    },
  });
  assert.equal(mismatch.status, 'PendingFanout');
  assert.equal(mismatch.remainingCurrentRevisionPages, 2);
  assert.equal(mismatch.remainingFanoutServiceUnits, null);
  assert.equal(mismatch.fairServiceCeilingBlocks, null);
  assert.equal(mismatch.estimateStatus, 'EvidenceMismatch');
  assert.equal(mismatch.estimateContextIdentity, null);
  assert.deepEqual(mismatch.evidenceMismatchReasons, ['runtime code mismatch']);
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
  assert.match(source, /expectedObservationFanoutBudget/);
  assert.match(source, /evidence: ObservationFanoutEvidence/);
  assert.doesNotMatch(source, /maxServiceUnitsPerBlock:\s*5/);
  assert.doesNotMatch(source, /getEntries/);
  assert.match(source, /canonicalObservationReadModel/);

  const transport = await readFile(
    new URL('../src/lib/adapters/blockchain/deos.ts', import.meta.url),
    'utf8',
  );
  assert.match(transport, /Core\.version\(\{ at: snapshot\.at \}\)/);
  assert.match(transport, /Metadata\.metadata_at_version/);
  assert.match(transport, /state_getStorage/);

  const evidenceSource = await readFile(
    new URL('../src/lib/observation/runtime-evidence.ts', import.meta.url),
    'utf8',
  );
  assert.match(evidenceSource, /decAnyMetadata/);
  assert.match(evidenceSource, /MaxObservationFanoutPagesPerBlock/);
  assert.match(evidenceSource, /ObservationFanoutWeightLimit/);

  const adapter = await readFile(
    new URL('../src/lib/adapters/blockchain/index.ts', import.meta.url),
    'utf8',
  );
  assert.match(adapter, /finalizedRuntimeEvidence\(snapshot\)/);
  assert.match(adapter, /compareObservationRuntimeEvidence/);
  assert.match(adapter, /status: 'EvidenceMismatch'/);
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
    'Occupied / current-revision pages',
    'Remaining service units',
    'Exclusive-budget lower bound',
    'Fair-service ceiling',
    'Estimate evidence',
    'Expected budget evidence',
    'Observed runtime evidence',
    'Numerical fanout estimates are unavailable',
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
