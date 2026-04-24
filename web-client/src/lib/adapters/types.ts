import type {
  AssetBalanceProjection,
  AutomationActorSnapshot,
  LogEntry,
  NativeCollatorLpPositionProjection,
  NativeGovernanceCustodyPositionProjection,
  SystemConfig,
  SystemSnapshot,
  SwapResult,
  Quote,
  PricePoint,
  TransactionProgress,
  TransferAssetKey,
} from "$lib/shared/types";
import type { DeosChainConnectionState } from "$lib/adapters/blockchain/deos";

export type Adapter = {
  init(
    overrides: Partial<SystemConfig>,
    initialForeign: number,
    onRefresh?: () => void,
    onTransactionProgress?: (progress: TransactionProgress) => void,
  ): void;
  destroy?(): void;
  getSnapshot(): SystemSnapshot | Promise<SystemSnapshot>;
  getConnectionState?(): DeosChainConnectionState | Promise<DeosChainConnectionState>;
  getUserBalance(): { native: bigint; foreign: bigint } | Promise<{ native: bigint; foreign: bigint }>;
  getKnownAssetBalances?(): AssetBalanceProjection[] | Promise<AssetBalanceProjection[]>;
  getAutomationActors?(): AutomationActorSnapshot[] | Promise<AutomationActorSnapshot[]>;
  buyNative(foreignAmount: bigint, slippageBps?: number): SwapResult | Promise<SwapResult>;
  sellNative(nativeAmount: bigint, slippageBps?: number): SwapResult | Promise<SwapResult>;
  depositForeign(amount: bigint): void | Promise<void>;
  transferAsset?(asset: TransferAssetKey, recipient: string, amount: bigint): void | Promise<void>;
  claimNominationReward?(epoch: number): void | Promise<void>;
  claimAndCompoundNominationReward?(epoch: number, operator: string): void | Promise<void>;
  lockNativeLpForCollator?(amount: bigint, operator: string): void | Promise<void>;
  requestUnlockNativeLp?(operator: string, amount: bigint): void | Promise<void>;
  withdrawUnlockedNativeLp?(operator: string): void | Promise<void>;
  redelegateNativeLp?(fromOperator: string, toOperator: string, amount: bigint): void | Promise<void>;
  lockNativeLpForGovernance?(amount: bigint): void | Promise<void>;
  requestUnlockNativeLpForGovernance?(amount: bigint): void | Promise<void>;
  withdrawUnlockedNativeLpForGovernance?(): void | Promise<void>;
  lockNativeAssetForGovernance?(assetId: number, amount: bigint): void | Promise<void>;
  requestUnlockNativeAssetForGovernance?(assetId: number, amount: bigint): void | Promise<void>;
  withdrawUnlockedNativeAssetForGovernance?(assetId: number): void | Promise<void>;
  getNativeCollatorLpPosition?(operator: string): NativeCollatorLpPositionProjection | null | Promise<NativeCollatorLpPositionProjection | null>;
  getNativeGovernanceCustodyPosition?(assetId: number): NativeGovernanceCustodyPositionProjection | null | Promise<NativeGovernanceCustodyPositionProjection | null>;
  getNativeNominationRewardClaimable?(epoch: number): bigint | null | Promise<bigint | null>;
  getQuoteBuy(foreignAmount: bigint): Quote | null | Promise<Quote | null>;
  getQuoteSell(nativeAmount: bigint): Quote | null | Promise<Quote | null>;
  getEffectiveMintPrice(probeAmount: bigint): number | Promise<number>;
  getHistoricalPricePoints?(limit: number, probeAmount: bigint): PricePoint[] | Promise<PricePoint[]>;
  getRecentNetworkLog?(limit: number): LogEntry[] | Promise<LogEntry[]>;
};
