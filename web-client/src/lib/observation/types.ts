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
  delivery: ObservationDeliveryInspection | null;
};

export type ObservationFanoutBudget = {
  runtimeIdentity: string;
  weightIdentity: string;
  maxServiceUnitsPerBlock: number;
  maxActiveDirtyFeeds: number;
  maxSubscriberPagesPerFeed: number;
};

export type ObservationFanoutEvidence =
  | {
      status: 'Verified';
      observedIdentity: string;
    }
  | {
      status: 'EvidenceMismatch';
      observedIdentity: string;
      reasons: readonly string[];
    };

export type ObservationEstimateStatus =
  | 'NotApplicable'
  | 'Available'
  | 'EvidenceMismatch';

export type ObservationDeliveryStatus =
  | 'Clean'
  | 'PendingFanout'
  | 'FanoutInProgress'
  | 'AwaitingCleanup';

export type ObservationActorAdmissionStatus =
  | 'ActorMissing'
  | 'NoPendingSignal'
  | 'PendingQueueAdmission'
  | 'Queued'
  | 'WakeupScheduled';

export type ObservationActorDeliveryInspection = {
  actorId: bigint;
  exists: boolean;
  pendingSignal: boolean | null;
  queueLane: 'System' | 'User' | null;
  queueTicket: bigint | null;
  wakeup: {
    block: number;
    pageId: bigint;
    slot: number;
  } | null;
  queueAdmissionStatus: ObservationActorAdmissionStatus;
};

export type ObservationDeliveryInspection = {
  status: ObservationDeliveryStatus;
  latestRevision: bigint | null;
  fanoutRevision: bigint | null;
  dirtySince: number | null;
  dirtyAgeBlocks: number | null;
  activeList: {
    head: ObservationFeedIdentity | null;
    tail: ObservationFeedIdentity | null;
    cursor: ObservationFeedIdentity | null;
    count: number;
    selectedPosition: number | null;
    cursorPosition: number | null;
  };
  nextSubscriberPage: number | null;
  occupiedPageCount: number;
  remainingCurrentRevisionPages: number;
  remainingFanoutServiceUnits: number | null;
  exclusiveBudgetLowerBoundBlocks: number | null;
  fairServiceCeilingBlocks: number | null;
  estimateStatus: ObservationEstimateStatus;
  estimateContextIdentity: string | null;
  evidenceMismatchReasons: readonly string[];
  observedEvidenceIdentity: string | null;
  budget: ObservationFanoutBudget;
  estimateAssumptions: readonly string[];
  selectedActor: ObservationActorDeliveryInspection | null;
};

export type ObservationInspectionProvider = {
  getObservationFeeds?(): Promise<ReadModelValue<ObservationFeedIdentity[]>>;
  getObservationInspection?(
    feed: ObservationFeedIdentity,
    maxAgeBlocks: number,
    actorId?: number,
  ): Promise<ReadModelValue<ObservationInspection>>;
};
