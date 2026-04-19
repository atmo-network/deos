// @ts-check

/**
 * @name `TMCTOL` Simulator
 * @note This simulator serves for preliminary economic testing, parameter optimization, and business logic formalization of composite tokenomics models.
 * @units Balances, prices and slope use `PRECISION` (10^12) for accuracy. Fractional values (fees, shares) use `PPB` (Parts Per Billion, 10^9) and require a '_ppb' suffix.
 * @version 1.2.0
 * @module model.js
 */

// --- Types ---

/** @typedef {import('./types').TolBucketConfig} TolBucketConfig */
/** @typedef {import('./types').TolConfig} TolConfig */
/** @typedef {import('./types').MintShareConfig} MintShareConfig */
/** @typedef {import('./types').TmcConfig} TmcConfig */
/** @typedef {import('./types').XykConfig} XykConfig */
/** @typedef {import('./types').RouterConfig} RouterConfig */
/** @typedef {import('./types').SystemConfig} SystemConfig */

// --- Helpers ---

/**
 * @template T
 * @typedef {{ ok: true, value: T } | { ok: false, error: string }} Result
 */
/** @type {<T>(value: T) => Result<T>} */
function Ok(value) {
  return { ok: true, value };
}
/** @type {<T>(error: string) => Result<T>} */
function Err(error) {
  return { ok: false, error };
}
/** @type {<T>(result: Result<T>) => result is { ok: true, value: T }} */
function isOk(result) {
  return result.ok === true;
}

// --- Constants ---

export const DECIMALS = 12n;
export const PRECISION = 10n ** DECIMALS;
export const PPB = 1_000_000_000n;

export const DEFAULT_CONFIG = /** @type {SystemConfig} */ ({
  router: {
    min_initial_foreign: 100n * PRECISION,
    min_swap_foreign: PRECISION / 100n,
    fee_router_ppb: (5n * PPB) / 1_000n,
  },
  xyk: {
    fee_xyk_ppb: 0n,
  },
  tmc: {
    price_initial: PRECISION / 1_000n,
    slope: PRECISION / 1_000_000n,
    mint_shares: {
      user_ppb: 333_333_333n,
      tol_ppb: 666_666_667n,
    },
  },
  tol: {
    bucket_shares: {
      a_ppb: 500_000_000n,
      b_ppb: 166_666_667n,
      c_ppb: 166_666_667n,
      d_ppb: 166_666_666n,
    },
  },
});

// --- Math Primitives ---

export class BigMath {
  static mul_div(
    /** @type {bigint} */ a,
    /** @type {bigint} */ b,
    /** @type {bigint} */ c,
  ) {
    if (c === 0n) {
      throw new Error("Division by zero");
    }
    return (a * b) / c;
  }

  static div_ceil(/** @type {bigint} */ a, /** @type {bigint} */ b) {
    if (b === 0n) {
      throw new Error("Division by zero");
    }
    return a % b === 0n ? a / b : a / b + 1n;
  }

  static isqrt(/** @type {bigint} */ n) {
    if (n < 0n) {
      throw new Error("Square root of negative number");
    }
    if (n < 2n) {
      return n;
    }
    let x = n;
    let y = (x + 1n) / 2n;
    while (y < x) {
      x = y;
      y = (x + n / x) / 2n;
    }
    return x;
  }

  static min(/** @type {bigint} */ a, /** @type {bigint} */ b) {
    return a < b ? a : b;
  }

  static max(/** @type {bigint} */ a, /** @type {bigint} */ b) {
    return a > b ? a : b;
  }

  static abs(/** @type {bigint} */ a) {
    return a < 0n ? -a : a;
  }
}

// --- Actors ---

export class User {
  constructor(
    /** @type {bigint} */ initial_native = 0n,
    /** @type {bigint} */ initial_foreign = 0n,
  ) {
    this.balance_native = initial_native;
    this.balance_foreign = initial_foreign;
    this.router = null;
  }

  set_router(/** @type {Router} */ router) {
    this.router = router;
  }

  get_balance() {
    return {
      native: this.balance_native,
      foreign: this.balance_foreign,
    };
  }

  buy_native(/** @type {bigint} */ foreign_amount) {
    if (!this.router) {
      throw new Error("Router not set");
    }
    if (this.balance_foreign < foreign_amount) {
      throw new Error("Insufficient foreign balance");
    }
    this.balance_foreign -= foreign_amount;
    const result = this.router.swap_foreign_to_native(foreign_amount);
    this.balance_native += result.native_out;
    return result;
  }

  sell_native(/** @type {bigint} */ native_amount) {
    if (!this.router) {
      throw new Error("Router not set");
    }
    if (this.balance_native < native_amount) {
      throw new Error("Insufficient native balance");
    }
    this.balance_native -= native_amount;
    const result = this.router.swap_native_to_foreign(native_amount);
    this.balance_foreign += result.foreign_out;
    return result;
  }

  deposit_foreign(/** @type {bigint} */ amount) {
    if (amount <= 0n) {
      throw new Error("Amount must be positive");
    }
    this.balance_foreign += amount;
  }

  deposit_native(/** @type {bigint} */ amount) {
    if (amount <= 0n) {
      throw new Error("Amount must be positive");
    }
    this.balance_native += amount;
  }
}

export class LiquidityBucket {
  constructor(/** @type {string} */ id) {
    this.id = id;
    this.lp_tokens = 0n;
    this.contributed_native = 0n;
    this.contributed_foreign = 0n;
  }

  get_balance() {
    return {
      balance_lp: this.lp_tokens,
      contributed_native: this.contributed_native,
      contributed_foreign: this.contributed_foreign,
    };
  }

  receive_lp_tokens(
    /** @type {bigint} */ lp,
    /** @type {bigint} */ native_used,
    /** @type {bigint} */ foreign_used,
  ) {
    this.lp_tokens += lp;
    this.contributed_native += native_used;
    this.contributed_foreign += foreign_used;
  }
}

// --- Economic Primitives ---

export class Tol {
  constructor(/** @type {Xyk} */ xyk, /** @type {TolConfig} */ config) {
    this.xyk = xyk;
    this.bucket_config = config.bucket_shares;
    this.bucket_keys = Object.keys(config.bucket_shares).map((key) =>
      key.replace(/_ppb$/, ""),
    );
    const sum_buckets = Object.values(config.bucket_shares).reduce(
      (sum, val) => sum + val,
      0n,
    );
    if (sum_buckets !== PPB) {
      throw new Error(`Bucket shares must sum to ${PPB}, got ${sum_buckets}`);
    }
    /** @type {Map<string, LiquidityBucket>} */
    this.buckets = new Map();
    this.bucket_keys.forEach((key) => {
      this.buckets.set(key, new LiquidityBucket(key));
    });
    this.buffer_native = 0n;
    this.buffer_foreign = 0n;
  }

  get_balance() {
    return Object.fromEntries(
      this.bucket_keys.map((key) => [
        `bucket_${key}`,
        this.buckets.get(key)?.get_balance(),
      ]),
    );
  }

  receive_mint_allocation(
    /** @type {bigint} */ total_native,
    /** @type {bigint} */ total_foreign,
  ) {
    this.buffer_native += total_native;
    this.buffer_foreign += total_foreign;
    const zap_result = this.#execute_zap(
      this.buffer_native,
      this.buffer_foreign,
    );
    if (!isOk(zap_result)) {
      console.warn("Zap operation failed:", zap_result.error);
      return this.#build_tol_result(0n, 0n, 0n);
    }
    const { value } = zap_result;
    this.buffer_native = value.leftover_native;
    this.buffer_foreign = value.leftover_foreign;
    return this.#build_tol_result(
      value.total_lp_minted,
      value.total_native_used,
      value.total_foreign_used,
    );
  }

  #build_tol_result(
    /** @type {bigint} */ total_lp,
    /** @type {bigint} */ total_native,
    /** @type {bigint} */ total_foreign,
  ) {
    const bucket_snapshots = Object.fromEntries(
      this.bucket_keys.map((key) => {
        const bucket = this.buckets.get(key);
        return [
          `bucket_${key}`,
          bucket
            ? {
                lp_tokens: bucket.lp_tokens,
                contributed_native: bucket.contributed_native,
                contributed_foreign: bucket.contributed_foreign,
              }
            : {
                lp_tokens: 0n,
                contributed_native: 0n,
                contributed_foreign: 0n,
              },
        ];
      }),
    );
    return {
      total_lp_minted: total_lp,
      total_native_used: total_native,
      total_foreign_used: total_foreign,
      ...bucket_snapshots,
    };
  }

  #try_initialize_pool(
    /** @type {bigint} */ total_native,
    /** @type {bigint} */ total_foreign,
  ) {
    if (total_native === 0n || total_foreign === 0n) {
      return Ok(this.#create_empty_zap_result());
    }
    try {
      const result = this.xyk.add_liquidity(total_native, total_foreign);
      this.#distribute_lp_tokens(
        result.lp_minted,
        result.native_used,
        result.foreign_used,
      );
      return Ok({
        total_lp_minted: result.lp_minted,
        total_native_used: result.native_used,
        total_foreign_used: result.foreign_used,
        leftover_native: total_native - result.native_used,
        leftover_foreign: total_foreign - result.foreign_used,
      });
    } catch (e) {
      return Err(`Pool initialization failed: ${String(e)}`);
    }
  }

  #execute_zap(
    /** @type {bigint} */ total_native,
    /** @type {bigint} */ total_foreign,
  ) {
    if (total_native === 0n && total_foreign === 0n) {
      return Ok(this.#create_empty_zap_result());
    }
    if (!this.xyk.has_liquidity()) {
      return this.#try_initialize_pool(total_native, total_foreign);
    }
    let unconsumed_native = total_native;
    let unconsumed_foreign = total_foreign;
    let lp_minted = 0n;
    let native_contributed = 0n;
    let foreign_contributed = 0n;
    if (unconsumed_native > 0n && unconsumed_foreign > 0n) {
      const foreign_needed_for_native = BigMath.mul_div(
        unconsumed_native,
        this.xyk.reserve_foreign,
        this.xyk.reserve_native,
      );
      const [native_for_liquidity, foreign_for_liquidity] =
        foreign_needed_for_native <= unconsumed_foreign
          ? [unconsumed_native, foreign_needed_for_native]
          : [
              BigMath.mul_div(
                unconsumed_foreign,
                this.xyk.reserve_native,
                this.xyk.reserve_foreign,
              ),
              unconsumed_foreign,
            ];
      if (native_for_liquidity > 0n && foreign_for_liquidity > 0n) {
        try {
          const add_result = this.xyk.add_liquidity(
            native_for_liquidity,
            foreign_for_liquidity,
          );
          lp_minted = add_result.lp_minted;
          native_contributed = add_result.native_used;
          foreign_contributed = add_result.foreign_used;
          unconsumed_native -= add_result.native_used;
          unconsumed_foreign -= add_result.foreign_used;
        } catch (e) {
          return Err(`Failed to add liquidity: ${String(e)}`);
        }
      }
    }
    if (unconsumed_foreign > 0n && this.xyk.has_liquidity()) {
      try {
        const swap_result = this.xyk.swap_foreign_to_native(unconsumed_foreign);
        unconsumed_native += swap_result.native_out;
        unconsumed_foreign = 0n;
      } catch (e) {
        return Err(`Failed to swap excess foreign: ${String(e)}`);
      }
    }
    this.#distribute_lp_tokens(
      lp_minted,
      native_contributed,
      foreign_contributed,
    );
    return Ok({
      total_lp_minted: lp_minted,
      total_native_used: native_contributed,
      total_foreign_used: foreign_contributed,
      leftover_native: unconsumed_native,
      leftover_foreign: unconsumed_foreign,
    });
  }

  #distribute_lp_tokens(
    /** @type {bigint} */ total_lp,
    /** @type {bigint} */ total_native_used,
    /** @type {bigint} */ total_foreign_used,
  ) {
    /** @type {Record<string, bigint>} */
    const lp_shares = this.#distribute_amount(total_lp);
    /** @type {Record<string, bigint>} */
    const native_shares = this.#distribute_amount(total_native_used);
    /** @type {Record<string, bigint>} */
    const foreign_shares = this.#distribute_amount(total_foreign_used);
    this.#for_each_bucket((bucket, key) => {
      bucket.receive_lp_tokens(
        lp_shares[key] || 0n,
        native_shares[key] || 0n,
        foreign_shares[key] || 0n,
      );
    });
  }

  #distribute_amount(/** @type {bigint} */ total) {
    /** @type {Record<string, bigint>} */
    const shares = {};
    /** @type {Record<string, bigint>} */
    const fractions = {};
    let sum_shares = 0n;
    this.bucket_keys.forEach((key) => {
      const ppb_key = `${key}_ppb`;
      const share = BigMath.mul_div(total, this.bucket_config[ppb_key], PPB);
      shares[key] = share;
      sum_shares += share;
      fractions[key] = (total * this.bucket_config[ppb_key]) % PPB;
    });
    const remainder = total - sum_shares;
    if (remainder > 0n && this.bucket_keys.length > 0) {
      const max_key = this.bucket_keys.reduce((max, key) =>
        fractions[key] > fractions[max] ? key : max,
      );
      shares[max_key] += remainder;
    }
    return shares;
  }

  #create_empty_zap_result() {
    return {
      total_lp_minted: 0n,
      total_native_used: 0n,
      total_foreign_used: 0n,
      leftover_native: 0n,
      leftover_foreign: 0n,
    };
  }

  #for_each_bucket(
    /** @type {(bucket: LiquidityBucket, key: string) => void} */ fn,
  ) {
    this.bucket_keys.forEach((key) => {
      const bucket = this.buckets.get(key);
      if (bucket) fn(bucket, key);
    });
  }
}

export class Xyk {
  constructor(/** @type {XykConfig} */ config) {
    if (config.fee_xyk_ppb >= PPB) {
      throw new Error("Fee must be < 100%");
    }
    this.fee_ppb = config.fee_xyk_ppb;
    this.reserve_native = 0n;
    this.reserve_foreign = 0n;
    this.supply_lp = 0n;
  }

  get_price() {
    if (this.reserve_native === 0n) {
      throw new Error("Cannot calculate price with zero native reserves");
    }
    if (this.reserve_foreign === 0n) {
      throw new Error("Cannot calculate price with zero foreign reserves");
    }
    return BigMath.mul_div(
      this.reserve_foreign,
      PRECISION,
      this.reserve_native,
    );
  }

  has_liquidity() {
    return this.reserve_native > 0n && this.reserve_foreign > 0n;
  }

  get_out_native(/** @type {bigint} */ foreign) {
    if (foreign <= 0n || !this.has_liquidity()) {
      return 0n;
    }
    return this.#calculate_swap_output(
      foreign,
      this.reserve_foreign,
      this.reserve_native,
    );
  }

  get_out_foreign(/** @type {bigint} */ native) {
    if (native <= 0n || !this.has_liquidity()) {
      return 0n;
    }
    return this.#calculate_swap_output(
      native,
      this.reserve_native,
      this.reserve_foreign,
    );
  }

  add_liquidity(/** @type {bigint} */ native, /** @type {bigint} */ foreign) {
    if (native <= 0n || foreign <= 0n) {
      throw new Error("Amounts must be positive");
    }
    if (!this.has_liquidity()) {
      const lp_minted = BigMath.isqrt(native * foreign);
      if (lp_minted === 0n) {
        throw new Error("Insufficient initial liquidity");
      }
      this.reserve_native = native;
      this.reserve_foreign = foreign;
      this.supply_lp = lp_minted;
      return {
        lp_minted,
        native_used: native,
        foreign_used: foreign,
        native_rest: 0n,
        foreign_rest: 0n,
      };
    }
    const lp_from_native = BigMath.mul_div(
      native,
      this.supply_lp,
      this.reserve_native,
    );
    const lp_from_foreign = BigMath.mul_div(
      foreign,
      this.supply_lp,
      this.reserve_foreign,
    );
    const lp_minted = BigMath.min(lp_from_native, lp_from_foreign);
    if (lp_minted === 0n) {
      throw new Error("Insufficient liquidity provided");
    }
    const native_used = BigMath.mul_div(
      this.reserve_native,
      lp_minted,
      this.supply_lp,
    );
    const foreign_used = BigMath.mul_div(
      this.reserve_foreign,
      lp_minted,
      this.supply_lp,
    );
    this.reserve_native += native_used;
    this.reserve_foreign += foreign_used;
    this.supply_lp += lp_minted;
    return {
      lp_minted,
      native_used,
      foreign_used,
      native_rest: native - native_used,
      foreign_rest: foreign - foreign_used,
    };
  }

  swap_native_to_foreign(
    /** @type {bigint} */ native_in,
    /** @type {bigint} */ min_foreign_out = 0n,
  ) {
    return this.#execute_swap(native_in, min_foreign_out, "native_to_foreign");
  }

  swap_foreign_to_native(
    /** @type {bigint} */ foreign_in,
    /** @type {bigint} */ min_native_out = 0n,
  ) {
    return this.#execute_swap(foreign_in, min_native_out, "foreign_to_native");
  }

  #execute_swap(
    /** @type {bigint} */ amount_in,
    /** @type {bigint} */ min_out,
    /** @type {"native_to_foreign" | "foreign_to_native"} */ direction,
  ) {
    const is_native_to_foreign = direction === "native_to_foreign";
    const price_before = this.get_price();
    const { amount_out, fee_charged, reserve_in_updated, reserve_out_updated } =
      this.#execute_swap_operation(amount_in, is_native_to_foreign);
    if (amount_out < min_out) {
      throw new Error("Slippage exceeded");
    }
    this.reserve_native = is_native_to_foreign
      ? reserve_in_updated
      : reserve_out_updated;
    this.reserve_foreign = is_native_to_foreign
      ? reserve_out_updated
      : reserve_in_updated;
    const price_after = this.get_price();
    const price_change = is_native_to_foreign
      ? price_before > price_after
        ? price_before - price_after
        : 0n
      : price_after > price_before
        ? price_after - price_before
        : 0n;
    const price_change_abs =
      price_after > price_before
        ? price_after - price_before
        : price_before - price_after;
    const price_impact_ppb =
      price_before > 0n
        ? BigMath.mul_div(price_change_abs, PPB, price_before)
        : 0n;
    const result = is_native_to_foreign
      ? {
          native_in: amount_in,
          native_out: 0n,
          foreign_in: 0n,
          foreign_out: amount_out,
        }
      : {
          native_in: 0n,
          native_out: amount_out,
          foreign_in: amount_in,
          foreign_out: 0n,
        };
    return {
      ...result,
      fee: fee_charged,
      price_before,
      price_after,
      price_impact_ppb,
    };
  }

  #execute_swap_operation(
    /** @type {bigint} */ amount_in,
    /** @type {boolean} */ is_native_in,
  ) {
    const reserve_in = is_native_in
      ? this.reserve_native
      : this.reserve_foreign;
    const reserve_out = is_native_in
      ? this.reserve_foreign
      : this.reserve_native;
    const amount_in_with_fee = BigMath.mul_div(
      amount_in,
      PPB - this.fee_ppb,
      PPB,
    );
    const fee_charged = amount_in - amount_in_with_fee;
    const numerator = amount_in_with_fee * reserve_out;
    const denominator = reserve_in + amount_in_with_fee;
    const amount_out = numerator / denominator;
    return {
      amount_out,
      fee_charged,
      reserve_in_updated: reserve_in + amount_in,
      reserve_out_updated: reserve_out - amount_out,
    };
  }

  #calculate_swap_output(
    /** @type {bigint} */ amount_in,
    /** @type {bigint} */ reserve_in,
    /** @type {bigint} */ reserve_out,
  ) {
    const amount_in_with_fee = BigMath.mul_div(
      amount_in,
      PPB - this.fee_ppb,
      PPB,
    );
    const numerator = amount_in_with_fee * reserve_out;
    const denominator = reserve_in + amount_in_with_fee;
    return numerator / denominator;
  }
}

export class Tmc {
  constructor(/** @type {Tol} */ tol, /** @type {TmcConfig} */ config) {
    if (config.price_initial <= 0n) {
      throw new Error("Initial price must be positive");
    }
    if (config.slope < 0n) {
      throw new Error("Slope must be non-negative");
    }
    this.price_initial = config.price_initial;
    this.slope = config.slope;
    this.user_ppb = config.mint_shares.user_ppb;
    this.tol_ppb = config.mint_shares.tol_ppb;
    this.tol = tol;
    this.supply = 0n;
    const sum_shares = this.user_ppb + this.tol_ppb;
    if (sum_shares !== PPB) {
      throw new Error(`Shares must sum to ${PPB}, got ${sum_shares}`);
    }
  }

  get_price() {
    const slope_component = BigMath.mul_div(this.slope, this.supply, PRECISION);
    return this.price_initial + slope_component;
  }

  calculate_mint(/** @type {bigint} */ foreign) {
    if (foreign <= 0n) {
      return 0n;
    }
    const price_initial = this.price_initial;
    const slope = this.slope;
    const supply = this.supply;
    if (slope === 0n) {
      return BigMath.mul_div(foreign, PRECISION, price_initial);
    }
    const a = slope;
    const b = 2n * price_initial * PRECISION + 2n * slope * supply;
    const c = 2n * foreign * PRECISION * PRECISION;
    const discriminant = b * b + 4n * a * c;
    if (discriminant < 0n) {
      return 0n;
    }
    const sqrt_discriminant = BigMath.isqrt(discriminant);
    const numerator = sqrt_discriminant - b;
    if (numerator <= 0n) {
      return 0n;
    }
    const delta_supply = numerator / (2n * a);
    return delta_supply;
  }

  mint_native(/** @type {bigint} */ foreign_in) {
    const price_before = this.get_price();
    const total_native = this.calculate_mint(foreign_in);
    if (total_native === 0n) {
      throw new Error("Insufficient amount");
    }
    this.supply += total_native;
    const distribution = this.#distribute(total_native);
    const tol_result = this.tol.receive_mint_allocation(
      distribution.tol,
      foreign_in,
    );
    const price_after = this.get_price();
    return {
      total_minted: total_native,
      user_native: distribution.user,
      tol_native: distribution.tol,
      price_before,
      price_after,
      tol: tol_result,
    };
  }

  get_mint_quote(/** @type {bigint} */ foreign) {
    const native_minted = this.calculate_mint(foreign);
    if (native_minted === 0n) {
      return null;
    }
    return {
      minted: native_minted,
      ...this.#distribute(native_minted),
    };
  }

  burn_native(/** @type {bigint} */ amount) {
    if (amount <= 0n) {
      throw new Error("Burn amount must be positive");
    }
    if (this.supply < amount) {
      throw new Error(
        `Insufficient supply for burn: ${this.supply} < ${amount}`,
      );
    }
    const supply_before = this.supply;
    this.supply -= amount;
    return {
      native_burned: amount,
      supply_before,
      supply_after: this.supply,
    };
  }

  #distribute(/** @type {bigint} */ total_native) {
    const user = BigMath.mul_div(total_native, this.user_ppb, PPB);
    const tol = BigMath.mul_div(total_native, this.tol_ppb, PPB);
    const remainder = total_native - user - tol;
    if (remainder > 0n) {
      const user_frac = (total_native * this.user_ppb) % PPB;
      const tol_frac = (total_native * this.tol_ppb) % PPB;
      if (user_frac >= tol_frac) {
        return { user: user + remainder, tol };
      } else {
        return { user, tol: tol + remainder };
      }
    }
    return { user, tol };
  }
}

// --- Coordination ---

export class FeeManager {
  constructor(
    /** @type {Xyk} */ xyk,
    /** @type {Tmc} */ tmc,
    /** @type {RouterConfig} */ config,
  ) {
    this.xyk = xyk;
    this.tmc = tmc;
    this.min_swap_foreign = config.min_swap_foreign;
    this.buffer_native = 0n;
    this.buffer_foreign = 0n;
    this.total_native_burned = 0n;
    this.total_foreign_swapped = 0n;
  }

  receive_fee_native(/** @type {bigint} */ native) {
    if (native <= 0n) return;
    this.buffer_native += native;
    const result = this.#execute_burn(this.buffer_native, 0n);
    if (isOk(result)) {
      const { value } = result;
      this.buffer_native = value.native_buffered;
      this.total_native_burned += value.native_burned;
    }
  }

  receive_fee_foreign(/** @type {bigint} */ foreign) {
    if (foreign <= 0n) return;
    this.buffer_foreign += foreign;
    const result = this.#execute_burn(0n, this.buffer_foreign);
    if (isOk(result)) {
      const { value } = result;
      this.buffer_foreign = value.foreign_buffered;
      this.total_native_burned += value.native_burned;
      this.total_foreign_swapped += value.foreign_swapped;
    }
  }

  #execute_burn(
    /** @type {bigint} */ amount_native_fee,
    /** @type {bigint} */ amount_foreign_fee,
  ) {
    let native_to_burn = amount_native_fee;
    let foreign_swapped = 0n;
    let foreign_buffered = amount_foreign_fee;
    if (
      amount_foreign_fee >= this.min_swap_foreign &&
      this.xyk.has_liquidity()
    ) {
      try {
        const spot_price = this.xyk.get_price();
        const expected_native = BigMath.mul_div(
          amount_foreign_fee,
          PRECISION,
          spot_price,
        );
        const min_native_out = BigMath.mul_div(
          expected_native,
          (90n * PPB) / 100n,
          PPB,
        );
        const swap_result = this.xyk.swap_foreign_to_native(
          amount_foreign_fee,
          min_native_out,
        );
        foreign_swapped = amount_foreign_fee;
        foreign_buffered = 0n;
        native_to_burn += swap_result.native_out;
      } catch (e) {
        return Err(`Swap failed, foreign fee buffered: ${String(e)}`);
      }
    }
    if (native_to_burn === 0n) {
      return Ok({
        native_burned: 0n,
        foreign_swapped,
        foreign_buffered,
        native_buffered: amount_native_fee,
      });
    }
    try {
      this.tmc.burn_native(native_to_burn);
      return Ok({
        native_burned: native_to_burn,
        foreign_swapped,
        foreign_buffered,
        native_buffered: 0n,
      });
    } catch (e) {
      return Err(`Burn failed, native fee buffered: ${String(e)}`);
    }
  }
}

export class RouteSelector {
  constructor(/** @type {Xyk} */ xyk, /** @type {Tmc} */ tmc) {
    this.xyk = xyk;
    this.tmc = tmc;
  }

  select_route_for_foreign_to_native(
    /** @type {bigint} */ foreign_net,
    /** @type {bigint} */ min_native_out,
  ) {
    const tmc_quote = this.tmc.get_mint_quote(foreign_net);
    const tmc_out = tmc_quote?.user ?? 0n;
    const xyk_out = this.xyk.has_liquidity()
      ? this.xyk.get_out_native(foreign_net)
      : 0n;
    const tmc_viable = tmc_quote && tmc_out > 0n && tmc_out >= min_native_out;
    const xyk_viable = xyk_out > 0n && xyk_out >= min_native_out;
    const use_tmc = tmc_viable && (!xyk_viable || tmc_out >= xyk_out);
    if (!use_tmc && !xyk_viable) {
      return Err(xyk_out > 0n ? "Slippage exceeded" : "No route available");
    }
    return Ok({
      use_tmc,
      tmc_out,
      xyk_out,
      tmc_viable,
      xyk_viable,
    });
  }
}

export class SwapExecutor {
  constructor(
    /** @type {Xyk} */ xyk,
    /** @type {Tmc} */ tmc,
    /** @type {FeeManager} */ fee_manager,
  ) {
    this.xyk = xyk;
    this.tmc = tmc;
    this.fee_manager = fee_manager;
  }

  execute_tmc_route(
    /** @type {bigint} */ foreign_net,
    /** @type {bigint} */ foreign_in,
    /** @type {bigint} */ foreign_fee,
  ) {
    const mint_result = this.tmc.mint_native(foreign_net);
    return {
      route: "TMC",
      native_out: mint_result.user_native,
      foreign_in: foreign_in,
      foreign_net: foreign_net,
      foreign_router_fee: foreign_fee,
      price_before: mint_result.price_before,
      price_after: mint_result.price_after,
      tol: mint_result.tol,
    };
  }

  execute_xyk_route(
    /** @type {bigint} */ foreign_net,
    /** @type {bigint} */ foreign_in,
    /** @type {bigint} */ foreign_fee,
    /** @type {bigint} */ min_native_out,
  ) {
    const swap_result = this.xyk.swap_foreign_to_native(
      foreign_net,
      min_native_out,
    );
    return {
      route: "XYK",
      native_out: swap_result.native_out,
      foreign_in: foreign_in,
      foreign_net: foreign_net,
      foreign_router_fee: foreign_fee,
      price_before: swap_result.price_before,
      price_after: swap_result.price_after,
      price_impact_ppb: swap_result.price_impact_ppb,
    };
  }

  execute_xyk_sell_route(
    /** @type {bigint} */ native_in,
    /** @type {bigint} */ native_fee,
    /** @type {bigint} */ min_foreign_out,
  ) {
    const swap_result = this.xyk.swap_native_to_foreign(
      native_in - native_fee,
      min_foreign_out,
    );
    return {
      route: "XYK",
      foreign_out: swap_result.foreign_out,
      native_in: native_in,
      native_router_fee: native_fee,
      native_net: native_in - native_fee,
      price_before: swap_result.price_before,
      price_after: swap_result.price_after,
      price_impact_ppb: swap_result.price_impact_ppb,
    };
  }
}

export class Router {
  constructor(
    /** @type {Xyk} */ xyk,
    /** @type {Tmc} */ tmc,
    /** @type {FeeManager} */ fee_manager,
    /** @type {RouterConfig} */ config,
  ) {
    this.xyk = xyk;
    this.tmc = tmc;
    this.fee_manager = fee_manager;
    this.route_selector = new RouteSelector(xyk, tmc);
    this.swap_executor = new SwapExecutor(xyk, tmc, fee_manager);
    this.fee_router_ppb = config.fee_router_ppb;
    this.min_swap_foreign = config.min_swap_foreign;
    this.min_initial_foreign = config.min_initial_foreign;
  }

  swap_foreign_to_native(
    /** @type {bigint} */ foreign_in,
    /** @type {bigint} */ min_native_out = 0n,
  ) {
    this.#validate_swap_input(
      foreign_in,
      this.min_swap_foreign,
      `Amount below minimum threshold (${this.min_swap_foreign} foreign)`,
    );
    if (!this.xyk.has_liquidity() && foreign_in < this.min_initial_foreign) {
      throw new Error(
        `Initial mint requires minimum ${this.min_initial_foreign} foreign tokens`,
      );
    }
    const foreign_fee = BigMath.mul_div(foreign_in, this.fee_router_ppb, PPB);
    const foreign_net = foreign_in - foreign_fee;
    if (foreign_net <= 0n) {
      throw new Error("Amount too small");
    }
    const route_result = this.route_selector.select_route_for_foreign_to_native(
      foreign_net,
      min_native_out,
    );
    if (!isOk(route_result)) {
      throw new Error(route_result.error);
    }
    this.fee_manager.receive_fee_foreign(foreign_fee);
    const { value } = route_result;
    return value.use_tmc
      ? this.swap_executor.execute_tmc_route(
          foreign_net,
          foreign_in,
          foreign_fee,
        )
      : this.swap_executor.execute_xyk_route(
          foreign_net,
          foreign_in,
          foreign_fee,
          min_native_out,
        );
  }

  swap_native_to_foreign(
    /** @type {bigint} */ native_in,
    /** @type {bigint} */ min_foreign_out = 0n,
  ) {
    this.#validate_swap_input(native_in, 1n, "Amount must be positive");
    if (!this.xyk.has_liquidity()) {
      throw new Error(
        "Pool not initialized. Cannot sell native tokens before initial liquidity",
      );
    }
    const native_fee = BigMath.mul_div(native_in, this.fee_router_ppb, PPB);
    const native_net = native_in - native_fee;
    const price_spot = this.xyk.get_price();
    if (price_spot === 0n) {
      throw new Error("Invalid pool state: no native reserves");
    }
    const native_net_as_foreign = BigMath.mul_div(
      native_net,
      price_spot,
      PRECISION,
    );
    if (native_net_as_foreign < this.min_swap_foreign) {
      throw new Error(
        `Amount below minimum threshold (${this.min_swap_foreign} foreign equivalent)`,
      );
    }
    this.fee_manager.receive_fee_native(native_fee);
    return this.swap_executor.execute_xyk_sell_route(
      native_in,
      native_fee,
      min_foreign_out,
    );
  }

  #validate_swap_input(
    /** @type {bigint} */ amount,
    /** @type {bigint} */ min_threshold,
    /** @type {string} */ error_msg,
  ) {
    if (amount <= 0n) {
      throw new Error("Amount must be positive");
    }
    if (amount < min_threshold) {
      throw new Error(error_msg);
    }
  }
}

// --- Factory ---

export function create_system(
  /** @type {Partial<SystemConfig>} */ config_override = {},
) {
  const config = {
    router: { ...DEFAULT_CONFIG.router, ...config_override.router },
    xyk: { ...DEFAULT_CONFIG.xyk, ...config_override.xyk },
    tmc: { ...DEFAULT_CONFIG.tmc, ...config_override.tmc },
    tol: { ...DEFAULT_CONFIG.tol, ...config_override.tol },
  };
  const xyk = new Xyk(config.xyk);
  const tol = new Tol(xyk, config.tol);
  const tmc = new Tmc(tol, config.tmc);
  const fee_manager = new FeeManager(xyk, tmc, config.router);
  const router = new Router(xyk, tmc, fee_manager, config.router);
  return { xyk, tol, tmc, fee_manager, router };
}
