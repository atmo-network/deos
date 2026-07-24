/*
Domain: Live adapter contract
Owns: Frontend adapter runtime context and capability interfaces connecting stores/widgets to providers.
Excludes: Concrete provider implementations, widget composition, and domain store state.
Zone: Adapter public contract; may reference domain projection types but must not import concrete adapters/widgets.
*/
import type { DeosChainConnectionState } from '$lib/adapters/blockchain/deos';
import type { AutomationActorSnapshot } from '$lib/automation/types';
import type { LogEntry, TransactionProgress } from '$lib/log/types';
import type { PricePoint, Quote, SwapResult } from '$lib/market/types';
import type {
  AssetBalanceProjection,
  TransferAssetKey,
} from '$lib/portfolio/types';
import type {
  NativeCollatorLpPositionProjection,
  NativeGovernanceCustodyPositionProjection,
} from '$lib/staking/types';
import type { SystemConfig, SystemSnapshot } from '$lib/system/types';

export type AdapterRuntimeContext = {
  getEndpoint: () => string;
  getSelectedAddress: () => string;
  dappName: string;
};

export type AdapterLifecycle = {
  init(
    overrides: Partial<SystemConfig>,
    initialForeign: number,
    context: AdapterRuntimeContext,
    onRefresh?: () => void,
    onTransactionProgress?: (progress: TransactionProgress) => void,
  ): void;
  destroy?(): void;
};

export type SystemReadAdapter = {
  getSnapshot(): SystemSnapshot | Promise<SystemSnapshot>;
  getConnectionState?():
    | DeosChainConnectionState
    | Promise<DeosChainConnectionState>;
  getUserBalance():
    | { native: bigint; foreign: bigint }
    | Promise<{ native: bigint; foreign: bigint }>;
};

export type PortfolioAdapter = {
  getKnownAssetBalances?():
    | AssetBalanceProjection[]
    | Promise<AssetBalanceProjection[]>;
  transferAsset?(
    asset: TransferAssetKey,
    recipient: string,
    amount: bigint,
  ): void | Promise<void>;
  depositForeign(amount: bigint): void | Promise<void>;
};

export type AutomationAdapter = {
  getAutomationActors?():
    | AutomationActorSnapshot[]
    | Promise<AutomationActorSnapshot[]>;
};

export type MarketAdapter = {
  buyNative(
    foreignAmount: bigint,
    slippageBps?: number,
  ): SwapResult | Promise<SwapResult>;
  sellNative(
    nativeAmount: bigint,
    slippageBps?: number,
  ): SwapResult | Promise<SwapResult>;
  getQuoteBuy(foreignAmount: bigint): Quote | null | Promise<Quote | null>;
  getQuoteSell(nativeAmount: bigint): Quote | null | Promise<Quote | null>;
  estimateSwapNetworkFee?(
    direction: 'buy' | 'sell',
    amountIn: bigint,
    minAmountOut: bigint,
  ): bigint | null | Promise<bigint | null>;
  getEffectiveMintPrice(probeAmount: bigint): number | Promise<number>;
  getHistoricalPricePoints?(
    limit: number,
    probeAmount: bigint,
  ): PricePoint[] | Promise<PricePoint[]>;
};

export type StakingAdapter = {
  claimNominationReward?(epoch: number): void | Promise<void>;
  claimAndCompoundNominationReward?(
    epoch: number,
    operator: string,
  ): void | Promise<void>;
  lockNativeLpForCollator?(
    amount: bigint,
    operator: string,
  ): void | Promise<void>;
  requestUnlockNativeLp?(
    operator: string,
    amount: bigint,
  ): void | Promise<void>;
  withdrawUnlockedNativeLp?(operator: string): void | Promise<void>;
  redelegateNativeLp?(
    fromOperator: string,
    toOperator: string,
    amount: bigint,
  ): void | Promise<void>;
  lockNativeLpForGovernance?(amount: bigint): void | Promise<void>;
  requestUnlockNativeLpForGovernance?(amount: bigint): void | Promise<void>;
  withdrawUnlockedNativeLpForGovernance?(): void | Promise<void>;
  lockNativeAssetForGovernance?(
    assetId: number,
    amount: bigint,
  ): void | Promise<void>;
  requestUnlockNativeAssetForGovernance?(
    assetId: number,
    amount: bigint,
  ): void | Promise<void>;
  withdrawUnlockedNativeAssetForGovernance?(
    assetId: number,
  ): void | Promise<void>;
  getNativeCollatorLpPosition?(
    operator: string,
  ):
    | NativeCollatorLpPositionProjection
    | null
    | Promise<NativeCollatorLpPositionProjection | null>;
  getNativeGovernanceCustodyPosition?(
    assetId: number,
  ):
    | NativeGovernanceCustodyPositionProjection
    | null
    | Promise<NativeGovernanceCustodyPositionProjection | null>;
  getNativeNominationRewardClaimable?(
    epoch: number,
  ): bigint | null | Promise<bigint | null>;
};

export type LogFeedAdapter = {
  getRecentNetworkLog?(limit: number): LogEntry[] | Promise<LogEntry[]>;
};

export type Adapter = AdapterLifecycle &
  SystemReadAdapter &
  PortfolioAdapter &
  AutomationAdapter &
  MarketAdapter &
  StakingAdapter &
  LogFeedAdapter;
