/*
Domain: Actors metadata-derived ABI manifest
Owns: Deterministic projection of Actors calls, events, errors, storage, constants, and recursively reachable SCALE types.
Excludes: Semantic classification, runtime behavior, compatibility acceptance, and release identity approval.
Zone: Web-client generation entrypoint; exact runtime metadata remains authoritative.
*/
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { format } from 'prettier';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webClientRoot = path.resolve(scriptDir, '..');
const metadataPath = path.join(webClientRoot, '.papi/metadata/deos.scale');
const outputPath = path.join(
  webClientRoot,
  'src/lib/automation/actors-abi-manifest.json',
);
const boundsOutputPath = path.join(
  webClientRoot,
  'src/lib/automation/actors-protocol-bounds.generated.ts',
);
const check = process.argv.includes('--check');

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value == null || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([, entry]) => entry !== undefined)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, canonical(entry)]),
  );
}

function fieldProjection(field) {
  return canonical({
    name: field.name,
    type: field.type,
    typeName: field.typeName,
  });
}

function typeProjection(entry) {
  return canonical({
    id: entry.id,
    path: entry.path,
    params: entry.params?.map((parameter) => ({
      name: parameter.name,
      type: parameter.type,
    })),
    def: entry.def,
  });
}

function variantProjection(metadata, typeId, label) {
  const entry = metadata.lookup[typeId];
  if (entry?.id !== typeId || entry.def.tag !== 'variant') {
    throw new Error(`${label} type ${typeId} must be a SCALE variant`);
  }
  return entry.def.value.map((variant) =>
    canonical({
      name: variant.name,
      index: variant.index,
      fields: variant.fields.map(fieldProjection),
    }),
  );
}

function storageTypeIds(type) {
  if (type.tag === 'plain') return [type.value];
  if (type.tag === 'map') return [type.value.key, type.value.value];
  throw new Error(`Unsupported storage type: ${type.tag}`);
}

function referencedTypeIds(entry) {
  const ids = entry.params.flatMap((parameter) =>
    parameter.type === undefined ? [] : [parameter.type],
  );
  const { def } = entry;
  if (def.tag === 'composite') {
    ids.push(...def.value.map((field) => field.type));
  } else if (def.tag === 'variant') {
    ids.push(
      ...def.value.flatMap((variant) =>
        variant.fields.map((field) => field.type),
      ),
    );
  } else if (def.tag === 'sequence' || def.tag === 'compact') {
    ids.push(def.value);
  } else if (def.tag === 'array') {
    ids.push(def.value.type);
  } else if (def.tag === 'tuple') {
    ids.push(...def.value);
  } else if (def.tag === 'bitSequence') {
    ids.push(def.value.bitStoreType, def.value.bitOrderType);
  }
  return ids;
}

function reachableTypes(metadata, roots) {
  const pending = [...new Set(roots)];
  const visited = new Set();
  while (pending.length > 0) {
    const typeId = pending.pop();
    if (visited.has(typeId)) continue;
    const entry = metadata.lookup[typeId];
    if (entry?.id !== typeId)
      throw new Error(`Missing metadata type ${typeId}`);
    visited.add(typeId);
    for (const referenced of referencedTypeIds(entry)) {
      if (!visited.has(referenced)) pending.push(referenced);
    }
  }
  return [...visited]
    .sort((left, right) => left - right)
    .map((typeId) => typeProjection(metadata.lookup[typeId]));
}

async function buildManifest() {
  const metadataBytes = new Uint8Array(await readFile(metadataPath));
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const pallet = metadata.pallets.find(
    (candidate) => candidate.name === 'Actors',
  );
  if (
    pallet == null ||
    pallet.calls == null ||
    pallet.events == null ||
    pallet.errors == null
  ) {
    throw new Error(
      'Runtime metadata must expose Actors calls, events, and errors',
    );
  }
  const storage = (pallet.storage?.items ?? []).map((item) =>
    canonical({
      name: item.name,
      modifier: item.modifier,
      type: item.type,
    }),
  );
  const constants = pallet.constants.map((constant) =>
    canonical({
      name: constant.name,
      type: constant.type,
      value: constant.value,
    }),
  );
  const roots = [
    pallet.calls.type,
    pallet.events.type,
    pallet.errors.type,
    ...storage.flatMap((item) => storageTypeIds(item.type)),
    ...constants.map((constant) => constant.type),
    ...pallet.associatedTypes.map((associated) => associated.type),
  ];
  return canonical({
    format: 'deos.actor.abi-manifest',
    formatVersion: 1,
    metadata: {
      sha256: createHash('sha256').update(metadataBytes).digest('hex'),
      version: metadata.version,
    },
    pallet: {
      name: pallet.name,
      index: pallet.index,
      calls: variantProjection(metadata, pallet.calls.type, 'Actors calls'),
      events: variantProjection(metadata, pallet.events.type, 'Actors events'),
      errors: variantProjection(metadata, pallet.errors.type, 'Actors errors'),
      storage,
      constants,
      associatedTypes: pallet.associatedTypes.map((associated) =>
        canonical({ name: associated.name, type: associated.type }),
      ),
    },
    types: reachableTypes(metadata, roots),
  });
}

function decodeUnsignedConstant(manifest, name, bytes) {
  const constant = manifest.pallet.constants.find(
    (candidate) => candidate.name === name,
  );
  if (
    !constant ||
    !new RegExp(`^0x[0-9a-f]{${bytes * 2}}$`, 'i').test(constant.value)
  ) {
    throw new Error(
      `Actors ABI manifest lacks a valid ${bytes}-byte ${name} constant`,
    );
  }
  const encoded = Buffer.from(constant.value.slice(2), 'hex');
  const decoded =
    bytes === 1
      ? BigInt(encoded.readUInt8(0))
      : bytes === 4
        ? BigInt(encoded.readUInt32LE(0))
        : encoded.readBigUInt64LE(0);
  const value = Number(decoded);
  if (!Number.isSafeInteger(value)) {
    throw new Error(
      `Actors constant ${name} exceeds browser-safe integer range`,
    );
  }
  return value;
}

const manifest = await buildManifest();
const generated = await format(JSON.stringify(manifest), { parser: 'json' });
const generatedBounds = await format(
  `/* Generated from Actors runtime metadata ${manifest.metadata.sha256}; do not edit. */\n` +
    `export const ACTORS_MAX_CONTRACT_STEPS = ${decodeUnsignedConstant(manifest, 'MaxContractSteps', 4)};\n` +
    `export const ACTORS_MAX_EXECUTION_DELAY_BLOCKS = ${decodeUnsignedConstant(manifest, 'MaxExecutionDelayBlocks', 4)};\n` +
    `export const ACTORS_MAX_TEMPORAL_DELAY_TICKS = ${decodeUnsignedConstant(manifest, 'MaxTemporalDelayTicks', 8)};\n` +
    `export const ACTORS_MAX_RETRY_ATTEMPTS = ${decodeUnsignedConstant(manifest, 'MaxRetryAttempts', 4)};\n` +
    `export const ACTORS_MAX_OPENING_SNAPSHOT_ENTRIES = ${decodeUnsignedConstant(manifest, 'MaxOpeningSnapshotEntries', 4)};\n` +
    `export const ACTORS_MAX_OPENING_PREDICATE_RESULTS = ${decodeUnsignedConstant(manifest, 'MaxOpeningPredicateResults', 4)};\n` +
    `export const ACTORS_MAX_PRECONDITION_CLAUSES = ${decodeUnsignedConstant(manifest, 'MaxPreconditionClauses', 4)};\n` +
    `export const ACTORS_MAX_PREDICATES_PER_CLAUSE = ${decodeUnsignedConstant(manifest, 'MaxPredicatesPerClause', 4)};\n` +
    `export const ACTORS_MAX_PREDICATES_PER_STEP = ${decodeUnsignedConstant(manifest, 'MaxPredicatesPerStep', 4)};\n` +
    `export const ACTORS_MAX_OWNER_SLOTS = ${decodeUnsignedConstant(manifest, 'MaxOwnerSlots', 1)};\n`,
  { parser: 'typescript' },
);
if (check) {
  const [existing, existingBounds] = await Promise.all([
    readFile(outputPath, 'utf8').catch(() => ''),
    readFile(boundsOutputPath, 'utf8').catch(() => ''),
  ]);
  if (existing !== generated || existingBounds !== generatedBounds) {
    console.error(
      'Actors ABI artifacts are stale; run npm run generate:actors-abi',
    );
    process.exit(1);
  }
  console.log('Actors ABI manifest and compact bounds are current');
} else {
  await Promise.all([
    writeFile(outputPath, generated),
    writeFile(boundsOutputPath, generatedBounds),
  ]);
  console.log(`Wrote ${path.relative(webClientRoot, outputPath)}`);
  console.log(`Wrote ${path.relative(webClientRoot, boundsOutputPath)}`);
}
