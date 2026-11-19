/*
Domain: Governance transport power decoding
Owns: Exact conversion of metadata U256 little-endian u64 limbs to domain bigint.
Excludes: Voting policy, domain state, and scalar transport compatibility.
*/
import type { FixedSizeArray } from 'polkadot-api';

export type GovernancePowerLimbs = FixedSizeArray<4, bigint>;

export function decodeGovernancePower(value: GovernancePowerLimbs): bigint {
  if (
    !Array.isArray(value) ||
    value.length !== 4 ||
    !Array.from(value).every(
      (limb) => typeof limb === 'bigint' && limb >= 0n && limb < 1n << 64n,
    )
  ) {
    throw new Error('Invalid Governance U256 limbs');
  }
  return value.reduceRight((power, limb) => (power << 64n) | limb, 0n);
}
