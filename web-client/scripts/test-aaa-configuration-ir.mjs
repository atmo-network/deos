/*
Domain: AAA configuration IR validation
Owns: Cross-format normalization, structural diagnostics/diff, canonical lowering, decode, and plan identity equivalence.
Excludes: Runtime submission, file-system loaders, UI presentation, and alternate execution semantics.
Zone: Web-client validation entrypoint; composes configuration IR with canonical authoring and artifact contracts.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  createAaaArtifactFromAuthoring,
  lowerAaaAuthoringProgram,
} from '../src/lib/automation/authoring.ts';
import {
  authoringProgramToConfigurationIr,
  configurationIrToAuthoringProgram,
  diagnoseAaaConfigurationIr,
  diffAaaConfigurationIr,
  lowerAaaConfigurationIr,
  parseAaaConfigurationJson,
  parseAaaConfigurationMarkdown,
  parseAaaConfigurationToml,
  serializeAaaConfigurationJson,
  serializeAaaConfigurationMarkdown,
  serializeAaaConfigurationToml,
} from '../src/lib/automation/configuration-ir.ts';
import { inspectAaaPlanArtifact } from '../src/lib/automation/plan-artifact.ts';

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
const program = {
  aaaType: 'System',
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
      conditionSet: {
        type: 'All',
        conditions: [
          {
            type: 'ObservationBelow',
            feed,
            threshold: '1500000000000',
            maxAgeBlocks: 20,
          },
          { type: 'BalanceAbove', asset: native, threshold: '100' },
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
    serialize: serializeAaaConfigurationJson,
    parse: parseAaaConfigurationJson,
  },
  {
    name: 'toml',
    serialize: serializeAaaConfigurationToml,
    parse: parseAaaConfigurationToml,
  },
  {
    name: 'markdown',
    serialize: serializeAaaConfigurationMarkdown,
    parse: parseAaaConfigurationMarkdown,
  },
];

test('JSON TOML and structured Markdown normalize to one configuration IR', () => {
  const ir = authoringProgramToConfigurationIr(program);
  for (const adapter of adapters) {
    const first = adapter.serialize(ir);
    const parsed = adapter.parse(first);
    assert.deepEqual(parsed, ir, adapter.name);
    assert.equal(adapter.serialize(parsed), first, adapter.name);
  }
  assert(
    !serializeAaaConfigurationJson(ir).includes(
      'presentation-key-does-not-lower',
    ),
  );
});

test('syntax comments and Markdown prose do not alter lowering or plan identity', () => {
  const ir = authoringProgramToConfigurationIr(program);
  const sources = [
    parseAaaConfigurationJson(serializeAaaConfigurationJson(ir)),
    parseAaaConfigurationToml(
      `# operator comment\n${serializeAaaConfigurationToml(ir)}`,
    ),
    parseAaaConfigurationMarkdown(
      serializeAaaConfigurationMarkdown(ir).replace(
        '<!-- deos.aaa.configuration-ir@1 -->',
        '<!-- deos.aaa.configuration-ir@1 -->\n\nOperator prose is not executable.',
      ),
    ),
  ];
  const canonicalLowering = lowerAaaAuthoringProgram(program);
  const canonicalArtifact = createAaaArtifactFromAuthoring({
    program,
    metadataBytes,
    runtime,
  });
  const canonicalInspection = inspectAaaPlanArtifact(
    canonicalArtifact,
    metadataBytes,
    runtime,
  );
  assert.equal(canonicalInspection.valid, true);
  for (const parsed of sources) {
    assert.deepEqual(lowerAaaConfigurationIr(parsed), canonicalLowering);
    const parsedProgram = configurationIrToAuthoringProgram(parsed);
    const artifact = createAaaArtifactFromAuthoring({
      program: parsedProgram,
      metadataBytes,
      runtime,
    });
    assert.equal(artifact.planId, canonicalArtifact.planId);
    const inspection = inspectAaaPlanArtifact(artifact, metadataBytes, runtime);
    assert.equal(inspection.valid, true);
    if (inspection.valid && canonicalInspection.valid) {
      assert.deepEqual(inspection.projection, canonicalInspection.projection);
    }
  }
});

test('structural diagnostics and diff remain path-addressed and deterministic', () => {
  const ir = authoringProgramToConfigurationIr(program);
  const changed = structuredClone(ir);
  changed.cooldownBlocks = 9;
  changed.steps[0].task.amountIn.value = '30';
  assert.deepEqual(diffAaaConfigurationIr(ir, changed), [
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
  const diagnostics = diagnoseAaaConfigurationIr(invalid);
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].severity, 'Error');
  assert.match(diagnostics[0].path, /steps\[0\]/);
  assert.match(diagnostics[0].message, /within 2\.\.10/);
});
