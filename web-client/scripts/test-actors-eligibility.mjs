/*
Domain: Actors eligibility projection validation
Owns: Read-only runtime `ActorEligibilityApi` signature evidence and canonical domain projection fixtures.
Excludes: Runtime execution, chain access, scheduler semantics, and plan authoring.
Zone: Web-client validation entrypoint; imports the automation eligibility contract and metadata builders.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  ACTORS_ELIGIBILITY_RUNTIME_API,
  ACTORS_ELIGIBILITY_RUNTIME_API_VERSION,
  ACTOR_CLOSE_REASONS,
  projectActorEligibility,
} from '../src/lib/automation/eligibility.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const actorsManifest = JSON.parse(
  await readFile(
    new URL('../src/lib/automation/actors-abi-manifest.json', import.meta.url),
    'utf8',
  ),
);

function eligibilityMethod() {
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const apis = metadata.apis.filter(
    (candidate) => candidate.name === 'ActorEligibilityApi',
  );
  assert.equal(apis.length, 1, 'metadata must expose ActorEligibilityApi once');
  assert.equal(
    apis[0].version,
    ACTORS_ELIGIBILITY_RUNTIME_API_VERSION,
    'ActorEligibilityApi version must match the client contract',
  );
  const methods = apis[0].methods.filter(
    (candidate) => candidate.name === 'actor_eligibility',
  );
  assert.equal(methods.length, 1, 'actor_eligibility must appear once');
  assert.deepEqual(
    methods[0].inputs.map((input) => input.name),
    ['actor_id'],
    'actor_eligibility must carry exactly the canonical actor_id input',
  );
  return { metadata, method: methods[0] };
}

function encodeProjection(value) {
  const { metadata, method } = eligibilityMethod();
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    method.output,
  );
  const encoded = codec.enc(value);
  const decoded = codec.dec(encoded);
  assert.deepEqual(
    encoded,
    codec.enc(decoded),
    'encoded eligibility Result must round-trip canonically',
  );
  return decoded;
}

test('runtime metadata binds the canonical eligibility API signature', () => {
  eligibilityMethod();
});

test('terminal reasons exactly match generated runtime metadata', () => {
  const closeReason = actorsManifest.types.find(
    (node) =>
      node.path.join('::') ===
      'pallet_deos_actors::types::lifecycle::CloseReason',
  );
  assert(closeReason, 'CloseReason must remain reachable from Actors metadata');
  assert.deepEqual(
    closeReason.def.value.map((variant) => variant.name),
    ACTOR_CLOSE_REASONS,
  );
});

test('projectActorEligibility projects canonical active classification', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Cadenced', value: { every_ticks: 120n } },
        pending_signal: true,
        placement: { type: 'Queue', value: 9n },
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'WaitingCadenceTick', value: 42n },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    type: 'Active',
    trigger: { type: 'Cadenced', everyTicks: 120n },
    pendingSignal: true,
    placement: { type: 'Queue', ticket: 9n },
    terminalReason: null,
    executionPhase: { type: 'WaitingCadenceTick', tick: 42 },
  });
});

test('projectActorEligibility preserves absence, dormancy, terminal reason, and execution phase', () => {
  for (const type of ['NotRegistered', 'Dormant']) {
    const decoded = encodeProjection({
      success: true,
      value: { type, value: undefined },
    });
    assert.deepEqual(projectActorEligibility(decoded), { type });
  }
  const active = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: { type: 'Manual', value: undefined },
        pending_signal: false,
        placement: { type: 'Unplaced', value: undefined },
        eligibility: {
          terminal_reason: { type: 'WindowExpired', value: undefined },
          execution_phase: { type: 'GlobalCircuitBreaker', value: undefined },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(active), {
    type: 'Active',
    trigger: { type: 'Manual' },
    pendingSignal: false,
    placement: { type: 'Unplaced' },
    terminalReason: 'WindowExpired',
    executionPhase: { type: 'GlobalCircuitBreaker' },
  });
});

test('projectActorEligibility preserves semantic Crossing activation state', () => {
  const feed = {
    asset_in: { type: 'Native', value: undefined },
    asset_out: { type: 'Local', value: 7 },
    method: { type: 'PreExecutionSpot', value: undefined },
    aggregation: { type: 'LastValue', value: undefined },
    scale: 12,
  };
  const decoded = encodeProjection({
    success: true,
    value: {
      type: 'Active',
      value: {
        trigger: {
          type: 'ObservationCrossing',
          value: {
            feed,
            direction: { type: 'Rising', value: undefined },
            threshold: 100n,
            rearm_threshold: 80n,
            phase: { type: 'WaitingForRearm', value: undefined },
            pending_revisions: 2,
            processing_revision: 7n,
          },
        },
        pending_signal: true,
        placement: {
          type: 'Wakeup',
          value: { type: 'Block', value: 44 },
        },
        eligibility: {
          terminal_reason: undefined,
          execution_phase: { type: 'WaitingBlock', value: 44 },
        },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    type: 'Active',
    trigger: {
      type: 'ObservationCrossing',
      feed,
      direction: 'Rising',
      threshold: 100n,
      rearmThreshold: 80n,
      phase: 'WaitingForRearm',
      pendingRevisions: 2,
      processingRevision: 7n,
    },
    pendingSignal: true,
    placement: { type: 'WakeupBlock', block: 44 },
    terminalReason: null,
    executionPhase: { type: 'WaitingBlock', block: 44 },
  });
});

test('projectActorEligibility rejects a typed runtime failure honestly', () => {
  const decoded = encodeProjection({
    success: false,
    value: { type: 'ContinuationInvariant' },
  });
  assert.throws(
    () => projectActorEligibility(decoded),
    /ContinuationInvariant/,
  );
});

test('projectActorEligibility rejects unknown runtime variants and malformed results', () => {
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: { type: 'Mystery' },
      }),
    /Unsupported runtime eligibility Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'Manual' },
            pending_signal: false,
            placement: { type: 'Unplaced' },
            eligibility: {
              terminal_reason: { type: 'Mystery' },
              execution_phase: { type: 'Ready' },
            },
          },
        },
      }),
    /Unsupported runtime close reason Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: { type: 'Manual' },
            pending_signal: false,
            placement: { type: 'Unplaced' },
            eligibility: {
              terminal_reason: undefined,
              execution_phase: { type: 'Mystery' },
            },
          },
        },
      }),
    /Unsupported runtime execution phase Mystery/,
  );
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: {
          type: 'Active',
          value: {
            trigger: {
              type: 'ObservationCrossing',
              value: { direction: { type: 'Sideways' } },
            },
            pending_signal: false,
            placement: { type: 'Unplaced' },
            eligibility: {
              terminal_reason: undefined,
              execution_phase: { type: 'Ready' },
            },
          },
        },
      }),
    /Unsupported runtime Crossing direction Sideways/,
  );
  assert.throws(
    () => projectActorEligibility({ success: 'maybe' }),
    /SCALE Result/,
  );
});
