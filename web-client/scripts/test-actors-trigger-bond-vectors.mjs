/*
Domain: Actors Trigger-state bond vector validation
Owns: Runtime-generated policy identity, exhaustive family coverage, and browser quote parity.
Excludes: Live runtime transport and reserve mutation.
*/
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  ACTORS_TRIGGER_BOND_VECTORS,
  actorTriggerStateBond,
} from '../src/lib/automation/trigger-bond-vectors.ts';

const metadata = await readFile(
  new URL('../.papi/metadata/deos.scale', import.meta.url),
);
const weights = await readFile(
  new URL(
    '../../template/runtime/src/weights/pallet_deos_actors.rs',
    import.meta.url,
  ),
);

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

test('runtime-generated Trigger bond vectors bind metadata and Actors weights', () => {
  assert.equal(ACTORS_TRIGGER_BOND_VECTORS.formatVersion, 1);
  assert.equal(ACTORS_TRIGGER_BOND_VECTORS.metadataSha256, sha256(metadata));
  assert.equal(ACTORS_TRIGGER_BOND_VECTORS.actorsWeightSha256, sha256(weights));
});

test('Trigger bond vectors are exhaustive, nonzero, and projected without a client formula', () => {
  const expected = {
    Manual: 1_000_000_000n,
    AddressEvent: 2_000_000_000n,
    ObservationChange: 2_000_000_000n,
    ObservationCrossing: 5_000_000_000n,
    Cadenced: 2_000_000_000n,
  };
  assert.deepEqual(
    ACTORS_TRIGGER_BOND_VECTORS.vectors.map((vector) => vector.triggerFamily),
    Object.keys(expected),
  );
  for (const [family, amount] of Object.entries(expected)) {
    assert.equal(actorTriggerStateBond(family), amount);
  }
});

test('Trigger bond projection fails closed when generated family evidence is absent', () => {
  assert.throws(
    () => actorTriggerStateBond('Unknown'),
    /Missing canonical Trigger-state bond vector/,
  );
});
