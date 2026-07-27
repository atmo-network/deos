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
    'latest-state reconsideration',
    'not fair-price',
    'intermediate revisions',
  ]) {
    assert.match(source, new RegExp(copy, 'i'));
  }
});
