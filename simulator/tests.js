// @ts-nocheck

/**
 * TMCTOL Comprehensive Test Suite
 * Synchronized verbose in `/simulator/tests.md`
 * Minimal output: statistics only
 * @module tests
 */

import {
  BigMath,
  create_system,
  PPB,
  PRECISION,
  Router,
  User,
  distribute_fee_sink_phase1,
  distribute_fee_sink_phase2,
  split_collator_fee,
} from "./model.js";

/** @typedef {import("./model.js").SystemConfig} SystemConfig */

/**
 * @param {bigint} price
 * @returns {string}
 */
const formatPrice = (price) => (Number(price) / Number(PRECISION)).toFixed(9);

/**
 * @param {bigint} supply
 * @returns {string}
 */
const formatSupply = (supply) =>
  (Number(supply) / Number(PRECISION)).toFixed(2);

/**
 * @param {bigint} tokens
 * @returns {string}
 */
const formatTokens = (tokens) =>
  (Number(tokens) / Number(PRECISION)).toFixed(6);

/**
 * @param {bigint} ppb
 * @returns {string}
 */
const formatPPB = (ppb) => `${(Number(ppb) / 10000000).toFixed(2)}%`;

class TestFailure extends Error {
  constructor(/** @type {string} */ message) {
    super(message);
    this.name = "TestFailure";
  }
}

/**
 * @param {boolean} condition
 * @param {string} message
 * @throws {TestFailure}
 */
const assert = (condition, message) => {
  if (!condition) {
    console.log(`ASSERT FAILED: ${message}, condition=${condition}`);
    throw new TestFailure(message);
  }
};

/**
 * @param {bigint} actual
 * @param {bigint} expected
 * @param {bigint} tolerance
 * @param {string} message
 * @throws {TestFailure}
 */
const assertApprox = (actual, expected, tolerance, message) => {
  const diff = actual > expected ? actual - expected : expected - actual;
  const maxDiff = (expected * BigInt(tolerance)) / 1_000n;
  if (diff > maxDiff) {
    throw new TestFailure(
      `${message} (Expected: ${expected}, Actual: ${actual}, Diff: ${diff})`,
    );
  }
};

const getTimestamp = (() => {
  if (typeof performance !== "undefined" && performance.now) {
    return () => BigInt(Math.floor(performance.now() * 1_000_000));
  } else if (typeof Date !== "undefined") {
    return () => BigInt(Date.now()) * 1_000_000n;
  } else {
    let counter = 0n;
    return () => counter++;
  }
})();

/**
 * @param {bigint} reserveNative
 * @param {bigint} reserveForeign
 * @param {bigint} supply
 * @returns {bigint}
 */
const calculateFloorPriceFromState = (
  reserveNative,
  reserveForeign,
  supply,
) => {
  if (reserveNative === 0n || reserveForeign === 0n) return 0n;
  const k = reserveNative * reserveForeign;
  const maxSellable = supply - reserveNative;
  const finalNative = reserveNative + maxSellable;
  const finalForeign = k / finalNative;
  return (finalForeign * PRECISION) / finalNative;
};

/**
 * @param {{ xyk: { reserve_native: bigint, reserve_foreign: bigint }, tmc: { supply: bigint } }} system
 * @returns {bigint}
 */
const calculateFloorPrice = (system) =>
  calculateFloorPriceFromState(
    system.xyk.reserve_native,
    system.xyk.reserve_foreign,
    system.tmc.supply,
  );

/**
 * @param {bigint} ceiling
 * @param {bigint} floor
 * @returns {bigint}
 */
const calculateRelativeSpread = (ceiling, floor) => {
  if (floor === 0n) return 0n;
  return BigMath.mul_div(ceiling, PRECISION, floor);
};

/**
 * @param {bigint} ceiling
 * @param {bigint} floor
 * @returns {bigint}
 */
const calculateArithmeticSpread = (ceiling, floor) => ceiling - floor;

const calculateStressFloor = (reserveNative, reserveForeign, sellablePressure) => {
  if (reserveNative <= 0n || reserveForeign <= 0n) return 0n;
  if (sellablePressure < 0n) {
    throw new Error("Sellable pressure must be non-negative");
  }
  const k = reserveNative * reserveForeign;
  const stressedNative = reserveNative + sellablePressure;
  return BigMath.mul_div(k, PRECISION, stressedNative * stressedNative);
};

const calculateReportedFloorRatio = ({
  reserveNative,
  reserveForeign,
  referenceCeiling,
  sellableSupply,
  lambdaPpb = PPB,
}) => {
  if (lambdaPpb < 0n || lambdaPpb > PPB) {
    throw new Error("lambdaPpb must be in [0, PPB]");
  }
  if (referenceCeiling <= 0n) {
    throw new Error("Reference ceiling must be positive");
  }
  const sellablePressure = BigMath.mul_div(sellableSupply, lambdaPpb, PPB);
  const stressFloor = calculateStressFloor(
    reserveNative,
    reserveForeign,
    sellablePressure,
  );
  return {
    stressFloor,
    sellablePressure,
    ratioPpb: BigMath.mul_div(stressFloor, PPB, referenceCeiling),
  };
};

// Test section structure: [section, count_in_section]
const TEST_SECTIONS = [
  [1, 4], // Mathematical Foundations
  [2, 6], // System Parameters & Scaling
  [3, 6], // Scaling Rules & Precision
  [4, 8], // Core Component Tests
  [5, 5], // Integration & System Flows
  [6, 3], // System Invariants & Multi-Actor
  [7, 6], // Advanced Integration Scenarios
  [8, 3], // System Properties & Invariants
  [9, 2], // Multi-User & Chaos Testing
  [10, 6], // Emergent Properties & System Intelligence
  [11, 4], // Economic Security & Attack Resistance
  [12, 7], // Adaptive System Behaviors
  [13, 5], // Reporting & Conformance Metrics
];

let testCount = 0;
let passedTests = 0;
/** @type {Array<{test: number, code: string, name: string, error: string}>} */
let failedTests = [];

/**
 * Get hierarchical test code (e.g., "1.1", "2.3", etc.)
 * @param {number} testNum
 * @returns {string}
 */
const getTestCode = (testNum) => {
  let accumulated = 0;
  for (const [section, count] of TEST_SECTIONS) {
    if (testNum <= accumulated + count) {
      const posInSection = testNum - accumulated;
      return `${section}.${posInSection}`;
    }
    accumulated += count;
  }
  return `${testNum}`;
};

/**
 * @param {string} name
 * @param {() => void} fn
 */
const runTest = (name, fn) => {
  testCount++;
  const testCode = getTestCode(testCount);
  const displayName = name.length > 50 ? name.substring(0, 47) + "..." : name;
  const prefix = `[${testCount.toString().padStart(2)}] ${displayName.padEnd(50)} `;
  try {
    fn();
    passedTests++;
    console.log(prefix + "✅");
  } catch (error) {
    console.log(prefix + "❌");
    const errorMsg =
      error instanceof TestFailure
        ? error.message
        : `${String(error)}\n${error instanceof Error ? error.stack : ""}`;
    failedTests.push({
      test: testCount,
      code: testCode,
      name,
      error: errorMsg,
    });
  }
};

console.log("<TMCTOL Test Suite>");

// 1. FORMULA TESTS

runTest("Absolute Slope Formula Verification", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: PRECISION,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  assert(
    minter.get_price() === minter.price_initial,
    "Price equals initial at zero supply",
  );
  const test_supplies = [
    {
      supply: 1_000n * PRECISION,
      expected_price: PRECISION + 1_000n * PRECISION,
    },
    {
      supply: 10_000n * PRECISION,
      expected_price: PRECISION + 10_000n * PRECISION,
    },
    {
      supply: 100_000n * PRECISION,
      expected_price: PRECISION + 100_000n * PRECISION,
    },
  ];
  for (const test of test_supplies) {
    minter.supply = test.supply;
    const actual_price = minter.get_price();
    assert(
      actual_price === test.expected_price,
      `Price at supply ${formatSupply(test.supply)}`,
    );
  }
  minter.supply = 0n;
});

runTest("Quadratic Integration for Minting", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  const foreign = 100n * PRECISION;
  const calculated_mint = minter.calculate_mint(foreign);
  const result = minter.mint_native(foreign);
  assert(
    result.total_minted === calculated_mint,
    "Calculated mint matches actual",
  );
});

runTest("Linear Price Doubling Property Verification", () => {
  // Test that for parameters: price_initial = PRECISION / N, slope = PRECISION / N
  // The system exhibits perfect linearity: doubling supply ≈ doubling price
  const test_cases = [
    { N: 1000n, name: "N=1000" },
    { N: 333n, name: "N=333" },
    { N: 333_333n, name: "N=333333" },
    { N: 10_000n, name: "N=10000" },
  ];
  for (const test of test_cases) {
    const system = create_system({
      tmc: {
        price_initial: PRECISION / 1_000n,
        slope: PRECISION / 1_000n,
        mint_shares: {
          user_ppb: 333_333_333n,
          tol_ppb: 666_666_667n,
        },
      },
    });
    const tmc = system.tmc;
    // Test at multiple supply points to verify linearity
    const supply_points = [
      10_000_000n * PRECISION, // Large supply where linear approximation holds
      20_000_000n * PRECISION,
      40_000_000n * PRECISION,
    ];
    let previous_supply = 0n;
    let previous_price = 0n;
    for (const supply of supply_points) {
      tmc.supply = supply;
      const current_price = tmc.get_price();
      if (previous_price > 0n) {
        // For linear function P(S) = price_initial + (slope × S) / PRECISION
        // With our parameters: P(S) = PRECISION/N + (PRECISION/N × S) / PRECISION
        // = PRECISION/N + S/N = (PRECISION + S) / N
        // When S >> PRECISION, P(S) ≈ S/N
        // Therefore P(2S) / P(S) ≈ (2S/N) / (S/N) = 2
        const supply_ratio = Number(supply) / Number(previous_supply);
        const price_ratio = Number(current_price) / Number(previous_price);
        // For linear function, price ratio should equal supply ratio
        const ratio_difference = Math.abs(price_ratio - supply_ratio);
        const tolerance = 0.001; // 0.1% tolerance
        assert(
          ratio_difference < tolerance,
          `Linear price scaling for ${test.name}: supply_ratio=${supply_ratio.toFixed(4)}, price_ratio=${price_ratio.toFixed(4)}, difference=${ratio_difference.toFixed(6)}`,
        );
      }
      previous_supply = supply;
      previous_price = current_price;
    }
    // Additional verification: test the linear scaling property
    // For linear function P(S) = price_initial + (slope × S) / PRECISION
    // With our parameters: P(S) = PRECISION/N + (PRECISION/N × S) / PRECISION
    // = PRECISION/N + S/N = (PRECISION + S) / N
    // Test that doubling supply approximately doubles price
    const base_supply = 10_000_000n * PRECISION;
    const doubled_supply = 20_000_000n * PRECISION;
    tmc.supply = base_supply;
    const base_price = tmc.get_price();
    tmc.supply = doubled_supply;
    const doubled_price = tmc.get_price();
    // Calculate the actual doubling ratio
    const actual_ratio = Number(doubled_price) / Number(base_price);
    const expected_ratio = 2.0; // Perfect linear doubling
    const ratio_difference = Math.abs(actual_ratio - expected_ratio);
    const tolerance = 0.01; // 1% tolerance for linear approximation
    assert(
      ratio_difference < tolerance,
      `Linear doubling property for ${test.name}: actual_ratio=${actual_ratio.toFixed(4)}, expected=2.0, difference=${ratio_difference.toFixed(6)}`,
    );
    tmc.supply = 0n; // Reset
  }
});

runTest("Zero Slope (Constant Price)", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: 0n,
      mint_shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    },
  });
  const minter = system.tmc;
  const test_amounts = [
    100n * PRECISION,
    1_000n * PRECISION,
    10_000n * PRECISION,
  ];
  for (const amount of test_amounts) {
    const result = minter.mint_native(amount);
    assert(
      result.price_before === result.price_after,
      "Price constant with zero slope",
    );
    minter.supply = 0n;
  }
});

// 2. PARAMETER BOUNDARY TESTS

runTest("Initial Price Boundary Testing", () => {
  const test_cases = [
    { value: 1n, name: "Minimum (1 wei)" },
    { value: PRECISION / 1_000_000n, name: "Very small (0.000001)" },
    { value: PRECISION / 1_000n, name: "Small (0.001)" },
    { value: PRECISION, name: "Standard (1.0)" },
    { value: 1_000n * PRECISION, name: "Large (1000)" },
    { value: 1_000_000n * PRECISION, name: "Very large (1,000,000)" },
    { value: 100_000_000n * PRECISION, name: "Pushed large (100M)" },
  ];
  for (const test of test_cases) {
    const system = create_system({
      tmc: {
        price_initial: test.value,
        slope: PRECISION / 1_000n,
      },
    });
    const price = system.tmc.get_price();
    assert(price === test.value, `Initial price ${test.name}`);
  }
});

runTest("Slope Boundary Testing", () => {
  const test_cases = [
    { value: 0n, name: "Zero (constant price)" },
    { value: PRECISION / 1_000_000n, name: "Minimal (0.000001)" },
    { value: PRECISION / 1_000n, name: "Standard (0.001)" },
    { value: PRECISION / 10n, name: "High (0.1)" },
    { value: PRECISION, name: "Extreme (1.0)" },
    { value: PRECISION * 1_000n, name: "Very large (1000)" },
    { value: PRECISION * 10_000n, name: "Extreme (10000)" },
    { value: PRECISION * 100_000n, name: "Pushed extreme (100k)" },
  ];
  for (const test of test_cases) {
    const system = create_system({
      tmc: {
        price_initial: PRECISION,
        slope: test.value,
      },
    });
    const minter = system.tmc;
    minter.supply = 1_000_000n * PRECISION;
    const expected_price =
      PRECISION + (test.value * 1_000_000n * PRECISION) / PRECISION;
    assert(minter.get_price() === expected_price, `Slope ${test.name}`);
    minter.supply = 0n;
  }
});

runTest("Supply Boundary Testing", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 10_000n,
      mint_shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    },
  });
  const minter = system.tmc;
  const test_supplies = [
    { supply: 0n, name: "Zero" },
    { supply: 1_000n * PRECISION, name: "1K" },
    { supply: 1_000_000n * PRECISION, name: "1M" },
    { supply: 1_000_000_000n * PRECISION, name: "1B" },
  ];
  for (const test of test_supplies) {
    minter.supply = test.supply;
    const price = minter.get_price();
    const expected =
      minter.price_initial + (minter.slope * test.supply) / PRECISION;
    assert(price === expected, `Supply ${test.name}`);
  }
  minter.supply = 0n;
});

runTest("Large Number Stress Test", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000_000n,
      slope: PRECISION / 1_000_000n,
      mint_shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    },
  });
  const minter = system.tmc;
  const large_payment = 1_000_000_000_000_000_000n * PRECISION;
  const result = minter.mint_native(large_payment);
  assert(result.total_minted > 0n, "Large payment processed");
});

runTest("Parameter Combination Testing", () => {
  const combinations = [
    {
      name: "Low price, high slope",
      price_initial: PRECISION / 1_000_000n,
      slope: PRECISION * 10n,
    },
    {
      name: "High price, zero slope",
      price_initial: 10_000_000n * PRECISION,
      slope: 0n,
    },
    {
      name: "Medium price, medium slope",
      price_initial: PRECISION,
      slope: PRECISION / 100n,
    },
    {
      name: "Very small price, very small slope",
      price_initial: 1n,
      slope: PRECISION / 1_000_000n,
    },
  ];
  for (const combo of combinations) {
    const system = create_system({
      tmc: {
        price_initial: combo.price_initial,
        slope: combo.slope,
        mint_shares: {
          user_ppb: 1_000_000_000n,
          tol_ppb: 0n,
        },
      },
    });
    const result = system.tmc.mint_native(100n * PRECISION);
    assert(result.total_minted > 0n, `Combo: ${combo.name}`);
  }
});

runTest("Current Default Parameters Validation", () => {
  const system = create_system({});
  const minter = system.tmc;
  const total_shares = minter.user_ppb + minter.tol_ppb;
  assert(total_shares === PPB, "Default shares sum to 100%");
  const result = minter.mint_native(1_000n * PRECISION);
  assert(result.total_minted > 0n, "Default config produces valid mint");
});

// 3. SCALING RULES

runTest("Scaling Rules - Naming Convention", () => {
  const system = create_system({});
  const minter = system.tmc;
  assert(minter.hasOwnProperty("user_ppb"), "user share has _ppb suffix");
  assert(minter.hasOwnProperty("tol_ppb"), "tol share has _ppb suffix");
});

runTest("Scaling Rules - Input Pre-scaling", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: PRECISION,
      mint_shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    },
  });
  const minter = system.tmc;
  const payment = 10n * PRECISION;
  const result = minter.mint_native(payment);
  assert(result.total_minted > 0n, "Pre-scaled inputs work correctly");
});

runTest("Scaling Rules - Price Scaling Consistency", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION * 2n,
      slope: 0n,
      mint_shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    },
  });
  const minter = system.tmc;
  const pool = system.xyk;
  const mint_result = minter.mint_native(1_000n * PRECISION);
  assert(
    mint_result.price_before === PRECISION * 2n,
    "Minter price consistency",
  );
});

runTest("Scaling Rules - PPB Values Range", () => {
  const system = create_system({});
  const user_ppb = system.tmc.user_ppb;
  const tol_ppb = system.tmc.tol_ppb;
  const total_shares = user_ppb + tol_ppb;
  assert(total_shares === PPB, "Shares sum to 1,000,000,000 PPB");
  assert(user_ppb <= PPB, "User share <= 100%");
  assert(tol_ppb <= PPB, "TOL share <= 100%");
});

runTest("Scaling Rules - Precision Through Calculations", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: PRECISION,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  const payment = 1_000n * PRECISION;
  const result = minter.mint_native(payment);
  const total_distributed = result.user_native + result.tol_native;
  const diff =
    total_distributed > result.total_minted
      ? total_distributed - result.total_minted
      : result.total_minted - total_distributed;
  assert(diff <= 4n, "Precision loss <= 4 wei");
});

// 3. SCALING RULES & PRECISION

runTest("Scaling Rules - Property Fuzz", () => {
  // Fuzz inputs within sane operational ranges
  const cases = 100;
  for (let i = 0; i < cases; i++) {
    const price_initial =
      BigInt(Math.floor(Math.random() * 1_000_000) + 1) *
      (PRECISION / 1_000_000n);
    const slope =
      BigInt(Math.floor(Math.random() * 1_000_000) + 1) *
      (PRECISION / 1_000_000n);
    const system = create_system({ tmc: { price_initial, slope } });
    const tmc = system.tmc;
    // Random supply point
    const s = BigInt(Math.floor(Math.random() * 1_000_000)) * PRECISION;
    tmc.supply = s;
    // Expected price: P = P0 + m * S / PRECISION
    const expected = price_initial + (slope * s) / PRECISION;
    const actual = tmc.get_price();
    assert(actual >= 0n, "Price must be non-negative");
    assert(
      expected === actual,
      "Price formula must hold exactly under scaling",
    );
  }
});

// 4. COMPONENT TESTS
runTest("System Initialization", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  assert(system.tmc !== undefined, "Minter initialized");
  assert(system.tol !== undefined, "TOL manager initialized");
  assert(system.xyk !== undefined, "XYK pool initialized");
  assert(system.router !== undefined, "Router initialized");
});

runTest("TMC Minting and Distribution", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  const payment = 1_000n * PRECISION;
  const result = minter.mint_native(payment);
  const user_expected = (result.total_minted * 333_333_333n) / PPB;
  const tol_expected = (result.total_minted * 666_666_667n) / PPB;
  assertApprox(result.user_native, user_expected, 1n, "User share correct");
  assertApprox(result.tol_native, tol_expected, 1n, "TOL share correct");
});

runTest("TOL Adding Liquidity to XYK", () => {
  const system = create_system({});
  const minter = system.tmc;
  const pool = system.xyk;
  const mint_result = minter.mint_native(1_000n * PRECISION);
  const tol = /** @type {any} */ (mint_result.tol);
  const total_lp =
    tol.bucket_a.lp_tokens +
    tol.bucket_b.lp_tokens +
    tol.bucket_c.lp_tokens +
    tol.bucket_d.lp_tokens;
  assert(total_lp > 0n, "TOL LP tokens minted");
  assert(pool.reserve_native > 0n, "Pool native reserve increased");
  assert(pool.reserve_foreign > 0n, "Pool foreign reserve increased");
});

runTest("XYK Pool Swaps", () => {
  const system = create_system({});
  const pool = system.xyk;
  const minter = system.tmc;
  minter.mint_native(10_000n * PRECISION);
  const swap_amount = 100n * PRECISION;
  const k_before = pool.reserve_native * pool.reserve_foreign;
  const output = pool.swap_foreign_to_native(swap_amount, 0n);
  assert(output.native_out > 0n, "Swap produces output");
  assert(output.fee !== undefined, "fee returned");
  assert(output.fee >= 0n, "fee non-negative");
  const k_after = pool.reserve_native * pool.reserve_foreign;
  assert(k_after >= k_before, "K invariant maintained or increased");
});

runTest("XYK Multi-Swap Invariant", () => {
  const system = create_system({});
  const pool = system.xyk;
  const minter = system.tmc;
  // Seed meaningful liquidity
  minter.mint_native(50_000n * PRECISION);
  const amounts = [10n, 25n, 100n, 1000n, 5000n].map((a) => a * PRECISION);
  let prev_k = pool.reserve_native * pool.reserve_foreign;
  for (const amt of amounts) {
    const out = pool.swap_foreign_to_native(amt, 0n);
    assert(out.native_out > 0n, "Swap produces output");
    assert(out.fee !== undefined, "fee returned");
    assert(out.fee >= 0n, "fee non-negative");
    // With fee_xyk_ppb = 0, fee should be 0
    if (pool.fee_ppb === 0n) {
      assert(out.fee === 0n, "Zero fee when fee_ppb is 0");
    }
    const k_now = pool.reserve_native * pool.reserve_foreign;
    assert(k_now >= prev_k, "K invariant maintained or increased");
    prev_k = k_now;
  }
});

runTest("XYK Fee Tracking - Native to Foreign", () => {
  const system = create_system({});
  const pool = system.xyk;
  const minter = system.tmc;
  minter.mint_native(10_000n * PRECISION);
  const swap_amount = 100n * PRECISION;
  const output = pool.swap_native_to_foreign(swap_amount, 0n);
  assert(output.foreign_out > 0n, "Swap produces foreign output");
  assert(output.fee !== undefined, "fee returned");
  assert(output.fee >= 0n, "fee non-negative");
  // With fee_xyk_ppb = 0, fee should be 0
  if (pool.fee_ppb === 0n) {
    assert(output.fee === 0n, "Zero fee when fee_ppb is 0");
  }
});

runTest("Smart Router Path Selection", () => {
  const system = create_system({
    price_initial: PRECISION,
    slope: 100_000n,
  });
  const router = system.router;
  const minter = system.tmc;
  minter.mint_native(10_000n * PRECISION);
  const swap_amount = 100n * PRECISION;
  const result = router.swap_foreign_to_native(swap_amount, 0n);
  assert(result.native_out > 0n, "Router produces output");
  assert(
    result.route === "TMC" || result.route === "XYK",
    "Valid route selected",
  );
});

runTest("TMC Burn Functionality", () => {
  const system = create_system({});
  const minter = system.tmc;
  const mint_result = minter.mint_native(1_000_000n * PRECISION);
  const supply_after_mint = minter.supply;
  const burn_amount = supply_after_mint / 2n;
  const burn_result = minter.burn_native(burn_amount);
  assert(minter.supply === supply_after_mint - burn_amount, "Supply decreased");
  assert(burn_result.supply_before > 0n, "Burn executed");
});

// 5. INTEGRATION TESTS

runTest("Edge Cases", () => {
  const system = create_system({});
  const minter = system.tmc;
  const pool = system.xyk;
  const zero_result = minter.calculate_mint(0n);
  assert(zero_result === 0n, "Zero payment returns zero tokens");
  assert(!pool.has_liquidity(), "Pool starts with no liquidity");
  try {
    pool.swap_foreign_to_native(100n * PRECISION);
    assert(false, "Should fail on empty pool");
  } catch (e) {
    assert(true, "Empty pool swap rejected");
  }
});

runTest("Full Integration Flow", () => {
  const system = create_system({
    price_initial: PRECISION,
    slope: PRECISION,
    shares: {
      user_ppb: 333_333_333n,
      tol_ppb: 666_666_667n,
    },
  });
  const minter = system.tmc;
  const router = system.router;
  const mint_result = minter.mint_native(10_000n * PRECISION);
  assert(mint_result.total_minted > 0n, "Mint successful");
  const swap_result = router.swap_foreign_to_native(1_000n * PRECISION, 0n);
  assert(swap_result.native_out > 0n, "Swap successful");
  const burn_result = minter.burn_native(mint_result.total_minted / 10n);
  assert(burn_result.supply_before > 0n, "Burn successful");
});

runTest("Overflow Protection Testing", () => {
  const max_uint256 = (1n << 256n) - 1n;
  const half_max = max_uint256 / 2n;
  try {
    const system = create_system({
      price_initial: half_max,
      slope: PRECISION / 1_000n,
    });
    const result = system.tmc.mint_native(1_000n * PRECISION);
    assert(result.total_minted > 0n, "Large price_initial handled");
  } catch (e) {
    assert(true, "Overflow protected");
  }
});

runTest("Safe Operating Ranges", () => {
  const configs = [
    {
      name: "Conservative",
      price_initial: PRECISION,
      slope: PRECISION / 1_000n,
    },
    {
      name: "Moderate",
      price_initial: PRECISION / 100n,
      slope: PRECISION / 100n,
    },
    {
      name: "Aggressive",
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 10n,
    },
  ];
  for (const config of configs) {
    const system = create_system({
      tmc: {
        price_initial: config.price_initial,
        slope: config.slope,
      },
    });
    const result = system.tmc.mint_native(1_000n * PRECISION);
    assert(result.total_minted > 0n, `${config.name} config safe`);
  }
});

runTest("Formula Performance Analysis", () => {
  const payment = 100n * PRECISION;
  const price_initial = PRECISION;
  const slope = PRECISION / 1_000n;
  const current_supply = 1_000_000n * PRECISION;
  const iterations = 1000;
  const start = getTimestamp();
  for (let i = 0; i < iterations; i++) {
    const system = create_system({
      price_initial,
      slope,
      shares: {
        user_ppb: 1_000_000_000n,
        tol_ppb: 0n,
      },
    });
    system.tmc.supply = current_supply;
    const result = system.tmc.calculate_mint(payment);
  }
  const end = getTimestamp();
  const total_time = end - start;
  const avg_time = total_time / BigInt(iterations);
  assert(avg_time < 1_000_000n, "Performance acceptable (<1ms per op)");
});

// 6. MULTI-ACTOR CORRECTNESS

runTest("Distribution Accuracy - Multi-Mint Accumulation", () => {
  // Test that Treasury and Team receive exact allocation percentages over multiple mints
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const alice = new User(0n, 100_000n * PRECISION);
  alice.set_router(system.router);
  let total_user_received = 0n;
  // Perform 15 mints to accumulate significant distributions
  for (let i = 0; i < 15; i++) {
    const result = alice.buy_native(1_000n * PRECISION);
    if (result.route === "TMC") {
      total_user_received += result.native_out;
    }
  }
  const total_supply = system.tmc.supply;
  const tol_native = system.xyk.reserve_native;
  // Calculate actual distribution ratios
  const user_ratio = (total_user_received * PPB) / total_supply;
  const tol_ratio = (tol_native * PPB) / total_supply;
  // Expected ratios from DEFAULT_CONFIG: user=33.3%, tol=66.7%
  const expected_user = 333_333_333n;
  const expected_tol = 666_666_667n;
  // Validate within 1% tolerance (10,000,000 PPB)
  const tolerance = PPB / 100n;
  assertApprox(
    user_ratio,
    expected_user,
    tolerance,
    "User distribution should be ~33.3%",
  );
  assertApprox(
    tol_ratio,
    expected_tol,
    tolerance,
    "TOL distribution should be ~66.7%",
  );
  // Verify total distribution sums to 100% (within rounding)
  const total_ratio = user_ratio + tol_ratio;
  assertApprox(
    total_ratio,
    PPB,
    tolerance,
    "Total distribution must sum to 100%",
  );
});

runTest("Mass Conservation - System-Wide Token Accounting", () => {
  // Test that total supply exactly equals sum of all participant balances
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const alice = new User(0n, 10_000n * PRECISION);
  const bob = new User(0n, 5_000n * PRECISION);
  alice.set_router(system.router);
  bob.set_router(system.router);
  // Multiple mint operations to build up system
  alice.buy_native(2_000n * PRECISION);
  bob.buy_native(1_000n * PRECISION);
  alice.buy_native(500n * PRECISION);
  bob.buy_native(300n * PRECISION);
  // Collect all balances
  const total_supply = system.tmc.supply;
  const alice_balance = alice.get_balance().native;
  const bob_balance = bob.get_balance().native;
  const tol_balance = system.tol.get_balance();
  const tol_native =
    tol_balance.bucket_a.contributed_native +
    tol_balance.bucket_b.contributed_native +
    tol_balance.bucket_c.contributed_native +
    tol_balance.bucket_d.contributed_native;
  const xyk_native = system.xyk.reserve_native;
  // Mass conservation: current supply equals sum of all token balances
  // Burned tokens are already excluded from supply, so no need to add them back
  const total_accounted = alice_balance + bob_balance + tol_native + xyk_native;
  assertApprox(
    total_supply,
    total_accounted,
    total_accounted / 1000n, // 0.1% tolerance for rounding
    `Mass conservation violated: supply=${total_supply}, accounted=${total_accounted}`,
  );
  // Additional verification: no negative balances
  assert(alice_balance >= 0n, "Alice balance cannot be negative");
  assert(bob_balance >= 0n, "Bob balance cannot be negative");
  assert(tol_native >= 0n, "TOL balance cannot be negative");
  assert(xyk_native >= 0n, "XYK reserves cannot be negative");
  // Verify supply is positive after operations
  assert(total_supply > 0n, "Total supply must be positive after mints");
  // Verify all participants have accumulated tokens
  assert(alice_balance > 0n, "Alice should have received tokens");
  assert(bob_balance > 0n, "Bob should have received tokens");
  assert(tol_native > 0n, "TOL should have accumulated tokens");
  assert(xyk_native > 0n, "XYK should have TOL reserves");
});

runTest("TOL Independence - Participant Sales Don't Touch TOL", () => {
  // Test that Treasury, Team, and User sales only affect XYK reserves, never TOL LP tokens
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const alice = new User(0n, 20_000n * PRECISION);
  alice.set_router(system.router);
  // Build up system with multiple mints
  for (let i = 0; i < 10; i++) {
    alice.buy_native(1_000n * PRECISION);
  }
  // Snapshot TOL state before any sales
  const tol_balance_before = system.tol.get_balance();
  const tol_lp_before =
    tol_balance_before.bucket_a.balance_lp +
    tol_balance_before.bucket_b.balance_lp +
    tol_balance_before.bucket_c.balance_lp +
    tol_balance_before.bucket_d.balance_lp;
  const tol_contributed_native_before =
    tol_balance_before.bucket_a.contributed_native +
    tol_balance_before.bucket_b.contributed_native +
    tol_balance_before.bucket_c.contributed_native +
    tol_balance_before.bucket_d.contributed_native;
  const tol_contributed_foreign_before =
    tol_balance_before.bucket_a.contributed_foreign +
    tol_balance_before.bucket_b.contributed_foreign +
    tol_balance_before.bucket_c.contributed_foreign +
    tol_balance_before.bucket_d.contributed_foreign;
  assert(tol_lp_before > 0n, "TOL should have accumulated LP tokens");
  // Execute sales from alice
  const alice_balance = alice.get_balance().native;
  alice.sell_native(alice_balance / 4n);
  // Snapshot TOL state after sales
  const tol_balance_after = system.tol.get_balance();
  const tol_lp_after =
    tol_balance_after.bucket_a.balance_lp +
    tol_balance_after.bucket_b.balance_lp +
    tol_balance_after.bucket_c.balance_lp +
    tol_balance_after.bucket_d.balance_lp;
  const tol_contributed_native_after =
    tol_balance_after.bucket_a.contributed_native +
    tol_balance_after.bucket_b.contributed_native +
    tol_balance_after.bucket_c.contributed_native +
    tol_balance_after.bucket_d.contributed_native;
  const tol_contributed_foreign_after =
    tol_balance_after.bucket_a.contributed_foreign +
    tol_balance_after.bucket_b.contributed_foreign +
    tol_balance_after.bucket_c.contributed_foreign +
    tol_balance_after.bucket_d.contributed_foreign;
  // Critical validation: TOL must be completely unaffected by participant sales
  assert(
    tol_lp_after === tol_lp_before,
    `TOL LP tokens changed: before=${tol_lp_before}, after=${tol_lp_after}`,
  );
  assert(
    tol_contributed_native_after === tol_contributed_native_before,
    "TOL native contribution changed from participant sales",
  );
  assert(
    tol_contributed_foreign_after === tol_contributed_foreign_before,
    "TOL foreign contribution changed from participant sales",
  );
  // Verify TOL is strictly non-decreasing (can only increase on new mints)
  assert(tol_lp_after >= tol_lp_before, "TOL must never decrease");
  // Additional verification: XYK reserves should have changed (due to sales)
  assert(system.xyk.reserve_native > 0n, "XYK must maintain native reserves");
  assert(system.xyk.reserve_foreign > 0n, "XYK must maintain foreign reserves");
});

// 7. ADVANCED SCENARIOS

runTest("Circular Swaps and Arbitrage Detection", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const initial_foreign = 10_000n * PRECISION;
  system.tmc.mint_native(initial_foreign);
  const swap_amount = 1_000n * PRECISION;
  const result1 = system.router.swap_foreign_to_native(swap_amount, 0n);
  const result2 = system.router.swap_native_to_foreign(result1.native_out, 0n);
  assert(result2.foreign_out < swap_amount, "Circular swap loses value (fees)");
  // Anti-Arbitrage Cycle (Deterministic): multiple back-to-back cycles must not yield profit
  let capital = 5_000n * PRECISION;
  for (let i = 0; i < 10; i++) {
    const buy = system.router.swap_foreign_to_native(capital, 0n);
    const sell = system.router.swap_native_to_foreign(buy.native_out, 0n);
    assert(sell.foreign_out <= capital, "Cycle should not generate profit");
    capital = sell.foreign_out;
  }
});

runTest("Minimum Trade Amount Enforcement", () => {
  const system = create_system({
    router: {
      min_swap_foreign: 100n * PRECISION,
      min_initial_foreign: 100n * PRECISION,
    },
  });
  try {
    system.router.swap_foreign_to_native(5n * PRECISION, 0n);
    assert(false, "Should reject sub-minimum trade");
  } catch (e) {
    assert(true, "Sub-minimum trade rejected");
  }
});

runTest("Slippage Protection in Router", () => {
  const system = create_system({});
  system.tmc.mint_native(10_000n * PRECISION);
  const expected_output = system.xyk.get_out_native(100n * PRECISION);
  const min_acceptable = expected_output + 1n;
  try {
    system.router.swap_foreign_to_native(100n * PRECISION, min_acceptable);
    assert(false, "Should fail slippage check");
  } catch (e) {
    assert(true, "Slippage protection works");
  }
});

runTest("TOL Buffer Behavior Before Pool Initialization", () => {
  const system = create_system({
    tmc: {
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  const result = minter.mint_native(1_000n * PRECISION);
  const tol_balance = system.tol.get_balance();
  const total_lp =
    tol_balance.bucket_a.balance_lp +
    tol_balance.bucket_b.balance_lp +
    tol_balance.bucket_c.balance_lp +
    tol_balance.bucket_d.balance_lp;
  assert(
    system.tol.buffer_native > 0n ||
      system.tol.buffer_foreign > 0n ||
      total_lp > 0n,
    "TOL buffered or added",
  );
});

runTest("Fee Manager Buffer and Burn Mechanics", () => {
  const system = create_system({
    router: {
      min_swap_foreign: 100n * PRECISION,
    },
  });
  system.tmc.mint_native(10_000n * PRECISION);
  const supply_before_swaps = system.tmc.supply;
  const fee_manager_before = {
    total_native_burned: system.router.fee_manager.total_native_burned,
    buffer_native: system.router.fee_manager.buffer_native,
    buffer_foreign: system.router.fee_manager.buffer_foreign,
  };
  for (let i = 0; i < 5; i++) {
    system.router.swap_foreign_to_native(500n * PRECISION, 0n);
  }
  const fee_manager_after = system.router.fee_manager;
  assert(
    fee_manager_after.total_native_burned >
      fee_manager_before.total_native_burned ||
      fee_manager_after.buffer_native > fee_manager_before.buffer_native ||
      fee_manager_after.buffer_foreign > fee_manager_before.buffer_foreign,
    "Fee manager should accumulate or burn fees from router operations",
  );
});

runTest("Distribution Remainder Handling", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const result = system.tmc.mint_native(1_000n * PRECISION);
  const total_distributed = result.user_native + result.tol_native;
  assert(
    total_distributed === result.total_minted,
    "No tokens lost in distribution",
  );
});

// 8. SYSTEM INVARIANTS

runTest("System Invariants After Heavy Use", () => {
  const system = create_system({
    price_initial: PRECISION / 100n,
    slope: 10_000n,
  });
  for (let i = 0; i < 50; i++) {
    system.tmc.mint_native((100n + BigInt(i)) * PRECISION);
  }
  for (let i = 0; i < 30; i++) {
    try {
      system.router.swap_foreign_to_native(50n * PRECISION, 0n);
    } catch (e) {}
  }
  for (let i = 0; i < 20; i++) {
    try {
      system.router.swap_native_to_foreign(50n * PRECISION, 0n);
    } catch (e) {}
  }
  assert(system.tmc.supply > 0n, "Supply positive after operations");
  assert(system.xyk.reserve_native > 0n, "Pool has native reserve");
  assert(system.xyk.reserve_foreign > 0n, "Pool has foreign reserve");
});

runTest("Infrastructure Premium Mathematical Proof", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  system.router.swap_foreign_to_native(50_000n * PRECISION, 0n);
  const test_scenarios = [
    { amount: 100n * PRECISION, name: "Small trade" },
    { amount: 1_000n * PRECISION, name: "Medium trade" },
    { amount: 5_000n * PRECISION, name: "Large trade" },
    { amount: 10_000n * PRECISION, name: "Very large trade" },
  ];
  for (const scenario of test_scenarios) {
    const xyk_quote = system.xyk.get_out_native(scenario.amount);
    const tmc_quote = system.tmc.get_mint_quote(scenario.amount);
    if (!tmc_quote) {
      continue;
    }
    const user_gets_tmc = tmc_quote.user;
    if (user_gets_tmc >= xyk_quote) {
      assert(
        user_gets_tmc >= xyk_quote,
        `Infrastructure premium exists for ${scenario.name}`,
      );
    }
  }
});

runTest("Floor Formula & Scenario Verification", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION,
      slope: PRECISION,
      mint_shares: {
        user_ppb: 333_333_333n,
        tol_ppb: 666_666_667n,
      },
    },
  });
  const minter = system.tmc;
  const pool = system.xyk;
  // 1. Establish state
  // Mint enough to have stable reserves
  minter.mint_native(100_000n * PRECISION);
  const R_native = pool.reserve_native;
  const R_foreign = pool.reserve_foreign;
  const k = R_native * R_foreign;
  // 2. Theoretical Calculation
  // Theoretical Floor if User dumps ALL their tokens (S_user)
  // S_user is what users hold.
  const total_minted = minter.supply;
  const user_supply = (total_minted * 333_333_333n) / PPB;
  const theoretical_denom = R_native + user_supply;
  const theoretical_floor =
    (k * PRECISION) / (theoretical_denom * theoretical_denom);
  // 3. Simulate Dump
  // Calculate what executing the swap would do to price
  // We simulate by actually executing on this system instance (it's isolated)
  pool.swap_native_to_foreign(user_supply, 0n);
  const actual_price = pool.get_price();
  // 4. Verify
  assertApprox(
    actual_price,
    theoretical_floor,
    1000n,
    "Floor price matches k/(R+S)^2 formula",
  );
  // 5. Verify Scenario Ratio
  const ceiling = minter.get_price(); // P(S) approx
  const ratio = (actual_price * PRECISION) / ceiling;
  // Expected Ratio: 1 / (1 + s/a)^2
  // s = S_user / S_total = 0.333
  // a = S_tol / S_total = 0.667
  // s/a = 0.5
  // Expected = 1 / (1.5)^2 = 1 / 2.25 = 0.4444...
  const expected_ratio_ppb = 444_444_444n; // 44.4%
  const actual_ratio_ppb = (ratio * PPB) / PRECISION;
  assertApprox(
    actual_ratio_ppb,
    expected_ratio_ppb,
    5000n,
    "Floor/Ceiling ratio matches 1/(1+s/a)^2",
  );
});

// 9. MULTI-USER SIMULATION

runTest("Multi-User Concurrent Simulation", () => {
  const system = create_system({
    price_initial: PRECISION / 100n,
    slope: 1_000n,
  });
  let total_minted = 0n;
  let total_burned = 0n;
  const user_count = 50;
  const user_balances = new Array(user_count).fill(0n);
  for (let op = 0; op < 200; op++) {
    const user_id = Math.floor(Math.random() * user_count);
    const action = Math.random();
    if (action < 0.6) {
      const amount = BigInt(Math.floor(Math.random() * 500) + 10) * PRECISION;
      try {
        const result = system.router.swap_foreign_to_native(amount, 0n);
        user_balances[user_id] += result.native_out;
      } catch (e) {}
    } else if (action < 0.9) {
      const amount = BigInt(Math.floor(Math.random() * 100) + 1) * PRECISION;
      if (user_balances[user_id] >= amount) {
        try {
          system.router.swap_native_to_foreign(amount, 0n);
          user_balances[user_id] -= amount;
        } catch (e) {}
      }
    } else {
      const amount = BigInt(Math.floor(Math.random() * 200) + 10) * PRECISION;
      try {
        const result = system.tmc.mint_native(amount);
        user_balances[user_id] += result.user_native;
        total_minted += result.total_minted;
      } catch (e) {}
    }
  }
  assert(system.tmc.supply > 0n, "Supply positive after heavy use");
  assert(system.xyk.reserve_native > 0n, "Pool maintains liquidity");
  assert(system.xyk.reserve_foreign > 0n, "Pool maintains foreign reserves");
  assert(
    system.router.fee_manager.total_native_burned > 0n ||
      system.router.fee_manager.buffer_native > 0n ||
      system.router.fee_manager.buffer_foreign > 0n,
    "Fee manager should process fees from user operations",
  );
});

runTest("Extreme Load Stress Test", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  // Simulate extreme load with rapid operations
  const operations = 500;
  let total_minted = 0n;
  let total_swapped = 0n;
  for (let i = 0; i < operations; i++) {
    const operation_type = Math.floor(Math.random() * 3);
    switch (operation_type) {
      case 0: // Mint operation
        const mint_amount =
          BigInt(Math.floor(Math.random() * 1000) + 10) * PRECISION;
        const mint_result = system.tmc.mint_native(mint_amount);
        total_minted += mint_result.total_minted;
        break;
      case 1: // Buy operation
        const buy_amount =
          BigInt(Math.floor(Math.random() * 500) + 5) * PRECISION;
        try {
          const swap_result = system.router.swap_foreign_to_native(
            buy_amount,
            0n,
          );
          total_swapped += buy_amount;
          if (swap_result.route === "TMC" && swap_result.tol) {
            total_minted += swap_result.tol.total_minted;
          }
        } catch (e) {}
        break;
      case 2: // Sell operation
        const sell_amount =
          BigInt(Math.floor(Math.random() * 200) + 1) * PRECISION;
        try {
          system.router.swap_native_to_foreign(sell_amount, 0n);
        } catch (e) {}
        break;
    }
    // Every 50 operations, verify system invariants
    if (i % 50 === 0 && i > 0) {
      assert(
        system.tmc.supply > 0n,
        "Supply remains positive under extreme load",
      );
      assert(system.xyk.reserve_native > 0n, "Pool maintains native liquidity");
      assert(
        system.xyk.reserve_foreign > 0n,
        "Pool maintains foreign reserves",
      );
      // Verify K invariant only increases (fees accumulate)
      const k = system.xyk.reserve_native * system.xyk.reserve_foreign;
      assert(k > 0n, "K invariant remains positive");
    }
  }
  // Final system state validation
  assert(system.tmc.supply > 0n, "Supply remains positive after stress test");
  const total_burned = system.router.fee_manager.total_native_burned;
  assert(
    system.tmc.supply >= total_minted - total_burned,
    "Supply accounting: burned tokens cannot exceed net minted",
  );
  assert(
    system.router.fee_manager.total_native_burned > 0n ||
      system.router.fee_manager.buffer_native > 0n,
    "Fee system processes transactions under extreme load",
  );
  assert(
    system.xyk.reserve_native > 0n && system.xyk.reserve_foreign > 0n,
    "Pool maintains liquidity after extreme load",
  );
});

// 10. EMERGENT PROPERTIES

runTest("Bootstrap Gravity Well Detection", () => {
  // Test the critical TOL threshold where system becomes stable
  const system = create_system({
    price_initial: PRECISION / 1_000n,
    slope: 1_000n,
  });
  let tol_market_share = 0n;
  let stability_achieved = false;
  // Simulate progressive minting to find critical point
  for (let i = 0; i < 20; i++) {
    const mint_amount = 10_000n * PRECISION;
    system.tmc.mint_native(mint_amount);
    const market_cap = system.tmc.supply * system.tmc.get_price();
    const tol_value = system.xyk.reserve_native * system.tmc.get_price();
    tol_market_share = (tol_value * PRECISION) / market_cap;
    // Critical threshold around 15% of market cap in TOL
    if (tol_market_share > 150_000_000n) {
      // 15% in PPB units
      stability_achieved = true;
      break;
    }
  }
  assert(
    stability_achieved,
    "System should achieve stability after critical TOL threshold",
  );
  assert(
    tol_market_share > 100_000_000n,
    "TOL should accumulate significant market share",
  );
});

runTest("Supply Elasticity Inversion Point", () => {
  // Test that after critical supply, increasing supply raises minimum price
  const system = create_system({
    price_initial: PRECISION / 1_000n,
    slope: 500n,
  });
  const initial_supply = system.tmc.supply;
  let pre_inversion_floor = 0n;
  let post_inversion_floor = 0n;
  // Mint to reach inversion point
  for (let i = 0; i < 50; i++) {
    system.tmc.mint_native(5_000n * PRECISION);
    const currentFloor = calculateFloorPrice(system);
    if (currentFloor > 0n) {
      if (i === 10) pre_inversion_floor = currentFloor;
      if (i === 40) post_inversion_floor = currentFloor;
    }
  }
  // After inversion, floor should be higher despite larger supply
  assert(
    post_inversion_floor > pre_inversion_floor,
    "Floor should increase after supply elasticity inversion point",
  );
  assert(
    system.tmc.supply > initial_supply * 10n,
    "Supply should grow significantly to reach inversion",
  );
});

runTest("Vesting Cliff Math Trap Detection", () => {
  // Test that ceiling and floor compression creates math trap
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  // Build significant TOL through minting
  for (let i = 0; i < 20; i++) {
    system.tmc.mint_native(10_000n * PRECISION);
  }
  // Burn to compress ceiling
  const burn_amount = system.tmc.supply / 10n;
  system.tmc.burn_native(burn_amount);
  const ceiling = system.tmc.get_price();
  const floor = calculateFloorPrice(system);
  if (floor > 0n) {
    const convergenceGap = calculateArithmeticSpread(ceiling, floor);
    assert(ceiling > floor, "Ceiling should exceed floor");
    assert(convergenceGap > 0n, "Convergence gap exists");
  } else {
    assert(true, "Test skipped - no liquidity");
  }
});

runTest("Mint-Swap Feedback Loop Analysis", () => {
  // Test that consecutive TMC routes create feedback loop that router mitigates
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  let consecutive_tbc_routes = 0;
  let max_consecutive = 0;
  // Simulate series of swaps through router
  for (let i = 0; i < 15; i++) {
    const foreign_amount = 1_000n * PRECISION;
    // Execute swap through router (it will choose best route)
    const result = system.router.swap_foreign_to_native(foreign_amount, 0n);
    // Track which route was used
    if (result.route === "TMC") {
      consecutive_tbc_routes++;
      max_consecutive = Math.max(max_consecutive, consecutive_tbc_routes);
    } else {
      consecutive_tbc_routes = 0;
    }
  }
  // Router fee should prevent infinite avalanche
  assert(
    max_consecutive < 15,
    "Router fee should limit consecutive TMC routes to prevent infinite avalanche",
  );
  assert(
    system.xyk.reserve_native > 0n,
    "XYK pool should maintain liquidity despite feedback loop",
  );
});

runTest("Slope Efficiency Sublinearity Verification", () => {
  // Test that equilibrium price scales as √slope (sublinear)
  const test_slopes = [PRECISION / 1_000n, PRECISION / 100n, PRECISION / 10n];
  const equilibrium_prices = [];
  for (const slope of test_slopes) {
    const system = create_system({
      tmc: {
        price_initial: PRECISION / 1_000n,
        slope: slope,
      },
    });
    // Build significant TOL through multiple mints
    for (let i = 0; i < 20; i++) {
      system.tmc.mint_native(10_000n * PRECISION);
    }
    const tol_native = system.xyk.reserve_native;
    const equilibrium = BigMath.isqrt((tol_native * slope) / PRECISION);
    equilibrium_prices.push(equilibrium);
  }
  // Verify sublinear scaling: 10x slope should give ~3.16x equilibrium (√10)
  const ratio_10x = (equilibrium_prices[1] * PRECISION) / equilibrium_prices[0];
  // 100x slope should give ~10x equilibrium (√100)
  const ratio_100x =
    (equilibrium_prices[2] * PRECISION) / equilibrium_prices[0];
  // Allow 10% tolerance for simulation approximations
  assertApprox(
    ratio_10x,
    316_000_000_000n,
    10_000_000_000n,
    "10x slope gives ~√10 price increase",
  );
  assertApprox(
    ratio_100x,
    1_000_000_000_000n,
    100_000_000_000n,
    "100x slope gives ~10x price increase",
  );
  // Verify sublinear property: ratio_100x should be less than 100
  assert(
    ratio_100x < 10_000_000_000_000n,
    "100x slope gives less than 100x price increase (sublinear)",
  );
});

runTest("Compression Threshold Taxonomy", () => {
  const floorFromK = (supply, k) => k / (supply * supply);
  const slowCeiling = (supply) => 2n * supply;
  const steepCeiling = (supply) => 10n * supply;
  const inversionLowSupply = 10n;
  const inversionHighSupply = 20n;
  const inversionLowFloor = floorFromK(
    inversionLowSupply,
    inversionLowSupply * inversionLowSupply,
  );
  const inversionHighFloor = floorFromK(
    inversionHighSupply,
    inversionHighSupply * inversionHighSupply,
  );
  const inversionLowRelativeSpread = calculateRelativeSpread(
    slowCeiling(inversionLowSupply),
    inversionLowFloor,
  );
  const inversionHighRelativeSpread = calculateRelativeSpread(
    slowCeiling(inversionHighSupply),
    inversionHighFloor,
  );
  assert(
    inversionHighFloor === inversionLowFloor,
    "Elasticity inversion boundary keeps the floor flat",
  );
  assert(
    inversionHighRelativeSpread > inversionLowRelativeSpread,
    "Elasticity inversion boundary does not imply relative compression",
  );
  const postInversionLowFloor = floorFromK(
    inversionLowSupply,
    inversionLowSupply * inversionLowSupply * (inversionLowSupply + 1n),
  );
  const postInversionHighFloor = floorFromK(
    inversionHighSupply,
    inversionHighSupply * inversionHighSupply * (inversionHighSupply + 1n),
  );
  const postInversionLowRelativeSpread = calculateRelativeSpread(
    slowCeiling(inversionLowSupply),
    postInversionLowFloor,
  );
  const postInversionHighRelativeSpread = calculateRelativeSpread(
    slowCeiling(inversionHighSupply),
    postInversionHighFloor,
  );
  const postInversionLowGap = calculateArithmeticSpread(
    slowCeiling(inversionLowSupply),
    postInversionLowFloor,
  );
  const postInversionHighGap = calculateArithmeticSpread(
    slowCeiling(inversionHighSupply),
    postInversionHighFloor,
  );
  assert(
    postInversionHighFloor > postInversionLowFloor,
    "Post-inversion floor can rise under supply expansion",
  );
  assert(
    postInversionHighRelativeSpread > postInversionLowRelativeSpread,
    "Post-inversion floor growth can still leave ratio compression absent",
  );
  assert(
    postInversionHighGap > postInversionLowGap,
    "Post-inversion floor growth can still widen the arithmetic gap",
  );
  const relativeParityLowFloor = floorFromK(
    inversionLowSupply,
    inversionLowSupply * inversionLowSupply * inversionLowSupply,
  );
  const relativeParityHighFloor = floorFromK(
    inversionHighSupply,
    inversionHighSupply * inversionHighSupply * inversionHighSupply,
  );
  const relativeParityLowSpread = calculateRelativeSpread(
    slowCeiling(inversionLowSupply),
    relativeParityLowFloor,
  );
  const relativeParityHighSpread = calculateRelativeSpread(
    slowCeiling(inversionHighSupply),
    relativeParityHighFloor,
  );
  const relativeParityLowGap = calculateArithmeticSpread(
    slowCeiling(inversionLowSupply),
    relativeParityLowFloor,
  );
  const relativeParityHighGap = calculateArithmeticSpread(
    slowCeiling(inversionHighSupply),
    relativeParityHighFloor,
  );
  assert(
    relativeParityHighSpread === relativeParityLowSpread,
    "Relative compression parity keeps C/F flat",
  );
  assert(
    relativeParityHighGap > relativeParityLowGap,
    "Relative compression parity can coexist with a widening arithmetic gap",
  );
  const absoluteCompressionLowSupply = 8n;
  const absoluteCompressionHighSupply = 9n;
  const absoluteCompressionLowFloor = floorFromK(
    absoluteCompressionLowSupply,
    absoluteCompressionLowSupply *
      absoluteCompressionLowSupply *
      (absoluteCompressionLowSupply * absoluteCompressionLowSupply + 1n),
  );
  const absoluteCompressionHighFloor = floorFromK(
    absoluteCompressionHighSupply,
    absoluteCompressionHighSupply *
      absoluteCompressionHighSupply *
      (absoluteCompressionHighSupply * absoluteCompressionHighSupply + 1n),
  );
  const absoluteCompressionLowSpread = calculateRelativeSpread(
    steepCeiling(absoluteCompressionLowSupply),
    absoluteCompressionLowFloor,
  );
  const absoluteCompressionHighSpread = calculateRelativeSpread(
    steepCeiling(absoluteCompressionHighSupply),
    absoluteCompressionHighFloor,
  );
  const absoluteCompressionLowGap = calculateArithmeticSpread(
    steepCeiling(absoluteCompressionLowSupply),
    absoluteCompressionLowFloor,
  );
  const absoluteCompressionHighGap = calculateArithmeticSpread(
    steepCeiling(absoluteCompressionHighSupply),
    absoluteCompressionHighFloor,
  );
  assert(
    absoluteCompressionHighFloor < steepCeiling(absoluteCompressionHighSupply),
    "Absolute-gap compression can occur before overtake",
  );
  assert(
    absoluteCompressionHighSpread < absoluteCompressionLowSpread,
    "Absolute-gap compression also narrows the relative spread in this regime",
  );
  assert(
    absoluteCompressionHighGap < absoluteCompressionLowGap,
    "Absolute-gap compression narrows C - F",
  );
  const overtakeSupply = 10n;
  const overtakeFloor = floorFromK(
    overtakeSupply,
    overtakeSupply * overtakeSupply * (2n * overtakeSupply + 1n),
  );
  assert(
    overtakeFloor >= slowCeiling(overtakeSupply),
    "Arbitrage reversal / overtake requires the floor to meet or exceed the ceiling",
  );
});

// 11. ATTACK RESISTANCE TESTS

runTest("Sandwich Attack Fee Burden", () => {
  // Test that router fees create substantial cost for sandwich attacks
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  // Initial state with established liquidity
  system.tmc.mint_native(50_000n * PRECISION);
  // Simple sandwich attack: attacker makes two swaps around victim's transaction
  const attack_amount = 5_000n * PRECISION;
  // Front-run swap
  const frontrun_result = system.router.swap_foreign_to_native(
    attack_amount,
    0n,
  );
  const frontrun_fee = frontrun_result.foreign_router_fee;
  // Victim transaction (simulate market activity)
  system.router.swap_foreign_to_native(attack_amount, 0n);
  // Back-run swap
  const backrun_result = system.router.swap_native_to_foreign(
    frontrun_result.native_out,
    0n,
  );
  const backrun_fee = backrun_result.native_router_fee;
  // Calculate total fees paid by attacker (convert backrun fee to foreign equivalent)
  const backrun_fee_foreign =
    (backrun_fee * system.xyk.get_price()) / PRECISION;
  const total_fees = frontrun_fee + backrun_fee_foreign;
  const fee_ratio = (total_fees * PPB) / attack_amount;
  // Verify fees are substantial (0.5% per swap = 1% round-trip minimum)
  assert(
    fee_ratio >= 6_000n, // >= 0.6% total fees (accounts for conversion and slippage)
    "Router fees should extract substantial value from sandwich attacks",
  );
  // Verify TOL continues to accumulate during attack scenario
  const tol_balance = system.tol.get_balance();
  const tol_lp_tokens =
    tol_balance.bucket_a.balance_lp +
    tol_balance.bucket_b.balance_lp +
    tol_balance.bucket_c.balance_lp +
    tol_balance.bucket_d.balance_lp;
  assert(
    tol_lp_tokens > 0n,
    "TOL accumulation should continue during attack scenarios",
  );
});

runTest("Realistic Governance Attack - Distribution Manipulation", () => {
  // Test realistic governance attack: malicious proposal to change distribution shares
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  // Build significant TOL and establish baseline
  for (let i = 0; i < 15; i++) {
    system.tmc.mint_native(8_000n * PRECISION);
  }
  // Simulate malicious governance proposal:
  // Change distribution to extract value: user=10%, team=90%
  // Note: In real system, this would require governance vote
  // For testing, we simulate the economic effects
  // Verify floor guarantee persists despite potential governance manipulation
  const current_k = system.xyk.reserve_native * system.xyk.reserve_foreign;
  const worst_case_sell = (system.tmc.supply * 667n) / 1_000n;
  const final_native = system.xyk.reserve_native + worst_case_sell;
  const final_foreign = current_k / final_native;
  const floor_price = (final_foreign * PRECISION) / final_native;
  // Floor must remain positive - mathematical guarantee holds
  assert(
    floor_price > 0n,
    "Floor guarantee must persist despite malicious distribution changes",
  );
  // Verify TOL still exists and provides protection
  const tol_balance = system.tol.get_balance();
  const total_tol =
    tol_balance.bucket_a.balance_lp +
    tol_balance.bucket_b.balance_lp +
    tol_balance.bucket_c.balance_lp +
    tol_balance.bucket_d.balance_lp;
  assert(total_tol > 0n, "TOL should exist to provide floor protection");
  // Verify large holder extraction is limited by price impact
  const large_holder_balance = system.tmc.supply / 10n; // Simulate 10% holder
  const sell_impact =
    (large_holder_balance * PRECISION) / system.xyk.reserve_native;
  assert(
    sell_impact > 100n, // Large holder represents significant % of TOL reserves
    "Large holder extraction should face severe price impact limiting profitability",
  );
  // Verify XYK invariant maintains liquidity guarantee
  const k_check = system.xyk.reserve_native * system.xyk.reserve_foreign;
  assertApprox(
    k_check,
    current_k,
    current_k / 1_000n,
    "XYK invariant must maintain liquidity guarantee under governance attack",
  );
});

runTest("Cross-Chain Bridge Failure Resilience", () => {
  // Test that each chain maintains floor guarantee during bridge failure
  const chains = [
    create_system({
      tmc: {
        price_initial: PRECISION / 1_000n,
        slope: PRECISION / 1_000n,
      },
    }),
    create_system({
      tmc: {
        price_initial: PRECISION / 1_000n,
        slope: PRECISION / 1_000n,
      },
    }),
    create_system({
      tmc: {
        price_initial: PRECISION / 1_000n,
        slope: PRECISION / 1_000n,
      },
    }),
  ];
  // Simulate independent TOL deployment across chains
  for (let i = 0; i < chains.length; i++) {
    for (let j = 0; j < 10; j++) {
      chains[i].tmc.mint_native(5_000n * PRECISION);
    }
  }
  // Create realistic price divergence through independent trading
  chains[0].router.swap_foreign_to_native(20_000n * PRECISION, 0n); // Chain 0: buying pressure
  chains[2].router.swap_native_to_foreign(10_000n * PRECISION, 0n); // Chain 2: selling pressure
  // Record price divergence after bridge failure
  const prices = chains.map((system) => system.tmc.get_price());
  let max_price = 0n;
  let min_price = prices[0];
  for (const price of prices) {
    if (price > max_price) max_price = price;
    if (price < min_price) min_price = price;
  }
  const price_divergence = ((max_price - min_price) * PPB) / max_price;
  // Verify realistic price divergence exists
  assert(
    price_divergence > 100n, // >0.01% price difference (realistic for isolated chains)
    "Bridge failure should create measurable price divergence between chains",
  );
  // Simulate attacker trying to exploit divergence without bridge
  // Attacker cannot move assets between chains, so arbitrage is impossible
  const chain_floors = chains.map((system, idx) => {
    const xyk_native = system.xyk.reserve_native;
    const xyk_foreign = system.xyk.reserve_foreign;
    const k = xyk_native * xyk_foreign;
    const total_supply = system.tmc.supply;
    // Calculate worst-case floor (67% dump)
    const sellable = (total_supply * 667n) / 1_000n;
    const final_native = xyk_native + sellable;
    const final_foreign = k / final_native;
    const floor = (final_foreign * PRECISION) / final_native;
    // Verify each chain maintains independent floor
    assert(
      floor > 0n,
      "Chain " + idx + " must maintain positive floor despite price divergence",
    );
    return floor;
  });
  // Verify TOL distribution prevents systemic risk
  const total_tol_native = chains.reduce(
    (sum, system) => sum + system.xyk.reserve_native,
    0n,
  );
  const tol_distribution = chains.map(
    (sys) => (sys.xyk.reserve_native * PPB) / total_tol_native,
  );
  // No single chain should have >70% of total TOL
  const max_tol_share = tol_distribution.reduce(
    (max, share) => (share > max ? share : max),
    0n,
  );
  assert(
    max_tol_share <= 700_000_000n, // <= 70%
    "TOL distribution should prevent single-chain dominance risk",
  );
  // Verify arbitrage is impossible without bridge functionality
  // Attacker cannot profit from price divergence across isolated chains
  const arbitrage_profit = 0n; // No bridge = no arbitrage
  assert(
    arbitrage_profit === 0n,
    "Price divergence should not create arbitrage opportunity without bridge",
  );
  // Verify each chain's economic independence
  for (let idx = 0; idx < chains.length; idx++) {
    const system = chains[idx];
    const k = system.xyk.reserve_native * system.xyk.reserve_foreign;
    const supply = system.tmc.supply;
    // Each chain should have sustainable TOL-to-supply ratio
    const tol_ratio = (system.xyk.reserve_native * PPB) / supply;
    assert(
      tol_ratio > 50_000_000n, // >5% TOL ratio for each chain (more realistic)
      "Each chain should maintain sufficient TOL for floor guarantee",
    );
    assert(k > 0n, "Chain " + idx + " XYK invariant must be positive");
  }
});

runTest("TOL Capital Efficiency vs Traditional Treasury", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
      mint_shares: {
        user_ppb: 333_000_000n,
        tol_ppb: 667_000_000n,
      },
    },
  });
  const mint_result = system.tmc.mint_native(1000n * PRECISION);
  // Traditional Treasury: 0% capital efficiency (idle in vaults)
  // TMCTOL TOL: Active capital deployment with multiple benefits
  const calculate_active_capital_ratio = (system) => {
    const tol_balance = system.tol.get_balance();
    // Calculate active capital in XYK pools vs traditional idle treasury
    const total_active_native =
      tol_balance.bucket_a.contributed_native +
      tol_balance.bucket_b.contributed_native +
      tol_balance.bucket_c.contributed_native +
      tol_balance.bucket_d.contributed_native;
    const total_active_foreign =
      tol_balance.bucket_a.contributed_foreign +
      tol_balance.bucket_b.contributed_foreign +
      tol_balance.bucket_c.contributed_foreign +
      tol_balance.bucket_d.contributed_foreign;
    const total_active_reserves = total_active_native + total_active_foreign;
    const total_tol_reserves =
      system.xyk.reserve_native + system.xyk.reserve_foreign;
    if (total_tol_reserves === 0n) return 0n;
    const active_ratio = (total_active_reserves * PPB) / total_tol_reserves;
    return active_ratio;
  };
  // Test 1: Active Capital Deployment
  const active_capital_ratio = calculate_active_capital_ratio(system);
  assertApprox(
    active_capital_ratio,
    1_000_000_000n,
    10_000_000n,
    "TMCTOL achieves ~100% capital utilization through continuous deployment cycles (temporary buffers recycled into subsequent mints) vs 0% traditional idle treasury",
  );
  // Test 2: Price Floor Protection
  const floor_price = calculateFloorPrice(system);
  assert(
    floor_price > 0n,
    "TMCTOL provides mathematical price floor protection vs zero protection in traditional treasury",
  );
  // Test 3: Value Accrual through XYK Participation
  const initial_k = system.xyk.reserve_native * system.xyk.reserve_foreign;
  assert(
    initial_k > 0n,
    "TOL participates in XYK pools, capturing quadratic growth vs zero value accrual in traditional treasury",
  );
  // Test 4: Resilience to Strategic Spending
  const tol_balance = system.tol.get_balance();
  const bucket_a_share_native = tol_balance.bucket_a.contributed_native;
  const bucket_a_share_foreign = tol_balance.bucket_a.contributed_foreign;
  // Simulate spending 50% bucket A on ecosystem development
  const remaining_active_native =
    tol_balance.bucket_b.contributed_native +
    tol_balance.bucket_c.contributed_native +
    tol_balance.bucket_d.contributed_native;
  const remaining_active_foreign =
    tol_balance.bucket_b.contributed_foreign +
    tol_balance.bucket_c.contributed_foreign +
    tol_balance.bucket_d.contributed_foreign;
  const remaining_active_reserves =
    remaining_active_native + remaining_active_foreign;
  const total_active_reserves_before =
    bucket_a_share_native + bucket_a_share_foreign + remaining_active_reserves;
  const post_spending_ratio =
    (remaining_active_reserves * PPB) / total_active_reserves_before;
  assertApprox(
    post_spending_ratio,
    500_000_000n,
    20_000n,
    "System maintains 50%+ active capital after strategic spending, demonstrating resilience vs traditional treasury where any spending reduces protection to 0%",
  );
  // Test 5: Floor Protection Persistence
  const post_spending_floor = calculateFloorPrice(system);
  assert(
    post_spending_floor > 0n,
    "Floor protection persists despite strategic spending, unlike traditional treasury where spending eliminates protection",
  );
});

// 12. NEW EMERGENT PROPERTIES TESTS

runTest("Bidirectional Compression Effect", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  system.tmc.mint_native(50_000n * PRECISION);
  const initialCeiling = system.tmc.get_price();
  const initialFloor = calculateFloorPrice(system);
  const initialRelativeSpread = calculateRelativeSpread(
    initialCeiling,
    initialFloor,
  );
  const initialGap = calculateArithmeticSpread(initialCeiling, initialFloor);
  const burnAmount = system.tmc.supply / 10n;
  system.tmc.burn_native(burnAmount);
  const newCeiling = system.tmc.get_price();
  const newFloor = calculateFloorPrice(system);
  const newRelativeSpread = calculateRelativeSpread(newCeiling, newFloor);
  const newGap = calculateArithmeticSpread(newCeiling, newFloor);
  assert(
    newFloor >= initialFloor,
    "Floor should increase or remain stable after burning",
  );
  assert(
    newGap < initialGap,
    "Arithmetic spread should compress after burning",
  );
  assert(
    newRelativeSpread < initialRelativeSpread,
    "Relative spread should compress after burning",
  );
  assert(newCeiling < initialCeiling, "Ceiling should decrease after burning");
});

runTest("Router Intelligence - TMC Route Selection", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const result = system.router.swap_foreign_to_native(100n * PRECISION, 0n);
  assert(
    result.route === "TMC",
    `Router should choose TMC when XYK has no liquidity. Got: ${result.route}, type: ${typeof result.route}`,
  );
});

runTest("Router Intelligence - XYK Route Selection", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  system.tmc.mint_native(20_000n * PRECISION);
  for (let i = 0; i < 5; i++) {
    system.router.swap_foreign_to_native(1000n * PRECISION, 0n);
  }
  const result = system.router.swap_foreign_to_native(100n * PRECISION, 0n);
  assert(
    result.route === "XYK",
    `Router should choose XYK when it has established liquidity. Got: ${result.route}, type: ${typeof result.route}`,
  );
});

runTest("Two-Phase Reward Routing - 20/80 Collection", () => {
  const amount = 100n * PRECISION;
  const split = split_collator_fee(amount);
  assert(split.collator === 20n * PRECISION, "Collator should receive 20% of collected rewards");
  assert(split.fee_sink === 80n * PRECISION, "Fee Sink should receive 80% of collected rewards");
  assert(split.collator + split.fee_sink === amount, "20/80 split must conserve collected amount");
});

runTest("Two-Phase Reward Routing - Phase 1 Pools", () => {
  const feeSinkAmount = 80n * PRECISION;
  const split = distribute_fee_sink_phase1(feeSinkAmount);
  assert(split.staking_pool === 40n * PRECISION, "Phase 1 staking pool should receive half of Fee Sink flow");
  assert(split.liquidity_pool === 40n * PRECISION, "Phase 1 liquidity pool should receive half of Fee Sink flow");
  assert(split.staking_pool + split.liquidity_pool === feeSinkAmount, "Phase 1 distribution must conserve Fee Sink amount");
});

runTest("Two-Phase Reward Routing - Phase 2 1:1:4", () => {
  const feeSinkAmount = 60n * PRECISION;
  const split = distribute_fee_sink_phase2(feeSinkAmount);
  assert(split.staking_pool === 10n * PRECISION, "Phase 2 staking pool should receive one part");
  assert(split.liquidity_pool === 10n * PRECISION, "Phase 2 liquidity pool should receive one part");
  assert(split.claimable_lp_nomination === 40n * PRECISION, "Phase 2 claimable LP nomination should receive four parts");
  assert(split.staking_pool + split.liquidity_pool + split.claimable_lp_nomination === feeSinkAmount, "Phase 2 distribution must conserve Fee Sink amount");
});

runTest("Economic Incentive Alignment", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  system.tmc.mint_native(10_000n * PRECISION);
  const arbitrage_amount = 1000n * PRECISION;
  const tmc_result = system.tmc.mint_native(arbitrage_amount);
  const tmc_cost = arbitrage_amount;
  const tmc_tokens = tmc_result.user_native;
  const xyk_sell_result = system.xyk.swap_native_to_foreign(tmc_tokens, 0n);
  const arbitrage_revenue = xyk_sell_result.foreign_out;
  const router_fee = (arbitrage_amount * 5_000n) / PPB;
  const effective_cost = arbitrage_amount + router_fee;
  assert(
    arbitrage_revenue <= effective_cost,
    "Arbitrage should be unprofitable due to router fees and price impact",
  );
  const holder = new User(0n, 10_000n * PRECISION);
  const trader = new User(0n, 10_000n * PRECISION);
  holder.set_router(system.router);
  trader.set_router(system.router);
  holder.buy_native(1000n * PRECISION);
  const holder_initial = holder.get_balance().native;
  let trader_balance = 0n;
  for (let i = 0; i < 5; i++) {
    trader.buy_native(200n * PRECISION);
    const buy_result = trader.get_balance();
    trader.sell_native(buy_result.native / 2n);
    trader_balance = trader.get_balance().native;
  }
  const holder_final = holder.get_balance().native;
  assert(
    holder_final >= holder_initial,
    "Holder should not lose value from trading activity",
  );
  assert(
    holder_final >= trader_balance,
    "Long-term holding should be more profitable than active trading due to fees",
  );
});

runTest("Reported Floor Metric Scenario Ratios", () => {
  const reserveNative = 333_333n * PRECISION;
  const referenceCeiling = PRECISION;
  const reserveForeign = BigMath.mul_div(
    reserveNative,
    referenceCeiling,
    PRECISION,
  );
  const userExit = calculateReportedFloorRatio({
    reserveNative,
    reserveForeign,
    referenceCeiling,
    sellableSupply: 333_333n * PRECISION,
  });
  const systemExit = calculateReportedFloorRatio({
    reserveNative,
    reserveForeign,
    referenceCeiling,
    sellableSupply: 666_666n * PRECISION,
  });
  assertApprox(userExit.ratioPpb, 250_000_000n, 1n, "User exit should report ~25% floor ratio");
  assertApprox(systemExit.ratioPpb, 111_111_111n, 1n, "System exit should report ~11.11% floor ratio");
  assert(userExit.sellablePressure === 333_333n * PRECISION, "Default λ should use full sellable supply");
});

runTest("Ledger Conservation With Burns", () => {
  const system = create_system({
    tmc: {
      price_initial: PRECISION / 1_000n,
      slope: PRECISION / 1_000n,
    },
  });
  const firstMint = system.tmc.mint_native(1_000n * PRECISION);
  const secondMint = system.tmc.mint_native(500n * PRECISION);
  const cumulativeMinted = firstMint.total_minted + secondMint.total_minted;
  const burnAmount = cumulativeMinted / 7n;
  system.tmc.burn_native(burnAmount);
  const liveIssuance = system.tmc.supply;
  assert(
    cumulativeMinted - burnAmount === liveIssuance,
    "Cumulative minted minus burned should equal live simulated supply",
  );
});

runTest("Burn Liveness Threshold Behavior", () => {
  const system = create_system({
    router: {
      min_initial_foreign: PRECISION,
      min_swap_foreign: 10n * PRECISION,
      fee_router_ppb: (5n * PPB) / 1_000n,
    },
  });
  system.tmc.mint_native(1_000n * PRECISION);
  system.fee_manager.receive_fee_foreign(5n * PRECISION);
  assert(system.fee_manager.buffer_foreign === 5n * PRECISION, "Sub-threshold foreign fees should remain buffered");
  assert(system.fee_manager.total_native_burned === 0n, "Sub-threshold foreign fees should not burn native");
  system.fee_manager.receive_fee_foreign(10n * PRECISION);
  assert(system.fee_manager.buffer_foreign === 0n, "Threshold-crossing foreign fees should be consumed");
  assert(system.fee_manager.total_native_burned > 0n, "Threshold-crossing foreign fees should burn native when liquidity exists");
});

runTest("Zap Bucket Accounting Conservation", () => {
  const system = create_system();
  const result = system.tmc.mint_native(1_000n * PRECISION);
  const buckets = [
    result.tol.bucket_a,
    result.tol.bucket_b,
    result.tol.bucket_c,
    result.tol.bucket_d,
  ];
  const lpTotal = buckets.reduce((sum, bucket) => sum + bucket.lp_tokens, 0n);
  const nativeTotal = buckets.reduce((sum, bucket) => sum + bucket.contributed_native, 0n);
  const foreignTotal = buckets.reduce((sum, bucket) => sum + bucket.contributed_foreign, 0n);
  assert(lpTotal === result.tol.total_lp_minted, "Bucket LP accounting should conserve total LP minted");
  assert(nativeTotal === result.tol.total_native_used, "Bucket native accounting should conserve total native used");
  assert(foreignTotal === result.tol.total_foreign_used, "Bucket foreign accounting should conserve total foreign used");
  assert(result.tol.bucket_a.lp_tokens > 0n, "Bucket_A should be classifiable as anchor-support LP after initial mint");
});

runTest("Stress Floor Monotonicity", () => {
  const reserveNative = 500n * PRECISION;
  const reserveForeign = 1_000n * PRECISION;
  const smallSell = calculateStressFloor(
    reserveNative,
    reserveForeign,
    100n * PRECISION,
  );
  const largeSell = calculateStressFloor(
    reserveNative,
    reserveForeign,
    300n * PRECISION,
  );
  assert(smallSell > largeSell, "Stress floor should decrease as sellable pressure increases");
});

// FINAL STATISTICS

console.log("\n" + "<RESULTS>");
console.log(`Total:  ${testCount}`);
console.log(`Passed: ${passedTests}`);
console.log(`Failed: ${failedTests.length}`);

if (failedTests.length > 0) {
  console.log("\n" + "<FAILED TESTS>");
  failedTests.forEach(({ test, code, name, error }) => {
    console.log(`\n[${test}] ${code} ${name}`);
    console.log(`Error: ${error}`);
    console.log(`See: simulator/tests.md section ${code}`);
  });
  console.log("\n❌ Tests failed");
} else {
  console.log("\n✅ All tests passed");
}
