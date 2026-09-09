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
const controlProjection = await readFile(
  new URL('../src/lib/adapters/blockchain/actor-control.ts', import.meta.url),
  'utf8',
);
const upgradeEvidence = await readFile(
  new URL('./upgrade-state-evidence.mjs', import.meta.url),
  'utf8',
);

test('browser Actor summaries consume compact Contract and Run heads only', () => {
  assert.match(adapter, /Actors\.ActorContractHead\.getValue/);
  assert.match(adapter, /Actors\.ActorRunHead\.getValue/);
  assert.match(adapter, /readActorControlProjection/);
  assert.doesNotMatch(adapter, /Actors\.ActorIdentities\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorHot\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorContract\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorRunState\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorContractTailChunk\.getValue/);
  assert.doesNotMatch(adapter, /Actors\.ActorRunPayload\.getValue/);
});

test('canonical browser control reads classify one bounded owner and corruption', () => {
  assert.match(controlProjection, /Actors\.ActorControlLocators\.getValue/);
  assert.match(
    controlProjection,
    /Actors\.ActorUnsignaledControlCells\.getValue/,
  );
  assert.match(controlProjection, /Actors\.ActorReadyFrameChunks\.getValue/);
  assert.match(controlProjection, /Actors\.ActorWaitingFrameChunks\.getValue/);
  assert.match(controlProjection, /both dormant and active control owners/);
});

test('upgrade evidence records canonical control and compact Contract head without reconstructing tails', () => {
  assert.match(upgradeEvidence, /readActorControlProjection/);
  assert.doesNotMatch(upgradeEvidence, /Actors\.ActorHot\.getValue/);
  assert.match(upgradeEvidence, /Actors\.ActorContractHead\.getValue/);
  assert.match(upgradeEvidence, /actor_contract_head: actorContractHead/);
  assert.doesNotMatch(upgradeEvidence, /Actors\.ActorContract\.getValue/);
  assert.doesNotMatch(
    upgradeEvidence,
    /Actors\.ActorContractTailChunk\.getValue/,
  );
});
