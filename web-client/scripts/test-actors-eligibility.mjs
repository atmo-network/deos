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
  projectActorEligibility,
} from '../src/lib/automation/eligibility.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
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

test('projectActorEligibility projects a ready result with next block', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      phase: { type: 'Ready' },
      next_eligible_block: 42,
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    phase: 'Ready',
    closeReason: null,
    nextEligibleBlock: 42,
  });
});

test('projectActorEligibility projects an absent next block as null', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      phase: { type: 'WaitingTemporal' },
    },
  });
  assert.deepEqual(projectActorEligibility(decoded), {
    phase: 'WaitingTemporal',
    closeReason: null,
    nextEligibleBlock: null,
  });
});

test('projectActorEligibility maps every phase variant through metadata', () => {
  const phases = [
    'NotRegistered',
    'Dormant',
    'Ready',
    'Paused',
    'GlobalCircuitBreaker',
    'WaitingSignal',
    'WaitingRetry',
    'WaitingTemporal',
  ];
  for (const phase of phases) {
    const decoded = encodeProjection({
      success: true,
      value: { phase: { type: phase } },
    });
    const projected = projectActorEligibility(decoded);
    assert.equal(projected.phase, phase);
  }
  const closeDue = encodeProjection({
    success: true,
    value: {
      phase: {
        type: 'CloseDue',
        value: { type: 'WindowExpired', value: undefined },
      },
    },
  });
  assert.deepEqual(projectActorEligibility(closeDue), {
    phase: 'CloseDue',
    closeReason: 'WindowExpired',
    nextEligibleBlock: null,
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

test('projectActorEligibility rejects unknown phases and malformed results', () => {
  assert.throws(
    () =>
      projectActorEligibility({
        success: true,
        value: { phase: { type: 'Mystery' } },
      }),
    /Unsupported runtime eligibility phase Mystery/,
  );
  assert.throws(
    () => projectActorEligibility({ success: 'maybe' }),
    /SCALE Result/,
  );
});
