/*
Domain: Typed observation inspection
Owns: Exact scalar formatting, current-state classification, and human-readable feed identity.
Excludes: Chain queries, history, plan execution, and UI layout.
Zone: Observation domain capability.
*/
import type {
  ObservationAssetIdentity,
  ObservationFeedIdentity,
  ObservationInspection,
} from './types';

import { type ReadModelStamp, fromChainStorage } from '../read-model.ts';

const U32_MAX = 0xffff_ffff;

export function formatObservationAsset(asset: ObservationAssetIdentity) {
  return asset.type === 'Native' ? 'Native' : `${asset.type}(${asset.id})`;
}

export function formatObservationFeed(feed: ObservationFeedIdentity) {
  const aggregation =
    feed.aggregation.type === 'LastValue'
      ? 'LastValue'
      : `EMA/${feed.aggregation.halfLifeBlocks}`;
  return `${formatObservationAsset(feed.assetIn)} → ${formatObservationAsset(feed.assetOut)} · ${feed.method} · ${aggregation} · 10^${feed.scale}`;
}

export function formatScaledObservation(value: bigint, scale: number) {
  if (!Number.isSafeInteger(scale) || scale < 0 || scale > 18) {
    throw new Error('Observation scale must be an integer from 0 to 18');
  }
  if (value < 0n) throw new Error('Observation value must be unsigned');
  if (scale === 0) return value.toString();
  const divisor = 10n ** BigInt(scale);
  const whole = value / divisor;
  const fractional = (value % divisor)
    .toString()
    .padStart(scale, '0')
    .replace(/0+$/, '');
  return fractional.length === 0 ? whole.toString() : `${whole}.${fractional}`;
}

type RuntimeFeedConfig = {
  producer: string;
  provenance: 'AxialRouterPreExecutionReserves';
  lifecycle: 'Active' | 'Paused' | 'Deactivated';
  scale: number;
  aggregation: ObservationFeedIdentity['aggregation'];
} | null;

type RuntimeObservation = {
  value: bigint;
  updatedAt: number;
  revision: bigint;
} | null;

export function canonicalObservationReadModel<T>(
  value: T,
  sourceRef: string,
  stamp?: ReadModelStamp,
) {
  return fromChainStorage(value, sourceRef, stamp);
}

export function projectObservationInspection(input: {
  feed: ObservationFeedIdentity;
  config: RuntimeFeedConfig;
  observation: RuntimeObservation;
  finalizedBlock: number;
  maxAgeBlocks: number;
}): ObservationInspection {
  if (
    !Number.isSafeInteger(input.maxAgeBlocks) ||
    input.maxAgeBlocks <= 0 ||
    input.maxAgeBlocks > U32_MAX
  ) {
    throw new Error('Authored maximum age must be a nonzero u32');
  }
  if (!Number.isSafeInteger(input.finalizedBlock) || input.finalizedBlock < 0) {
    throw new Error('Finalized block must be a non-negative safe integer');
  }
  const unavailable =
    input.config === null || input.config.lifecycle === 'Deactivated';
  const currentObservation = unavailable ? null : input.observation;
  const ageBlocks =
    currentObservation === null
      ? null
      : Math.max(input.finalizedBlock - currentObservation.updatedAt, 0);
  const status = unavailable
    ? 'Unavailable'
    : currentObservation === null
      ? 'Uninitialized'
      : ageBlocks! <= input.maxAgeBlocks
        ? 'Fresh'
        : 'Stale';
  return {
    feed: input.feed,
    status,
    lifecycle: input.config?.lifecycle ?? 'Unavailable',
    producer: input.config?.producer ?? null,
    provenance: input.config?.provenance ?? 'Unavailable',
    aggregation: input.config?.aggregation ?? input.feed.aggregation,
    scale: input.config?.scale ?? input.feed.scale,
    value: currentObservation?.value ?? null,
    formattedValue:
      currentObservation == null
        ? null
        : formatScaledObservation(
            currentObservation.value,
            input.config?.scale ?? input.feed.scale,
          ),
    updatedAt: currentObservation?.updatedAt ?? null,
    revision: currentObservation?.revision ?? null,
    ageBlocks,
    authoredMaxAgeBlocks: input.maxAgeBlocks,
    latestStateCoalescing: true,
    fairPriceProof: false,
  };
}
