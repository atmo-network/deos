/*
Domain: Governance advisory payloads
Owns: Advisory payload draft validation, byte encoding/decoding, and payload hash helpers.
Excludes: Proposal submission, store lifecycle, treasury payload encoding, and UI rendering.
Zone: Governance payload helper; pure encoding/validation boundary for advisory proposals.
*/
import { hexToU8a, u8aToHex } from '@polkadot/util';

const advisoryPayloadTextEncoder = new TextEncoder();

export const GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES = 128;
export const GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES = 96;

export type GovernanceAdvisoryPayloadDraft = {
  summary: string;
  docCid: string;
  referencedPayloadHash: string;
};

export type GovernanceAdvisoryPayloadEncoding = {
  payloadBytes: Uint8Array;
  payloadHex: string;
  payloadByteLength: number;
  summaryByteLength: number;
  docCidByteLength: number;
  referencedPayloadHash: string | null;
};

export type GovernanceAdvisoryPayloadDraftState = {
  summaryValid: boolean;
  summaryByteLength: number;
  docCidValid: boolean;
  docCidByteLength: number;
  referencedPayloadHashValid: boolean;
  encoding: GovernanceAdvisoryPayloadEncoding | null;
};

type ParsedTextBytes = {
  valid: boolean;
  bytes: Uint8Array | null;
  byteLength: number;
};

type ParsedReferencedPayloadHash = {
  valid: boolean;
  bytes: Uint8Array | null;
  normalized: string | null;
};

function compactLengthBytes(length: number) {
  if (length < 1 << 6) {
    return Uint8Array.of(length << 2);
  }
  if (length < 1 << 14) {
    const encoded = (length << 2) | 0b01;
    return Uint8Array.of(encoded & 0xff, (encoded >> 8) & 0xff);
  }
  const encoded = (length << 2) | 0b10;
  return Uint8Array.of(
    encoded & 0xff,
    (encoded >> 8) & 0xff,
    (encoded >> 16) & 0xff,
    (encoded >> 24) & 0xff,
  );
}

function concatBytes(parts: Uint8Array[]) {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const output = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function encodeScaleBytes(bytes: Uint8Array) {
  return concatBytes([compactLengthBytes(bytes.length), bytes]);
}

function encodeScaleOption(innerBytes: Uint8Array | null) {
  return innerBytes == null
    ? Uint8Array.of(0)
    : concatBytes([Uint8Array.of(1), innerBytes]);
}

function parseRequiredSummary(value: string): ParsedTextBytes {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return { valid: false, bytes: null, byteLength: 0 };
  }
  const bytes = advisoryPayloadTextEncoder.encode(trimmed);
  return {
    valid: bytes.length <= GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES,
    bytes: bytes.length <= GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES ? bytes : null,
    byteLength: bytes.length,
  };
}

function parseOptionalDocCid(value: string): ParsedTextBytes {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return { valid: true, bytes: null, byteLength: 0 };
  }
  const bytes = advisoryPayloadTextEncoder.encode(trimmed);
  return {
    valid: bytes.length <= GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES,
    bytes: bytes.length <= GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES ? bytes : null,
    byteLength: bytes.length,
  };
}

function parseOptionalReferencedPayloadHash(
  value: string,
): ParsedReferencedPayloadHash {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return {
      valid: true,
      bytes: null,
      normalized: null,
    };
  }
  if (!/^0x[0-9a-fA-F]{64}$/.test(trimmed)) {
    return {
      valid: false,
      bytes: null,
      normalized: null,
    };
  }
  const normalized = trimmed.toLowerCase();
  return {
    valid: true,
    bytes: hexToU8a(normalized),
    normalized,
  };
}

let advisoryPayloadHasherPromise: Promise<
  (payloadBytes: Uint8Array) => string
> | null = null;

export async function hashGovernanceAdvisoryPayloadBytes(
  payloadBytes: Uint8Array,
): Promise<string> {
  if (advisoryPayloadHasherPromise == null) {
    advisoryPayloadHasherPromise = import('@polkadot/util-crypto').then(
      ({ blake2AsHex }) =>
        (bytes: Uint8Array) =>
          blake2AsHex(bytes, 256),
    );
  }
  return (await advisoryPayloadHasherPromise)(payloadBytes);
}

export function deriveGovernanceAdvisoryPayloadDraftState(
  draft: GovernanceAdvisoryPayloadDraft,
): GovernanceAdvisoryPayloadDraftState {
  const summary = parseRequiredSummary(draft.summary);
  const docCid = parseOptionalDocCid(draft.docCid);
  const referencedPayloadHash = parseOptionalReferencedPayloadHash(
    draft.referencedPayloadHash,
  );
  if (
    !summary.valid ||
    summary.bytes == null ||
    !docCid.valid ||
    !referencedPayloadHash.valid
  ) {
    return {
      summaryValid: summary.valid,
      summaryByteLength: summary.byteLength,
      docCidValid: docCid.valid,
      docCidByteLength: docCid.byteLength,
      referencedPayloadHashValid: referencedPayloadHash.valid,
      encoding: null,
    };
  }
  const payloadBytes = concatBytes([
    encodeScaleOption(referencedPayloadHash.bytes),
    encodeScaleBytes(summary.bytes),
    encodeScaleOption(
      docCid.bytes == null ? null : encodeScaleBytes(docCid.bytes),
    ),
  ]);
  return {
    summaryValid: true,
    summaryByteLength: summary.byteLength,
    docCidValid: true,
    docCidByteLength: docCid.byteLength,
    referencedPayloadHashValid: true,
    encoding: {
      payloadBytes,
      payloadHex: u8aToHex(payloadBytes),
      payloadByteLength: payloadBytes.length,
      summaryByteLength: summary.byteLength,
      docCidByteLength: docCid.byteLength,
      referencedPayloadHash: referencedPayloadHash.normalized,
    },
  };
}
