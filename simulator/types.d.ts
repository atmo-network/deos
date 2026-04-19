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

export type SystemSnapshot = {
  supply: bigint;
  priceTmc: bigint;
  priceXyk: bigint | null;
  reserveNative: bigint;
  reserveForeign: bigint;
  totalBurned: bigint;
  supplyLp: bigint;
  hasPool: boolean;
  gravityWellRatio: number;
  buckets: Map<string, BucketBalance>;
  bufferNative: bigint;
  bufferForeign: bigint;
};
