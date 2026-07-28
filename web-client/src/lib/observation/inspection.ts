/*
Domain: Typed observation inspection
Owns: Exact scalar formatting, current-state classification, and human-readable feed identity.
Excludes: Chain queries, history, plan execution, and UI layout.
Zone: Observation domain capability.
*/
import type {
  ObservationActorDeliveryInspection,
  ObservationAssetIdentity,
  ObservationDeliveryInspection,
  ObservationFanoutBudget,
  ObservationFeedIdentity,
  ObservationInspection,
} from './types';

import { type ReadModelStamp, fromChainStorage } from '../read-model.ts';

const U32_MAX = 0xffff_ffff;

function requireBoundedInteger(
  value: number,
  label: string,
  maximum = U32_MAX,
) {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new Error(`${label} must be a bounded non-negative integer`);
  }
}

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

export function projectObservationActorDeliveryInspection(input: {
  aaaId: bigint;
  hot: {
    actorClass: 'System' | 'User';
    pendingSignal: boolean;
    queueTicket: bigint | null;
    wakeup: {
      block: number;
      pageId: bigint;
      slot: number;
    } | null;
  } | null;
}): ObservationActorDeliveryInspection {
  if (input.aaaId < 0n || input.aaaId > 0xffff_ffff_ffff_ffffn) {
    throw new Error('AAA id must be an unsigned u64');
  }
  if (input.hot === null) {
    return {
      aaaId: input.aaaId,
      exists: false,
      pendingSignal: null,
      queueLane: null,
      queueTicket: null,
      wakeup: null,
      queueAdmissionStatus: 'ActorMissing',
    };
  }
  if (input.hot.queueTicket !== null && input.hot.queueTicket < 0n) {
    throw new Error('Queue ticket must be unsigned');
  }
  if (input.hot.queueTicket !== null && input.hot.wakeup !== null) {
    throw new Error(
      'Actor cannot own queue and wakeup pointers simultaneously',
    );
  }
  if (input.hot.wakeup !== null) {
    requireBoundedInteger(
      input.hot.wakeup.block,
      'Wakeup block',
      Number.MAX_SAFE_INTEGER,
    );
    requireBoundedInteger(input.hot.wakeup.slot, 'Wakeup slot');
    if (input.hot.wakeup.pageId < 0n) {
      throw new Error('Wakeup page id must be unsigned');
    }
  }
  const queueAdmissionStatus =
    input.hot.queueTicket !== null
      ? 'Queued'
      : input.hot.wakeup !== null
        ? 'WakeupScheduled'
        : input.hot.pendingSignal
          ? 'PendingQueueAdmission'
          : 'NoPendingSignal';
  return {
    aaaId: input.aaaId,
    exists: true,
    pendingSignal: input.hot.pendingSignal,
    queueLane: input.hot.actorClass,
    queueTicket: input.hot.queueTicket,
    wakeup: input.hot.wakeup,
    queueAdmissionStatus,
  };
}

export function projectObservationDeliveryInspection(input: {
  oracleRevision: bigint | null;
  dirty: {
    latestRevision: bigint;
    fanoutRevision: bigint;
    dirtySince: number;
    nextSubscriberPage: number | null;
  } | null;
  activeList: ObservationDeliveryInspection['activeList'];
  occupiedPageCount: number;
  remainingPageCount: number;
  finalizedBlock: number;
  budget: ObservationFanoutBudget;
  selectedActor?: ObservationActorDeliveryInspection;
}): ObservationDeliveryInspection {
  requireBoundedInteger(
    input.finalizedBlock,
    'Finalized block',
    Number.MAX_SAFE_INTEGER,
  );
  requireBoundedInteger(input.activeList.count, 'Active dirty-feed count');
  requireBoundedInteger(
    input.occupiedPageCount,
    'Occupied subscriber-page count',
  );
  requireBoundedInteger(
    input.remainingPageCount,
    'Remaining fanout-page count',
  );
  requireBoundedInteger(
    input.budget.maxPagesPerBlock,
    'Fanout pages per block',
  );
  requireBoundedInteger(
    input.budget.maxActiveDirtyFeeds,
    'Maximum active dirty feeds',
  );
  requireBoundedInteger(
    input.budget.maxSubscriberPagesPerFeed,
    'Maximum subscriber pages per feed',
  );
  if (
    input.budget.maxPagesPerBlock === 0 ||
    input.budget.maxActiveDirtyFeeds === 0 ||
    input.budget.maxSubscriberPagesPerFeed === 0
  ) {
    throw new Error('Fanout production bounds must be nonzero');
  }
  if (input.activeList.count > input.budget.maxActiveDirtyFeeds) {
    throw new Error(
      'Active dirty-feed count exceeds the identified runtime bound',
    );
  }
  if (input.occupiedPageCount > input.budget.maxSubscriberPagesPerFeed) {
    throw new Error(
      'Occupied subscriber pages exceed the identified runtime bound',
    );
  }
  if (
    input.budget.runtimeIdentity.length === 0 ||
    input.budget.weightIdentity.length === 0
  ) {
    throw new Error('Fanout estimates require runtime and weight identities');
  }
  if (input.remainingPageCount > input.occupiedPageCount) {
    throw new Error('Remaining fanout pages exceed occupied subscriber pages');
  }

  const selectedPosition = input.activeList.selectedPosition;
  if (input.dirty === null) {
    if (selectedPosition !== null || input.remainingPageCount !== 0) {
      throw new Error(
        'Clean feed cannot own active-list position or remaining fanout pages',
      );
    }
    return {
      status: 'Clean',
      latestRevision: input.oracleRevision,
      fanoutRevision: null,
      dirtySince: null,
      dirtyAgeBlocks: null,
      activeList: input.activeList,
      nextSubscriberPage: null,
      occupiedPageCount: input.occupiedPageCount,
      estimatedRemainingFanoutPages: 0,
      estimatedRemainingBlocks: 0,
      budget: input.budget,
      estimateAssumptions: [],
      selectedActor: input.selectedActor ?? null,
    };
  }

  const dirty = input.dirty;
  if (dirty.latestRevision <= 0n || dirty.fanoutRevision < 0n) {
    throw new Error(
      'Dirty revisions must be non-negative with a nonzero latest revision',
    );
  }
  if (dirty.fanoutRevision > dirty.latestRevision) {
    throw new Error('Fanout revision cannot exceed latest revision');
  }
  if (
    input.oracleRevision !== null &&
    input.oracleRevision !== dirty.latestRevision
  ) {
    throw new Error(
      'Oracle and AAA latest revisions must match at one finalized state',
    );
  }
  requireBoundedInteger(
    dirty.dirtySince,
    'Dirty-since block',
    Number.MAX_SAFE_INTEGER,
  );
  if (dirty.dirtySince > input.finalizedBlock) {
    throw new Error(
      'Dirty-since block cannot exceed the finalized snapshot block',
    );
  }
  if (
    selectedPosition === null ||
    !Number.isSafeInteger(selectedPosition) ||
    selectedPosition < 0 ||
    selectedPosition >= input.activeList.count
  ) {
    throw new Error('Dirty feed must own an in-range active-list position');
  }

  const pendingRestart = dirty.fanoutRevision < dirty.latestRevision;
  if (pendingRestart && input.remainingPageCount !== input.occupiedPageCount) {
    throw new Error(
      'Pending revision must restart from every occupied subscriber page',
    );
  }
  if (
    !pendingRestart &&
    input.remainingPageCount > 0 &&
    dirty.nextSubscriberPage === null
  ) {
    throw new Error(
      'In-progress fanout requires an exact next subscriber page',
    );
  }
  if (dirty.nextSubscriberPage !== null) {
    requireBoundedInteger(dirty.nextSubscriberPage, 'Next subscriber page');
  }

  const status = pendingRestart
    ? 'PendingFanout'
    : input.remainingPageCount > 0
      ? 'FanoutInProgress'
      : 'AwaitingCleanup';
  return {
    status,
    latestRevision: dirty.latestRevision,
    fanoutRevision: dirty.fanoutRevision,
    dirtySince: dirty.dirtySince,
    dirtyAgeBlocks: input.finalizedBlock - dirty.dirtySince,
    activeList: input.activeList,
    nextSubscriberPage: dirty.nextSubscriberPage,
    occupiedPageCount: input.occupiedPageCount,
    estimatedRemainingFanoutPages: input.remainingPageCount,
    estimatedRemainingBlocks: Math.ceil(
      input.remainingPageCount / input.budget.maxPagesPerBlock,
    ),
    budget: input.budget,
    estimateAssumptions: [
      'The finalized snapshot remains the common source for every field.',
      'No newer observation revision restarts latest-state fanout.',
      'The identified fanout page budget remains available each block.',
      'The estimate covers fanout pages, not actor queue admission or execution.',
    ],
    selectedActor: input.selectedActor ?? null,
  };
}

export function projectObservationInspection(input: {
  feed: ObservationFeedIdentity;
  config: RuntimeFeedConfig;
  observation: RuntimeObservation;
  finalizedBlock: number;
  maxAgeBlocks: number;
  delivery?: ObservationDeliveryInspection;
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
    delivery: input.delivery ?? null,
  };
}
