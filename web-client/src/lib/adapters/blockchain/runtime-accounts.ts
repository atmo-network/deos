import {
  Binary,
  Blake2256,
  fromBufferToBase58,
  u64,
} from "@polkadot-api/substrate-bindings";

export const AAA_PALLET_ID = "aaactor0";
export const SYSTEM_AAA_LABEL = "system";
export const SS58_FORMAT = 42;
export const ZAP_MANAGER_AAA_ID = 2;
export const TOL_BUCKETS = [
  { key: "a", aaaId: 3 },
  { key: "b", aaaId: 4 },
  { key: "c", aaaId: 5 },
  { key: "d", aaaId: 6 },
] as const;
export const KNOWN_SYSTEM_ACTORS = [
  { aaaId: 0, label: "Burning Manager", role: "Protocol fee burn" },
  { aaaId: 1, label: "Fee Sink", role: "Unified fee collector" },
  { aaaId: 2, label: "Zap Manager", role: "Liquidity composer" },
  { aaaId: 3, label: "TOL Bucket A", role: "Anchor LP" },
  { aaaId: 4, label: "TOL Bucket B", role: "Building unwind" },
  { aaaId: 5, label: "TOL Bucket C", role: "Capital unwind" },
  { aaaId: 6, label: "TOL Bucket D", role: "Dormant LP" },
  { aaaId: 7, label: "Treasury B", role: "Building treasury" },
  { aaaId: 8, label: "Treasury C", role: "Capital treasury" },
  { aaaId: 9, label: "Treasury D", role: "Dormant treasury" },
  { aaaId: 10, label: "BLDR Splitter", role: "BLDR distribution" },
  { aaaId: 11, label: "BLDR Zap Manager", role: "BLDR liquidity" },
  { aaaId: 12, label: "BLDR Bucket A", role: "BLDR anchor LP" },
  { aaaId: 13, label: "BLDR Treasury", role: "BLDR treasury" },
] as const;

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const totalLength = parts.reduce((sum, part) => sum + part.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}

export function deriveSystemAaaSovereignAccount(aaaId: number): string {
  const seed = concatBytes(
    Binary.fromText(AAA_PALLET_ID),
    Binary.fromText(SYSTEM_AAA_LABEL),
    u64.enc(BigInt(aaaId)),
  );
  return fromBufferToBase58(SS58_FORMAT)(Blake2256(seed));
}
