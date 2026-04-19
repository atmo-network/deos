import type {
  AssetBalanceProjection,
  AutomationActorSnapshot,
  LogEntry,
  SystemConfig,
  SystemSnapshot,
  SwapResult,
  Quote,
  PricePoint,
  TransactionProgress,
  TransferAssetKey,
} from "$lib/shared/types";
import type { TmctolChainConnectionState } from "$lib/adapters/blockchain/deos";

export type Adapter = {
  init(
    overrides: Partial<SystemConfig>,
    initialForeign: number,
    onRefresh?: () => void,
    onTransactionProgress?: (progress: TransactionProgress) => void,
  ): void;
  destroy?(): void;
  getSnapshot(): SystemSnapshot | Promise<SystemSnapshot>;
  getConnectionState?(): TmctolChainConnectionState | Promise<TmctolChainConnectionState>;
  getUserBalance(): { native: bigint; foreign: bigint } | Promise<{ native: bigint; foreign: bigint }>;
  getKnownAssetBalances?(): AssetBalanceProjection[] | Promise<AssetBalanceProjection[]>;
  getAutomationActors?(): AutomationActorSnapshot[] | Promise<AutomationActorSnapshot[]>;
  buyNative(foreignAmount: bigint, slippageBps?: number): SwapResult | Promise<SwapResult>;
  sellNative(nativeAmount: bigint, slippageBps?: number): SwapResult | Promise<SwapResult>;
  depositForeign(amount: bigint): void | Promise<void>;
  transferAsset?(asset: TransferAssetKey, recipient: string, amount: bigint): void | Promise<void>;
  getQuoteBuy(foreignAmount: bigint): Quote | null | Promise<Quote | null>;
  getQuoteSell(nativeAmount: bigint): Quote | null | Promise<Quote | null>;
  getEffectiveMintPrice(probeAmount: bigint): number | Promise<number>;
  getHistoricalPricePoints?(limit: number, probeAmount: bigint): PricePoint[] | Promise<PricePoint[]>;
  getRecentNetworkLog?(limit: number): LogEntry[] | Promise<LogEntry[]>;
};
