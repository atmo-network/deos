/*
Domain: AAA governance-composition validation
Owns: Exact metadata-bound RuntimeCall bytes, origin classification, preimage identity, and unsupported-governance fixtures.
Excludes: Signing, preimage noting, proposal submission, voting, enactment, and chain mutation.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { composeAaaRuntimeCall } from '../src/lib/automation/governance-composition.ts';
import {
  createAaaPlanArtifact,
  encodeAaaProgramValue,
} from '../src/lib/automation/plan-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const owner = '5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT';

function artifact(aaaType, programScale = '0x00') {
  return createAaaPlanArtifact({
    metadataBytes,
    runtime,
    aaaType,
    mutability: 'Mutable',
    programScale,
  });
}

test('User AAA create and slot calls encode exact direct-call bytes', () => {
  const direct = composeAaaRuntimeCall({
    artifact: artifact('User'),
    metadataBytes,
    runtime,
    target: { type: 'Create' },
  });
  assert.equal(direct.call.bytes, '0x37000000');
  assert.equal(direct.call.byteLength, 4);
  assert.equal(direct.authority.requiredOrigin, 'OwnerSigned');
  assert.equal(direct.authority.governanceDomain, null);
  assert.equal(direct.preimage.governanceAdmission, 'DirectCallOnly');
  assert.equal(direct.preimage.hash, direct.call.hash);

  const slotted = composeAaaRuntimeCall({
    artifact: artifact('User'),
    metadataBytes,
    runtime,
    target: { type: 'Create', ownerSlot: 7 },
  });
  assert.equal(slotted.call.bytes, '0x3701070000');
});

test('System AAA composition exposes exact Root call but denies current governance admission', () => {
  const programScale = encodeAaaProgramValue(metadataBytes, {
    type: 'Active',
    value: {
      schedule: {
        trigger: { type: 'Manual', value: undefined },
        cooldown_blocks: 5,
      },
      schedule_window: undefined,
      execution_plan: [
        {
          conditions: [],
          task: {
            type: 'Mint',
            value: {
              asset: { type: 'Native', value: undefined },
              amount: { type: 'Fixed', value: 10n },
            },
          },
          on_error: { type: 'AbortCycle', value: undefined },
        },
      ],
      funding_source_policy: { type: 'OwnerOnly', value: undefined },
    },
  });
  const composed = composeAaaRuntimeCall({
    artifact: artifact('System', programScale),
    metadataBytes,
    runtime,
    target: { type: 'Create', owner },
  });

  assert.equal(composed.call.bytes.startsWith('0x3702'), true);
  assert.equal(composed.authority.requiredOrigin, 'Root');
  assert.equal(composed.authority.governanceDomain, 'StrategicNative');
  assert.equal(composed.preimage.governanceAdmission, 'UnsupportedAaaRootCall');
  assert.match(composed.preimage.reason, /runtime-upgrade payload/);
});

test('activation and reopening preserve artifact identity and reject invalid targets', () => {
  const userArtifact = artifact('User');
  const activation = composeAaaRuntimeCall({
    artifact: userArtifact,
    metadataBytes,
    runtime,
    target: { type: 'Activate', aaaId: 9n },
  });
  assert.equal(activation.call.bytes, '0x3715090000000000000000');
  assert.equal(activation.planId, userArtifact.planId);

  assert.throws(
    () =>
      composeAaaRuntimeCall({
        artifact: userArtifact,
        metadataBytes,
        runtime,
        target: { type: 'ReopenSystem', aaaId: 9n, owner },
      }),
    /System AAA artifact/,
  );
  assert.throws(
    () =>
      composeAaaRuntimeCall({
        artifact: userArtifact,
        metadataBytes,
        runtime: { ...runtime, specVersion: 2 },
        target: { type: 'Create' },
      }),
    /specVersion does not match/,
  );
});
