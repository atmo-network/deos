/*
Domain: Runtime account derivation
Owns: System Actors sovereign-account derivation constants and helpers for the reference runtime.
Excludes: Account selection, wallet signing, balances, and presentation labels.
Zone: Transport/runtime helper; depends on Polkadot SCALE bindings only.
*/
import {
  Binary,
  Blake2256,
  fromBufferToBase58,
  u64,
} from '@polkadot-api/substrate-bindings';

export const ACTORS_PALLET_ID = 'deactors';
export const SYSTEM_ACTORS_LABEL = 'system';
export const SS58_FORMAT = 42;
export const LIQUIDITY_ACTOR_ACTORS_ID = 2;

type TolBucket = {
  key: 'a' | 'b' | 'c' | 'd';
  actorId: number;
};

type KnownSystemActor = {
  actorId: number;
  label: string;
  role: string;
};

export const TOL_BUCKETS: readonly TolBucket[] = [
  { key: 'a', actorId: 3 },
  { key: 'b', actorId: 4 },
  { key: 'c', actorId: 5 },
  { key: 'd', actorId: 6 },
];
export const KNOWN_SYSTEM_ACTORS: readonly KnownSystemActor[] = [
  { actorId: 0, label: 'Burn Actor', role: 'Protocol fee burn' },
  { actorId: 1, label: 'Fee Sink', role: 'Unified fee collector' },
  { actorId: 2, label: 'Liquidity Actor', role: 'Native/foreign LP composer' },
  { actorId: 3, label: 'TOL Bucket A', role: 'Anchor LP' },
  { actorId: 4, label: 'TOL Bucket B', role: 'Building unwind' },
  { actorId: 5, label: 'TOL Bucket C', role: 'Capital unwind' },
  { actorId: 6, label: 'TOL Bucket D', role: 'Dormant LP' },
  { actorId: 7, label: 'Treasury B', role: 'Building treasury' },
  { actorId: 8, label: 'Treasury C', role: 'Capital treasury' },
  { actorId: 9, label: 'Treasury D', role: 'Dormant treasury' },
  { actorId: 10, label: 'BLDR Splitter', role: 'BLDR distribution' },
  { actorId: 11, label: 'BLDR Liquidity Actor', role: 'NTVE/BLDR LP composer' },
  { actorId: 12, label: 'BLDR Bucket A', role: 'BLDR anchor LP' },
  { actorId: 13, label: 'BLDR Treasury', role: 'BLDR treasury' },
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

export function deriveSystemActorSovereignAccount(actorId: number): string {
  const seed = concatBytes(
    Binary.fromText(ACTORS_PALLET_ID),
    Binary.fromText(SYSTEM_ACTORS_LABEL),
    u64.enc(BigInt(actorId)),
  );
  return fromBufferToBase58(SS58_FORMAT)(Blake2256(seed));
}
