/*
Domain: AAA control-plane validation
Owns: Deterministic artifact, SCALE round-trip, and structural-diff regression fixtures.
Excludes: Runtime queries, simulation, governance submission, and browser rendering.
Zone: Web-client validation entrypoint; imports the automation public contract only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  createAaaPlanArtifact,
  diffAaaPlanArtifacts,
  encodeAaaProgramValue,
  inspectAaaPlanArtifact,
} from '../src/lib/automation/plan-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};

function dormantArtifact() {
  return createAaaPlanArtifact({
    metadataBytes,
    runtime,
    aaaType: 'User',
    mutability: 'Mutable',
    programScale: '0x00',
  });
}

test('canonical dormant artifact is deterministic and round-trips exact SCALE', () => {
  const artifact = dormantArtifact();
  assert.equal(
    artifact.planId,
    '0xff832beabbac961192fdb21e93fec881424e4b42a1c291b06901248425014791',
  );
  const inspection = inspectAaaPlanArtifact(artifact, metadataBytes, runtime);
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.deepEqual(inspection.projection, {
      type: 'Dormant',
      value: { $none: true },
    });
  }
});

test('active ProgramInput encodes and projects every nested value losslessly', () => {
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
          conditions: [{ type: 'BlockNumberAbove', value: { threshold: 1 } }],
          task: {
            type: 'Transfer',
            value: {
              to: '5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT',
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
  const artifact = createAaaPlanArtifact({
    metadataBytes,
    runtime,
    aaaType: 'User',
    mutability: 'Mutable',
    programScale,
  });
  const inspection = inspectAaaPlanArtifact(artifact, metadataBytes, runtime);
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.deepEqual(
      inspection.projection.value.execution_plan[0].task.value.amount.value,
      { $integer: '10', $runtimeType: 'bigint' },
    );
  }
});

test('artifact inspection rejects identity drift and noncanonical bytes', () => {
  const artifact = dormantArtifact();
  const stale = inspectAaaPlanArtifact(artifact, metadataBytes, {
    ...runtime,
    specVersion: 2,
  });
  assert.equal(stale.valid, false);
  if (!stale.valid) {
    assert(
      stale.errors.includes(
        'specVersion does not match the live runtime identity',
      ),
    );
  }
  const corrupted = inspectAaaPlanArtifact(
    { ...artifact, planId: `0x${'00'.repeat(32)}` },
    metadataBytes,
    runtime,
  );
  assert.equal(corrupted.valid, false);
  if (!corrupted.valid) {
    assert(
      corrupted.errors.includes(
        'planId does not match the canonical artifact fields',
      ),
    );
  }
  assert.throws(
    () =>
      createAaaPlanArtifact({
        metadataBytes,
        runtime,
        aaaType: 'User',
        mutability: 'Mutable',
        programScale: '0x0000',
      }),
    /exact SCALE bytes/,
  );
});

test('ordered structural diff distinguishes moves, insertion, and metadata incompatibility', () => {
  const artifact = dormantArtifact();
  const taskA = { task: 'A' };
  const taskB = { task: 'B' };
  const taskX = { task: 'X' };
  const moved = diffAaaPlanArtifacts(
    { artifact, projection: { steps: [taskA, taskB] } },
    { artifact, projection: { steps: [taskB, taskA] } },
  );
  assert.deepEqual(moved, {
    compatible: true,
    changes: [
      { kind: 'move', from: '/steps/0', path: '/steps/1', value: taskA },
    ],
  });

  const inserted = diffAaaPlanArtifacts(
    { artifact, projection: { steps: [taskA, taskB] } },
    { artifact, projection: { steps: [taskX, taskA, taskB] } },
  );
  assert.deepEqual(inserted, {
    compatible: true,
    changes: [{ kind: 'add', path: '/steps/0', value: taskX }],
  });

  const incompatible = diffAaaPlanArtifacts(
    { artifact, projection: { steps: [] } },
    {
      artifact: { ...artifact, metadataHash: `0x${'22'.repeat(32)}` },
      projection: { steps: [] },
    },
  );
  assert.deepEqual(incompatible, {
    compatible: false,
    reason: 'IncompatibleUntilRebound',
    mismatches: ['metadataHash'],
  });
});
