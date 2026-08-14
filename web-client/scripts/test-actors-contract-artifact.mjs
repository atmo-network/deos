/*
Domain: Actors control-plane validation
Owns: Deterministic artifact, SCALE round-trip, and structural-diff regression fixtures.
Excludes: Runtime queries, simulation, governance submission, and browser rendering.
Zone: Web-client validation entrypoint; imports the automation public contract only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  createActorContractArtifact,
  diffActorContractArtifacts,
  encodeActorContractValue,
  inspectActorContractArtifact,
} from '../src/lib/automation/contract-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};

function dormantArtifact() {
  return createActorContractArtifact({
    metadataBytes,
    runtime,
    actorType: 'User',
    mutability: 'Mutable',
    contractScale: '0x00',
  });
}

test('canonical dormant artifact is deterministic and round-trips exact SCALE', () => {
  const artifact = dormantArtifact();
  assert.equal(
    artifact.contractId,
    '0x7be5d17ce09332bbb771c2dbb378db5e422be5c3a95741898af928ce82312639',
  );
  const inspection = inspectActorContractArtifact(
    artifact,
    metadataBytes,
    runtime,
  );
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.deepEqual(inspection.projection, {
      type: 'Dormant',
      value: { $none: true },
    });
  }
});

test('active ContractInput encodes and projects every nested value losslessly', () => {
  const contractScale = encodeActorContractValue(metadataBytes, {
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
      steps: [
        {
          preconditions: {
            type: 'AnyOf',
            value: [
              [
                {
                  timing: { type: 'Current', value: undefined },
                  predicate: {
                    type: 'BlockNumberAbove',
                    value: { threshold: 1 },
                  },
                },
              ],
            ],
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
      completion: { type: 'Persistent', value: undefined },
      funding: { type: 'OwnerOnly', value: undefined },
    },
  });
  const artifact = createActorContractArtifact({
    metadataBytes,
    runtime,
    actorType: 'User',
    mutability: 'Mutable',
    contractScale,
  });
  const inspection = inspectActorContractArtifact(
    artifact,
    metadataBytes,
    runtime,
  );
  assert.equal(inspection.valid, true);
  if (inspection.valid) {
    assert.deepEqual(
      inspection.projection.value.steps[0].task.value.amount.value,
      { $integer: '10', $runtimeType: 'bigint' },
    );
  }
});

test('DNF timing and clause topology change canonical identity and remain diff-visible', () => {
  const makeArtifact = (timing, separateClauses = false) => {
    const predicate = {
      timing: { type: timing, value: undefined },
      predicate: {
        type: 'BlockNumberAbove',
        value: { threshold: 1 },
      },
    };
    const second = {
      timing: { type: 'Current', value: undefined },
      predicate: {
        type: 'BlockNumberBelow',
        value: { threshold: 10 },
      },
    };
    const preconditions = {
      type: 'AnyOf',
      value: separateClauses ? [[predicate], [second]] : [[predicate, second]],
    };
    return createActorContractArtifact({
      metadataBytes,
      runtime,
      actorType: 'User',
      mutability: 'Mutable',
      contractScale: encodeActorContractValue(metadataBytes, {
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
          steps: [
            {
              preconditions,
              task: { type: 'StopCycle', value: undefined },
              on_error: { type: 'AbortCycle', value: undefined },
            },
          ],
          completion: { type: 'Persistent', value: undefined },
          funding: { type: 'OwnerOnly', value: undefined },
        },
      }),
    });
  };
  const inspected = [
    makeArtifact('Opening'),
    makeArtifact('Current'),
    makeArtifact('Current', true),
  ].map((artifact) => {
    const inspection = inspectActorContractArtifact(
      artifact,
      metadataBytes,
      runtime,
    );
    assert.equal(inspection.valid, true);
    if (!inspection.valid) throw new Error('fixture must inspect');
    assert.equal(
      inspection.projection.value.steps[0].preconditions.type,
      'AnyOf',
    );
    return inspection;
  });
  assert.equal(
    new Set(inspected.map(({ artifact }) => artifact.contractId)).size,
    3,
  );
  const changedMode = diffActorContractArtifacts(inspected[0], inspected[1]);
  assert.equal(changedMode.compatible, true);
  if (changedMode.compatible) {
    assert(
      changedMode.changes.some(
        (change) =>
          change.kind === 'replace' &&
          change.path.includes('/preconditions/value/0/0/timing/type') &&
          change.before === 'Opening' &&
          change.after === 'Current',
      ),
    );
  }
});

test('trigger admission diff stays inside the trigger tree and never invents contract control', () => {
  const inspectTrigger = (trigger) => {
    const artifact = createActorContractArtifact({
      metadataBytes,
      runtime,
      actorType: 'User',
      mutability: 'Mutable',
      contractScale: encodeActorContractValue(metadataBytes, {
        type: 'Active',
        value: {
          schedule: { trigger, cooldown_blocks: 0 },
          schedule_window: undefined,
          steps: [
            {
              preconditions: { type: 'Unconditional', value: undefined },
              task: { type: 'StopCycle', value: undefined },
              on_error: { type: 'AbortCycle', value: undefined },
            },
          ],
          completion: { type: 'Persistent', value: undefined },
          funding: { type: 'OwnerOnly', value: undefined },
        },
      }),
    });
    const inspection = inspectActorContractArtifact(
      artifact,
      metadataBytes,
      runtime,
    );
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
  const observationDiff = diffActorContractArtifacts(immediate, observation);
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
  const diff = diffActorContractArtifacts(immediate, cadenced);
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
  const stale = inspectActorContractArtifact(artifact, metadataBytes, {
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
  const corrupted = inspectActorContractArtifact(
    { ...artifact, contractId: `0x${'00'.repeat(32)}` },
    metadataBytes,
    runtime,
  );
  assert.equal(corrupted.valid, false);
  if (!corrupted.valid) {
    assert(
      corrupted.errors.includes(
        'contractId does not match the canonical Actor Contract fields',
      ),
    );
  }
  assert.throws(
    () =>
      createActorContractArtifact({
        metadataBytes,
        runtime,
        actorType: 'User',
        mutability: 'Mutable',
        contractScale: '0x0000',
      }),
    /exact SCALE bytes/,
  );
});

test('ordered structural diff distinguishes moves, insertion, and metadata incompatibility', () => {
  const artifact = dormantArtifact();
  const taskA = { task: 'A' };
  const taskB = { task: 'B' };
  const taskX = { task: 'X' };
  const moved = diffActorContractArtifacts(
    { artifact, projection: { steps: [taskA, taskB] } },
    { artifact, projection: { steps: [taskB, taskA] } },
  );
  assert.deepEqual(moved, {
    compatible: true,
    changes: [
      { kind: 'move', from: '/steps/0', path: '/steps/1', value: taskA },
    ],
  });

  const inserted = diffActorContractArtifacts(
    { artifact, projection: { steps: [taskA, taskB] } },
    { artifact, projection: { steps: [taskX, taskA, taskB] } },
  );
  assert.deepEqual(inserted, {
    compatible: true,
    changes: [{ kind: 'add', path: '/steps/0', value: taskX }],
  });

  const incompatible = diffActorContractArtifacts(
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
