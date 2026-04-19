import type { Adapter } from "$lib/adapters/types";
import {
  fromClientBoundedProjection,
  type ReadModelValue,
} from "$lib/shared/read-model";
import type {
  AssetBalanceProjection,
  AssetPresentation,
  SystemSnapshot,
  TransferAssetKey,
} from "$lib/shared/types";

export type { TransferAssetKey } from "$lib/shared/types";

export type KnownClientAssetKey = TransferAssetKey;
export type KnownClientAsset = {
  key: KnownClientAssetKey;
  transferKey: TransferAssetKey | null;
  presentation: AssetPresentation;
  symbol: string;
  balance: bigint;
  kind: AssetPresentation["kind"];
  assetId: number | null;
  isCanonical: boolean;
  isPrimaryRouteAsset: boolean;
  transferEnabled: boolean;
};

type PortfolioBridge = {
  getAdapter: () => Adapter;
  getSnapshot: () => SystemSnapshot | null;
  refreshSystem: () => Promise<void>;
};

function fallbackAsset(
  key: TransferAssetKey,
  symbol: string,
  kind: AssetPresentation["kind"],
  isCanonical: boolean,
  isPrimaryRouteAsset: boolean,
): KnownClientAsset {
  const presentation: AssetPresentation = {
    kind,
    assetId: null,
    symbol,
    isCanonical,
  };
  return {
    key,
    transferKey: key,
    presentation,
    symbol,
    balance: 0n,
    kind,
    assetId: null,
    isCanonical,
    isPrimaryRouteAsset,
    transferEnabled: true,
  };
}

function knownAssetKey(
  projection: AssetBalanceProjection,
): KnownClientAssetKey {
  if (projection.presentation.kind === "Native") {
    return "native";
  }
  if (projection.isPrimaryRouteAsset) {
    return "foreign";
  }
  return `asset:${projection.presentation.assetId ?? 0}`;
}

function mapKnownAsset(
  projection: AssetBalanceProjection,
): KnownClientAsset {
  const key = knownAssetKey(projection);
  return {
    key,
    transferKey: key,
    presentation: projection.presentation,
    symbol: projection.presentation.symbol,
    balance: projection.balance,
    kind: projection.presentation.kind,
    assetId: projection.presentation.assetId,
    isCanonical: projection.presentation.isCanonical,
    isPrimaryRouteAsset: projection.isPrimaryRouteAsset,
    transferEnabled: true,
  };
}

class PortfolioStore {
  userBalance: { native: bigint; foreign: bigint } = $state({
    native: 0n,
    foreign: 0n,
  });
  knownAssetBalances: AssetBalanceProjection[] = $state([]);
  private bridge: PortfolioBridge | null = null;

  bind(bridge: PortfolioBridge) {
    this.bridge = bridge;
  }

  reset() {
    this.userBalance = {
      native: 0n,
      foreign: 0n,
    };
    this.knownAssetBalances = [];
  }

  setUserBalance(balance: { native: bigint; foreign: bigint }) {
    this.userBalance = balance;
  }

  setKnownAssetBalances(balances: AssetBalanceProjection[]) {
    this.knownAssetBalances = balances;
    const native = balances.find((asset) => asset.presentation.kind === "Native")?.balance ?? 0n;
    const primaryRoute = balances.find((asset) => asset.isPrimaryRouteAsset)?.balance ?? 0n;
    this.userBalance = {
      native,
      foreign: primaryRoute,
    };
  }

  private adapter(): Adapter | null {
    return this.bridge?.getAdapter() ?? null;
  }

  private snapshot(): SystemSnapshot | null {
    return this.bridge?.getSnapshot() ?? null;
  }

  get knownAssets(): KnownClientAsset[] {
    if (this.knownAssetBalances.length > 0) {
      return this.knownAssetBalances.map(mapKnownAsset);
    }
    const snapshot = this.snapshot();
    if (!snapshot) {
      return [
        fallbackAsset("native", "NTVE", "Native", true, false),
        fallbackAsset("foreign", "FOREIGN", "Foreign", false, true),
      ];
    }
    return [
      mapKnownAsset({
        presentation: snapshot.nativeAsset,
        balance: this.userBalance.native,
        isPrimaryRouteAsset: false,
      }),
      mapKnownAsset({
        presentation: snapshot.foreignAsset,
        balance: this.userBalance.foreign,
        isPrimaryRouteAsset: true,
      }),
    ];
  }

  get knownAssetsView(): ReadModelValue<KnownClientAsset[]> {
    return fromClientBoundedProjection(
      this.knownAssets,
      "portfolioStore.knownAssets <- bounded wallet balances + snapshot asset presentations",
      "live",
      {
        asOfBlock: this.snapshot()?.blockNumber ?? undefined,
      },
    );
  }

  get transferAssets(): KnownClientAsset[] {
    return this.knownAssets.filter((asset) => asset.transferEnabled);
  }

  findAsset(key: TransferAssetKey): KnownClientAsset {
    return this.transferAssets.find((asset) => asset.transferKey === key) ?? this.knownAssets[0];
  }

  async depositForeign(amount: bigint) {
    const adapter = this.adapter();
    if (!adapter || !this.bridge) {
      throw new Error("Portfolio bridge not initialized");
    }
    await adapter.depositForeign(amount);
    if (adapter.getKnownAssetBalances) {
      this.setKnownAssetBalances(await adapter.getKnownAssetBalances());
      return;
    }
    this.userBalance = await adapter.getUserBalance();
  }

  async transferAsset(
    asset: TransferAssetKey,
    recipient: string,
    amount: bigint,
  ) {
    const adapter = this.adapter();
    if (!adapter || !this.bridge) {
      throw new Error("Portfolio bridge not initialized");
    }
    if (!adapter.transferAsset) {
      throw new Error("Live transfer surface not available in the current adapter");
    }
    await adapter.transferAsset(asset, recipient, amount);
    await this.bridge.refreshSystem();
  }
}

export const portfolioStore = new PortfolioStore();
