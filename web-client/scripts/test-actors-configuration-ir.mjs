/*
Domain: Actors configuration IR validation
Owns: Cross-format normalization, structural diagnostics/diff, canonical lowering, decode, and Actor Contract identity equivalence.
Excludes: Runtime submission, file-system loaders, UI presentation, and alternate execution semantics.
Zone: Web-client validation entrypoint; composes configuration IR with canonical authoring and artifact contracts.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  createActorArtifactFromAuthoring,
  lowerActorAuthoringContract,
} from '../src/lib/automation/authoring.ts';
import {
  authoringContractToConfigurationIr,
  configurationIrToAuthoringContract,
  diagnoseActorConfigurationIr,
  diffActorConfigurationIr,
  lowerActorConfigurationIr,
  parseActorConfigurationJson,
  parseActorConfigurationMarkdown,
  parseActorConfigurationToml,
  serializeActorConfigurationJson,
  serializeActorConfigurationMarkdown,
  serializeActorConfigurationToml,
} from '../src/lib/automation/configuration-ir.ts';
import { inspectActorContractArtifact } from '../src/lib/automation/contract-artifact.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'55'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const native = { type: 'Native' };
const local = { type: 'Local', id: 7 };
const feed = {
  assetIn: native,
  assetOut: local,
  method: 'PreExecutionSpot',
  aggregation: { type: 'Ema', halfLifeBlocks: 100 },
  scale: 12,
};
const contract = {
  actorType: 'System',
  mutability: 'Mutable',
  completionPolicy: 'CloseAfterProductiveCycle',
  trigger: {
    type: 'Immediate',
    sources: [{ type: 'OnObservationChange', feed }],
  },
  cooldownBlocks: 4,
  scheduleWindow: { start: 10, end: 1_000 },
  fundingPolicy: { type: 'RuntimePolicy' },
  steps: [
    {
      key: 'presentation-key-does-not-lower',
      preconditions: {
        type: 'AnyOf',
        clauses: [
          [
            {
              timing: 'Opening',
              predicate: {
                type: 'ObservationBelow',
                feed,
                threshold: '1500000000000',
                maxAgeBlocks: 20,
              },
            },
            {
              timing: 'Current',
              predicate: {
                type: 'BalanceAbove',
                asset: native,
                threshold: '100',
              },
            },
          ],
        ],
      },
      task: {
        type: 'SwapIn',
        assetIn: native,
        amountIn: { type: 'Fixed', value: '25' },
        assetOut: local,
        slippageParts: 10_000_000,
      },
      errorPolicy: { type: 'RetryLater', maxAttempts: 3 },
    },
  ],
};

const adapters = [
  {
    name: 'json',
    serialize: serializeActorConfigurationJson,
    parse: parseActorConfigurationJson,
  },
  {
    name: 'toml',
    serialize: serializeActorConfigurationToml,
    parse: parseActorConfigurationToml,
  },
  {
    name: 'markdown',
    serialize: serializeActorConfigurationMarkdown,
    parse: parseActorConfigurationMarkdown,
  },
];

test('JSON TOML and structured Markdown normalize to one configuration IR', () => {
  const ir = authoringContractToConfigurationIr(contract);
  for (const adapter of adapters) {
    const first = adapter.serialize(ir);
    const parsed = adapter.parse(first);
    assert.deepEqual(parsed, ir, adapter.name);
    assert.equal(adapter.serialize(parsed), first, adapter.name);
  }
  assert(
    !serializeActorConfigurationJson(ir).includes(
      'presentation-key-does-not-lower',
    ),
  );
});

test('syntax comments and Markdown prose do not alter lowering or Actor Contract identity', () => {
  const ir = authoringContractToConfigurationIr(contract);
  const sources = [
    parseActorConfigurationJson(serializeActorConfigurationJson(ir)),
    parseActorConfigurationToml(
      `# operator comment\n${serializeActorConfigurationToml(ir)}`,
    ),
    parseActorConfigurationMarkdown(
      serializeActorConfigurationMarkdown(ir).replace(
        '<!-- deos.actor.configuration-ir@1 -->',
        '<!-- deos.actor.configuration-ir@1 -->\n\nOperator prose is not executable.',
      ),
    ),
  ];
  const canonicalLowering = lowerActorAuthoringContract(contract);
  const canonicalArtifact = createActorArtifactFromAuthoring({
    contract: contract,
    metadataBytes,
    runtime,
  });
  const canonicalInspection = inspectActorContractArtifact(
    canonicalArtifact,
    metadataBytes,
    runtime,
  );
  assert.equal(canonicalInspection.valid, true);
  for (const parsed of sources) {
    assert.deepEqual(lowerActorConfigurationIr(parsed), canonicalLowering);
    const parsedContract = configurationIrToAuthoringContract(parsed);
    const artifact = createActorArtifactFromAuthoring({
      contract: parsedContract,
      metadataBytes,
      runtime,
    });
    assert.equal(artifact.contractId, canonicalArtifact.contractId);
    const inspection = inspectActorContractArtifact(
      artifact,
      metadataBytes,
      runtime,
    );
    assert.equal(inspection.valid, true);
    if (inspection.valid && canonicalInspection.valid) {
      assert.deepEqual(inspection.projection, canonicalInspection.projection);
    }
  }
});

test('structural diagnostics and diff remain path-addressed and deterministic', () => {
  const ir = authoringContractToConfigurationIr(contract);
  const changed = structuredClone(ir);
  changed.cooldownBlocks = 9;
  changed.steps[0].task.amountIn.value = '30';
  assert.deepEqual(diffActorConfigurationIr(ir, changed), [
    {
      path: '/cooldownBlocks',
      kind: 'Replaced',
      before: 4,
      after: 9,
    },
    {
      path: '/steps/0/task/amountIn/value',
      kind: 'Replaced',
      before: '25',
      after: '30',
    },
  ]);
  const invalid = structuredClone(ir);
  invalid.steps[0].errorPolicy.maxAttempts = 0;
  const diagnostics = diagnoseActorConfigurationIr(invalid);
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].severity, 'Error');
  assert.match(diagnostics[0].path, /steps\[0\]/);
  assert.match(diagnostics[0].message, /within 2\.\.10/);
});
