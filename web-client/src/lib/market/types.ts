/*
Domain: Market contracts
Owns: Swap quote/result, TOL projection, price point, and market dashboard data shapes.
Excludes: Store refresh logic, adapter transport, portfolio balances, and presentation components.
Zone: Market public contract; safe for adapters, stores, and widgets to import.
*/
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
  route: 'TMC' | 'XYK';
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
  route: 'TMC' | 'XYK';
  effectivePrice: number;
  fee: bigint;
  totalFee: bigint;
  priceImpactPpb: bigint;
  tmcOut: bigint;
  xykOut: bigint;
  isSell: boolean;
};

export type PricePoint = {
  step: number;
  blockNumber: number | null;
  priceEffTMC: number;
  priceXYK: number;
  priceRouter: number | null;
  routeRouter: 'TMC' | 'XYK' | null;
  supply: number;
};
