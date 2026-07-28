/*
Domain: Blockchain observation adapter
Owns: Exact bounded DEOS Oracle and reactive AAA inspection reads at one finalized block.
Excludes: Observation history, plan authoring, widget state, actor execution, and fair-price claims.
Zone: Transport adapter; projects canonical Oracle and AAA storage into the observation domain.
*/
import { Enum as PapiEnum } from 'polkadot-api';

import {
  canonicalObservationReadModel,
  formatObservationFeed,
  projectObservationActorDeliveryInspection,
  projectObservationDeliveryInspection,
  projectObservationInspection,
} from '$lib/observation/inspection';
import { expectedObservationFanoutBudget } from '$lib/observation/runtime-evidence';
import type {
  ObservationAggregation,
  ObservationAssetIdentity,
  ObservationDeliveryInspection,
  ObservationFanoutBudget,
  ObservationFanoutEvidence,
  ObservationFeedIdentity,
  ObservationInspection,
} from '$lib/observation/types';

import type { DeosChainSnapshot } from './deos';
import type { RuntimeAssetKind } from './runtime-assets';

export const DEOS_OBSERVATION_FANOUT_BUDGET: ObservationFanoutBudget =
  expectedObservationFanoutBudget();

function assetIdentity(asset: RuntimeAssetKind): ObservationAssetIdentity {
  return asset.type === 'Native'
    ? { type: 'Native' }
    : { type: asset.type, id: asset.value };
}

function runtimeAsset(asset: ObservationAssetIdentity): RuntimeAssetKind {
  return asset.type === 'Native'
    ? PapiEnum('Native')
    : PapiEnum(asset.type, asset.id);
}

function aggregationIdentity(value: {
  type: 'LastValue' | 'Ema';
  value?: { half_life_blocks: number };
}): ObservationAggregation {
  return value.type === 'LastValue'
    ? { type: 'LastValue' }
    : {
        type: 'Ema',
        halfLifeBlocks: value.value?.half_life_blocks ?? 0,
      };
}

function runtimeAggregation(value: ObservationAggregation) {
  return value.type === 'LastValue'
    ? PapiEnum('LastValue')
    : PapiEnum('Ema', { half_life_blocks: value.halfLifeBlocks });
}

function feedIdentity(feed: {
  asset_in: RuntimeAssetKind;
  asset_out: RuntimeAssetKind;
  method: { type: 'PreExecutionSpot' };
  aggregation: {
    type: 'LastValue' | 'Ema';
    value?: { half_life_blocks: number };
  };
  scale: number;
}): ObservationFeedIdentity {
  if (feed.method.type !== 'PreExecutionSpot') {
    throw new Error(`Unsupported observation method: ${feed.method.type}`);
  }
  return {
    assetIn: assetIdentity(feed.asset_in),
    assetOut: assetIdentity(feed.asset_out),
    method: 'PreExecutionSpot',
    aggregation: aggregationIdentity(feed.aggregation),
    scale: feed.scale,
  };
}

function runtimeFeed(feed: ObservationFeedIdentity) {
  return {
    asset_in: runtimeAsset(feed.assetIn),
    asset_out: runtimeAsset(feed.assetOut),
    method: PapiEnum('PreExecutionSpot'),
    aggregation: runtimeAggregation(feed.aggregation),
    scale: feed.scale,
  };
}

function optionalFeedIdentity(
  feed: ReturnType<typeof runtimeFeed> | null | undefined,
) {
  return feed == null ? null : feedIdentity(feed);
}

function stamp(snapshot: DeosChainSnapshot) {
  return {
    asOfBlock: snapshot.finalizedBlockNumber,
    asOfHash: snapshot.at,
  };
}

export class BlockchainObservationReader {
  async feeds(snapshot: DeosChainSnapshot) {
    const feeds = await snapshot.typedApi.query.Oracle.FeedIds.getValue({
      at: snapshot.at,
    });
    const projected = feeds.map(feedIdentity);
    return canonicalObservationReadModel(
      projected,
      'Oracle.FeedIds',
      stamp(snapshot),
    );
  }

  private async dirtyFeedPositions(
    snapshot: DeosChainSnapshot,
    selected: ReturnType<typeof runtimeFeed>,
    activeList: {
      head?: ReturnType<typeof runtimeFeed>;
      cursor?: ReturnType<typeof runtimeFeed>;
      count: number;
    },
  ) {
    if (activeList.count === 0) {
      return { selectedPosition: null, cursorPosition: null };
    }
    if (activeList.head == null || activeList.cursor == null) {
      throw new Error('AAA active dirty-feed list is missing head or cursor');
    }
    const selectedIdentity = formatObservationFeed(feedIdentity(selected));
    const cursorIdentity = formatObservationFeed(
      feedIdentity(activeList.cursor),
    );
    let selectedPosition: number | null = null;
    let cursorPosition: number | null = null;
    let current: ReturnType<typeof runtimeFeed> | undefined = activeList.head;
    for (let position = 0; position < activeList.count; position += 1) {
      if (current == null) {
        throw new Error('AAA active dirty-feed list ended before its count');
      }
      const identity = formatObservationFeed(feedIdentity(current));
      if (identity === selectedIdentity) selectedPosition = position;
      if (identity === cursorIdentity) cursorPosition = position;
      const state =
        await snapshot.typedApi.query.AAA.DirtyObservationFeeds.getValue(
          current,
          { at: snapshot.at },
        );
      if (state == null) {
        throw new Error('AAA active dirty-feed member is missing');
      }
      current = state.next_dirty_feed;
    }
    if (current != null) {
      throw new Error('AAA active dirty-feed links exceed the list count');
    }
    if (selectedPosition === null || cursorPosition === null) {
      throw new Error(
        'AAA selected feed or fair cursor is outside the active list',
      );
    }
    return { selectedPosition, cursorPosition };
  }

  private async remainingSubscriberPages(
    snapshot: DeosChainSnapshot,
    key: ReturnType<typeof runtimeFeed>,
    firstPage: number | undefined,
    occupiedCount: number,
  ) {
    let remaining = 0;
    let page = firstPage;
    while (page != null) {
      remaining += 1;
      if (remaining > occupiedCount) {
        throw new Error(
          'AAA subscriber-page links exceed the occupied-page bound',
        );
      }
      const state =
        await snapshot.typedApi.query.AAA.ObservationSubscriberPages.getValue(
          key,
          page,
          { at: snapshot.at },
        );
      if (state == null) {
        throw new Error('AAA next subscriber page is missing');
      }
      page = state.next;
    }
    return remaining;
  }

  private async selectedActorInspection(
    snapshot: DeosChainSnapshot,
    aaaId: number | undefined,
  ) {
    if (aaaId === undefined) return undefined;
    if (!Number.isSafeInteger(aaaId) || aaaId < 0) {
      throw new Error('Selected AAA id must be a non-negative safe integer');
    }
    const runtimeAaaId = BigInt(aaaId);
    const hot = await snapshot.typedApi.query.AAA.ActorHot.getValue(
      runtimeAaaId,
      {
        at: snapshot.at,
      },
    );
    return projectObservationActorDeliveryInspection({
      aaaId: runtimeAaaId,
      hot:
        hot == null
          ? null
          : {
              actorClass: hot.actor_class.type,
              pendingSignal: hot.pending_signal,
              queueTicket: hot.queue_ticket ?? null,
              wakeup:
                hot.wakeup_pointer == null
                  ? null
                  : {
                      block: hot.wakeup_pointer.block,
                      pageId: hot.wakeup_pointer.page_id,
                      slot: hot.wakeup_pointer.slot,
                    },
            },
    });
  }

  private async deliveryInspection(
    snapshot: DeosChainSnapshot,
    key: ReturnType<typeof runtimeFeed>,
    oracleRevision: bigint | null,
    aaaId: number | undefined,
    evidence: ObservationFanoutEvidence,
  ): Promise<ObservationDeliveryInspection> {
    const [dirty, activeList, subscriberPages, selectedActor] =
      await Promise.all([
        snapshot.typedApi.query.AAA.DirtyObservationFeeds.getValue(key, {
          at: snapshot.at,
        }),
        snapshot.typedApi.query.AAA.DirtyObservationListState.getValue({
          at: snapshot.at,
        }),
        snapshot.typedApi.query.AAA.ObservationSubscriberPageLists.getValue(
          key,
          {
            at: snapshot.at,
          },
        ),
        this.selectedActorInspection(snapshot, aaaId),
      ]);
    const occupiedPageCount = subscriberPages?.count ?? 0;
    const positions =
      dirty == null
        ? { selectedPosition: null, cursorPosition: null }
        : await this.dirtyFeedPositions(snapshot, key, activeList);
    const remainingCurrentRevisionPages =
      dirty == null
        ? 0
        : dirty.fanout_revision === 0n
          ? occupiedPageCount
          : await this.remainingSubscriberPages(
              snapshot,
              key,
              dirty.next_subscriber_page,
              occupiedPageCount,
            );
    return projectObservationDeliveryInspection({
      oracleRevision,
      dirty:
        dirty == null
          ? null
          : {
              latestRevision: dirty.latest_revision,
              fanoutRevision: dirty.fanout_revision,
              dirtySince: dirty.dirty_since,
              nextSubscriberPage: dirty.next_subscriber_page ?? null,
            },
      activeList: {
        head: optionalFeedIdentity(activeList.head),
        tail: optionalFeedIdentity(activeList.tail),
        cursor: optionalFeedIdentity(activeList.cursor),
        count: activeList.count,
        selectedPosition: positions.selectedPosition,
        cursorPosition: positions.cursorPosition,
      },
      occupiedPageCount,
      remainingCurrentRevisionPages,
      finalizedBlock: snapshot.finalizedBlockNumber,
      budget: DEOS_OBSERVATION_FANOUT_BUDGET,
      evidence,
      selectedActor,
    });
  }

  async inspection(
    snapshot: DeosChainSnapshot,
    evidence: ObservationFanoutEvidence,
    feed: ObservationFeedIdentity,
    maxAgeBlocks: number,
    aaaId?: number,
  ) {
    const key = runtimeFeed(feed);
    const [config, observation] = await Promise.all([
      snapshot.typedApi.query.Oracle.Feeds.getValue(key, { at: snapshot.at }),
      snapshot.typedApi.query.Oracle.Observations.getValue(key, {
        at: snapshot.at,
      }),
    ]);
    const delivery = await this.deliveryInspection(
      snapshot,
      key,
      observation?.revision ?? null,
      aaaId,
      evidence,
    );
    const projection: ObservationInspection = projectObservationInspection({
      feed,
      config:
        config == null
          ? null
          : {
              producer: config.producer,
              provenance:
                config.provenance.type === 'AxialRouterPreExecutionReserves'
                  ? 'AxialRouterPreExecutionReserves'
                  : (() => {
                      throw new Error(
                        `Unsupported observation provenance: ${config.provenance.type}`,
                      );
                    })(),
              lifecycle: config.lifecycle.type,
              scale: config.scale,
              aggregation: aggregationIdentity(config.aggregation),
            },
      observation:
        observation == null
          ? null
          : {
              value: observation.value,
              updatedAt: observation.updated_at,
              revision: observation.revision,
            },
      finalizedBlock: snapshot.finalizedBlockNumber,
      maxAgeBlocks,
      delivery,
    });
    return canonicalObservationReadModel(
      projection,
      `Oracle.Feeds + Oracle.Observations / ${formatObservationFeed(feed)}`,
      stamp(snapshot),
    );
  }
}
