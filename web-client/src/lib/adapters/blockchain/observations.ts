/*
Domain: Blockchain observation adapter
Owns: Exact bounded Oracle registry reads and current typed observation projection at one finalized block.
Excludes: Observation history, plan authoring, widget state, and fair-price claims.
Zone: Transport adapter; projects canonical Oracle storage into the observation domain.
*/
import { Enum as PapiEnum } from 'polkadot-api';

import {
  canonicalObservationReadModel,
  formatObservationFeed,
  projectObservationInspection,
} from '$lib/observation/inspection';
import type {
  ObservationAggregation,
  ObservationAssetIdentity,
  ObservationFeedIdentity,
  ObservationInspection,
} from '$lib/observation/types';

import type { DeosChainSnapshot } from './deos';
import type { RuntimeAssetKind } from './runtime-assets';

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

  async inspection(
    snapshot: DeosChainSnapshot,
    feed: ObservationFeedIdentity,
    maxAgeBlocks: number,
  ) {
    const key = runtimeFeed(feed);
    const [config, observation] = await Promise.all([
      snapshot.typedApi.query.Oracle.Feeds.getValue(key, { at: snapshot.at }),
      snapshot.typedApi.query.Oracle.Observations.getValue(key, {
        at: snapshot.at,
      }),
    ]);
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
    });
    return canonicalObservationReadModel(
      projection,
      `Oracle.Feeds + Oracle.Observations / ${formatObservationFeed(feed)}`,
      stamp(snapshot),
    );
  }
}
