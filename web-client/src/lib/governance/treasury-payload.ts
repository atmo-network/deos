import { u8aToHex } from "@polkadot/util";
import { decodeAddress } from "@polkadot/util-crypto";

const U128_MAX = (1n << 128n) - 1n;

export type GovernanceTreasuryPayloadDraft = {
  beneficiary: string;
  payoutAsset: string;
  baseAmount: string;
};

export type GovernanceTreasuryPayloadEncoding = {
  payloadBytes: Uint8Array;
  payloadHex: string;
  payloadByteLength: number;
  beneficiary: string;
  payoutAssetId: number;
  baseAmount: bigint;
  fundingSource: "BldrTreasury";
};

export type GovernanceTreasuryPayloadDraftState = {
  beneficiaryValid: boolean;
  payoutAssetValid: boolean;
  baseAmountValid: boolean;
  encoding: GovernanceTreasuryPayloadEncoding | null;
};

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

function encodeU32(value: number) {
  const bytes = new Uint8Array(4);
  let remaining = value >>> 0;
  for (let index = 0; index < 4; index += 1) {
    bytes[index] = remaining & 0xff;
    remaining >>>= 8;
  }
  return bytes;
}

function encodeU128(value: bigint) {
  const bytes = new Uint8Array(16);
  let remaining = value;
  for (let index = 0; index < 16; index += 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return bytes;
}

function parseBeneficiary(value: string) {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return { valid: false, bytes: null as Uint8Array | null, normalized: "" };
  }
  try {
    const bytes = decodeAddress(trimmed);
    if (bytes.length !== 32) {
      return { valid: false, bytes: null as Uint8Array | null, normalized: trimmed };
    }
    return { valid: true, bytes: Uint8Array.from(bytes), normalized: trimmed };
  } catch {
    return { valid: false, bytes: null as Uint8Array | null, normalized: trimmed };
  }
}

function parsePayoutAsset(value: string) {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) {
    return { valid: false, parsed: null as number | null };
  }
  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 0xffff_ffff) {
    return { valid: false, parsed: null as number | null };
  }
  return { valid: true, parsed };
}

function parseBaseAmount(value: string) {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) {
    return { valid: false, parsed: null as bigint | null };
  }
  const parsed = BigInt(trimmed);
  if (parsed <= 0n || parsed > U128_MAX) {
    return { valid: false, parsed: null as bigint | null };
  }
  return { valid: true, parsed };
}

export function deriveGovernanceTreasuryPayloadDraftState(
  draft: GovernanceTreasuryPayloadDraft,
): GovernanceTreasuryPayloadDraftState {
  const beneficiary = parseBeneficiary(draft.beneficiary);
  const payoutAsset = parsePayoutAsset(draft.payoutAsset);
  const baseAmount = parseBaseAmount(draft.baseAmount);
  if (
    !beneficiary.valid ||
    beneficiary.bytes == null ||
    !payoutAsset.valid ||
    payoutAsset.parsed == null ||
    !baseAmount.valid ||
    baseAmount.parsed == null
  ) {
    return {
      beneficiaryValid: beneficiary.valid,
      payoutAssetValid: payoutAsset.valid,
      baseAmountValid: baseAmount.valid,
      encoding: null,
    };
  }
  const payloadBytes = concatBytes([
    beneficiary.bytes,
    encodeU32(payoutAsset.parsed),
    encodeU128(baseAmount.parsed),
    Uint8Array.of(0),
  ]);
  return {
    beneficiaryValid: true,
    payoutAssetValid: true,
    baseAmountValid: true,
    encoding: {
      payloadBytes,
      payloadHex: u8aToHex(payloadBytes),
      payloadByteLength: payloadBytes.length,
      beneficiary: beneficiary.normalized,
      payoutAssetId: payoutAsset.parsed,
      baseAmount: baseAmount.parsed,
      fundingSource: "BldrTreasury",
    },
  };
}
