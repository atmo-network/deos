/*
Domain: AAA normative surface drift gate
Owns: Comparison of the named public contract in specification Sections 11-12
with the metadata-derived ABI manifest (calls, events, errors, variants,
constants). Detects missing/extra variants, field-name drift, stale task
shapes, stale bounds, and stale host-contract declarations.
Excludes: Numeric index pinning (metadata-owned before launch), semantic
classification, release identity approval.
Zone: Web-client validation entrypoint; reads the canonical specification and
the generated ABI manifest.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '../..');
const specPath = path.join(
  repoRoot,
  'template/pallets/aaa/docs/specification.en.md',
);
const manifestPath = path.join(
  scriptDir,
  '../src/lib/automation/aaa-abi-manifest.json',
);

const spec = await readFile(specPath, 'utf8');
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
function specSection(startMarker, endMarker) {
  const start = spec.indexOf(startMarker);
  assert.ok(start >= 0, `spec marker not found: ${startMarker}`);
  const end = spec.indexOf(endMarker, start);
  assert.ok(end >= 0, `spec end marker not found: ${endMarker}`);
  return spec.slice(start, end);
}

function rustVariantNames(block) {
  const names = [];
  for (const line of block.split('\n')) {
    if (line.includes('```')) continue;
    // Events are flush-left; enum variants are four-space indented.
    const match = line.match(/^\s*([A-Z][A-Za-z0-9_]*)\s*(?:[,({]|$)/);
    if (match) names.push(match[1]);
  }
  return names;
}

function specEvents() {
  const block = specSection('### 11.1 Events', '### 11.2');
  return block
    .split('\n')
    .map((line) => line.match(/^([A-Z][A-Za-z0-9_]*)\s*\{([^}]*)\}$/))
    .filter(Boolean)
    .map((match) => ({
      name: match[1],
      fields: match[2]
        .split(',')
        .map((field) => field.trim())
        .filter(Boolean)
        .map((field) => field.split(':')[0].trim()),
    }));
}

function specErrors() {
  const block = specSection('### 12.2 Errors', '## 13');
  return rustVariantNames(block);
}

function specTypeBlock(name) {
  // Exact enum boundary: match the enum name followed by optional generics and {
  // but not a longer identifier such as ConditionSet when searching Condition.
  const marker = new RegExp(`enum ${name}(?:<[^>]*>)?\\s*\\{`);
  const match = spec.match(marker);
  assert.ok(match, `spec enum not found: ${name}`);
  const start = match.index;
  const from = start + match[0].length;
  const lines = spec.slice(from).split('\n');
  // The enum's own opening brace was consumed by the header match; variants are
  // single-line or multi-line and may contain nested braces in generics.
  let depth = 1;
  const blockLines = [];
  for (const line of lines) {
    depth += (line.match(/{/g) ?? []).length - (line.match(/}/g) ?? []).length;
    blockLines.push(line);
    if (depth === 0 && blockLines.length > 1) break;
  }
  return blockLines.join('\n');
}

function specTypeSurface(name) {
  const surface = specTypeBlock(name)
    .split('\n')
    .map((line) =>
      line.match(
        /^\s*([A-Z][A-Za-z0-9_]*)(?:\s*\{([^}]*)\}|\s*\(([^)]*)\))?\s*,?$/,
      ),
    )
    .filter(Boolean)
    .map((match) => ({
      name: match[1],
      fields: match[2]
        ? [...match[2].matchAll(/(?:^|,)\s*([a-z][A-Za-z0-9_]*)\s*:/g)].map(
            (field) => field[1],
          )
        : match[3]
          ? match[3]
              .split(',')
              .map((field) => field.trim())
              .filter(Boolean)
              .map(() => '<unnamed>')
          : [],
    }));
  assert.ok(surface.length > 0, `enum ${name} must declare variants`);
  return surface;
}

function specTypeVariants(name) {
  return specTypeSurface(name).map((variant) => variant.name);
}

const failures = [];

function duplicates(names) {
  return [
    ...new Set(names.filter((name, index) => names.indexOf(name) !== index)),
  ];
}

function orderedDiff(label, expected, actual) {
  const expectedDuplicates = duplicates(expected);
  const actualDuplicates = duplicates(actual);
  if (expectedDuplicates.length > 0) {
    failures.push(
      `${label}: duplicate specification variants: ${expectedDuplicates.join(', ')}`,
    );
  }
  if (actualDuplicates.length > 0) {
    failures.push(
      `${label}: duplicate metadata variants: ${actualDuplicates.join(', ')}`,
    );
  }
  const length = Math.max(expected.length, actual.length);
  for (let index = 0; index < length; index += 1) {
    if (expected[index] !== actual[index]) {
      failures.push(
        `${label}: ordered drift at index ${index}: specification=${expected[index] ?? '<missing>'}, metadata=${actual[index] ?? '<missing>'}`,
      );
    }
  }
}

const expectedEvents = specEvents();
const actualEvents = manifest.pallet.events;
orderedDiff(
  'events',
  expectedEvents.map((event) => event.name),
  actualEvents.map((event) => event.name),
);
for (
  let index = 0;
  index < Math.min(expectedEvents.length, actualEvents.length);
  index += 1
) {
  if (expectedEvents[index].name !== actualEvents[index].name) continue;
  orderedDiff(
    `event ${expectedEvents[index].name} fields`,
    expectedEvents[index].fields,
    actualEvents[index].fields.map((field) => field.name ?? '<unnamed>'),
  );
}

orderedDiff(
  'errors',
  specErrors(),
  manifest.pallet.errors.map((entry) => entry.name),
);

for (const enumName of ['Task', 'Condition', 'AmountResolution']) {
  const expected = specTypeSurface(enumName);
  const matches = manifest.types.filter(
    (entry) => entry.path?.join('::') === `pallet_aaa::types::${enumName}`,
  );
  if (matches.length !== 1) {
    failures.push(
      `${enumName} metadata path must resolve exactly once, found ${matches.length}`,
    );
    continue;
  }
  const actual = matches[0].def?.tag === 'variant' ? matches[0].def.value : [];
  orderedDiff(
    `${enumName} variants`,
    expected.map((variant) => variant.name),
    actual.map((variant) => variant.name),
  );
  for (
    let index = 0;
    index < Math.min(expected.length, actual.length);
    index += 1
  ) {
    if (expected[index].name !== actual[index].name) continue;
    orderedDiff(
      `${enumName}.${expected[index].name} fields`,
      expected[index].fields,
      (actual[index].fields ?? []).map((field) => field.name ?? '<unnamed>'),
    );
  }
}

// ConditionSet variants live in a spec enum and appear as an ABI variant type.
const conditionSetExpected = specTypeVariants('ConditionSet');
function variantNamesOfType(typeEntry) {
  const def = typeEntry?.def;
  return def?.tag === 'variant'
    ? (def.value ?? []).map((variant) => variant.name)
    : [];
}
const conditionSetTypes = manifest.types.filter(
  (entry) => entry.path?.join('::') === 'pallet_aaa::types::ConditionSet',
);
if (conditionSetTypes.length !== 1) {
  failures.push(
    `ConditionSet metadata path must resolve exactly once, found ${conditionSetTypes.length}`,
  );
}
const conditionSetActual = variantNamesOfType(conditionSetTypes[0]);
orderedDiff('ConditionSet variants', conditionSetExpected, conditionSetActual);

const constantNames = new Set(
  manifest.pallet.constants.map((entry) => entry.name),
);
const expectedConstants = new Set([
  'MaxExecutionPlanSteps',
  'MaxRetryAttempts',
  'MaxOwnerSlots',
  'MaxActiveActors',
  'MaxQueueLength',
  'MaxWakeupsPerBlock',
  'MaxObservationFanoutPagesPerBlock',
  'MaxTriggerSources',
  'MaxContinuationSnapshotEntries',
  'MinUserBalance',
  'MinWindowLength',
  'MaxExecutionDelayBlocks',
  'NativeAssetId',
]);
const missingConstants = [...expectedConstants].filter(
  (name) => !constantNames.has(name),
);
if (missingConstants.length > 0) {
  failures.push(
    `runtime constants: missing from metadata: ${missingConstants.join(', ')}`,
  );
}

if (failures.length > 0) {
  console.error('AAA normative surface drift detected:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('AAA normative surface drift gate passed');
