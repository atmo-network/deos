/*
Domain: AAA eligibility projection validation
Owns: Read-only runtime `AaaEligibilityApi` signature evidence and canonical domain projection fixtures.
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
  AAA_ELIGIBILITY_RUNTIME_API,
  AAA_ELIGIBILITY_RUNTIME_API_VERSION,
  projectAaaEligibility,
} from '../src/lib/automation/eligibility.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);

function eligibilityMethod() {
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const apis = metadata.apis.filter(
    (candidate) => candidate.name === 'AaaEligibilityApi',
  );
  assert.equal(apis.length, 1, 'metadata must expose AaaEligibilityApi once');
  assert.equal(
    apis[0].version,
    AAA_ELIGIBILITY_RUNTIME_API_VERSION,
    'AaaEligibilityApi version must match the client contract',
  );
  const methods = apis[0].methods.filter(
    (candidate) => candidate.name === 'aaa_eligibility',
  );
  assert.equal(methods.length, 1, 'aaa_eligibility must appear once');
  assert.deepEqual(
    methods[0].inputs.map((input) => input.name),
    ['aaa_id'],
    'aaa_eligibility must carry exactly the canonical aaa_id input',
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

test('projectAaaEligibility projects a ready result with next block', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      phase: { type: 'Ready' },
      next_eligible_block: 42,
    },
  });
  assert.deepEqual(projectAaaEligibility(decoded), {
    phase: 'Ready',
    closeReason: null,
    nextEligibleBlock: 42,
  });
});

test('projectAaaEligibility projects an absent next block as null', () => {
  const decoded = encodeProjection({
    success: true,
    value: {
      phase: { type: 'WaitingTemporal' },
    },
  });
  assert.deepEqual(projectAaaEligibility(decoded), {
    phase: 'WaitingTemporal',
    closeReason: null,
    nextEligibleBlock: null,
  });
});

test('projectAaaEligibility maps every phase variant through metadata', () => {
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
    const projected = projectAaaEligibility(decoded);
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
  assert.deepEqual(projectAaaEligibility(closeDue), {
    phase: 'CloseDue',
    closeReason: 'WindowExpired',
    nextEligibleBlock: null,
  });
});

test('projectAaaEligibility rejects a typed runtime failure honestly', () => {
  const decoded = encodeProjection({
    success: false,
    value: { type: 'ContinuationInvariant' },
  });
  assert.throws(() => projectAaaEligibility(decoded), /ContinuationInvariant/);
});

test('projectAaaEligibility rejects unknown phases and malformed results', () => {
  assert.throws(
    () =>
      projectAaaEligibility({
        success: true,
        value: { phase: { type: 'Mystery' } },
      }),
    /Unsupported runtime eligibility phase Mystery/,
  );
  assert.throws(
    () => projectAaaEligibility({ success: 'maybe' }),
    /SCALE Result/,
  );
});
