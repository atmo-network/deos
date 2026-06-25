# TMCTOL Simulator

Historical and still-authoritative economic hypothesis simulator for TMCTOL, implemented in JavaScript/BigInt.

This module is the **spec-side mathematical proving ground** used to:

- Test tokenomic hypotheses before they become runtime contracts
- Validate economic formulas before or alongside runtime implementation
- Explore parameter behavior (`price_initial`, `slope`, fees, mint shares)
- Exercise deterministic regression scenarios (`66` tests)

---

## What is inside

- `model.js` — core model (TMC, TOL, XYK, Router, FeeManager)
- `tests.js` — executable test suite
- `tests.md` — verbose mirror/explanations for test sections
- `types.d.ts` — simulator type shapes

---

## Quick start

From repo root:

```bash
node ./simulator/tests.js
```

Expected output ends with:

- `Total: 66`
- `Passed: 66`
- `Failed: 0`

---

## Core API

`model.js` exports:

- `PRECISION = 10^12`
- `PPB = 10^9`
- `create_system(config_override?)`
- Classes: `Tmc`, `Tol`, `Xyk`, `Router`, `FeeManager`, `User`, `BigMath`
- Reward routing helpers: `split_collator_fee`, `distribute_fee_sink_phase1`, `distribute_fee_sink_phase2`

Minimal example:

```js
import { create_system, PRECISION } from "./model.js";

const system = create_system({
  tmc: {
    price_initial: PRECISION / 1_000n,
    slope: PRECISION / 1_000_000n,
    mint_shares: { user_ppb: 333_333_333n, tol_ppb: 666_666_667n },
  },
});

const buy = system.router.swap_foreign_to_native(100n * PRECISION);
console.log(buy.route, buy.native_out?.toString());
```

---

## Unit and scaling conventions

- Token balances and prices use fixed-point `PRECISION` (`10^12`)
- Fractions/ratios use `PPB` (`10^9`) and `_ppb` naming
- Arithmetic is done in `BigInt` only

This mirrors the project rule: spec model prioritizes mathematical clarity and precision before runtime constraints.

---

## Role and proof boundary

The simulator exists to test TMCTOL economic hypotheses and keep the math executable. Treat it as the mathematical reference capsule for the standard, not as a shadow implementation of the DEOS runtime.

It is authoritative for:

- TMC formula behavior and integral mint calculations
- TOL accumulation, floor scenarios, and compression-threshold taxonomy
- Deterministic tokenomic scenarios and conservation checks
- Parameter exploration before a mechanism is promoted into runtime logic

It is **not** authoritative for:

- Substrate block weight accounting or proof size
- Origin, permission, dispatch, and pallet storage semantics
- AAA scheduling, actor lifecycle, or runtime adapter behavior
- Governance execution, XCM, collator/session logic, or frontend flows
- Market/MEV guarantees beyond the explicit economic assumptions modeled in the tests

For runtime behavior and pallet wiring, see implementation docs in `docs/` and tests under `template/`.

---

## Related docs

- Main protocol spec: [`../docs/tmctol.specification.en.md`](../docs/tmctol.specification.en.md)
- Core runtime architecture: [`../docs/core.architecture.en.md`](../docs/core.architecture.en.md)
- TMC implementation architecture: [`../docs/tmc.architecture.en.md`](../docs/tmc.architecture.en.md)
- Test explanations: [`./tests.md`](./tests.md)
