/*
Domain: System contracts
Owns: Cross-slice aggregate client state, runtime status, transaction request, and snapshot shapes.
Excludes: Slice-local domain contracts, adapter transport implementation, and widget rendering.
Zone: System public contract; may reference domain contracts without owning their semantics.
*/
import type { PricePoint, Quote, SwapResult } from '$lib/market/types';
import type { AssetPresentation } from '$lib/portfolio/types';
import type { NativeStakingProjection } from '$lib/staking/types';

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

export type { PricePoint, Quote, SwapResult };
