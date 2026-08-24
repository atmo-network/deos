/*
Domain: Actors normative surface drift gate
Owns: Comparison of the named public contract in specification Sections 3 and
7-10 with the metadata-derived ABI manifest (calls, events, errors, variants,
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
  'template/pallets/actors/docs/specification.en.md',
);
const manifestPath = path.join(
  scriptDir,
  '../src/lib/automation/actors-abi-manifest.json',
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
  return specTypeSurface('Event');
}

function specErrors() {
  const block = specSection(
    '### 12.4 Errors and projections',
    '## 13. Storage, upgrades, configuration, and conformance',
  );
  const body = block.match(/enum Error\s*\{([\s\S]*?)\}/)?.[1];
  assert.ok(body, 'spec Error enum body is missing');
  return splitTopLevel(body)
    .map((variant) => variant.match(/^([A-Z][A-Za-z0-9_]*)/)?.[1])
    .filter(Boolean);
}

function splitTopLevel(value) {
  const parts = [];
  let start = 0;
  const closing = { '<': '>', '(': ')', '{': '}', '[': ']' };
  const stack = [];
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (closing[character]) stack.push(closing[character]);
    else if (character === stack.at(-1)) stack.pop();
    else if (character === ',' && stack.length === 0) {
      parts.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(value.slice(start).trim());
  return parts.filter(Boolean);
}

function specTypeBlock(name) {
  // Match the exact enum name, not a longer identifier such as ConditionSet
  // when searching for Predicate, then extract its balanced body.
  const marker = new RegExp(`enum ${name}(?:<[^>]*>)?\\s*\\{`);
  const match = spec.match(marker);
  assert.ok(match, `spec enum not found: ${name}`);
  const from = match.index + match[0].length;
  let depth = 1;
  for (let index = from; index < spec.length; index += 1) {
    if (spec[index] === '{') depth += 1;
    if (spec[index] === '}') depth -= 1;
    if (depth === 0) return spec.slice(from, index);
  }
  assert.fail(`spec enum is not closed: ${name}`);
}

function specTypeSurface(name) {
  const surface = splitTopLevel(specTypeBlock(name)).map((variant) => {
    const match = variant.match(
      /^([A-Z][A-Za-z0-9_]*)(?:\s*\{([\s\S]*)\}|\s*\(([\s\S]*)\))?$/,
    );
    assert.ok(match, `invalid ${name} variant declaration: ${variant}`);
    return {
      name: match[1],
      fields: match[2]
        ? splitTopLevel(match[2]).map((field) => field.split(':')[0].trim())
        : match[3]
          ? splitTopLevel(match[3]).map(() => '<unnamed>')
          : [],
    };
  });
  assert.ok(surface.length > 0, `enum ${name} must declare variants`);
  return surface;
}

function specTypeVariants(name) {
  return specTypeSurface(name).map((variant) => variant.name);
}

function specStructFields(name) {
  const marker = new RegExp(`struct ${name}(?:<[^>]*>)?\\s*\\{`);
  const match = spec.match(marker);
  assert.ok(match, `spec struct not found: ${name}`);
  const from = match.index + match[0].length;
  let depth = 1;
  for (let index = from; index < spec.length; index += 1) {
    if (spec[index] === '{') depth += 1;
    if (spec[index] === '}') depth -= 1;
    if (depth === 0) {
      return splitTopLevel(spec.slice(from, index)).map((field) =>
        field.split(':')[0].trim(),
      );
    }
  }
  assert.fail(`spec struct is not closed: ${name}`);
}

function specCalls() {
  const block = specSection(
    '### 12.1 Calls and authorization',
    '### 12.2 Events',
  );
  const contract = block.match(/```text\n([\s\S]*?)```/);
  assert.ok(contract, 'calls contract block not found');
  return [...contract[1].matchAll(/\b[a-z][a-z0-9_]+\b/g)].map(
    (match) => match[0],
  );
}

const failures = [];

function duplicates(names) {
  return [
    ...new Set(names.filter((name, index) => names.indexOf(name) !== index)),
  ];
}

function unorderedDiff(label, expected, actual) {
  const expectedDuplicates = duplicates(expected);
  const actualDuplicates = duplicates(actual);
  if (expectedDuplicates.length > 0) {
    failures.push(
      `${label}: duplicate specification entries: ${expectedDuplicates.join(', ')}`,
    );
  }
  if (actualDuplicates.length > 0) {
    failures.push(
      `${label}: duplicate metadata entries: ${actualDuplicates.join(', ')}`,
    );
  }
  const expectedSet = new Set(expected);
  const actualSet = new Set(actual);
  const missing = expected.filter((name) => !actualSet.has(name));
  const extra = actual.filter((name) => !expectedSet.has(name));
  if (missing.length > 0)
    failures.push(`${label}: missing: ${missing.join(', ')}`);
  if (extra.length > 0) failures.push(`${label}: extra: ${extra.join(', ')}`);
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

const headingNumbers = new Set(
  [...spec.matchAll(/^#{2,3} ([0-9]+(?:\.[0-9]+)?)\b/gm)].map(
    (match) => match[1],
  ),
);
for (const match of spec.matchAll(/(?:\bSections? |§)([0-9]+(?:\.[0-9]+)?)/g)) {
  if (!headingNumbers.has(match[1])) {
    failures.push(`section reference: missing Section ${match[1]}`);
  }
}
for (const [label, names] of [
  ['calls', specCalls()],
  ['events', specEvents().map((event) => event.name)],
  ['errors', specErrors()],
]) {
  const repeated = duplicates(names);
  if (repeated.length > 0) {
    failures.push(
      `${label}: duplicate specification entries: ${repeated.join(', ')}`,
    );
  }
}
for (const typeName of ['Trigger', 'Task', 'Predicate', 'AmountResolution']) {
  const repeated = duplicates(specTypeVariants(typeName));
  if (repeated.length > 0) {
    failures.push(
      `${typeName}: duplicate specification variants: ${repeated.join(', ')}`,
    );
  }
}

if (process.argv.includes('--spec-only')) {
  if (failures.length > 0) {
    console.error('Actors specification audit failed:');
    for (const failure of failures) console.error(`- ${failure}`);
    process.exit(1);
  }
  console.log('Actors specification audit passed');
  process.exit(0);
}

unorderedDiff(
  'calls',
  specCalls(),
  manifest.pallet.calls.map((entry) => entry.name),
);

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

function isActorsType(entry, name) {
  return (
    entry.path?.[0] === 'pallet_deos_actors' &&
    entry.path?.[1] === 'types' &&
    entry.path?.at(-1) === name
  );
}

for (const structName of ['ActorContract', 'Step', 'Precondition']) {
  const matches = manifest.types.filter((entry) =>
    isActorsType(entry, structName),
  );
  if (matches.length !== 1) {
    failures.push(
      `${structName} metadata path must resolve exactly once, found ${matches.length}`,
    );
    continue;
  }
  const actual =
    matches[0].def?.tag === 'composite' ? matches[0].def.value : [];
  orderedDiff(
    `${structName} fields`,
    specStructFields(structName),
    actual.map((field) => field.name ?? '<unnamed>'),
  );
}

for (const [specEnumName, metadataEnumName] of [
  ['Trigger', 'Trigger'],
  ['Task', 'Task'],
  ['Predicate', 'Predicate'],
  ['AmountResolution', 'AmountResolution'],
]) {
  const expected = specTypeSurface(specEnumName);
  const matches = manifest.types.filter((entry) =>
    isActorsType(entry, metadataEnumName),
  );
  if (matches.length !== 1) {
    failures.push(
      `${metadataEnumName} metadata path must resolve exactly once, found ${matches.length}`,
    );
    continue;
  }
  const actual = matches[0].def?.tag === 'variant' ? matches[0].def.value : [];
  orderedDiff(
    `${specEnumName} variants`,
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
      `${specEnumName}.${expected[index].name} fields`,
      expected[index].fields,
      (actual[index].fields ?? []).map((field) => field.name ?? '<unnamed>'),
    );
  }
}

const forbiddenTypeNames = new Set(['ResolutionSurface', 'ExecutionPlanOf']);
const staleTypePaths = manifest.types
  .filter((entry) => entry.path?.some((part) => forbiddenTypeNames.has(part)))
  .map((entry) => entry.path.join('::'));
if (staleTypePaths.length > 0) {
  failures.push(
    `stale compatibility types remain: ${staleTypePaths.join(', ')}`,
  );
}

const pluralPreconditionTypes = manifest.types.filter((entry) =>
  entry.path?.includes('Preconditions'),
);
if (pluralPreconditionTypes.length > 0) {
  failures.push('plural Preconditions compatibility type remains in metadata');
}

const constantNames = new Set(
  manifest.pallet.constants.map((entry) => entry.name),
);
const expectedConstants = new Set([
  'MaxContractSteps',
  'MaxRetryAttempts',
  'MaxOwnerSlots',
  'MaxActiveActors',
  'MaxQueueLength',
  'MaxWakeupsPerBlock',
  'MaxObservationFanoutPagesPerBlock',
  'MaxOpeningSnapshotEntries',
  'MaxOpeningPredicateResults',
  'MaxPreconditionClauses',
  'MaxPredicatesPerClause',
  'MaxPredicatesPerStep',
  'MinUserBalance',
  'MinWindowLength',
  'MaxExecutionDelayBlocks',
  'TargetBlockTime',
  'FeeNativeAssetId',
]);
const missingConstants = [...expectedConstants].filter(
  (name) => !constantNames.has(name),
);
if (missingConstants.length > 0) {
  failures.push(
    `runtime constants: missing from metadata: ${missingConstants.join(', ')}`,
  );
}
const forbiddenConstants = [
  'MaxContinuationSnapshotEntries',
  'MaxExecutionPlanSteps',
];
const staleConstants = forbiddenConstants.filter((name) =>
  constantNames.has(name),
);
if (staleConstants.length > 0) {
  failures.push(
    `runtime constants: stale compatibility names remain: ${staleConstants.join(', ')}`,
  );
}

if (failures.length > 0) {
  console.error('Actors normative surface drift detected:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('Actors normative surface drift gate passed');
