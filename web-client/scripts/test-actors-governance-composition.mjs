/*
Domain: Actors governance-composition validation
Owns: Exact metadata-bound RuntimeCall bytes, origin classification, preimage identity, and unsupported-governance fixtures.
Excludes: Signing, preimage noting, proposal submission, voting, enactment, and chain mutation.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { ACTORS_MAX_OWNER_SLOTS } from '../src/lib/automation/actors-protocol-bounds.ts';
import { composeActorRuntimeCall } from '../src/lib/automation/governance-composition.ts';
import {
  createActorPlanArtifact,
  encodeActorProgramValue,
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

function artifact(actorType, programScale = '0x00') {
  return createActorPlanArtifact({
    metadataBytes,
    runtime,
    actorType,
    mutability: 'Mutable',
    programScale,
  });
}

test('User Actors create and slot calls encode exact direct-call bytes', () => {
  const direct = composeActorRuntimeCall({
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

  const slotted = composeActorRuntimeCall({
    artifact: artifact('User'),
    metadataBytes,
    runtime,
    target: { type: 'Create', ownerSlot: 7 },
  });
  assert.equal(slotted.call.bytes, '0x3701070000');
});

test('User owner-slot authoring enforces the metadata-derived MaxOwnerSlots bound', () => {
  assert.equal(
    ACTORS_MAX_OWNER_SLOTS,
    255,
    'reference baseline is 255 valid slots',
  );
  // Highest valid slot is accepted.
  const highest = composeActorRuntimeCall({
    artifact: artifact('User'),
    metadataBytes,
    runtime,
    target: { type: 'Create', ownerSlot: ACTORS_MAX_OWNER_SLOTS - 1 },
  });
  assert.equal(highest.call.bytes, '0x3701fe0000');
  // The hard ceiling itself is rejected before artifact submission.
  assert.throws(
    () =>
      composeActorRuntimeCall({
        artifact: artifact('User'),
        metadataBytes,
        runtime,
        target: { type: 'Create', ownerSlot: ACTORS_MAX_OWNER_SLOTS },
      }),
    /MaxOwnerSlots/,
  );
});

test('System Actors composition exposes exact Root call but denies current governance admission', () => {
  const programScale = encodeActorProgramValue(metadataBytes, {
    type: 'Active',
    value: {
      schedule: {
        trigger: {
          type: 'Immediate',
          value: {
            sources: [{ type: 'Manual', value: undefined }],
          },
        },
        cooldown_blocks: 5,
      },
      schedule_window: undefined,
      execution_plan: [
        {
          conditions: {
            type: 'Any',
            value: [{ type: 'BlockNumberAbove', value: { threshold: 1 } }],
          },
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
      completion_policy: {
        type: 'CloseAfterProductiveCycle',
        value: undefined,
      },
      funding_source_policy: { type: 'OwnerOnly', value: undefined },
    },
  });
  const composed = composeActorRuntimeCall({
    artifact: artifact('System', programScale),
    metadataBytes,
    runtime,
    target: { type: 'Create', owner },
  });

  assert.equal(composed.call.bytes.startsWith('0x3702'), true);
  assert.equal(composed.authority.requiredOrigin, 'Root');
  assert.equal(composed.authority.governanceDomain, 'StrategicNative');
  assert.equal(
    composed.preimage.governanceAdmission,
    'UnsupportedActorRootCall',
  );
  assert.match(composed.preimage.reason, /runtime-upgrade payload/);
});

test('activation and custody reattachment preserve artifact identity and reject invalid targets', () => {
  const userArtifact = artifact('User');
  const activation = composeActorRuntimeCall({
    artifact: userArtifact,
    metadataBytes,
    runtime,
    target: { type: 'Activate', actorId: 9n },
  });
  assert.equal(activation.call.bytes, '0x3711090000000000000000');
  assert.equal(activation.planId, userArtifact.planId);

  assert.throws(
    () =>
      composeActorRuntimeCall({
        artifact: userArtifact,
        metadataBytes,
        runtime,
        target: { type: 'ReattachSystem', sovereignId: 9n, owner },
      }),
    /System Actors artifact/,
  );
  assert.throws(
    () =>
      composeActorRuntimeCall({
        artifact: userArtifact,
        metadataBytes,
        runtime: { ...runtime, specVersion: 2 },
        target: { type: 'Create' },
      }),
    /specVersion does not match/,
  );
});
