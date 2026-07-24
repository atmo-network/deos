/*
Domain: Blockchain quote helpers
Owns: Router/TMC quote normalization, route labels, and local quote math used by the chain adapter.
Excludes: Swap form state, transaction submission, and market store ownership.
Zone: Transport read helper; depends on economics constants, market quote type, and runtime asset helpers.
*/
import type { ResultPayload } from 'polkadot-api';

import { PRECISION } from '$lib/economics';
import type { Quote } from '$lib/market/types';

import type { DeosChainSnapshot } from './deos';
import { NATIVE_ASSET, type RuntimeAssetKind } from './runtime-assets';

export function bigintSqrt(value: bigint): bigint {
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

export function toOptionalValue<T>(
  result: ResultPayload<T, unknown> | undefined | null,
): T | null {
  if (!result || !result.success) {
    return null;
  }
  return result.value;
}

export function routeFromMechanism(mechanism: { type: string }): 'TMC' | 'XYK' {
  return mechanism.type === 'DirectMint' ? 'TMC' : 'XYK';
}

export async function tmcMintQuote(
  snapshot: DeosChainSnapshot,
  foreignNet: bigint,
): Promise<bigint> {
  const curve =
    await snapshot.typedApi.query.TokenMintingCurve.TokenCurves.getValue(
      NATIVE_ASSET,
      { at: snapshot.at },
    );
  if (!curve || curve.foreign_asset.type === 'Native' || foreignNet <= 0n) {
    return 0n;
  }
  const supply = await snapshot.typedApi.query.Balances.TotalIssuance.getValue({
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

export async function quoteBuyAtSnapshot(
  snapshot: DeosChainSnapshot,
  accountId: string | null,
  foreignAsset: RuntimeAssetKind,
  foreignAmount: bigint,
  xykReserves: { native: bigint; foreign: bigint } | null,
): Promise<Quote | null> {
  if (foreignAmount <= 0 || !accountId) {
    return null;
  }
  const minForeignSwapAmount =
    await snapshot.typedApi.constants.AxialRouter.MinSwapForeign();
  if (foreignAmount < minForeignSwapAmount) {
    return null;
  }
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
  const foreignNet = foreignAmount > routerFee ? foreignAmount - routerFee : 0n;
  const tmcOut = await tmcMintQuote(snapshot, foreignNet);
  const xykOut = xykReserves
    ? (foreignNet * xykReserves.native) / (xykReserves.foreign + foreignNet)
    : 0n;
  return {
    out: authoritativeQuote.amount_out,
    route: routeFromMechanism(authoritativeQuote.mechanism),
    effectivePrice:
      Number(foreignAmount) / Number(authoritativeQuote.amount_out),
    fee: authoritativeQuote.router_fee,
    totalFee: authoritativeQuote.total_fees,
    priceImpactPpb: BigInt(authoritativeQuote.price_impact),
    tmcOut,
    xykOut,
    isSell: false,
  };
}

export async function quoteSellAtSnapshot(
  snapshot: DeosChainSnapshot,
  accountId: string | null,
  foreignAsset: RuntimeAssetKind,
  nativeAmount: bigint,
): Promise<Quote | null> {
  if (nativeAmount <= 0n || !accountId) {
    return null;
  }
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
    totalFee: authoritativeQuote.total_fees,
    priceImpactPpb: BigInt(authoritativeQuote.price_impact),
    tmcOut: 0n,
    xykOut: authoritativeQuote.amount_out,
    isSell: true,
  };
}
