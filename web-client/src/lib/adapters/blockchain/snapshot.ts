/*
Domain: Blockchain snapshot projection
Owns: Building bounded system, asset-balance, pool, bucket, and native-staking read models from a chain snapshot.
Excludes: Live connection lifecycle, transaction writes, widget rendering, and store mutation.
Zone: Transport read-model adapter internals; may depend on domain projection types and runtime asset helpers.
*/
import { PRECISION } from '$lib/economics';
import type {
  AssetBalanceProjection,
  AssetPresentation,
} from '$lib/portfolio/types';
import type { NativeStakingProjection } from '$lib/staking/types';
import type { SystemSnapshot } from '$lib/system/types';

import type { DeosChainSnapshot } from './deos';
import { toOptionalValue } from './quotes';
import {
  LIQUIDITY_ACTOR_AAA_ID,
  TOL_BUCKETS,
  deriveSystemAaaSovereignAccount,
} from './runtime-accounts';
import {
  NATIVE_ASSET,
  type RuntimeAssetKind,
  TYPE_FOREIGN,
  decodeBytes,
  dedupeRuntimeAssets,
  fallbackAssetSymbol,
  foreignAsset,
  isAssetWithId,
  isForeignAssetKind,
  localAsset,
  nativeFreeBalance,
  runtimeAssetKey,
} from './runtime-assets';

export class BlockchainSnapshotBuilder {
  constructor(private readonly selectedAddress: () => string) {}

  async buildSystemSnapshot(
    snapshot: DeosChainSnapshot,
  ): Promise<SystemSnapshot> {
    const canonicalForeignAsset = await this.canonicalForeignAsset(snapshot);
    const foreignAsset =
      canonicalForeignAsset ??
      (await this.resolvePrimaryForeignAsset(snapshot));
    const trackedAssets = await this.trackedAssets(snapshot);
    const trackedForeignAssetCount =
      trackedAssets.filter(isForeignAssetKind).length;
    const minForeignSwapAmount =
      await snapshot.typedApi.constants.AxialRouter.MinSwapForeign();
    const [
      curve,
      reserves,
      lpAssetId,
      supply,
      nativeAssetPresentation,
      foreignAssetPresentation,
    ] = await Promise.all([
      this.nativeCurve(snapshot),
      this.xykReserves(snapshot, foreignAsset),
      this.poolLpAssetId(snapshot, foreignAsset),
      snapshot.typedApi.query.Balances.TotalIssuance.getValue({
        at: snapshot.at,
      }),
      this.describeAsset(snapshot, NATIVE_ASSET),
      this.describeAsset(
        snapshot,
        foreignAsset,
        canonicalForeignAsset !== null,
      ),
    ]);
    const reserveNative = reserves?.native ?? 0n;
    const reserveForeign = reserves?.foreign ?? 0n;
    const hasPool = reserveNative > 0n && reserveForeign > 0n;
    const supplyLp = await this.lpSupply(snapshot, lpAssetId);
    const [buckets, liquidityActorBuffers, nativeStaking] = await Promise.all([
      this.bucketBalances(
        snapshot,
        foreignAsset,
        reserves,
        lpAssetId,
        supplyLp,
      ),
      this.liquidityActorBuffers(snapshot, foreignAsset),
      this.nativeStakingReadModel(snapshot),
    ]);
    const priceXyk = hasPool
      ? (reserveForeign * PRECISION) / reserveNative
      : null;
    const effectiveSupply =
      curve && supply > curve.initial_issuance
        ? supply - curve.initial_issuance
        : 0n;
    const priceTmc = curve
      ? curve.initial_price + (curve.slope * effectiveSupply) / PRECISION
      : 0n;
    const protocolOwnedNative = Array.from(buckets.values()).reduce(
      (sum, bucket) => sum + bucket.contributed_native,
      0n,
    );
    const gravityWellRatio =
      supply > 0n
        ? Number((protocolOwnedNative * 1_000_000n) / supply) / 1_000_000
        : 0;
    const totalBurned =
      await snapshot.typedApi.view.TokenMintingCurve.total_native_burned({
        at: snapshot.at,
      });
    return {
      blockNumber: snapshot.finalizedBlockNumber,
      supply,
      priceTmc,
      priceXyk,
      reserveNative,
      reserveForeign,
      totalBurned: totalBurned ?? null,
      supplyLp,
      hasPool,
      hasNativeCurve: curve !== null,
      trackedForeignAssetCount,
      minForeignSwapAmount,
      gravityWellRatio,
      buckets,
      bufferNative: liquidityActorBuffers.native,
      bufferForeign: liquidityActorBuffers.foreign,
      nativeAsset: nativeAssetPresentation,
      foreignAsset: foreignAssetPresentation,
      nativeStaking,
    };
  }

  async knownAssetBalancesAtSnapshot(
    snapshot: DeosChainSnapshot,
    address: string,
  ): Promise<AssetBalanceProjection[]> {
    const [
      canonicalForeignAsset,
      primaryRouteAsset,
      trackedAssets,
      nativeAccount,
      nativeStakingPool,
    ] = await Promise.all([
      this.canonicalForeignAsset(snapshot),
      this.resolvePrimaryForeignAsset(snapshot),
      this.trackedAssets(snapshot),
      snapshot.typedApi.query.System.Account.getValue(address, {
        at: snapshot.at,
      }),
      snapshot.typedApi.view.Staking.native_staking_liquidity_pool({
        at: snapshot.at,
      }),
    ]);
    const canonicalAssetKey = canonicalForeignAsset
      ? runtimeAssetKey(canonicalForeignAsset)
      : null;
    const primaryRouteKey = runtimeAssetKey(primaryRouteAsset);
    const nativeStakingAssets: RuntimeAssetKind[] = nativeStakingPool
      ? [
          localAsset(nativeStakingPool.native_asset_id),
          localAsset(nativeStakingPool.staked_asset_id),
          localAsset(nativeStakingPool.lp_asset_id),
        ]
      : [];
    const candidateAssets = dedupeRuntimeAssets([
      NATIVE_ASSET,
      primaryRouteAsset,
      ...trackedAssets,
      ...nativeStakingAssets,
    ]);
    return await Promise.all(
      candidateAssets.map(async (asset) => {
        const assetKey = runtimeAssetKey(asset);
        const isCanonical =
          asset.type === 'Native' || canonicalAssetKey === assetKey;
        const balance = isAssetWithId(asset)
          ? ((await snapshot.typedApi.view.Assets.balance_of(
              address,
              asset.value,
              { at: snapshot.at },
            )) ?? 0n)
          : nativeFreeBalance(nativeAccount);
        return {
          presentation: await this.describeAsset(snapshot, asset, isCanonical),
          balance,
          isPrimaryRouteAsset: assetKey === primaryRouteKey,
        };
      }),
    );
  }

  async resolvePrimaryForeignAsset(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind> {
    const canonicalForeignAsset = await this.canonicalForeignAsset(snapshot);
    if (canonicalForeignAsset) {
      return canonicalForeignAsset;
    }
    const trackedAssets = await this.trackedAssets(snapshot);
    return trackedAssets.find(isForeignAssetKind) ?? foreignAsset(TYPE_FOREIGN);
  }

  async xykReserves(
    snapshot: DeosChainSnapshot,
    foreignAsset: RuntimeAssetKind,
  ): Promise<{ native: bigint; foreign: bigint } | null> {
    const reserves = toOptionalValue(
      await snapshot.typedApi.view.AssetConversion.get_reserves(
        foreignAsset,
        NATIVE_ASSET,
        {
          at: snapshot.at,
        },
      ),
    );
    if (!reserves) {
      return null;
    }
    const [foreign, native] = reserves;
    if (native === 0n || foreign === 0n) {
      return null;
    }
    return { native, foreign };
  }

  private async canonicalForeignAsset(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind | null> {
    const nativeCurve =
      await snapshot.typedApi.query.TokenMintingCurve.TokenCurves.getValue(
        NATIVE_ASSET,
        { at: snapshot.at },
      );
    if (
      nativeCurve?.foreign_asset &&
      nativeCurve.foreign_asset.type !== 'Native'
    ) {
      return nativeCurve.foreign_asset;
    }
    return null;
  }

  private async trackedAssets(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind[]> {
    return (
      (await snapshot.typedApi.query.AxialRouter.TrackedAssets.getValue({
        at: snapshot.at,
      })) ?? []
    );
  }

  private async describeAsset(
    snapshot: DeosChainSnapshot,
    asset: RuntimeAssetKind,
    isCanonical = asset.type === 'Native',
  ): Promise<AssetPresentation> {
    if (asset.type === 'Native') {
      return {
        kind: 'Native',
        assetId: null,
        symbol: fallbackAssetSymbol(asset),
        isCanonical,
      };
    }
    const metadata = await snapshot.typedApi.view.Assets.get_metadata(
      asset.value,
      { at: snapshot.at },
    );
    return {
      kind: asset.type,
      assetId: asset.value,
      symbol: decodeBytes(metadata?.symbol) ?? fallbackAssetSymbol(asset),
      isCanonical,
    };
  }

  private async nativeCurve(snapshot: DeosChainSnapshot) {
    return await snapshot.typedApi.query.TokenMintingCurve.TokenCurves.getValue(
      NATIVE_ASSET,
      {
        at: snapshot.at,
      },
    );
  }

  private async poolLpAssetId(
    snapshot: DeosChainSnapshot,
    foreignAsset: RuntimeAssetKind,
  ): Promise<number | null> {
    const direct = await snapshot.typedApi.query.AssetConversion.Pools.getValue(
      [NATIVE_ASSET, foreignAsset],
      { at: snapshot.at },
    );
    if (direct !== undefined) {
      return direct;
    }
    const reverse =
      await snapshot.typedApi.query.AssetConversion.Pools.getValue(
        [foreignAsset, NATIVE_ASSET],
        { at: snapshot.at },
      );
    return reverse ?? null;
  }

  private async lpSupply(
    snapshot: DeosChainSnapshot,
    lpAssetId: number | null,
  ): Promise<bigint> {
    if (lpAssetId === null) {
      return 0n;
    }
    return (
      (
        await snapshot.typedApi.view.Assets.asset_details(lpAssetId, {
          at: snapshot.at,
        })
      )?.supply ?? 0n
    );
  }

  private async bucketBalances(
    snapshot: DeosChainSnapshot,
    _foreignAsset: RuntimeAssetKind,
    reserves: { native: bigint; foreign: bigint } | null,
    lpAssetId: number | null,
    lpSupply: bigint,
  ): Promise<
    Map<
      string,
      {
        lp_tokens: bigint;
        contributed_native: bigint;
        contributed_foreign: bigint;
      }
    >
  > {
    const buckets = new Map<
      string,
      {
        lp_tokens: bigint;
        contributed_native: bigint;
        contributed_foreign: bigint;
      }
    >();
    if (lpAssetId === null) {
      return buckets;
    }
    await Promise.all(
      TOL_BUCKETS.map(async ({ key, aaaId }) => {
        const account = deriveSystemAaaSovereignAccount(aaaId);
        const lpTokens =
          (await snapshot.typedApi.view.Assets.balance_of(account, lpAssetId, {
            at: snapshot.at,
          })) ?? 0n;
        const contributedNative =
          reserves && lpSupply > 0n && lpTokens > 0n
            ? (reserves.native * lpTokens) / lpSupply
            : 0n;
        const contributedForeign =
          reserves && lpSupply > 0n && lpTokens > 0n
            ? (reserves.foreign * lpTokens) / lpSupply
            : 0n;
        buckets.set(key, {
          lp_tokens: lpTokens,
          contributed_native: contributedNative,
          contributed_foreign: contributedForeign,
        });
      }),
    );
    return buckets;
  }

  private async nativeStakingReadModel(
    snapshot: DeosChainSnapshot,
  ): Promise<NativeStakingProjection> {
    const accountAddress = this.selectedAddress() || null;
    const unavailable = {
      isAvailable: false,
      accountAddress,
      exchangeRate: null,
      pool: null,
      accountPosition: null,
    } satisfies NativeStakingProjection;
    try {
      const [exchangeRate, pool, accountPosition] = await Promise.all([
        snapshot.typedApi.view.Staking.native_staking_exchange_rate({
          at: snapshot.at,
        }),
        snapshot.typedApi.view.Staking.native_staking_liquidity_pool({
          at: snapshot.at,
        }),
        accountAddress
          ? snapshot.typedApi.view.Staking.native_locked_lp_position(
              accountAddress,
              { at: snapshot.at },
            )
          : Promise.resolve(null),
      ]);
      return {
        isAvailable: pool !== null,
        accountAddress,
        exchangeRate: exchangeRate ?? null,
        pool: pool
          ? {
              nativeAssetId: pool.native_asset_id,
              stakedAssetId: pool.staked_asset_id,
              lpAssetId: pool.lp_asset_id,
              reserveNative: pool.reserve_native,
              reserveStaked: pool.reserve_staked,
              lpTotalIssuance: pool.lp_total_issuance,
            }
          : null,
        accountPosition: accountPosition
          ? {
              totalLockedLp: accountPosition.total_locked_lp,
              collatorLockedLp: accountPosition.collator_locked_lp,
              governanceLockedLp: accountPosition.governance_locked_lp,
              conservativeNativeValue:
                accountPosition.conservative_native_value ?? null,
            }
          : null,
      };
    } catch {
      return unavailable;
    }
  }

  private async liquidityActorBuffers(
    snapshot: DeosChainSnapshot,
    foreignAsset: RuntimeAssetKind,
  ): Promise<{ native: bigint; foreign: bigint }> {
    const liquidityActorAccount = deriveSystemAaaSovereignAccount(
      LIQUIDITY_ACTOR_AAA_ID,
    );
    const nativeAccount = await snapshot.typedApi.query.System.Account.getValue(
      liquidityActorAccount,
      { at: snapshot.at },
    );
    const foreign = isAssetWithId(foreignAsset)
      ? ((await snapshot.typedApi.view.Assets.balance_of(
          liquidityActorAccount,
          foreignAsset.value,
          { at: snapshot.at },
        )) ?? 0n)
      : 0n;
    return {
      native: nativeFreeBalance(nativeAccount),
      foreign,
    };
  }
}
