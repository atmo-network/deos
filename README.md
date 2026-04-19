# DEOS: Deterministic Economic Operating System

> A forkable parachain framework for launching sovereign token economies with protocol-owned liquidity and deterministic automation.

Uncontrolled downside risk in conventional token economies comes from two sources: redeemability and political governance. DEOS eliminates redemption and codifies the rules of liquidity into the protocol itself. System survival does not depend on governance cycles — it depends on unchangeable "physics" parameters. Governance still exists, but it directs growth and execution within rigid boundaries. See the [Manifesto](./docs/manifesto.en.md) for the full argument.

This repository serves as a Specification & Reference Framework for a self-sustaining economic model where liquidity accumulation transforms unlimited downside risk into calculable bounded risk. It combines a rigorous [economic specification](./docs/tmctol.specification.en.md) (the TMCTOL standard) with a forkable reference stack: living contracts and architecture in [`/docs`](./docs/), a generated newcomer-facing knowledge layer in [`/wiki`](./wiki/index.en.md), a modern Polkadot Omni Node-oriented runtime kernel in [`/template`](./template/), a browser-facing reference client in [`/web-client`](./web-client/), and operator/developer automation in [`/scripts`](./scripts/).

DEOS is intentionally the foundation layer, not the finished product ecosystem. Its full product value appears downstream when a team forks the framework, commits to a concrete product philosophy, and ships the dApps, user flows, and operating culture that turn the protocol substrate into a living ecosystem.

`Why the name DEOS`:

- `Deterministic`: the framework tries to make protocol-managed economic reactions explicit and repeatable. Given the same on-chain state, typed payloads, and token flows, the runtime should take the same bounded economic transition. This does not claim that the whole market becomes predictable; it claims that the protocol's economic kernel is not left to ad hoc operator discretion.
- `Economic`: the managed domain is capital formation and allocation — minting curves, routing, treasury-owned liquidity, fee burning, staking, and governance-conditioned capital flows — rather than arbitrary general-purpose application logic.
- `Operating`: DEOS is meant as the execution substrate for those economic processes. The runtime kernel, AAA scheduler, routing, governance, staking, and read-model contracts together act like domain-specific operating services for forked ecosystems.
- `System`: the repository is not one isolated tokenomic gadget. It is a coordinated stack of kernel, automation, governance, client, and operator tooling on top of which concrete standards such as TMCTOL can run.

This is why the repository identity stays `DEOS`, while `TMCTOL` remains the current flagship economic standard running on top of it.

---

## 1. Key Mechanics

### Unidirectional Minting Curve (TMC)

The protocol issues new tokens along a linear price curve ($P = P_0 + slope \cdot s$). There is no redemption path — minting is the only way to move along the curve. The ceiling price increases deterministically with supply.

→ [TMC Architecture](./docs/tmc.architecture.en.md) · [`pallet-tmc`](./template/pallets/tmc/) · [Runtime config](./template/runtime/src/configs/tmc_config.rs)

### Treasury-Owned Liquidity (TOL)

When tokens are minted, the protocol automatically allocates a fixed share into XYK liquidity pools. This creates a hyperbolic price floor that cannot reach zero. TOL liquidity is split across four [buckets](./docs/aaa.architecture.en.md) with different roles:

| Bucket       | Role                                           | Share |
| ------------ | ---------------------------------------------- | ----- |
| A — Anchor   | Permanent pool liquidity, never withdrawn      | 50%   |
| B — Building | LP unwind → BLDR buyback for ecosystem funding | 16.6% |
| C — Capital  | Gradual LP unwind for treasury operations      | 16.6% |
| D — Dormant  | LP held until governance decides future policy | 16.6% |

This enables governance to trade off floor hardness and expansion while preserving a mathematical minimum.

→ [TMCTOL Specification](./docs/tmctol.specification.en.md) · [Governance Specification](./docs/governance.specification.en.md) · [Ecosystem constants](./template/primitives/src/ecosystem.rs)

### Distribution Split

Every mint splits **33.3%** to users and **66.6%** into TOL. This invariant ensures that liquidity grows alongside supply and prevents separate treasury emission paths.

→ [Allocation constants](./template/primitives/src/ecosystem.rs) (`TMC_USER_ALLOCATION`, `TMC_ZAP_ALLOCATION`)

### Supply Compression

A fee is taken on trades and burned, reducing the supply over time. Burning lowers the minting ceiling and raises the pool-based floor, compressing the price range upward — a bidirectional squeeze.

→ [Axial Router Architecture](./docs/axial-router.architecture.en.md) · [`pallet-axial-router`](./template/pallets/axial-router/) · [Router config](./template/runtime/src/configs/axial_router_config.rs)

### Governance Dependencies

The mathematical floor is only guaranteed if TOL remains in the pools, the 66.6%/33.3% distribution is maintained, and burning continues. Withdrawing reserves or altering allocations weakens the floor.

---

## 2. AAA Runtime

The [`pallet-aaa`](./template/pallets/aaa/) (Account Abstraction Actors) [specification](./docs/aaa.specification.en.md) defines the deterministic execution engine used to orchestrate protocol operations within the DEOS runtime. In this repository, DEOS currently ships with TMCTOL as its flagship economic standard. AAA combines deterministic scheduling, bounded execution, explicit lifecycle and fee rules, and event-driven triggers. Asset ingress can function like a trigger-message, and many workflow changes can be expressed by reconfiguring actor graphs instead of rewriting the runtime.

### Stability Contract

- Identical network state produces identical results
- Each runtime path has explicit maximum bounds
- Actors are destroyed in place without automatic refunds

### Task Model

Actors execute a static list of steps on a schedule or trigger. Steps support tasks such as token transfer, split transfer (multi-recipient), swap, adding/removing liquidity, mint, burn and noop. Each task is atomic, and pipelines gracefully handle errors via policies (abort cycle or continue to next step).

→ [Pipeline execution](./template/pallets/aaa/src/execution.rs) · [Task types](./template/pallets/aaa/src/types.rs)

### Triggers

Execution can be driven by timers (with optional probability), address events (incoming transfers) or manual triggers. In the DEOS model, matched asset ingress can act like a trigger-message that wakes the next bounded reaction, but it does so inside a stricter contract of deterministic scheduling, schedule windows, cooldown periods, and bounded admission. Address-event trigger ingress is wired by runtime producers through a dedicated adapter path (`AddressEventIngress` → `notify_address_event*`) with bounded scanner admission and overflow carry-over queueing for event-heavy blocks.

→ [Scheduler](./template/pallets/aaa/src/scheduler.rs)

### Fee and Weight Accounting

Each step charges evaluation and execution fees; actors reserve fees at cycle start and spend only what is available. There is no recurring rent — only non-refundable creation fees.

### Observability

Events are emitted for creation, funding, pausing/resuming, cycle execution and task completion, providing transparency for indexers and dashboards.

→ [AAA Architecture](./docs/aaa.architecture.en.md)

---

## 3. Integration

The shipped DEOS reference configuration currently relies on TMCTOL's automated minting, burning, liquidity provision and swapping to uphold its risk boundaries. The AAA runtime provides deterministic execution plans for these operations:

- **Mint execution plan**: distributes minted tokens to users and TOL, adds TOL liquidity to the pools
- **Burn execution plan**: periodically sells accrued fees and removes the corresponding supply
- **Zap execution plan**: converts raw emission into LP tokens and distributes across TOL buckets
- **Bucket execution plans**: execute per-bucket policy (hold, unwind, buyback)

By encoding these flows into actors, DEOS ensures that protocol physics execute deterministically, while governance uses predefined levers (bucket parameters, fee rates) to steer growth. In the current reference line, those physics are instantiated by the TMCTOL standard. This ties together the [manifesto's philosophy](./docs/manifesto.en.md), the [standard's math](./docs/tmctol.specification.en.md), and the [runtime's execution](./docs/aaa.architecture.en.md).

→ [Core Architecture](./docs/core.architecture.en.md) · [Execution-plan builders](./template/runtime/src/configs/aaa_config.rs) · [Runtime adapters](./template/pallets/aaa/src/adapters.rs)

---

## 4. Project Topology

Current maintenance and framework-stabilization attention now centers on:

1. [`/docs`](./docs/) — living contracts, architecture notes, and the primary conceptual control plane
2. [`/template`](./template/) — the runtime kernel and reusable parachain implementation
3. [`/web-client`](./web-client/) — the browser-facing reference client for live TMCTOL flows
4. [`/scripts`](./scripts/) — operator/developer automation around local validation, bootstrap, and probes

Supporting reference surfaces:

- [`/simulator`](./simulator/) — historical proving ground and still-authoritative mathematical reference when tokenomics, formulas, thresholds, or invariants change
- [`/template`](./template/) — Rust / Substrate workspace with [`pallets/`](./template/pallets/), [`runtime/`](./template/runtime/), [`primitives/`](./template/primitives/), and [`research/`](./template/research/)
- [`/docs`](./docs/) — Documentation Hub for specs, architecture notes, and navigation across the reference stack
- [`/wiki`](./wiki/index.en.md) — generated onboarding and navigation layer derived from `/docs`, optimized for newcomer orientation and frontend rendering
- [`/web-client`](./web-client/) — TypeScript / SvelteKit workspace for browser-facing UI and visualizations
- [`/scripts`](./scripts/) — atomic shell entrypoints, orchestrators, and local admin utilities

---

## 5. Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Node.js](https://nodejs.org/) (for the repository-local web client)
- [Polkadot Omni Node](https://github.com/paritytech/polkadot-sdk) (current repo line: Polkadot SDK `2603` / node `1.22.0`)

### Development Setup

1. Start with the documentation hub
   Read [`./docs/README.md`](./docs/README.md) before touching code so the current contracts and architecture notes frame the task correctly:

2. Validate the economics when tokenomics or invariants change
   Ensure the mathematical guarantees still hold:

```bash
node ./simulator/tests.js
```

3. Build the parachain runtime
   Compile the Rust implementation:

```bash
cd template

# Build the runtime workspace
cargo build --release --workspace

# Run implementation tests
cargo test --workspace
```

4. Run the node
   Start a local development chain:

```bash
polkadot-omni-node --dev --tmp
```

5. Run the web client when you need the browser-facing reference surface

```bash
cd web-client
npm install
npm run dev
```

---

## 6. Documentation

- 📖 [Complete Documentation Index](./docs/README.md) — Start here for technical guides.
- 🧭 [Generated Wiki Index](./wiki/index.en.md) — Newcomer-facing navigation layer derived from `/docs`.
- 🤖 [Agent Conventions](./AGENTS.md) — Development protocols and project context.
- 🗂️ [Canonical Backlog](./BACKLOG.md) — Open work, gated items, and next slices.
- 📝 [Delivery History](./CHANGELOG.md) — Completed work rotated out of the backlog.
- 🛠️ [Scripts Layer Map](./scripts/README.md) — Atomic local ops, orchestrators, and admin utilities.
- 🦀 [Template Workspace README](./template/README.md) — Rust reference-implementation workspace entrypoint.
- 🌐 [Web Client README](./web-client/README.md) — SvelteKit frontend workspace.
- 🧪 [Simulator README](./simulator/README.md) — Math-side executable reference for tokenomic validation.

### Specification

- [Manifesto](./docs/manifesto.en.md) — Why physics over politics
- [TMCTOL Specification](./docs/tmctol.specification.en.md) — Framework foundation, math, invariants
- [DEOS Governance Specification](./docs/governance.specification.en.md) — DEOS's bounded dual-track alternative to OpenGov for protocol and tactical domains

### Architecture

- [Core Architecture](./docs/core.architecture.en.md) — Token-Driven Economic Automaton
- [TMC Architecture](./docs/tmc.architecture.en.md) — Dynamic supply issuance
- [Axial Router Architecture](./docs/axial-router.architecture.en.md) — Multi-AMM trading infrastructure
- [Asset Registry Architecture](./docs/asset-registry.architecture.en.md) — Digital Twin pattern for foreign assets
- [Randomness Strategy](./docs/randomness.strategy.en.md) — Post-VRF simplification note and relay-beacon-first future direction

### AAA (Account Abstraction Actors)

- [AAA Specification](./docs/aaa.specification.en.md) — Stability contract, task model, triggers
- [AAA Architecture](./docs/aaa.architecture.en.md) — Pallet internals: scheduler, execution, adapters
- [TMCTOL System AAA Topology](./docs/aaa.architecture.en.md#current-tmctol-system-aaa-topology-on-deos) — System AAA flows and governance operations
