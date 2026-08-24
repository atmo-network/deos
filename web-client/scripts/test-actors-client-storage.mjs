/*
Domain: Actors browser storage boundaries
Owns: Static regression evidence that browser summaries consume compact canonical heads.
Excludes: Runtime API execution, full Contract reconstruction, archive history, and transport behavior.
Zone: Web-client validation entrypoint; prevents retired monolithic storage and physical tail/payload reads.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const adapter = await readFile(
  new URL('../src/lib/adapters/blockchain/index.ts', import.meta.url),
  'utf8',
);
const upgradeEvidence = await readFile(
  new URL('./upgrade-state-evidence.mjs', import.meta.url),
  'utf8',
);

test('browser Actor summaries consume compact Contract and Run heads only', () => {
  assert.match(adapter, /Actors\.ActorContractHead\.getValue/);
  assert.match(adapter, /Actors\.ActorRunHead\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorContract\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorRunState\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorContractTailChunk\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorRunPayload\.getValue/);
});

test('upgrade evidence records the compact Contract head without reconstructing tails', () => {
  assert.match(upgradeEvidence, /Actors\.ActorContractHead\.getValue/);
  assert.match(upgradeEvidence, /actor_contract_head: actorContractHead/);
  assert.doesNotMatch(upgradeEvidence, /Actors\.ActorContract\.getValue/);
  assert.doesNotMatch(
    upgradeEvidence,
    /Actors\.ActorContractTailChunk\.getValue/,
  );
});
