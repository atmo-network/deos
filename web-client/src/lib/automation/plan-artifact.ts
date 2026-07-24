/*
Domain: AAA control-plane artifacts
Owns: Metadata-bound plan identity, SCALE round trips, lossless projections, and structural diffs.
Excludes: Runtime state queries, amount/fee forecasts, simulation, governance submission, and history persistence.
Zone: Automation domain capability; safe for adapters, stores, and widgets to import.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  type UnifiedMetadata,
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import { blake2AsHex } from '@polkadot/util-crypto';

export const AAA_PLAN_FORMAT = 'deos.aaa.plan' as const;
export const AAA_PLAN_FORMAT_VERSION = 1 as const;

export type AaaPlanType = 'User' | 'System';
export type AaaPlanMutability = 'Mutable' | 'Immutable';
export type AaaPlanHex = `0x${string}`;

export type AaaPlanArtifact = {
  format: typeof AAA_PLAN_FORMAT;
  formatVersion: typeof AAA_PLAN_FORMAT_VERSION;
  genesisHash: AaaPlanHex;
  specVersion: number;
  transactionVersion: number;
  metadataHash: AaaPlanHex;
  aaaType: AaaPlanType;
  mutability: AaaPlanMutability;
  programScale: AaaPlanHex;
  planId: AaaPlanHex;
};

export type AaaPlanProjection =
  | null
  | boolean
  | string
  | { $bytes: AaaPlanHex }
  | { $integer: string; $runtimeType: 'number' | 'bigint' }
  | { $none: true }
  | AaaPlanProjection[]
  | { [key: string]: AaaPlanProjection };

export type AaaPlanInspection =
  | {
      valid: true;
      artifact: AaaPlanArtifact;
      projection: AaaPlanProjection;
      runtimeValue: unknown;
    }
  | { valid: false; errors: string[] };

export type AaaPlanRuntimeIdentity = {
  genesisHash: AaaPlanHex;
  specVersion: number;
  transactionVersion: number;
};

export type AaaPlanDiff =
  | {
      kind: 'add';
      path: string;
      value: AaaPlanProjection;
    }
  | {
      kind: 'remove';
      path: string;
      value: AaaPlanProjection;
    }
  | {
      kind: 'replace';
      path: string;
      before: AaaPlanProjection;
      after: AaaPlanProjection;
    }
  | {
      kind: 'move';
      path: string;
      from: string;
      value: AaaPlanProjection;
    };

export type AaaPlanDiffResult =
  | { compatible: true; changes: AaaPlanDiff[] }
  | {
      compatible: false;
      reason: 'IncompatibleUntilRebound';
      mismatches: Array<'genesisHash' | 'metadataHash'>;
    };

const PLAN_DOMAIN_BYTES = new TextEncoder().encode('deos:aaa-plan:v1');
const HEX_PATTERN = /^0x(?:[0-9a-f]{2})*$/;
const HASH_PATTERN = /^0x[0-9a-f]{64}$/;

function concatBytes(parts: Uint8Array[]) {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const result = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

function encodeLeU32(value: number) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new Error('Runtime versions must be unsigned 32-bit integers');
  }
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value, true);
  return bytes;
}

function bytesToHex(bytes: Uint8Array): AaaPlanHex {
  let value = '0x';
  for (const byte of bytes) value += byte.toString(16).padStart(2, '0');
  return value as AaaPlanHex;
}

function hexToBytes(value: string) {
  if (!HEX_PATTERN.test(value))
    throw new Error('Expected canonical lowercase hex bytes');
  const bytes = new Uint8Array((value.length - 2) / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(
      value.slice(2 + index * 2, 4 + index * 2),
      16,
    );
  }
  return bytes;
}

function metadataEntry(metadata: UnifiedMetadata, path: string) {
  const matches = metadata.lookup.filter(
    (entry) => entry.path?.join('::') === path,
  );
  if (matches.length !== 1) {
    throw new Error(`Runtime metadata must expose exactly one ${path} type`);
  }
  return matches[0];
}

function enumDiscriminant(
  metadata: UnifiedMetadata,
  path: string,
  variantName: string,
) {
  const entry = metadataEntry(metadata, path);
  if (entry.def.tag !== 'variant')
    throw new Error(`${path} must remain a SCALE enum`);
  const variant = entry.def.value.find(
    (candidate) => candidate.name === variantName,
  );
  if (variant == null || variant.fields.length !== 0 || variant.index > 0xff) {
    throw new Error(
      `${path}.${variantName} must remain a fieldless SCALE variant`,
    );
  }
  return Uint8Array.of(variant.index);
}

function programCodec(metadata: UnifiedMetadata) {
  const entry = metadataEntry(metadata, 'pallet_aaa::types::ProgramInput');
  return getDynamicBuilder(getLookupFn(metadata)).buildDefinition(entry.id);
}

function decodeProgram(metadata: UnifiedMetadata, programScale: AaaPlanHex) {
  const codec = programCodec(metadata);
  const sourceBytes = hexToBytes(programScale);
  const runtimeValue = codec.dec(sourceBytes);
  const roundTrip = codec.enc(runtimeValue);
  if (bytesToHex(roundTrip) !== programScale) {
    throw new Error(
      'ProgramInput must decode and re-encode to the exact SCALE bytes',
    );
  }
  return { runtimeValue, projection: projectRuntimeValue(runtimeValue) };
}

function projectRuntimeValue(value: unknown): AaaPlanProjection {
  if (value === undefined) return { $none: true };
  if (
    value === null ||
    typeof value === 'boolean' ||
    typeof value === 'string'
  ) {
    return value;
  }
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value))
      throw new Error('Unsafe numeric runtime projection');
    return { $integer: value.toString(), $runtimeType: 'number' };
  }
  if (typeof value === 'bigint') {
    return { $integer: value.toString(), $runtimeType: 'bigint' };
  }
  if (value instanceof Uint8Array) return { $bytes: bytesToHex(value) };
  if (Array.isArray(value)) return value.map(projectRuntimeValue);
  if (typeof value === 'object') {
    const projection: Record<string, AaaPlanProjection> = {};
    for (const key of Object.keys(value).sort()) {
      projection[key] = projectRuntimeValue(
        (value as Record<string, unknown>)[key],
      );
    }
    return projection;
  }
  throw new Error(`Unsupported runtime projection value: ${typeof value}`);
}

function planIdBytes(
  metadata: UnifiedMetadata,
  artifact: Omit<AaaPlanArtifact, 'planId'>,
) {
  return concatBytes([
    PLAN_DOMAIN_BYTES,
    encodeLeU32(artifact.specVersion),
    encodeLeU32(artifact.transactionVersion),
    hexToBytes(artifact.genesisHash),
    hexToBytes(artifact.metadataHash),
    enumDiscriminant(metadata, 'pallet_aaa::types::AaaType', artifact.aaaType),
    enumDiscriminant(
      metadata,
      'pallet_aaa::types::Mutability',
      artifact.mutability,
    ),
    hexToBytes(artifact.programScale),
  ]);
}

function calculatePlanId(
  metadata: UnifiedMetadata,
  artifact: Omit<AaaPlanArtifact, 'planId'>,
) {
  return blake2AsHex(planIdBytes(metadata, artifact), 256) as AaaPlanHex;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

function isU32(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isSafeInteger(value) &&
    value >= 0 &&
    value <= 0xffff_ffff
  );
}

function parseArtifact(
  value: unknown,
  errors: string[],
): AaaPlanArtifact | null {
  if (!isRecord(value)) {
    errors.push('Artifact must be an object');
    return null;
  }
  if (value.format !== AAA_PLAN_FORMAT)
    errors.push(`format must be ${AAA_PLAN_FORMAT}`);
  if (value.formatVersion !== AAA_PLAN_FORMAT_VERSION) {
    errors.push(`formatVersion must be ${AAA_PLAN_FORMAT_VERSION}`);
  }
  for (const field of ['genesisHash', 'metadataHash', 'planId'] as const) {
    if (typeof value[field] !== 'string' || !HASH_PATTERN.test(value[field])) {
      errors.push(`${field} must be canonical lowercase 32-byte hex`);
    }
  }
  if (
    typeof value.programScale !== 'string' ||
    !HEX_PATTERN.test(value.programScale)
  ) {
    errors.push('programScale must be canonical lowercase hex');
  }
  if (!isU32(value.specVersion))
    errors.push('specVersion must be an unsigned u32');
  if (!isU32(value.transactionVersion)) {
    errors.push('transactionVersion must be an unsigned u32');
  }
  if (value.aaaType !== 'User' && value.aaaType !== 'System') {
    errors.push('aaaType must be User or System');
  }
  if (value.mutability !== 'Mutable' && value.mutability !== 'Immutable') {
    errors.push('mutability must be Mutable or Immutable');
  }
  return errors.length === 0 ? (value as AaaPlanArtifact) : null;
}

export function encodeAaaProgramValue(
  metadataBytes: Uint8Array,
  runtimeValue: unknown,
): AaaPlanHex {
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const programScale = bytesToHex(programCodec(metadata).enc(runtimeValue));
  decodeProgram(metadata, programScale);
  return programScale;
}

export function createAaaPlanArtifact(input: {
  metadataBytes: Uint8Array;
  runtime: AaaPlanRuntimeIdentity;
  aaaType: AaaPlanType;
  mutability: AaaPlanMutability;
  programScale: AaaPlanHex;
}): AaaPlanArtifact {
  const metadata = unifyMetadata(decAnyMetadata(input.metadataBytes));
  decodeProgram(metadata, input.programScale);
  const artifact: Omit<AaaPlanArtifact, 'planId'> = {
    format: AAA_PLAN_FORMAT,
    formatVersion: AAA_PLAN_FORMAT_VERSION,
    ...input.runtime,
    metadataHash: blake2AsHex(input.metadataBytes, 256) as AaaPlanHex,
    aaaType: input.aaaType,
    mutability: input.mutability,
    programScale: input.programScale,
  };
  return { ...artifact, planId: calculatePlanId(metadata, artifact) };
}

export function inspectAaaPlanArtifact(
  value: unknown,
  metadataBytes: Uint8Array,
  expectedRuntime?: AaaPlanRuntimeIdentity,
): AaaPlanInspection {
  const errors: string[] = [];
  const artifact = parseArtifact(value, errors);
  if (artifact == null) return { valid: false, errors };

  try {
    const metadataHash = blake2AsHex(metadataBytes, 256) as AaaPlanHex;
    if (artifact.metadataHash !== metadataHash)
      errors.push('metadataHash does not match metadata');
    if (expectedRuntime != null) {
      for (const field of [
        'genesisHash',
        'specVersion',
        'transactionVersion',
      ] as const) {
        if (artifact[field] !== expectedRuntime[field]) {
          errors.push(`${field} does not match the live runtime identity`);
        }
      }
    }
    const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
    const decoded = decodeProgram(metadata, artifact.programScale);
    const { planId: _planId, ...identity } = artifact;
    if (calculatePlanId(metadata, identity) !== artifact.planId) {
      errors.push('planId does not match the canonical artifact fields');
    }
    return errors.length === 0
      ? { valid: true, artifact, ...decoded }
      : { valid: false, errors };
  } catch (error) {
    errors.push(error instanceof Error ? error.message : String(error));
    return { valid: false, errors };
  }
}

function pointerSegment(value: string | number) {
  return String(value).replaceAll('~', '~0').replaceAll('/', '~1');
}

function childPath(path: string, segment: string | number) {
  return `${path}/${pointerSegment(segment)}`;
}

function projectionFingerprint(value: AaaPlanProjection) {
  return JSON.stringify(value);
}

function longestCommonSubsequence(
  before: AaaPlanProjection[],
  after: AaaPlanProjection[],
) {
  const rows = before.length + 1;
  const columns = after.length + 1;
  const lengths = Array.from({ length: rows }, () =>
    Array<number>(columns).fill(0),
  );
  for (let left = before.length - 1; left >= 0; left -= 1) {
    for (let right = after.length - 1; right >= 0; right -= 1) {
      lengths[left][right] =
        projectionFingerprint(before[left]) ===
        projectionFingerprint(after[right])
          ? lengths[left + 1][right + 1] + 1
          : Math.max(lengths[left + 1][right], lengths[left][right + 1]);
    }
  }
  const pairs: Array<[number, number]> = [];
  let left = 0;
  let right = 0;
  while (left < before.length && right < after.length) {
    if (
      projectionFingerprint(before[left]) ===
      projectionFingerprint(after[right])
    ) {
      pairs.push([left, right]);
      left += 1;
      right += 1;
    } else if (lengths[left + 1][right] >= lengths[left][right + 1]) {
      left += 1;
    } else {
      right += 1;
    }
  }
  return pairs;
}

function diffArrays(
  before: AaaPlanProjection[],
  after: AaaPlanProjection[],
  path: string,
  changes: AaaPlanDiff[],
) {
  const matchedBefore = new Set<number>();
  const matchedAfter = new Set<number>();
  for (const [left, right] of longestCommonSubsequence(before, after)) {
    matchedBefore.add(left);
    matchedAfter.add(right);
  }

  const unmatchedBefore = before
    .map((value, index) => ({ value, index }))
    .filter(({ index }) => !matchedBefore.has(index));
  const unmatchedAfter = after
    .map((value, index) => ({ value, index }))
    .filter(({ index }) => !matchedAfter.has(index));

  for (const left of unmatchedBefore) {
    const fingerprint = projectionFingerprint(left.value);
    const candidates = unmatchedAfter.filter(
      (right) =>
        !matchedAfter.has(right.index) &&
        projectionFingerprint(right.value) === fingerprint,
    );
    if (candidates.length === 1) {
      const [right] = candidates;
      matchedBefore.add(left.index);
      matchedAfter.add(right.index);
      changes.push({
        kind: 'move',
        from: childPath(path, left.index),
        path: childPath(path, right.index),
        value: right.value,
      });
    }
  }

  const remainingBefore = unmatchedBefore.filter(
    ({ index }) => !matchedBefore.has(index),
  );
  const remainingAfter = unmatchedAfter.filter(
    ({ index }) => !matchedAfter.has(index),
  );
  const pairedLength = Math.min(remainingBefore.length, remainingAfter.length);
  for (let index = 0; index < pairedLength; index += 1) {
    const left = remainingBefore[index];
    const right = remainingAfter[index];
    diffProjection(
      left.value,
      right.value,
      childPath(path, right.index),
      changes,
    );
  }
  for (const { value, index } of remainingBefore.slice(pairedLength)) {
    changes.push({ kind: 'remove', path: childPath(path, index), value });
  }
  for (const { value, index } of remainingAfter.slice(pairedLength)) {
    changes.push({ kind: 'add', path: childPath(path, index), value });
  }
}

function diffProjection(
  before: AaaPlanProjection,
  after: AaaPlanProjection,
  path: string,
  changes: AaaPlanDiff[],
) {
  if (projectionFingerprint(before) === projectionFingerprint(after)) return;
  if (Array.isArray(before) && Array.isArray(after)) {
    diffArrays(before, after, path, changes);
    return;
  }
  if (isRecord(before) && isRecord(after)) {
    const beforeRecord = before as Record<string, AaaPlanProjection>;
    const afterRecord = after as Record<string, AaaPlanProjection>;
    const keys = [
      ...new Set([...Object.keys(beforeRecord), ...Object.keys(afterRecord)]),
    ].sort();
    for (const key of keys) {
      const nextPath = childPath(path, key);
      if (!(key in beforeRecord)) {
        changes.push({ kind: 'add', path: nextPath, value: afterRecord[key] });
      } else if (!(key in afterRecord)) {
        changes.push({
          kind: 'remove',
          path: nextPath,
          value: beforeRecord[key],
        });
      } else {
        diffProjection(beforeRecord[key], afterRecord[key], nextPath, changes);
      }
    }
    return;
  }
  changes.push({ kind: 'replace', path, before, after });
}

export function diffAaaPlanArtifacts(
  before: { artifact: AaaPlanArtifact; projection: AaaPlanProjection },
  after: { artifact: AaaPlanArtifact; projection: AaaPlanProjection },
): AaaPlanDiffResult {
  const mismatches: Array<'genesisHash' | 'metadataHash'> = [];
  if (before.artifact.genesisHash !== after.artifact.genesisHash) {
    mismatches.push('genesisHash');
  }
  if (before.artifact.metadataHash !== after.artifact.metadataHash) {
    mismatches.push('metadataHash');
  }
  if (mismatches.length > 0) {
    return {
      compatible: false,
      reason: 'IncompatibleUntilRebound',
      mismatches,
    };
  }
  const changes: AaaPlanDiff[] = [];
  diffProjection(before.projection, after.projection, '', changes);
  return { compatible: true, changes };
}
