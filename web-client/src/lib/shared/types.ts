export type MintShareConfig = {
  user_ppb: bigint;
  tol_ppb: bigint;
};

export type TmcConfig = {
  price_initial: bigint;
  slope: bigint;
  mint_shares: MintShareConfig;
};

export type XykConfig = {
  fee_xyk_ppb: bigint;
};

export type RouterConfig = {
  fee_router_ppb: bigint;
  min_swap_foreign: bigint;
  min_initial_foreign: bigint;
};

export type TolBucketConfig = Record<string, bigint>;

export type TolConfig = {
  bucket_shares: TolBucketConfig;
};

export type SystemConfig = {
  router: RouterConfig;
  xyk: XykConfig;
  tmc: TmcConfig;
  tol: TolConfig;
};

export type BucketBalance = {
  lp_tokens: bigint;
  contributed_native: bigint;
  contributed_foreign: bigint;
};

export type TolResult = {
  total_lp_minted: bigint;
  total_native_used: bigint;
  total_foreign_used: bigint;
  [bucketKey: `bucket_${string}`]: {
    lp_tokens: bigint;
    contributed_native: bigint;
    contributed_foreign: bigint;
  };
};

export type SwapResult = {
  route: "TMC" | "XYK";
  native_out?: bigint;
  foreign_out?: bigint;
  native_in?: bigint;
  foreign_in?: bigint;
  foreign_net?: bigint;
  native_net?: bigint;
  foreign_router_fee?: bigint;
  native_router_fee?: bigint;
  price_before: bigint;
  price_after: bigint;
  price_impact_ppb?: bigint;
  tol?: TolResult;
};

export type Quote = {
  out: bigint;
  route: "TMC" | "XYK";
  effectivePrice: number;
  fee: bigint;
  tmcOut: bigint;
  xykOut: bigint;
  isSell: boolean;
};

export type AssetPresentation = {
  kind: "Native" | "Local" | "Foreign";
  assetId: number | null;
  symbol: string;
  isCanonical: boolean;
};

export type TransferAssetKey = "native" | "foreign" | `asset:${number}`;

export type AssetBalanceProjection = {
  presentation: AssetPresentation;
  balance: bigint;
  isPrimaryRouteAsset: boolean;
};

export type AutomationActorSnapshot = {
  aaaId: number;
  label: string;
  role: string;
  exists: boolean;
  paused: boolean;
  lastCycleBlock: number | null;
  triggerLabel: string;
  nativeBalance: bigint;
};

export type NativeStakingPoolProjection = {
  nativeAssetId: number;
  stakedAssetId: number;
  lpAssetId: number;
  reserveNative: bigint;
  reserveStaked: bigint;
  lpTotalIssuance: bigint;
};

export type NativeLockedLpPositionProjection = {
  totalLockedLp: bigint;
  collatorLockedLp: bigint;
  governanceLockedLp: bigint;
  conservativeNativeValue: bigint | null;
};

export type NativeCollatorLpPositionProjection = {
  lpAssetId: number | null;
  lockedLp: bigint;
  pendingUnlockLp: bigint;
  pendingUnlockBlock: number | null;
  conservativeNativeValue: bigint | null;
};

export type NativeGovernanceCustodyPositionProjection = {
  lpAssetId: number | null;
  governanceLockedLp: bigint;
  pendingGovernanceLpUnlock: bigint;
  pendingGovernanceLpUnlockBlock: number | null;
  assetId: number;
  assetLocked: bigint;
  pendingAssetUnlock: bigint;
  pendingAssetUnlockBlock: number | null;
};

export type NativeStakingProjection = {
  isAvailable: boolean;
  accountAddress: string | null;
  exchangeRate: bigint | null;
  pool: NativeStakingPoolProjection | null;
  accountPosition: NativeLockedLpPositionProjection | null;
};

export type SystemSnapshot = {
  blockNumber: number | null;
  supply: bigint;
  priceTmc: bigint;
  priceXyk: bigint | null;
  reserveNative: bigint;
  reserveForeign: bigint;
  totalBurned: bigint | null;
  supplyLp: bigint;
  hasPool: boolean;
  hasNativeCurve: boolean;
  trackedForeignAssetCount: number;
  minForeignSwapAmount: bigint;
  gravityWellRatio: number;
  buckets: Map<string, BucketBalance>;
  bufferNative: bigint;
  bufferForeign: bigint;
  nativeAsset: AssetPresentation;
  foreignAsset: AssetPresentation;
  nativeStaking: NativeStakingProjection;
};

// Runtime constants (not in .d.ts)
export const DECIMALS = 12n;
export const PRECISION = 10n ** DECIMALS;
export const PPM = 1_000_000n;
export const PPB = 1_000_000_000n;

// ============ Chart/Log Types (UI-specific) ============

export type PricePoint = {
  step: number;
  blockNumber: number | null;
  priceEffTMC: number;
  priceXYK: number;
  priceRouter: number | null;
  routeRouter: "TMC" | "XYK" | null;
  supply: number;
};

export type LogType = "info" | "buy" | "sell" | "error";

export type LogEntry = {
  id: number | string;
  step: number;
  blockNumber: number | null;
  message: string;
  type: LogType;
  label?: string;
  accountId?: string | null;
};

export type TransactionProgress =
  | { kind: "idle"; message: string; actionLabel?: undefined; highlights?: undefined }
  | { kind: "signed"; txHash: string; message: string; actionLabel: string; highlights?: undefined }
  | { kind: "broadcasted"; txHash: string; message: string; actionLabel: string; highlights?: undefined }
  | {
      kind: "best";
      txHash: string;
      blockNumber: number;
      ok: boolean;
      eventsCount: number;
      message: string;
      actionLabel: string;
      highlights?: string[];
    }
  | {
      kind: "finalized";
      txHash: string;
      blockNumber: number;
      ok: boolean;
      eventsCount: number;
      dispatchError: string | null;
      message: string;
      actionLabel: string;
      highlights?: string[];
    }
  | { kind: "error"; txHash: string | null; message: string; actionLabel?: string; highlights?: undefined };
