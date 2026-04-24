import type { Adapter } from "$lib/adapters/types";
import { PRECISION } from "$lib/shared/types";
import type {
  AssetBalanceProjection,
  AssetPresentation,
  AutomationActorSnapshot,
  NativeCollatorLpPositionProjection,
  NativeGovernanceCustodyPositionProjection,
  NativeStakingProjection,
  LogEntry,
  SystemConfig,
  SystemSnapshot,
  SwapResult,
  Quote,
  PricePoint,
  TransactionProgress,
  TransferAssetKey,
} from "$lib/shared/types";
import {
  Enum as PapiEnum,
  type ResultPayload,
} from "polkadot-api";
import {
  KNOWN_SYSTEM_ACTORS,
  TOL_BUCKETS,
  ZAP_MANAGER_AAA_ID,
  deriveSystemAaaSovereignAccount,
} from "./runtime-accounts";
import { DEFAULT_DEOS_DAPP_NAME, connectDeosSigner } from "./signer";
import { walletStore } from "$lib/wallet/index.svelte";
import { getBlockchainEndpoint } from "$lib/system/endpoint";
import type {
  DeosChainConnectionState,
  DeosPapiConnection,
  DeosChainSnapshot,
} from "./deos";

export {
  DEFAULT_DEOS_DAPP_NAME,
  connectDevSigner,
  connectInjectedSigner,
  connectDeosSigner,
  discoverInjectedSignerAccounts,
  hasBuiltInDevSigner,
  injectedSignerAvailability,
  isValidTmctolAddress,
  injectedSignerExtensionNames,
  type TmctolDevSignerPreset,
  type TmctolInjectedSignerAccount,
  type TmctolInjectedSignerAvailability,
  type TmctolInjectedSignerMatch,
  type TmctolSignerMatch,
  TMCTOL_DEV_SIGNER_PRESETS,
} from "./signer";

const TYPE_FOREIGN = 0xf000_0000;

type RuntimeAssetKind =
  | { type: "Native"; value: undefined }
  | { type: "Local"; value: number }
  | { type: "Foreign"; value: number };
type RuntimeAssetWithId = { type: "Local" | "Foreign"; value: number };

const NATIVE_ASSET: RuntimeAssetKind = PapiEnum("Native") as RuntimeAssetKind;

function foreignAsset(assetId: number): RuntimeAssetKind {
  return PapiEnum("Foreign", assetId) as RuntimeAssetKind;
}

function isForeignAssetKind(
  asset: RuntimeAssetKind,
): asset is { type: "Foreign"; value: number } {
  return asset.type === "Foreign";
}

function isAssetWithId(asset: RuntimeAssetKind): asset is RuntimeAssetWithId {
  return asset.type === "Local" || asset.type === "Foreign";
}

function runtimeAssetKey(asset: RuntimeAssetKind): string {
  return isAssetWithId(asset) ? `${asset.type}:${asset.value}` : asset.type;
}

function dedupeRuntimeAssets(assets: RuntimeAssetKind[]): RuntimeAssetKind[] {
  const deduped: RuntimeAssetKind[] = [];
  const seen = new Set<string>();
  for (const asset of assets) {
    const key = runtimeAssetKey(asset);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    deduped.push(asset);
  }
  return deduped;
}

function nativeFreeBalance(
  account: { data?: { free?: bigint } } | null | undefined,
): bigint {
  return account?.data?.free ?? 0n;
}

function bigintSqrt(value: bigint): bigint {
  if (value <= 0n) {
    return 0n;
  }
  let x0 = value;
  let x1 = (x0 + 1n) >> 1n;
  while (x1 < x0) {
    x0 = x1;
    x1 = (x1 + value / x1) >> 1n;
  }
  return x0;
}

function toOptionalValue<T>(
  result: ResultPayload<T, unknown> | undefined | null,
): T | null {
  if (!result || !result.success) {
    return null;
  }
  return result.value;
}

function routeFromMechanism(mechanism: { type: string }): "TMC" | "XYK" {
  return mechanism.type === "DirectMint" ? "TMC" : "XYK";
}

function decodeBytes(bytes: Uint8Array | undefined): string | null {
  if (!bytes || bytes.length === 0) {
    return null;
  }
  const decoded = new TextDecoder().decode(bytes).replace(/\0/g, "").trim();
  return decoded.length > 0 ? decoded : null;
}

function fallbackAssetSymbol(asset: RuntimeAssetKind): string {
  switch (asset.type) {
    case "Native":
      return "NTVE";
    case "Local":
      return `LOCAL-${asset.value}`;
    case "Foreign":
      return `FOREIGN-${asset.value.toString(16).toUpperCase()}`;
  }
}

function automationTriggerLabel(trigger?: { type: string; value?: unknown }): string {
  if (!trigger) {
    return "Unavailable";
  }
  switch (trigger.type) {
    case "Timer": {
      const timer = trigger.value as {
        every_blocks: number;
        probability?: unknown;
      };
      return timer.probability
        ? `Timer/${timer.every_blocks} + p`
        : `Timer/${timer.every_blocks}`;
    }
    case "OnAddressEvent":
      return "Address event";
    case "Manual":
      return "Manual";
    default:
      return trigger.type;
  }
}

export class BlockchainAdapter implements Adapter {
  private papi: DeosPapiConnection | null = null;
  private papiLoading: Promise<DeosPapiConnection> | null = null;
  private currentEndpoint: string | null = null;
  private loadingEndpoint: string | null = null;
  private connectionGeneration = 0;
  private cancelFinalizedBlockSub: (() => void) | null = null;
  private onRefresh: (() => void) | null = null;
  private onTransactionProgress: ((progress: TransactionProgress) => void) | null = null;
  private networkLogSupported = true;

  private async canonicalForeignAsset(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind | null> {
    const nativeCurve =
      await snapshot.typedApi.query.TokenMintingCurve.TokenCurves.getValue(
        NATIVE_ASSET,
        {
          at: snapshot.at,
        },
      );
    if (
      nativeCurve?.foreign_asset &&
      nativeCurve.foreign_asset.type !== "Native"
    ) {
      return nativeCurve.foreign_asset;
    }
    return null;
  }

  private async resolvePrimaryForeignAsset(
    snapshot: DeosChainSnapshot,
  ): Promise<RuntimeAssetKind> {
    const canonicalForeignAsset = await this.canonicalForeignAsset(snapshot);
    if (canonicalForeignAsset) {
      return canonicalForeignAsset;
    }
    const trackedAssets = await this.trackedAssets(snapshot);
    return trackedAssets.find(isForeignAssetKind) ?? foreignAsset(TYPE_FOREIGN);
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
    isCanonical = asset.type === "Native",
  ): Promise<AssetPresentation> {
    if (asset.type === "Native") {
      return {
        kind: "Native",
        assetId: null,
        symbol: fallbackAssetSymbol(asset),
        isCanonical,
      };
    }
    const metadata = await snapshot.typedApi.view.Assets.get_metadata(
      asset.value,
      {
        at: snapshot.at,
      },
    );
    return {
      kind: asset.type,
      assetId: asset.value,
      symbol: decodeBytes(metadata?.symbol) ?? fallbackAssetSymbol(asset),
      isCanonical,
    };
  }

  private async knownAssetBalancesAtSnapshot(
    snapshot: DeosChainSnapshot,
    address: string,
  ): Promise<AssetBalanceProjection[]> {
    const [canonicalForeignAsset, primaryRouteAsset, trackedAssets, nativeAccount, nativeStakingPool] =
      await Promise.all([
        this.canonicalForeignAsset(snapshot),
        this.resolvePrimaryForeignAsset(snapshot),
        this.trackedAssets(snapshot),
        snapshot.typedApi.query.System.Account.getValue(address, {
          at: snapshot.at,
        }),
        snapshot.typedApi.view.Staking.native_staking_liquidity_pool({ at: snapshot.at }),
      ]);
    const canonicalAssetKey = canonicalForeignAsset
      ? runtimeAssetKey(canonicalForeignAsset)
      : null;
    const primaryRouteKey = runtimeAssetKey(primaryRouteAsset);
    const nativeStakingAssets: RuntimeAssetKind[] = nativeStakingPool
      ? [
          PapiEnum("Local", nativeStakingPool.native_asset_id) as RuntimeAssetKind,
          PapiEnum("Local", nativeStakingPool.staked_asset_id) as RuntimeAssetKind,
          PapiEnum("Local", nativeStakingPool.lp_asset_id) as RuntimeAssetKind,
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
          asset.type === "Native" || canonicalAssetKey === assetKey;
        const balance = isAssetWithId(asset)
          ? ((await snapshot.typedApi.view.Assets.balance_of(
              address,
              asset.value,
              {
                at: snapshot.at,
              },
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

  private async xykReserves(
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
      {
        at: snapshot.at,
      },
    );
    if (direct !== undefined) {
      return direct;
    }
    const reverse =
      await snapshot.typedApi.query.AssetConversion.Pools.getValue(
        [foreignAsset, NATIVE_ASSET],
        {
          at: snapshot.at,
        },
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
    foreignAsset: RuntimeAssetKind,
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
    const accountAddress = walletStore.state.selectedAddress || null;
    const unavailable = {
      isAvailable: false,
      accountAddress,
      exchangeRate: null,
      pool: null,
      accountPosition: null,
    } satisfies NativeStakingProjection;
    try {
      const [exchangeRate, pool, accountPosition] = await Promise.all([
        snapshot.typedApi.view.Staking.native_staking_exchange_rate({ at: snapshot.at }),
        snapshot.typedApi.view.Staking.native_staking_liquidity_pool({ at: snapshot.at }),
        accountAddress
          ? snapshot.typedApi.view.Staking.native_locked_lp_position(accountAddress, { at: snapshot.at })
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
              conservativeNativeValue: accountPosition.conservative_native_value ?? null,
            }
          : null,
      };
    } catch {
      return unavailable;
    }
  }

  async getNativeCollatorLpPosition(
    operator: string,
  ): Promise<NativeCollatorLpPositionProjection | null> {
    const accountAddress = walletStore.state.selectedAddress || null;
    const normalizedOperator = operator.trim();
    if (!accountAddress || normalizedOperator.length === 0) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    const position = await snapshot.typedApi.view.Staking.native_collator_lp_position(
      accountAddress,
      normalizedOperator,
      { at: snapshot.at },
    );
    return {
      lpAssetId: position.lp_asset_id ?? null,
      lockedLp: position.locked_lp,
      pendingUnlockLp: position.pending_unlock_lp,
      pendingUnlockBlock: position.pending_unlock_block ?? null,
      conservativeNativeValue: position.conservative_native_value ?? null,
    };
  }

  async getNativeGovernanceCustodyPosition(
    assetId: number,
  ): Promise<NativeGovernanceCustodyPositionProjection | null> {
    const accountAddress = walletStore.state.selectedAddress || null;
    if (!accountAddress || !Number.isFinite(assetId)) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    const position = await snapshot.typedApi.view.Staking.native_governance_custody_position(
      accountAddress,
      assetId,
      { at: snapshot.at },
    );
    return {
      lpAssetId: position.lp_asset_id ?? null,
      governanceLockedLp: position.governance_locked_lp,
      pendingGovernanceLpUnlock: position.pending_governance_lp_unlock,
      pendingGovernanceLpUnlockBlock: position.pending_governance_lp_unlock_block ?? null,
      assetId: position.asset_id,
      assetLocked: position.asset_locked,
      pendingAssetUnlock: position.pending_asset_unlock,
      pendingAssetUnlockBlock: position.pending_asset_unlock_block ?? null,
    };
  }

  async getNativeNominationRewardClaimable(epoch: number): Promise<bigint | null> {
    const accountAddress = walletStore.state.selectedAddress || null;
    if (!accountAddress || !Number.isInteger(epoch) || epoch < 0) {
      return null;
    }
    const snapshot = await (await this.ensurePapi()).snapshot();
    return (
      await snapshot.typedApi.view.Staking.native_nomination_reward_claimable(
        epoch,
        accountAddress,
        { at: snapshot.at },
      )
    ) ?? null;
  }

  private async zapManagerBuffers(
    snapshot: DeosChainSnapshot,
    foreignAsset: RuntimeAssetKind,
  ): Promise<{ native: bigint; foreign: bigint }> {
    const zapManager = deriveSystemAaaSovereignAccount(ZAP_MANAGER_AAA_ID);
    const nativeAccount = await snapshot.typedApi.query.System.Account.getValue(
      zapManager,
      {
        at: snapshot.at,
      },
    );
    const foreign = isAssetWithId(foreignAsset)
      ? ((await snapshot.typedApi.view.Assets.balance_of(
          zapManager,
          foreignAsset.value,
          {
            at: snapshot.at,
          },
        )) ?? 0n)
      : 0n;
    return {
      native: nativeFreeBalance(nativeAccount),
      foreign,
    };
  }

  private async tmcMintQuote(
    snapshot: DeosChainSnapshot,
    foreignNet: bigint,
  ): Promise<bigint> {
    const curve = await this.nativeCurve(snapshot);
    if (!curve || curve.foreign_asset.type === "Native" || foreignNet <= 0n) {
      return 0n;
    }
    const supply =
      await snapshot.typedApi.query.Balances.TotalIssuance.getValue({
        at: snapshot.at,
      });
    const effectiveSupply =
      supply > curve.initial_issuance ? supply - curve.initial_issuance : 0n;
    const pCurrent =
      curve.initial_price + (curve.slope * effectiveSupply) / PRECISION;
    if (curve.slope === 0n) {
      return curve.initial_price === 0n
        ? 0n
        : (foreignNet * PRECISION) / curve.initial_price;
    }
    const kp = PRECISION * pCurrent;
    const insideSqrt =
      kp * kp + 2n * curve.slope * PRECISION * PRECISION * foreignNet;
    const sqrtRes = bigintSqrt(insideSqrt);
    if (sqrtRes <= kp) {
      return 0n;
    }
    return (sqrtRes - kp) / curve.slope;
  }

  private async quoteBuyAtSnapshot(
    snapshot: DeosChainSnapshot,
    foreignAmount: bigint,
  ): Promise<Quote | null> {
    if (foreignAmount <= 0n) {
      return null;
    }
    const minForeignSwapAmount = await snapshot.typedApi.constants.AxialRouter.MinSwapForeign();
    if (foreignAmount < minForeignSwapAmount) {
      return null;
    }
    const accountId = this.selectedQuoteAccountId();
    if (!accountId) {
      return null;
    }
    try {
      const foreignAsset = await this.resolvePrimaryForeignAsset(snapshot);
      const authoritativeQuote = toOptionalValue(
        await snapshot.typedApi.view.AxialRouter.quote_exact_input(
          accountId,
          foreignAsset,
          NATIVE_ASSET,
          foreignAmount,
          { at: snapshot.at },
        ),
      );
      if (!authoritativeQuote || authoritativeQuote.amount_out <= 0n) {
        return null;
      }
      const routerFee = authoritativeQuote.router_fee;
      const foreignNet =
        foreignAmount > routerFee ? foreignAmount - routerFee : 0n;
      const [tmcOut, reserves] = await Promise.all([
        this.tmcMintQuote(snapshot, foreignNet),
        this.xykReserves(snapshot, foreignAsset),
      ]);
      const xykOut = reserves
        ? (foreignNet * reserves.native) / (reserves.foreign + foreignNet)
        : 0n;
      return {
        out: authoritativeQuote.amount_out,
        route: routeFromMechanism(authoritativeQuote.mechanism),
        effectivePrice:
          Number(foreignAmount) / Number(authoritativeQuote.amount_out),
        fee: authoritativeQuote.router_fee,
        tmcOut,
        xykOut,
        isSell: false,
      };
    } catch {
      return null;
    }
  }

  private async buildSystemSnapshot(
    snapshot: DeosChainSnapshot,
  ): Promise<SystemSnapshot> {
    const canonicalForeignAsset = await this.canonicalForeignAsset(snapshot);
    const foreignAsset =
      canonicalForeignAsset ??
      (await this.resolvePrimaryForeignAsset(snapshot));
    const trackedAssets = await this.trackedAssets(snapshot);
    const trackedForeignAssetCount = trackedAssets.filter(isForeignAssetKind).length;
    const minForeignSwapAmount = await snapshot.typedApi.constants.AxialRouter.MinSwapForeign();
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
    const [buckets, zapBuffers, nativeStaking] = await Promise.all([
      this.bucketBalances(
        snapshot,
        foreignAsset,
        reserves,
        lpAssetId,
        supplyLp,
      ),
      this.zapManagerBuffers(snapshot, foreignAsset),
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
      bufferNative: zapBuffers.native,
      bufferForeign: zapBuffers.foreign,
      nativeAsset: nativeAssetPresentation,
      foreignAsset: foreignAssetPresentation,
      nativeStaking,
    };
  }

  private resetConnection(): void {
    if (this.cancelFinalizedBlockSub) {
      this.cancelFinalizedBlockSub();
      this.cancelFinalizedBlockSub = null;
    }
    if (this.papi) {
      this.papi.destroy();
      this.papi = null;
    }
    this.papiLoading = null;
    this.currentEndpoint = null;
    this.loadingEndpoint = null;
  }

  private startPapiLoad(endpoint: string): Promise<DeosPapiConnection> {
    const generation = ++this.connectionGeneration;
    this.loadingEndpoint = endpoint;
    this.papiLoading = import("./deos").then(({ DeosPapiConnection }) => {
      if (generation !== this.connectionGeneration) {
        throw new Error("Adapter initialization superseded");
      }
      const papi = new DeosPapiConnection(endpoint);
      this.cancelFinalizedBlockSub = papi.subscribeToFinalizedBlocks(() => {
        if (this.onRefresh) {
          this.onRefresh();
        }
      });
      this.papi = papi;
      this.currentEndpoint = endpoint;
      this.loadingEndpoint = null;
      return papi;
    }).catch((error) => {
      if (generation === this.connectionGeneration) {
        this.papiLoading = null;
        this.loadingEndpoint = null;
      }
      throw error;
    });
    void this.papiLoading.then(() => {
      if (generation === this.connectionGeneration && this.onRefresh) {
        this.onRefresh();
      }
    }).catch(() => {});
    return this.papiLoading;
  }

  private async ensurePapi(): Promise<DeosPapiConnection> {
    const endpoint = getBlockchainEndpoint();
    if (this.papi && this.currentEndpoint === endpoint) {
      return this.papi;
    }
    if (this.papiLoading && this.loadingEndpoint === endpoint) {
      return await this.papiLoading;
    }
    if (!this.papi && !this.papiLoading && !this.onRefresh && !this.onTransactionProgress) {
      throw new Error("Adapter not initialized");
    }
    this.resetConnection();
    return await this.startPapiLoad(endpoint);
  }

  init(
    _overrides: Partial<SystemConfig>,
    _initialForeign: number,
    onRefresh?: () => void,
    onTransactionProgress?: (progress: TransactionProgress) => void,
  ): void {
    this.resetConnection();
    this.onRefresh = onRefresh || null;
    this.onTransactionProgress = onTransactionProgress || null;
    this.networkLogSupported = true;
    void this.startPapiLoad(getBlockchainEndpoint());
  }

  destroy(): void {
    this.connectionGeneration += 1;
    this.onRefresh = null;
    this.onTransactionProgress = null;
    this.resetConnection();
  }

  async getSnapshot(): Promise<SystemSnapshot> {
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await this.buildSystemSnapshot(snapshot);
    } catch {
      return {
        blockNumber: null,
        supply: 0n,
        priceTmc: 0n,
        priceXyk: null,
        reserveNative: 0n,
        reserveForeign: 0n,
        totalBurned: null,
        supplyLp: 0n,
        hasPool: false,
        hasNativeCurve: false,
        trackedForeignAssetCount: 0,
        minForeignSwapAmount: PRECISION,
        gravityWellRatio: 0,
        buckets: new Map(),
        bufferNative: 0n,
        bufferForeign: 0n,
        nativeAsset: {
          kind: "Native",
          assetId: null,
          symbol: "NTVE",
          isCanonical: true,
        },
        foreignAsset: {
          kind: "Foreign",
          assetId: TYPE_FOREIGN,
          symbol: "FOREIGN",
          isCanonical: false,
        },
        nativeStaking: {
          isAvailable: false,
          accountAddress: walletStore.state.selectedAddress || null,
          exchangeRate: null,
          pool: null,
          accountPosition: null,
        },
      };
    }
  }

  /**
   * ChartWidget now prefers live bounded sampling from the active session.
   * Historical/archive backfill must come from an explicit indexed provider,
   * not opportunistic RPC probing against a standard local node.
   */
  async getHistoricalPricePoints(
    _limit: number,
    _probeAmount: bigint,
  ): Promise<PricePoint[]> {
    return [];
  }

  getConnectionState(): DeosChainConnectionState {
    return this.papi?.connectionState() ?? {
      status: "unconfigured",
      label: "DEOS blockchain provider",
      endpoint: getBlockchainEndpoint() || null,
      chainName: null,
      nodeName: null,
      nodeVersion: null,
      genesisHash: null,
      finalizedBlockHash: null,
      finalizedBlockNumber: null,
      message: this.papiLoading ? "Connecting to websocket endpoint" : "PAPI connection not checked yet",
    };
  }

  async getKnownAssetBalances(): Promise<AssetBalanceProjection[]> {
    const address = walletStore.selectedAddress.trim();
    if (!address) {
      return [];
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await this.knownAssetBalancesAtSnapshot(snapshot, address);
    } catch {
      return [];
    }
  }

  async getAutomationActors(): Promise<AutomationActorSnapshot[]> {
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await Promise.all(
        KNOWN_SYSTEM_ACTORS.map(async (actor) => {
          const instance = await snapshot.typedApi.query.AAA.AaaInstances.getValue(
            BigInt(actor.aaaId),
            { at: snapshot.at },
          );
          const readiness = await snapshot.typedApi.query.AAA.AaaReadiness.getValue(
            BigInt(actor.aaaId),
            { at: snapshot.at },
          );
          const sovereignAccount =
            instance?.sovereign_account ??
            deriveSystemAaaSovereignAccount(actor.aaaId);
          const account = await snapshot.typedApi.query.System.Account.getValue(
            sovereignAccount,
            { at: snapshot.at },
          );
          return {
            aaaId: actor.aaaId,
            label: actor.label,
            role: actor.role,
            exists: instance != null,
            paused: instance?.is_paused ?? false,
            lastCycleBlock:
              instance?.last_cycle_block ?? readiness?.last_cycle_block ?? null,
            triggerLabel: automationTriggerLabel(
              readiness?.trigger ?? instance?.schedule.trigger,
            ),
            nativeBalance: account?.data?.free ?? 0n,
          } satisfies AutomationActorSnapshot;
        }),
      );
    } catch {
      return [];
    }
  }

  async getUserBalance(): Promise<{ native: bigint; foreign: bigint }> {
    const knownAssets = await this.getKnownAssetBalances();
    return {
      native: knownAssets.find((asset) => asset.presentation.kind === "Native")?.balance ?? 0n,
      foreign: knownAssets.find((asset) => asset.isPrimaryRouteAsset)?.balance ?? 0n,
    };
  }

  private selectedQuoteAccountId(): string | null {
    const accountId = walletStore.selectedAddress.trim();
    return accountId.length > 0 ? accountId : null;
  }

  private routeSnapshotPrice(
    snapshot: SystemSnapshot,
    route: "TMC" | "XYK",
  ): bigint {
    return route === "TMC" ? snapshot.priceTmc : (snapshot.priceXyk ?? 0n);
  }

  private accountRecipient(accountId: string) {
    return PapiEnum("Id", accountId);
  }

  private async resolveNativeStakingLpAssetId(): Promise<number> {
    const snapshot = await (await this.ensurePapi()).snapshot();
    const pool = await snapshot.typedApi.view.Staking.native_staking_liquidity_pool({ at: snapshot.at });
    if (!pool) {
      throw new Error("Canonical NTVE/stNTVE liquidity pool is unavailable");
    }
    return pool.lp_asset_id;
  }

  private emitTransactionProgress(progress: TransactionProgress): void {
    this.onTransactionProgress?.(progress);
  }

  private formatActionLabel(actionLabel: string, message: string): string {
    return `${actionLabel} · ${message}`;
  }

  private formatAmount(value: bigint): string {
    return (Number(value) / 1e12).toFixed(6);
  }

  private describeDispatchError(
    dispatchError: { type: string; value: unknown } | undefined,
  ): string | null {
    if (!dispatchError) {
      return null;
    }
    if (typeof dispatchError.value === "string" && dispatchError.value.length > 0) {
      return `${dispatchError.type}: ${dispatchError.value}`;
    }
    if (
      typeof dispatchError.value === "object" &&
      dispatchError.value !== null &&
      "type" in dispatchError.value &&
      typeof dispatchError.value.type === "string"
    ) {
      return `${dispatchError.type}: ${dispatchError.value.type}`;
    }
    return dispatchError.type;
  }

  private unwrapEventRecord(record: any): any {
    if (record && typeof record === "object" && "event" in record) {
      return record.event;
    }
    return record;
  }

  private formatChainEventLabel(event: any): string {
    const pallet = typeof event?.type === "string" ? event.type : "Runtime";
    const eventName = typeof event?.value?.type === "string" ? event.value.type : "Event";
    return `${pallet}.${eventName}`;
  }

  private formatChainEventMessage(event: any): string {
    if (event?.type === "AxialRouter" && event.value?.type === "SwapExecuted") {
      return `Swap ${event.value.value.mechanism.type} · in ${this.formatAmount(event.value.value.amount_in)} · out ${this.formatAmount(event.value.value.amount_out)}`;
    }
    if (event?.type === "Balances" && event.value?.type === "Transfer") {
      return `Native transfer ${this.formatAmount(event.value.value.amount)}`;
    }
    if (event?.type === "Assets" && event.value?.type === "Transferred") {
      return `Asset ${event.value.value.asset_id} transfer ${this.formatAmount(event.value.value.amount)}`;
    }
    if (event?.type === "Governance") {
      return this.formatChainEventLabel(event);
    }
    return this.formatChainEventLabel(event);
  }

  private classifyChainEvent(event: any): LogEntry["type"] {
    if (event?.type === "System" && event.value?.type === "ExtrinsicFailed") {
      return "error";
    }
    return "info";
  }

  private buildTransactionHighlights(events: Array<any> | undefined): string[] {
    if (!events || events.length === 0) {
      return [];
    }
    const highlights: string[] = [];
    for (const event of events) {
      if (event.type === "AxialRouter" && event.value?.type === "SwapExecuted") {
        highlights.push(
          `Swap ${event.value.value.mechanism.type} · in ${this.formatAmount(event.value.value.amount_in)} · out ${this.formatAmount(event.value.value.amount_out)}`,
        );
        continue;
      }
      if (event.type === "AxialRouter" && event.value?.type === "FeeCollected") {
        highlights.push(
          `Router fee ${this.formatAmount(event.value.value.amount)} ${event.value.value.asset.type}`,
        );
        continue;
      }
      if (event.type === "Balances" && event.value?.type === "Transfer") {
        highlights.push(
          `Native transfer ${this.formatAmount(event.value.value.amount)}`,
        );
        continue;
      }
      if (event.type === "Assets" && event.value?.type === "Transferred") {
        highlights.push(
          `Asset ${event.value.value.asset_id} transfer ${this.formatAmount(event.value.value.amount)}`,
        );
        continue;
      }
      if (
        event.type === "TransactionPayment" &&
        event.value?.type === "TransactionFeePaid"
      ) {
        highlights.push(
          `Tx fee ${this.formatAmount(event.value.value.actual_fee)}`,
        );
      }
    }
    return highlights.slice(0, 4);
  }

  private async watchSubmittedTransaction(
    watcher: { subscribe: (observer: {
      next: (event: any) => void;
      error: (error: unknown) => void;
    }) => { unsubscribe(): void } },
    actionLabel: string,
  ): Promise<{ txHash: string; blockNumber: number; ok: boolean; dispatchError: string | null }> {
    return await new Promise((resolve, reject) => {
      const subscription = watcher.subscribe({
        next: (event) => {
          switch (event.type) {
            case "signed":
              this.emitTransactionProgress({
                kind: "signed",
                txHash: event.txHash,
                message: this.formatActionLabel(actionLabel, `Signed ${event.txHash}`),
                actionLabel,
              });
              return;
            case "broadcasted":
              this.emitTransactionProgress({
                kind: "broadcasted",
                txHash: event.txHash,
                message: this.formatActionLabel(actionLabel, `Broadcasted ${event.txHash}`),
                actionLabel,
              });
              return;
            case "txBestBlocksState":
              if (!event.found) {
                this.emitTransactionProgress({
                  kind: "broadcasted",
                  txHash: event.txHash,
                  message: event.isValid
                    ? this.formatActionLabel(actionLabel, `Broadcasted ${event.txHash}; waiting for inclusion`)
                    : this.formatActionLabel(actionLabel, `Transaction ${event.txHash} became invalid before inclusion`),
                  actionLabel,
                });
                if (!event.isValid) {
                  subscription.unsubscribe();
                  reject(new Error(`Transaction ${event.txHash} became invalid before inclusion`));
                }
                return;
              }
              this.emitTransactionProgress({
                kind: "best",
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                eventsCount: event.events.length,
                message: event.ok
                  ? this.formatActionLabel(actionLabel, `Included in best block #${event.block.number}`)
                  : this.formatActionLabel(actionLabel, `Included in best block #${event.block.number} with dispatch error`),
                actionLabel,
                highlights: this.buildTransactionHighlights(event.events),
              });
              return;
            case "finalized":
              this.emitTransactionProgress({
                kind: "finalized",
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                eventsCount: event.events.length,
                dispatchError: this.describeDispatchError(event.dispatchError),
                message: event.ok
                  ? this.formatActionLabel(actionLabel, `Finalized in block #${event.block.number}`)
                  : this.formatActionLabel(actionLabel, `Finalized with dispatch error in block #${event.block.number}`),
                actionLabel,
                highlights: this.buildTransactionHighlights(event.events),
              });
              subscription.unsubscribe();
              resolve({
                txHash: event.txHash,
                blockNumber: event.block.number,
                ok: event.ok,
                dispatchError: this.describeDispatchError(event.dispatchError),
              });
              return;
          }
        },
        error: (error) => {
          subscription.unsubscribe();
          const message = error instanceof Error ? error.message : "Live transaction failed";
          this.emitTransactionProgress({
            kind: "error",
            txHash: null,
            message: this.formatActionLabel(actionLabel, message),
            actionLabel,
          });
          reject(error);
        },
      });
    });
  }

  private async submitSigned(
    submitter: (
      snapshot: DeosChainSnapshot,
      accountId: string,
      signer: NonNullable<Awaited<ReturnType<typeof connectDeosSigner>>>,
    ) => { subscribe: (observer: {
      next: (event: any) => void;
      error: (error: unknown) => void;
    }) => { unsubscribe(): void } },
    missingSignerMessage: string,
    actionLabel: string,
  ): Promise<{ txHash: string; blockNumber: number; ok: boolean; dispatchError: string | null }> {
    const papi = await this.ensurePapi();
    const accountId = walletStore.selectedAddress.trim();
    if (!accountId) {
      throw new Error("Select an account before submitting a live transaction");
    }
    const signer = await connectDeosSigner(
      accountId,
      DEFAULT_DEOS_DAPP_NAME,
    );
    if (!signer) {
      throw new Error(missingSignerMessage);
    }
    try {
      const snapshot = await papi.snapshot();
      const watcher = submitter(snapshot, accountId, signer);
      const result = await this.watchSubmittedTransaction(watcher, actionLabel);
      await papi.syncConnectionState();
      return result;
    } finally {
      signer.disconnect();
    }
  }

  private async submitRouterSwap(
    from: RuntimeAssetKind,
    to: RuntimeAssetKind,
    amountIn: bigint,
    minAmountOut: bigint,
  ): Promise<SystemSnapshot> {
    try {
      await this.submitSigned(
        (snapshot, accountId, signer) => {
          return snapshot.typedApi.tx.AxialRouter.swap({
            from,
            to,
            amount_in: amountIn,
            min_amount_out: minAmountOut,
            recipient: accountId,
            deadline: snapshot.finalizedBlockNumber + 50,
          }).signSubmitAndWatch(signer.signer);
        },
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        `${from.type}->${to.type} swap`,
      );
      return await this.getSnapshot();
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Live swap submission failed",
      );
    }
  }

  async buyNative(foreignAmount: bigint, slippageBps = 50): Promise<SwapResult> {
    if (foreignAmount <= 0n) {
      throw new Error("Buy amount must be greater than zero");
    }
    const snapshotBefore = await this.getSnapshot();
    if (foreignAmount < snapshotBefore.minForeignSwapAmount) {
      throw new Error(
        `Buy amount is below runtime minimum ${snapshotBefore.minForeignSwapAmount.toString()}`,
      );
    }
    const quote = await this.getQuoteBuy(foreignAmount);
    if (!quote) {
      throw new Error("No live buy route is available for this size right now");
    }
    const priceBefore = this.routeSnapshotPrice(snapshotBefore, quote.route);
    const minAmountOut = quote.out > 0n
      ? (quote.out * BigInt(Math.max(0, 10_000 - slippageBps))) / 10_000n
      : 0n;
    const snapshotAfter = await this.submitRouterSwap(
      await this.resolvePrimaryForeignAsset(await (await this.ensurePapi()).snapshot()),
      NATIVE_ASSET,
      foreignAmount,
      minAmountOut,
    );
    const priceAfter = this.routeSnapshotPrice(snapshotAfter, quote.route);
    return {
      route: quote.route,
      native_out: quote.out,
      foreign_in: foreignAmount,
      foreign_router_fee: quote.fee,
      price_before: priceBefore,
      price_after: priceAfter,
    };
  }

  async sellNative(nativeAmount: bigint, slippageBps = 50): Promise<SwapResult> {
    if (nativeAmount <= 0n) {
      throw new Error("Sell amount must be greater than zero");
    }
    const quote = await this.getQuoteSell(nativeAmount);
    if (!quote) {
      throw new Error("No live sell route is available for this size right now");
    }
    const snapshotBefore = await this.getSnapshot();
    const priceBefore = this.routeSnapshotPrice(snapshotBefore, quote.route);
    const minAmountOut = quote.out > 0n
      ? (quote.out * BigInt(Math.max(0, 10_000 - slippageBps))) / 10_000n
      : 0n;
    const foreignAsset = await this.resolvePrimaryForeignAsset(
      await (await this.ensurePapi()).snapshot(),
    );
    const snapshotAfter = await this.submitRouterSwap(
      NATIVE_ASSET,
      foreignAsset,
      nativeAmount,
      minAmountOut,
    );
    const priceAfter = this.routeSnapshotPrice(snapshotAfter, quote.route);
    return {
      route: quote.route,
      foreign_out: quote.out,
      native_in: nativeAmount,
      native_router_fee: quote.fee,
      price_before: priceBefore,
      price_after: priceAfter,
    };
  }

  async depositForeign(_amount: bigint): Promise<void> {
    return;
  }

  async claimNominationReward(epoch: number): Promise<void> {
    if (!Number.isInteger(epoch) || epoch < 0) {
      throw new Error("Reward epoch must be a non-negative integer");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.claim_nomination_reward({
          epoch,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        `Claim nomination reward #${epoch}`,
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native nomination reward claim failed",
      );
    }
  }

  async claimAndCompoundNominationReward(epoch: number, operator: string): Promise<void> {
    if (!Number.isInteger(epoch) || epoch < 0) {
      throw new Error("Reward epoch must be a non-negative integer");
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error("Collator/operator address is required for compound locking");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.claim_and_compound_nomination_reward({
          epoch,
          operator: normalizedOperator,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        `Compound nomination reward #${epoch}`,
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native nomination reward compound failed",
      );
    }
  }

  async lockNativeLpForCollator(amount: bigint, operator: string): Promise<void> {
    if (amount <= 0n) {
      throw new Error("LP lock amount must be greater than zero");
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error("Collator/operator address is required");
    }
    try {
      const lpAssetId = await this.resolveNativeStakingLpAssetId();
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.lock_native_lp_for_collator({
          lp_asset_id: lpAssetId,
          amount,
          operator: normalizedOperator,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Lock native staking LP",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native LP collator lock failed",
      );
    }
  }

  async requestUnlockNativeLp(operator: string, amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error("LP unlock amount must be greater than zero");
    }
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error("Collator/operator address is required");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.request_unlock_native_lp({
          operator: normalizedOperator,
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Request native LP unlock",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native LP unlock request failed",
      );
    }
  }

  async withdrawUnlockedNativeLp(operator: string): Promise<void> {
    const normalizedOperator = operator.trim();
    if (normalizedOperator.length === 0) {
      throw new Error("Collator/operator address is required");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.withdraw_unlocked_native_lp({
          operator: normalizedOperator,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Withdraw unlocked native LP",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native LP withdrawal failed",
      );
    }
  }

  async redelegateNativeLp(fromOperator: string, toOperator: string, amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error("LP redelegation amount must be greater than zero");
    }
    const normalizedFromOperator = fromOperator.trim();
    const normalizedToOperator = toOperator.trim();
    if (normalizedFromOperator.length === 0 || normalizedToOperator.length === 0) {
      throw new Error("Both source and target collator/operator addresses are required");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.redelegate_native_lp({
          from_operator: normalizedFromOperator,
          to_operator: normalizedToOperator,
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Redelegate native LP",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native LP redelegation failed",
      );
    }
  }

  async lockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error("Governance LP lock amount must be greater than zero");
    }
    try {
      const lpAssetId = await this.resolveNativeStakingLpAssetId();
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.lock_native_lp_for_governance({
          lp_asset_id: lpAssetId,
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Lock governance native LP",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance LP lock failed",
      );
    }
  }

  async requestUnlockNativeLpForGovernance(amount: bigint): Promise<void> {
    if (amount <= 0n) {
      throw new Error("Governance LP unlock amount must be greater than zero");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.request_unlock_native_lp_for_governance({
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Request governance LP unlock",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance LP unlock request failed",
      );
    }
  }

  async withdrawUnlockedNativeLpForGovernance(): Promise<void> {
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.withdraw_unlocked_native_lp_for_governance().signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Withdraw governance LP",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance LP withdrawal failed",
      );
    }
  }

  async lockNativeAssetForGovernance(assetId: number, amount: bigint): Promise<void> {
    if (!Number.isFinite(assetId)) {
      throw new Error("Governance asset id is required");
    }
    if (amount <= 0n) {
      throw new Error("Governance asset lock amount must be greater than zero");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.lock_native_asset_for_governance({
          asset_id: assetId,
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Lock governance native asset",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance asset lock failed",
      );
    }
  }

  async requestUnlockNativeAssetForGovernance(assetId: number, amount: bigint): Promise<void> {
    if (!Number.isFinite(assetId)) {
      throw new Error("Governance asset id is required");
    }
    if (amount <= 0n) {
      throw new Error("Governance asset unlock amount must be greater than zero");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.request_unlock_native_asset_for_governance({
          asset_id: assetId,
          amount,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Request governance asset unlock",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance asset unlock request failed",
      );
    }
  }

  async withdrawUnlockedNativeAssetForGovernance(assetId: number): Promise<void> {
    if (!Number.isFinite(assetId)) {
      throw new Error("Governance asset id is required");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => snapshot.typedApi.tx.Staking.withdraw_unlocked_native_asset_for_governance({
          asset_id: assetId,
        }).signSubmitAndWatch(signer.signer),
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        "Withdraw governance asset",
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Native governance asset withdrawal failed",
      );
    }
  }

  async transferAsset(
    asset: TransferAssetKey,
    recipient: string,
    amount: bigint,
  ): Promise<void> {
    if (amount <= 0n) {
      throw new Error("Transfer amount must be greater than zero");
    }
    const normalizedRecipient = recipient.trim();
    if (normalizedRecipient.length === 0) {
      throw new Error("Recipient address is required");
    }
    try {
      await this.submitSigned(
        (snapshot, _accountId, signer) => {
          if (asset === "native") {
            return snapshot.typedApi.tx.Balances.transfer_keep_alive({
              dest: this.accountRecipient(normalizedRecipient),
              value: amount,
            }).signSubmitAndWatch(signer.signer);
          }
          const assetIdPromise = asset === "foreign"
            ? this.resolvePrimaryForeignAsset(snapshot).then((resolvedForeignAsset) => {
                if (!isAssetWithId(resolvedForeignAsset)) {
                  throw new Error("No transferable primary route asset is registered yet");
                }
                return resolvedForeignAsset.value;
              })
            : Promise.resolve(Number(asset.slice("asset:".length)));
          return {
            subscribe: (observer) => {
              let nestedSubscription: { unsubscribe(): void } | null = null;
              let cancelled = false;
              void assetIdPromise
                .then((resolvedAssetId) => {
                  if (!Number.isFinite(resolvedAssetId)) {
                    throw new Error("Selected asset is not transferable");
                  }
                  if (cancelled) {
                    return;
                  }
                  nestedSubscription = snapshot.typedApi.tx.Assets.transfer_keep_alive({
                    id: resolvedAssetId,
                    target: this.accountRecipient(normalizedRecipient),
                    amount,
                  }).signSubmitAndWatch(signer.signer).subscribe(observer);
                })
                .catch((error) => {
                  observer.error(error);
                });
              return {
                unsubscribe: () => {
                  cancelled = true;
                  nestedSubscription?.unsubscribe();
                },
              };
            },
          };
        },
        `No signer is available for ${walletStore.selectedAddress.trim()}. Use an injected wallet account or a built-in Zombienet dev identity.`,
        `${asset} transfer`,
      );
    } catch (error) {
      throw new Error(
        error instanceof Error ? error.message : "Live transfer failed",
      );
    }
  }

  async getQuoteBuy(foreignAmount: bigint): Promise<Quote | null> {
    if (foreignAmount <= 0n) {
      return null;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      return await this.quoteBuyAtSnapshot(snapshot, foreignAmount);
    } catch {
      return null;
    }
  }

  async getQuoteSell(nativeAmount: bigint): Promise<Quote | null> {
    if (nativeAmount <= 0n) {
      return null;
    }
    const accountId = this.selectedQuoteAccountId();
    if (!accountId) {
      return null;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const foreignAsset = await this.resolvePrimaryForeignAsset(snapshot);
      const authoritativeQuote = toOptionalValue(
        await snapshot.typedApi.view.AxialRouter.quote_exact_input(
          accountId,
          NATIVE_ASSET,
          foreignAsset,
          nativeAmount,
          { at: snapshot.at },
        ),
      );
      if (!authoritativeQuote || authoritativeQuote.amount_out <= 0n) {
        return null;
      }
      return {
        out: authoritativeQuote.amount_out,
        route: routeFromMechanism(authoritativeQuote.mechanism),
        effectivePrice:
          Number(authoritativeQuote.amount_out) / Number(nativeAmount),
        fee: authoritativeQuote.router_fee,
        tmcOut: 0n,
        xykOut: authoritativeQuote.amount_out,
        isSell: true,
      };
    } catch {
      return null;
    }
  }

  async getRecentNetworkLog(limit: number): Promise<LogEntry[]> {
    if (limit <= 0 || !this.networkLogSupported) {
      return [];
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const records = await snapshot.typedApi.query.System.Events.getValue({ at: snapshot.at });
      if (!records || records.length === 0) {
        return [];
      }
      const entries: LogEntry[] = [];
      for (let index = records.length - 1; index >= 0 && entries.length < limit; index -= 1) {
        const event = this.unwrapEventRecord(records[index]);
        if (!event) {
          continue;
        }
        const label = this.formatChainEventLabel(event);
        entries.push({
          id: `${snapshot.finalizedBlockNumber}-${index}-${label}`,
          step: snapshot.finalizedBlockNumber,
          blockNumber: snapshot.finalizedBlockNumber,
          message: this.formatChainEventMessage(event),
          type: this.classifyChainEvent(event),
          label,
          accountId: null,
        });
      }
      return entries;
    } catch {
      this.networkLogSupported = false;
      return [];
    }
  }

  async getEffectiveMintPrice(probeAmount: bigint): Promise<number> {
    if (probeAmount <= 0n) {
      return 0;
    }
    try {
      const snapshot = await (await this.ensurePapi()).snapshot();
      const out = await this.tmcMintQuote(snapshot, probeAmount);
      return out > 0n ? Number(probeAmount) / Number(out) : 0;
    } catch {
      return 0;
    }
  }
}
