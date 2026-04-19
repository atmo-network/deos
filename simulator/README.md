# TMCTOL Simulator

Reference economic simulator for TMCTOL, implemented in JavaScript/BigInt.

This module is the **spec-side executable model** used to:

- Validate economic formulas before runtime implementation
- Explore parameter behavior (`price_initial`, `slope`, fees, mint shares)
- Run deterministic regression scenarios (`57` tests)

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

- `Total: 57`
- `Passed: 57`
- `Failed: 0`

---

## Core API

`model.js` exports:

- `PRECISION = 10^12`
- `PPB = 10^9`
- `create_system(config_override?)`
- Classes: `Tmc`, `Tol`, `Xyk`, `Router`, `FeeManager`, `User`, `BigMath`

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

## Model boundary

Simulator provides **mathematical/behavioral truth** for tokenomics.

It does **not** model:

- Substrate block weight accounting
- Origin/permission system from runtime
- Full pallet storage semantics

For runtime behavior and pallet wiring, see implementation docs in `docs/`.

---

## Related docs

- Main protocol spec: [`../docs/tmctol.specification.en.md`](../docs/tmctol.specification.en.md)
- Core runtime architecture: [`../docs/core.architecture.en.md`](../docs/core.architecture.en.md)
- TMC implementation architecture: [`../docs/tmc.architecture.en.md`](../docs/tmc.architecture.en.md)
- Test explanations: [`./tests.md`](./tests.md)
