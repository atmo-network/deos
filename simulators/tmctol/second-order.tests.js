/**
 * Second-order TMCTOL (`$BLDR`) test suite.
 * Run: node ./simulators/tmctol/second-order.tests.js
 * Fixed deterministic cases only; assumptions are explicit and none is a market guarantee.
 * @module second-order.tests
 */

import { PPB, PRECISION, create_system } from "./model.js";
import {
  anchor_support,
  bridge_round,
  composite_floor,
  create_builder_system,
  stress_floor,
} from "./second-order.js";

let total = 0;
const failures = [];
const test = (name, fn) => {
  total++;
  try {
    fn();
    console.log(`[${String(total).padStart(2)}] ${name.padEnd(58)} ✅`);
  } catch (error) {
    console.log(`[${String(total).padStart(2)}] ${name.padEnd(58)} ❌`);
    failures.push({ name, error: error instanceof Error ? error.message : String(error) });
  }
};
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};
const near = (actual, expected, tolerance, message) =>
  assert(
    (actual > expected ? actual - expected : expected - actual) <= tolerance,
    `${message}: ${actual} vs ${expected}`,
  );

/** Parent first-order economy with live Bucket B, plus a builder contour seeded through its own TMC. */
function economy(override = {}) {
  const parent = create_system(override);
  parent.router.swap_foreign_to_native(20_000n * PRECISION);
  const builder = create_builder_system(override);
  builder.router.swap_foreign_to_native(1_000n * PRECISION);
  return { parent, builder };
}
const NO_FEE = { router: { fee_router_ppb: 0n } };
const bucket_b = (/** @type {ReturnType<typeof create_system>} */ parent) => parent.tol.buckets.get("b");
const net_issuance = (/** @type {{ minted: bigint, burn: bigint }} */ leg) => leg.minted - leg.burn;

test("Mint split conserves issuance and halves the protocol share", () => {
  const builder = create_builder_system();
  const result = builder.tmc.mint_native(5_000n * PRECISION);
  const treasury = builder.tol.treasury_native;
  const anchor_issuance = result.tol_native - treasury;
  assert(treasury === result.tol_native / 2n, "direct treasury is floor(T / 2)");
  assert(result.user_native + anchor_issuance + treasury === result.total_minted, "U + anchor + treasury = M");
  const tolerance = result.total_minted / PPB + 3n;
  near(result.user_native * 3n, result.total_minted, tolerance, "recipient is one third");
  near(anchor_issuance * 3n, result.total_minted, tolerance, "anchor lane is one third");
  near(treasury * 3n, result.total_minted, tolerance, "treasury is one third");
});

test("All NTVE collateral is anchor-directed", () => {
  const builder = create_builder_system();
  let paid = 0n;
  for (const amount of [1_000n, 700n, 2_500n]) {
    builder.tmc.mint_native(amount * PRECISION);
    paid += amount * PRECISION;
  }
  assert(builder.xyk.reserve_foreign + builder.tol.buffer_foreign === paid, "collateral sits in the pool or the anchor lane buffer only");
});

test("Anchor support counts only the live LP reserve claim", () => {
  const { builder } = economy();
  const support = anchor_support(builder);
  assert(support.lp === builder.xyk.supply_lp, "the sealed Anchor owns every protocol-created LP");
  assert(support.bldr <= builder.xyk.reserve_native && support.ntve <= builder.xyk.reserve_foreign, "claim stays within reserves");
  builder.tol.buffer_native += 999n * PRECISION;
  builder.tol.buffer_foreign += 999n * PRECISION;
  const padded = anchor_support(builder);
  assert(padded.bldr === support.bldr && padded.ntve === support.ntve, "unmatched lane balances add no support");
});

test("Contours share the one-third recipient share but not collateral custody", () => {
  const parent = create_system();
  const builder = create_builder_system();
  const first = parent.tmc.mint_native(5_000n * PRECISION);
  const second = builder.tmc.mint_native(5_000n * PRECISION);
  near(first.user_native * 3n, first.total_minted, first.total_minted / PPB + 3n, "first-order recipient share");
  near(second.user_native * 3n, second.total_minted, second.total_minted / PPB + 3n, "second-order recipient share");
  const lp = (/** @type {string} */ key) => parent.tol.buckets.get(key).lp_tokens;
  const parent_lp = ["a", "b", "c", "d"].reduce((sum, key) => sum + lp(key), 0n);
  near(lp("a") * 2n, parent_lp, 4n, "first-order Bucket A holds half of the LP");
  assert(builder.tol.anchor_lp() === builder.xyk.supply_lp, "BLDR Anchor holds all of the LP");
});

test("Bridge consumes every released reserve asset", () => {
  const { parent, builder } = economy();
  const { released, ntve_leg, foreign_leg } = bridge_round(parent, builder, bucket_b(parent).lp_tokens / 4n);
  assert(ntve_leg.ntve_routed + ntve_leg.router_fee === released.native_out, "NTVE reserve = routed + fee");
  assert(foreign_leg.parent.foreign_in === released.foreign_out, "foreign reserve enters the parent route whole");
  assert(foreign_leg.ntve_routed + foreign_leg.router_fee === foreign_leg.parent.ntve_out, "parent proceeds = routed + fee");
});

test("Each leg splits acquisition exactly into burn and recycled treasury", () => {
  const { parent, builder } = economy();
  const { ntve_leg, foreign_leg } = bridge_round(parent, builder, bucket_b(parent).lp_tokens / 4n);
  for (const leg of [ntve_leg, foreign_leg]) {
    assert(leg.burn === leg.recipient_out / 2n, "burn is floor(Q / 2)");
    assert(leg.burn + leg.recycled_treasury === leg.recipient_out, "burn + recycled treasury = Q");
  }
});

test("Parent reserves leave the pool pro rata and Bucket B falls exactly", () => {
  const { parent, builder } = economy();
  const bucket = bucket_b(parent);
  const before = { lp: bucket.lp_tokens, native: parent.xyk.reserve_native, foreign: parent.xyk.reserve_foreign, supply: parent.xyk.supply_lp };
  const release = before.lp / 5n;
  const { released } = bridge_round(parent, builder, release);
  assert(bucket.lp_tokens === before.lp - release, "bucket LP falls by the release");
  assert(released.native_out === (before.native * release) / before.supply, "native leaves pro rata");
  assert(released.foreign_out === (before.foreign * release) / before.supply, "foreign leaves pro rata");
});

test("A release above the Bucket B balance fails before any change", () => {
  const { parent, builder } = economy();
  const state = () => [parent.xyk.reserve_native, parent.xyk.supply_lp, bucket_b(parent).lp_tokens, builder.tmc.supply].join(":");
  const before = state();
  let rejected = false;
  try {
    bridge_round(parent, builder, bucket_b(parent).lp_tokens + 1n);
  } catch {
    rejected = true;
  }
  assert(rejected, "over-release is rejected");
  assert(state() === before, "state is unchanged");
});

test("A TMC leg changes supply by issuance minus burn", () => {
  const { parent, builder } = economy(NO_FEE);
  const before = builder.tmc.supply;
  const { ntve_leg, foreign_leg } = bridge_round(parent, builder, bucket_b(parent).lp_tokens / 50n);
  // The recipient gets only one third of a TMC mint, so the pool wins the first leg; its price impact hands the second leg to the TMC.
  assert(ntve_leg.route === "XYK" && foreign_leg.route === "TMC", "routes are chosen by recipient output, leg by leg");
  assert(foreign_leg.minted > 0n && foreign_leg.direct_treasury > 0n, "a TMC leg issues and funds the direct treasury");
  const tolerance = foreign_leg.minted / PPB + 3n;
  near(foreign_leg.recipient_out * 3n, foreign_leg.minted, tolerance, "recipient share of the issuance is attributable");
  near(foreign_leg.direct_treasury * 3n, foreign_leg.minted, tolerance, "direct treasury share of the issuance is attributable");
  assert(builder.tmc.supply - before === net_issuance(ntve_leg) + net_issuance(foreign_leg), "supply delta = sum of leg (issuance - burn)");
});

test("A market leg changes supply by the burn alone", () => {
  const { parent, builder } = economy(NO_FEE);
  // Selling BLDR into the pool pushes its price below the curve, so the market becomes the best route.
  builder.router.swap_native_to_foreign(builder.xyk.reserve_native / 4n);
  const before = builder.tmc.supply;
  const { ntve_leg, foreign_leg } = bridge_round(parent, builder, bucket_b(parent).lp_tokens / 50n);
  assert(ntve_leg.route === "XYK", "below-curve pool price selects the market route");
  assert(ntve_leg.minted === 0n && ntve_leg.direct_treasury === 0n, "a market leg issues nothing");
  assert(net_issuance(ntve_leg) === -ntve_leg.burn, "its net issuance is -burn");
  assert(builder.tmc.supply - before === net_issuance(ntve_leg) + net_issuance(foreign_leg), "supply delta = sum of leg (issuance - burn)");
});

test("BLDR supply equals the sum of its holders across routes and fees", () => {
  const { parent, builder } = economy();
  const seeded = builder.tmc.supply - builder.tol.treasury_native - builder.tol.buffer_native - builder.xyk.reserve_native;
  const external = seeded - builder.fee_manager.buffer_native;
  const buy = builder.router.swap_foreign_to_native(300n * PRECISION);
  bridge_round(parent, builder, bucket_b(parent).lp_tokens / 3n);
  const holders =
    external + buy.native_out + builder.tol.treasury_native + builder.xyk.reserve_native + builder.tol.buffer_native + builder.fee_manager.buffer_native;
  assert(builder.tmc.supply === holders, "no BLDR appears or disappears outside mint and burn");
});

test("Stress floor falls monotonically with pressure and never exceeds spot", () => {
  const { builder } = economy();
  const { reserve_native, reserve_foreign } = builder.xyk;
  const spot = builder.xyk.get_price();
  let previous = stress_floor(reserve_native, reserve_foreign, 0n);
  near(previous, spot, spot / 1_000n, "zero pressure reports spot");
  for (const pressure of [1n, 10n, 100n, 1_000n]) {
    const floor = stress_floor(reserve_native, reserve_foreign, pressure * PRECISION);
    assert(floor < previous && floor <= spot, "floor decreases below spot");
    previous = floor;
  }
});

test("Correlated first-order stress and invoice payout lower the composite floor", () => {
  const { parent, builder } = economy();
  const payout = builder.tol.treasury_native;
  const calm = composite_floor(parent, builder);
  const invoice = composite_floor(parent, builder, { bldr_pressure: payout });
  const correlated = composite_floor(parent, builder, { ntve_pressure: parent.tmc.supply / 3n, bldr_pressure: payout });
  assert(invoice.bldr_in_parent < calm.bldr_in_parent, "invoice-payout pressure lowers the floor");
  assert(correlated.bldr_in_parent < invoice.bldr_in_parent, "first-order stress lowers it further");
  assert(correlated.bldr_in_parent > 0n, "the stressed floor stays positive");
});

test("Identical sequences reproduce identical state", () => {
  const run = () => {
    const { parent, builder } = economy();
    bridge_round(parent, builder, bucket_b(parent).lp_tokens / 4n);
    return [builder.tmc.supply, builder.tol.treasury_native, builder.xyk.reserve_native, builder.xyk.reserve_foreign, builder.tol.anchor_lp()].join(":");
  };
  assert(run() === run(), "BigInt arithmetic is deterministic");
});

console.log("\n<RESULTS>");
console.log(`Total:  ${total}`);
console.log(`Passed: ${total - failures.length}`);
console.log(`Failed: ${failures.length}`);
for (const { name, error } of failures) console.log(`\n[${name}]\nError: ${error}`);
if (failures.length) process.exitCode = 1;
else console.log("\n✅ All second-order tests passed");
