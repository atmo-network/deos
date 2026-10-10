/*
Domain: Actors resource/control SCALE contract
Owns: Named v2 machine flags and their isolated or committed metadata conformance.
Excludes: Metadata generation, descriptors, storage mutation, and resource accounting policy.
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
import test from 'node:test';

import { readActorsTestMetadata } from './actors-test-metadata.mjs';

import {
  ACTORS_RESOURCE_RUNTIME_API_VERSION,
  projectActorMachineControlFlags,
} from '../src/lib/automation/resource.ts';

const metadata = unifyMetadata(decAnyMetadata(await readActorsTestMetadata()));

test('resource API v2 exposes a named input-free machine control projection', () => {
  const apis = metadata.apis.filter((api) => api.name === 'ActorResourceApi');
  assert.equal(apis.length, 1);
  assert.equal(apis[0].version, ACTORS_RESOURCE_RUNTIME_API_VERSION);
  assert.deepEqual(
    apis[0].methods.map((method) => method.name),
    [
      'machine_control_flags',
      'block_resource_budget',
      'current_block_resource_state',
      'finalized_block_resource_snapshot',
    ],
  );
  const method = apis[0].methods.find(
    (method) => method.name === 'machine_control_flags',
  );
  assert.deepEqual(method.inputs, []);
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    method.output,
  );
  for (const userCreationEnabled of [false, true]) {
    for (const machineFrozen of [false, true]) {
      const encoded = codec.enc({
        user_creation_enabled: userCreationEnabled,
        machine_frozen: machineFrozen,
      });
      assert.equal(encoded.length, 2);
      const decoded = codec.dec(encoded);
      assert.deepEqual(encoded, codec.enc(decoded));
      assert.deepEqual(projectActorMachineControlFlags(decoded), {
        userCreationEnabled,
        machineFrozen,
      });
    }
  }
});
