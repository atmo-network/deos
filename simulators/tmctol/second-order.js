/**
 * Second-order TMCTOL contour: the `$BLDR` economy.
 *
 * Reuses the first-order primitives unchanged. In this contour "native" is `$BLDR`
 * and "foreign" is the parent asset `$NTVE`, so the unchanged Router already compares
 * the TMC and XYK routes by recipient output. Only the mint custody differs:
 * the protocol share `T = M - U` splits into a floor half for BLDR Treasury and the
 * remainder for BLDR Anchor, and all collateral is anchor-directed.
 * Contract: docs/deos/builder-economy.contract.en.md.
 * @module second-order
 */

import {
  BigMath,
  DEFAULT_CONFIG,
  FeeManager,
  PPB,
  PRECISION,
  Router,
  Tmc,
  Tol,
  Xyk,
  create_system,
} from "./model.js";

export const BUILDER_DEFAULT_CONFIG = {
  router: DEFAULT_CONFIG.router,
  xyk: DEFAULT_CONFIG.xyk,
  tmc: DEFAULT_CONFIG.tmc,
};

/** Single sealed Anchor-type bucket plus the independent spendable Treasury. */
export class BuilderTol extends Tol {
  constructor(/** @type {Xyk} */ xyk) {
    super(xyk, { bucket_shares: { anchor_ppb: PPB } });
    this.treasury_native = 0n;
  }

  /** Protocol share T: floor(T / 2) to Treasury, the rest with all collateral to the Anchor lane. */
  receive_mint_allocation(
    /** @type {bigint} */ protocol_native,
    /** @type {bigint} */ collateral,
  ) {
    const direct_treasury = protocol_native / 2n;
    this.treasury_native += direct_treasury;
    return super.receive_mint_allocation(
      protocol_native - direct_treasury,
      collateral,
    );
  }

  anchor_lp() {
    return this.buckets.get("anchor")?.lp_tokens ?? 0n;
  }
}

export function create_builder_system(
  /** @type {import("./model.js").SystemConfigOverride} */ override = {},
) {
  const router_config = { ...BUILDER_DEFAULT_CONFIG.router, ...override.router };
  const xyk = new Xyk({ ...BUILDER_DEFAULT_CONFIG.xyk, ...override.xyk });
  const tol = new BuilderTol(xyk);
  const tmc = new Tmc(tol, {
    ...BUILDER_DEFAULT_CONFIG.tmc,
    ...override.tmc,
    mint_shares: {
      ...BUILDER_DEFAULT_CONFIG.tmc.mint_shares,
      ...override.tmc?.mint_shares,
    },
  });
  const fee_manager = new FeeManager(xyk, tmc, router_config);
  const router = new Router(xyk, tmc, fee_manager, router_config);
  return { xyk, tol, tmc, fee_manager, router };
}

/** Only the live pro-rata LP reserve claim counts as anchor support; unmatched lane balances do not. */
export function anchor_support(/** @type {ReturnType<typeof create_builder_system>} */ builder) {
  const { xyk, tol } = builder;
  const lp = tol.anchor_lp();
  if (xyk.supply_lp === 0n) return { lp, bldr: 0n, ntve: 0n };
  return {
    lp,
    bldr: BigMath.mul_div(xyk.reserve_native, lp, xyk.supply_lp),
    ntve: BigMath.mul_div(xyk.reserve_foreign, lp, xyk.supply_lp),
  };
}

/** Price (quote per base) after `pressure` base units are sold into a constant-product pool. */
export function stress_floor(
  /** @type {bigint} */ reserve_base,
  /** @type {bigint} */ reserve_quote,
  /** @type {bigint} */ pressure,
) {
  if (reserve_base <= 0n || reserve_quote <= 0n) return 0n;
  if (pressure < 0n) throw new Error("Sellable pressure must be non-negative");
  const stressed = reserve_base + pressure;
  return BigMath.mul_div(reserve_base * reserve_quote, PRECISION, stressed * stressed);
}

/**
 * Conditional `$BLDR` floor in parent-quote terms. Both stresses are explicit assumptions:
 * `ntve_pressure` sold into the first-order pool and `bldr_pressure` (for example an
 * invoice payout) sold into the `$NTVE/$BLDR` pool. Not a market guarantee.
 */
export function composite_floor(
  /** @type {ReturnType<typeof create_system>} */ parent,
  /** @type {ReturnType<typeof create_builder_system>} */ builder,
  /** @type {{ ntve_pressure?: bigint, bldr_pressure?: bigint }} */ { ntve_pressure = 0n, bldr_pressure = 0n } = {},
) {
  const bldr_in_ntve = stress_floor(builder.xyk.reserve_native, builder.xyk.reserve_foreign, bldr_pressure);
  const ntve_in_parent = stress_floor(parent.xyk.reserve_native, parent.xyk.reserve_foreign, ntve_pressure);
  return {
    bldr_in_ntve,
    ntve_in_parent,
    bldr_in_parent: BigMath.mul_div(bldr_in_ntve, ntve_in_parent, PRECISION),
  };
}

/**
 * Parent Capital Bridge round. Bucket B releases a bounded LP portion to Treasury B; each
 * reserve asset routes into `$BLDR` independently; each leg's recipient output splits
 * floor(Q / 2) burn and the remainder to BLDR Treasury. Burn is applied to supply;
 * the minted quantity of a TMC leg is reported apart from the burn.
 */
export function bridge_round(
  /** @type {ReturnType<typeof create_system>} */ parent,
  /** @type {ReturnType<typeof create_builder_system>} */ builder,
  /** @type {bigint} */ release_lp,
) {
  const bucket = parent.tol.buckets.get("b");
  if (!bucket || release_lp <= 0n || release_lp > bucket.lp_tokens) {
    throw new Error("Bucket B release must be positive and within its LP balance");
  }
  const lp_before = bucket.lp_tokens;
  bucket.lp_tokens -= release_lp;
  bucket.contributed_native -= BigMath.mul_div(bucket.contributed_native, release_lp, lp_before);
  bucket.contributed_foreign -= BigMath.mul_div(bucket.contributed_foreign, release_lp, lp_before);
  const released = parent.xyk.remove_liquidity(release_lp);

  const acquire = (/** @type {bigint} */ ntve_in) => {
    const supply_before = builder.tmc.supply;
    const fee_burn_before = builder.fee_manager.total_native_burned;
    const treasury_before = builder.tol.treasury_native;
    const swap = builder.router.swap_foreign_to_native(ntve_in);
    const fee_burn = builder.fee_manager.total_native_burned - fee_burn_before;
    const minted = builder.tmc.supply - supply_before + fee_burn;
    const burn = swap.native_out / 2n;
    if (burn > 0n) builder.tmc.burn_native(burn);
    const recycled = swap.native_out - burn;
    builder.tol.treasury_native += recycled;
    return {
      route: swap.route,
      ntve_in,
      router_fee: swap.foreign_router_fee,
      ntve_routed: swap.foreign_net,
      recipient_out: swap.native_out,
      burn,
      recycled_treasury: recycled,
      minted,
      direct_treasury: builder.tol.treasury_native - treasury_before - recycled,
    };
  };

  const ntve_leg = acquire(released.native_out);
  const parent_swap = parent.router.swap_foreign_to_native(released.foreign_out);
  const foreign_leg = {
    parent: { route: parent_swap.route, foreign_in: parent_swap.foreign_in, ntve_out: parent_swap.native_out },
    ...acquire(parent_swap.native_out),
  };
  return { released, ntve_leg, foreign_leg };
}
