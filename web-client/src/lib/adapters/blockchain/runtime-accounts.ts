/*
Domain: Runtime account derivation
Owns: System AAA sovereign-account derivation constants and helpers for the reference runtime.
Excludes: Account selection, wallet signing, balances, and presentation labels.
Zone: Transport/runtime helper; depends on Polkadot SCALE bindings only.
*/
import {
  Binary,
  Blake2256,
  fromBufferToBase58,
  u64,
} from '@polkadot-api/substrate-bindings';

export const AAA_PALLET_ID = 'aaactor0';
export const SYSTEM_AAA_LABEL = 'system';
export const SS58_FORMAT = 42;
export const LIQUIDITY_ACTOR_AAA_ID = 2;

type TolBucket = {
  key: 'a' | 'b' | 'c' | 'd';
  aaaId: number;
};

type KnownSystemActor = {
  aaaId: number;
  label: string;
  role: string;
};

export const TOL_BUCKETS: readonly TolBucket[] = [
  { key: 'a', aaaId: 3 },
  { key: 'b', aaaId: 4 },
  { key: 'c', aaaId: 5 },
  { key: 'd', aaaId: 6 },
];
export const KNOWN_SYSTEM_ACTORS: readonly KnownSystemActor[] = [
  { aaaId: 0, label: 'Burn Actor', role: 'Protocol fee burn' },
  { aaaId: 1, label: 'Fee Sink', role: 'Unified fee collector' },
  { aaaId: 2, label: 'Liquidity Actor', role: 'Native/foreign LP composer' },
  { aaaId: 3, label: 'TOL Bucket A', role: 'Anchor LP' },
  { aaaId: 4, label: 'TOL Bucket B', role: 'Building unwind' },
  { aaaId: 5, label: 'TOL Bucket C', role: 'Capital unwind' },
  { aaaId: 6, label: 'TOL Bucket D', role: 'Dormant LP' },
  { aaaId: 7, label: 'Treasury B', role: 'Building treasury' },
  { aaaId: 8, label: 'Treasury C', role: 'Capital treasury' },
  { aaaId: 9, label: 'Treasury D', role: 'Dormant treasury' },
  { aaaId: 10, label: 'BLDR Splitter', role: 'BLDR distribution' },
  { aaaId: 11, label: 'BLDR Liquidity Actor', role: 'NTVE/BLDR LP composer' },
  { aaaId: 12, label: 'BLDR Bucket A', role: 'BLDR anchor LP' },
  { aaaId: 13, label: 'BLDR Treasury', role: 'BLDR treasury' },
];

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
