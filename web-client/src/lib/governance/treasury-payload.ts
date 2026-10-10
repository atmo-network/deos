/*
Domain: Governance treasury payloads
Owns: Invoice draft validation, address/amount parsing and canonical source-schema encoding.
Excludes: Full runtime CID/treasury-authority validation, proposal submission and UI rendering.
Zone: Pure governance payload helper; the runtime remains admission authority.
*/
import { compactAddLength, u8aToHex } from '@polkadot/util';
import {
  base32Decode,
  base32Encode,
  decodeAddress,
} from '@polkadot/util-crypto';

import {
  parseUnsignedDecimalBigInt,
  parseUnsignedDecimalNumber,
} from '../format/numeric.ts';

const U128_MAX = (1n << 128n) - 1n;

export type GovernanceTreasuryPayloadDraft = {
  invoiceCid: string;
  treasuryAccount: string;
  beneficiary: string;
  payoutAsset: string;
  baseAmount: string;
};

export type GovernanceTreasuryPayloadEncoding = {
  payloadBytes: Uint8Array;
  payloadHex: string;
  payloadByteLength: number;
  invoiceCid: string;
  treasuryAccount: string;
  beneficiary: string;
  payoutAssetId: number;
  baseAmount: bigint;
};

export type GovernanceTreasuryPayloadDraftState = {
  capabilityReady: boolean;
  invoiceCidValid: boolean;
  treasuryAccountValid: boolean;
  beneficiaryValid: boolean;
  payoutAssetValid: boolean;
  baseAmountValid: boolean;
  encoding: GovernanceTreasuryPayloadEncoding | null;
};

function concatBytes(parts: Uint8Array[]) {
  const output = new Uint8Array(
    parts.reduce((sum, part) => sum + part.length, 0),
  );
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function encodeUnsigned(value: bigint, length: number) {
  const bytes = new Uint8Array(length);
  for (let index = 0; index < length; index += 1) {
    bytes[index] = Number(value & 0xffn);
    value >>= 8n;
  }
  return bytes;
}

function parseAccount(value: string) {
  const normalized = value.trim();
  try {
    const bytes = decodeAddress(normalized);
    return bytes.length === 32
      ? { bytes: Uint8Array.from(bytes), normalized }
      : null;
  } catch {
    return null;
  }
}

// This is a bounded local format check, not a complete CID parser or authority check.
function hasInvoiceCidFormat(value: string, maxBytes: number | null) {
  if (
    maxBytes == null ||
    value.length === 0 ||
    value.length > maxBytes ||
    !/^b[a-z2-7]+$/.test(value)
  )
    return false;
  try {
    const bytes = base32Decode(value, true);
    return (
      bytes.length >= 4 && bytes[0] === 1 && base32Encode(bytes, true) === value
    );
  } catch {
    return false;
  }
}

export function deriveGovernanceTreasuryPayloadDraftState(
  draft: GovernanceTreasuryPayloadDraft,
  maxCidBytes: number | null,
): GovernanceTreasuryPayloadDraftState {
  const capabilityReady =
    maxCidBytes != null &&
    Number.isInteger(maxCidBytes) &&
    maxCidBytes > 0 &&
    maxCidBytes <= 0xffff_ffff;
  const invoiceCidValid =
    capabilityReady && hasInvoiceCidFormat(draft.invoiceCid, maxCidBytes);
  const treasury = parseAccount(draft.treasuryAccount);
  const beneficiary = parseAccount(draft.beneficiary);
  const payoutAsset = parseUnsignedDecimalNumber(draft.payoutAsset, {
    max: 0xffff_ffff,
  });
  const baseAmount = parseUnsignedDecimalBigInt(draft.baseAmount, {
    min: 1n,
    max: U128_MAX,
  });
  const validity = {
    capabilityReady,
    invoiceCidValid,
    treasuryAccountValid: treasury != null,
    beneficiaryValid: beneficiary != null,
    payoutAssetValid: payoutAsset != null,
    baseAmountValid: baseAmount != null,
  };
  if (
    !invoiceCidValid ||
    !treasury ||
    !beneficiary ||
    payoutAsset == null ||
    baseAmount == null
  ) {
    return { ...validity, encoding: null };
  }
  const payloadBytes = concatBytes([
    compactAddLength(new TextEncoder().encode(draft.invoiceCid)),
    treasury.bytes,
    beneficiary.bytes,
    encodeUnsigned(BigInt(payoutAsset), 4),
    encodeUnsigned(baseAmount, 16),
  ]);
  return {
    ...validity,
    encoding: {
      payloadBytes,
      payloadHex: u8aToHex(payloadBytes),
      payloadByteLength: payloadBytes.length,
      invoiceCid: draft.invoiceCid,
      treasuryAccount: treasury.normalized,
      beneficiary: beneficiary.normalized,
      payoutAssetId: payoutAsset,
      baseAmount,
    },
  };
}
