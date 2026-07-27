/*
Domain: Typed observation inspection
Owns: Directional feed identity, canonical current-state inspection, and provider contracts.
Excludes: Runtime storage access, plan authoring, historical indexing, and widget layout.
Zone: Top-level observation domain contract.
*/
import type { ReadModelValue } from '$lib/read-model';

export type ObservationAssetIdentity =
  | { type: 'Native' }
  | { type: 'Local' | 'Foreign'; id: number };

export type ObservationAggregation =
  | { type: 'LastValue' }
  | { type: 'Ema'; halfLifeBlocks: number };

export type ObservationFeedIdentity = {
  assetIn: ObservationAssetIdentity;
  assetOut: ObservationAssetIdentity;
  method: 'PreExecutionSpot';
  aggregation: ObservationAggregation;
  scale: number;
};

export type ObservationCurrentStatus =
  | 'Fresh'
  | 'Stale'
  | 'Unavailable'
  | 'Uninitialized';

export type ObservationInspection = {
  feed: ObservationFeedIdentity;
  status: ObservationCurrentStatus;
  lifecycle: 'Active' | 'Paused' | 'Deactivated' | 'Unavailable';
  producer: string | null;
  provenance: 'AxialRouterPreExecutionReserves' | 'Unavailable';
  aggregation: ObservationAggregation;
  scale: number;
  value: bigint | null;
  formattedValue: string | null;
  updatedAt: number | null;
  revision: bigint | null;
  ageBlocks: number | null;
  authoredMaxAgeBlocks: number;
  latestStateCoalescing: true;
  fairPriceProof: false;
};

export type ObservationInspectionProvider = {
  getObservationFeeds?(): Promise<ReadModelValue<ObservationFeedIdentity[]>>;
  getObservationInspection?(
    feed: ObservationFeedIdentity,
    maxAgeBlocks: number,
  ): Promise<ReadModelValue<ObservationInspection>>;
};
