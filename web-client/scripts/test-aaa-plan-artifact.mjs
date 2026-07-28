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
    '0x8d127113b22fa7744646d1eb32862cac52045332db0531a41e03f5c9976de3cb',
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
            type: 'All',
            value: [{ type: 'BlockNumberAbove', value: { threshold: 1 } }],
          },
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
      completion_policy: { type: 'Persistent', value: undefined },
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

test('condition aggregate mode changes canonical identity and remains diff-visible', () => {
  const makeArtifact = (mode) => {
    const conditions =
      mode === 'Always'
        ? { type: 'Always', value: undefined }
        : {
            type: mode,
            value: [{ type: 'BlockNumberAbove', value: { threshold: 1 } }],
          };
    return createAaaPlanArtifact({
      metadataBytes,
      runtime,
      aaaType: 'User',
      mutability: 'Mutable',
      programScale: encodeAaaProgramValue(metadataBytes, {
        type: 'Active',
        value: {
          schedule: {
            trigger: {
              type: 'Immediate',
              value: {
                sources: [{ type: 'Manual', value: undefined }],
              },
            },
            cooldown_blocks: 0,
          },
          schedule_window: undefined,
          execution_plan: [
            {
              conditions,
              task: { type: 'StopCycle', value: undefined },
              on_error: { type: 'AbortCycle', value: undefined },
            },
          ],
          completion_policy: { type: 'Persistent', value: undefined },
          funding_source_policy: { type: 'OwnerOnly', value: undefined },
        },
      }),
    });
  };
  const inspected = ['Always', 'All', 'Any'].map((mode) => {
    const artifact = makeArtifact(mode);
    const inspection = inspectAaaPlanArtifact(artifact, metadataBytes, runtime);
    assert.equal(inspection.valid, true);
    if (!inspection.valid) throw new Error('fixture must inspect');
    assert.equal(
      inspection.projection.value.execution_plan[0].conditions.type,
      mode,
    );
    return inspection;
  });
  assert.equal(
    new Set(inspected.map(({ artifact }) => artifact.planId)).size,
    3,
  );
  const changedMode = diffAaaPlanArtifacts(inspected[1], inspected[2]);
  assert.equal(changedMode.compatible, true);
  if (changedMode.compatible) {
    assert(
      changedMode.changes.some(
        (change) =>
          change.kind === 'replace' &&
          change.path.endsWith('/conditions/type') &&
          change.before === 'All' &&
          change.after === 'Any',
      ),
    );
  }
});

test('trigger admission diff stays inside the trigger tree and never invents plan control', () => {
  const inspectTrigger = (trigger) => {
    const artifact = createAaaPlanArtifact({
      metadataBytes,
      runtime,
      aaaType: 'User',
      mutability: 'Mutable',
      programScale: encodeAaaProgramValue(metadataBytes, {
        type: 'Active',
        value: {
          schedule: { trigger, cooldown_blocks: 0 },
          schedule_window: undefined,
          execution_plan: [
            {
              conditions: { type: 'Always', value: undefined },
              task: { type: 'StopCycle', value: undefined },
              on_error: { type: 'AbortCycle', value: undefined },
            },
          ],
          completion_policy: { type: 'Persistent', value: undefined },
          funding_source_policy: { type: 'OwnerOnly', value: undefined },
        },
      }),
    });
    const inspection = inspectAaaPlanArtifact(artifact, metadataBytes, runtime);
    assert.equal(inspection.valid, true);
    if (!inspection.valid) throw new Error('fixture must inspect');
    return inspection;
  };
  const manual = [{ type: 'Manual', value: undefined }];
  const immediate = inspectTrigger({
    type: 'Immediate',
    value: { sources: manual },
  });
  const observation = inspectTrigger({
    type: 'Immediate',
    value: {
      sources: [
        {
          type: 'OnObservationChange',
          value: {
            feed: {
              asset_in: { type: 'Native', value: undefined },
              asset_out: { type: 'Local', value: 7 },
              method: { type: 'PreExecutionSpot', value: undefined },
              aggregation: {
                type: 'Ema',
                value: { half_life_blocks: 100 },
              },
              scale: 12,
            },
          },
        },
      ],
    },
  });
  const observationSource =
    observation.projection.value.schedule.trigger.value.sources[0];
  assert.equal(observationSource.type, 'OnObservationChange');
  assert.deepEqual(Object.keys(observationSource.value), ['feed']);
  assert.equal(observationSource.value.feed.aggregation.type, 'Ema');
  assert.equal(
    observationSource.value.feed.aggregation.value.half_life_blocks.$integer,
    '100',
  );
  assert.equal(observationSource.value.feed.scale.$integer, '12');
  const observationDiff = diffAaaPlanArtifacts(immediate, observation);
  assert.equal(observationDiff.compatible, true);
  if (observationDiff.compatible) {
    assert(
      observationDiff.changes.every((change) =>
        ('path' in change ? change.path : change.from).startsWith(
          '/value/schedule/trigger',
        ),
      ),
    );
  }
  const cadenced = inspectTrigger({
    type: 'Cadenced',
    value: {
      every_blocks: 10,
      mode: { type: 'WhenSignalled', value: manual },
    },
  });
  const diff = diffAaaPlanArtifacts(immediate, cadenced);
  assert.equal(diff.compatible, true);
  if (diff.compatible) {
    assert(diff.changes.length > 0);
    assert(
      diff.changes.every((change) =>
        ('path' in change ? change.path : change.from).startsWith(
          '/value/schedule/trigger',
        ),
      ),
    );
    assert(
      diff.changes.some(
        (change) =>
          change.kind === 'replace' &&
          change.path === '/value/schedule/trigger/type' &&
          change.before === 'Immediate' &&
          change.after === 'Cadenced',
      ),
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
