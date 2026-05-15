# DEOS: Deterministic Economic Operating System

> A forkable parachain framework for launching sovereign token economies with protocol-owned liquidity and deterministic automation.

DEOS replaces unbounded political governance and vulnerable redemption models with explicit economic physics. By encoding liquidity accumulation and token distribution directly into the protocol, it turns unbounded downside narratives into calculable, condition-dependent risk surfaces.

DEOS is intentionally the foundation layer, not a finished product. It provides the execution substrate for partner teams to fork, customize, and launch their own dApps and living ecosystems.

**Why the name DEOS**:

- `Deterministic`: Protocol-managed economic reactions (minting, distributions, liquidity provision) are explicit and repeatable. They are executed by the runtime scheduler, not left to ad-hoc operator discretion.
- `Economic`: The managed domain is capital formation and allocation—not arbitrary general-purpose application logic.
- `Operating System`: The runtime kernel, AAA scheduler, routing, and governance act as domain-specific operating services for forked token economies.

_TMCTOL (Token Minting Curve with Treasury-Owned Liquidity) is the flagship economic standard currently running on top of DEOS._

---

## 1. Key Mechanics

### Unidirectional Minting (TMC)

The protocol issues new tokens along a linear price curve. There is no redemption path—minting is the only way to move along the curve. This strict unidirectional rule ensures the ceiling price increases deterministically with supply.

### Treasury-Owned Liquidity (TOL)

When tokens are minted, the protocol automatically allocates a fixed share (66.6% in TMCTOL) into treasury-owned liquidity. Counted floor support is reported through the canonical stress-floor metric in the TMCTOL specification, including reserve scope, bucket state, supply basis, sellable-pressure assumption, and governance state.

TOL liquidity is split across four buckets:

- `A — Anchor`: protected floor-support liquidity, 50% of TOL
- `B — Building`: governed LP unwind / BLDR buyback / ecosystem development, 16.6% of TOL
- `C — Capital`: gradual LP unwind for treasury operations, 16.6% of TOL
- `D — Dormant`: LP held until governance decides future policy, 16.6% of TOL

### Bidirectional Compression & The Ratchet Effect

A built-in fee is taken on trades and burned, reducing live supply when burn execution remains live. Under the named preconditions in the TMCTOL specification — counted reserves stay protected, sellable-pressure assumptions are explicit, and burn/Zap execution remains healthy — the mechanism can compress the corridor from both sides: lower live-supply ceiling pressure and stronger stress-floor support. The ratchet is therefore a condition-dependent protocol dynamic, not an unconditional price promise.

---

## 2. Deterministic Automation (AAA)

The `pallet-aaa` (Account Abstraction Actors) scheduler is DEOS's deterministic execution engine. Instead of relying on external keepers or centralized bots, DEOS orchestrates protocol operations from within the runtime.

- **Stability Contract**: Each actor pipeline has strict weight bounds, distinct fee accounting, and atomicity guarantees. Identical network state produces identical outcomes.
- **Event-Driven**: Actors can be triggered by scheduled timers or on-chain events (e.g., specific asset ingress).
- **Execution Plans**: Complex economic flows are codified into predictable pipelines. DEOS currently ships with plans for **Minting** (distributing users/TOL shares), **Zapping** (converting raw emissions to LP), and **Burning** (selling accrued fees).

---

## 3. Project Topology

- [`/docs`](./docs/) — Core architectural contracts, specifications, and architecture notes.
- [`/template`](./template/) — The Polkadot Omni Node-ready Rust runtime workspace (`pallets/`, `runtime/`, `primitives/`).
- [`/web-client`](./web-client/) — The SvelteKit browser reference UI and transaction flows.
- [`/scripts`](./scripts/) — Operator and developer automation.
- [`/simulator`](./simulator/) — The mathematical proving ground for validating core tokenomics.
- [`/wiki`](./wiki/index.en.md) — Generated documentation optimized for frontend and newcomer onboarding.

---

## 4. Getting Started

DEOS provides a unified local bootstrap script that automates the network environment: downloading the Polkadot SDK binaries (including the Omni Node), building the reference runtime, generating the chain spec, and spinning up a local Zombienet test network.

**Prerequisites**: [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/).

Open **Terminal 1** for the network:

```bash
# Bootstrap the local network
# (Downloads binaries, builds runtime, starts Omni Node via Zombienet, and seeds state)
./scripts/bootstrap-local-network.sh
```

Open **Terminal 2** for the web client:

```bash
# Install dependencies and start the UI
cd web-client
npm install
npm run dev
```

_(Note: When altering tokenomics or invariants, validate the math via `node ./simulator/tests.js` before touching the runtime)_

---

## 5. Documentation Index

**Entrypoints**

- [Complete Docs Index](./docs/README.md) · [Generated Wiki](./wiki/index.en.md) · [Agent Protocols](./AGENTS.md)
- [Backlog](./BACKLOG.md) · [Changelog](./CHANGELOG.md)

**Specifications**

- [Manifesto](./docs/manifesto.en.md) — Why physics over politics
- [TMCTOL Spec](./docs/tmctol.specification.en.md) — Foundation, math, invariants
- [Governance Spec](./docs/governance.specification.en.md) — Dual-track bounded governance
- [AAA Spec](./docs/aaa.specification.en.md) — Deterministic actor automation

**Architecture Notes**

- [Core Architecture](./docs/core.architecture.en.md)
- [TMC Architecture](./docs/tmc.architecture.en.md)
- [Axial Router Architecture](./docs/axial-router.architecture.en.md)
- [Asset Registry Architecture](./docs/asset-registry.architecture.en.md)
- [AAA Internals](./docs/aaa.architecture.en.md)
