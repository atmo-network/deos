# DEOS

> A forkable economic runtime where issuance, liquidity, routing, governance, and automation operate as one bounded cybernetic system.

DEOS is a Polkadot SDK framework for building protocol economies. It moves recurring economic coordination out of multisigs, bots, and operator convention into explicit runtime mechanisms that can be inspected, tested, configured, and forked.

**TMCTOL** is the flagship standard implemented on DEOS. DEOS provides the reusable substrate; TMCTOL defines one concrete economy running on it.

[Start in 60 seconds](./wiki/getting-started/deos-in-60-seconds.en.md) | [Evaluate DEOS](./wiki/getting-started/partner-pitch.en.md) | [Read the specification](./docs/tmctol.specification.en.md) | [Run locally](#run-locally)

---

## One Economic Circuit

- **TMC — deterministic issuance.** A unidirectional curve defines how assets enter circulation without promising protocol redemption.
- **TOL — owned liquidity.** Mint output can accumulate into protected and governed liquidity lanes under an explicit bucket policy.
- **Axial Router — max-output execution.** A bounded set of XYK, TMC, and Native-anchored routes compete by expected recipient output.
- **AAA — autonomous operations.** Typed execution plans drive the Burn Actor, Liquidity Actor, splitters, buckets, and treasuries within storage and block-weight bounds.
- **Governance — constrained change.** Domain-scoped primary/protection tracks execute typed payloads instead of exposing unrestricted administration.

DEOS makes deterministic execution claims, not deterministic market-outcome claims. Liquidity support, burn dynamics, oracle guards, and governance safety hold only under the preconditions defined in the specifications.

---

## Framework, Not Product Policy

DEOS owns reusable mechanisms: runtime primitives, invariants, adapters, bounded read models, validation gates, and reference implementations.

A downstream ecosystem owns its identity, dApps, launch policy, labor model, treasury choices, token names, and concrete economic parameters.

Read the [Framework / Instance Contract](./docs/framework-instance.contract.en.md) before turning the reference topology into a product.

---

## Run Locally

### Requirements

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/)
- Linux environment suitable for Polkadot SDK tooling

### Start the network

```bash
./scripts/bootstrap-local-network.sh
```

This builds the runtime, prepares the chain specification, downloads the required operator binaries, and starts a local Zombienet network through Omni Node.

### Start the reference client

```bash
cd web-client
npm install
npm run dev
```

Optional demo-state seeding, from the repository root:

```bash
./scripts/seed-web-client-state.sh
```

See [`scripts/README.md`](./scripts/README.md) for individual operator workflows and [`template/README.md`](./template/README.md) for Rust workspace commands.

---

## Repository

| Path | Role | Entrypoint |
| --- | --- | --- |
| `docs/` | Specifications and shipped architecture | [`docs/README.md`](./docs/README.md) |
| `template/` | Runtime, pallets, primitives, weights, tests | [`template/README.md`](./template/README.md) |
| `web-client/` | Browser reference client | [`web-client/README.md`](./web-client/README.md) |
| `simulator/` | Deterministic TMCTOL mathematical reference | [`simulator/README.md`](./simulator/README.md) |
| `scripts/` | Bootstrap, validation, benchmarks, administration | [`scripts/README.md`](./scripts/README.md) |
| `wiki/` | Guided newcomer and frontend knowledge layer | [`wiki/index.en.md`](./wiki/index.en.md) |

### Core contracts

- [TMCTOL Specification](./docs/tmctol.specification.en.md)
- [AAA Specification](./docs/aaa.specification.en.md)
- [AAA External Runtime Embedding Guide](./template/pallets/aaa/EMBEDDING.md)
- [AAA Control-Plane Contract](./docs/aaa-control-plane.contract.en.md)
- [Governance Specification](./docs/governance.specification.en.md)
- [Read-Model Contract](./docs/read-model.contract.en.md)
- [Core Architecture](./docs/core.architecture.en.md)

---

## Validate

Default changed-scope completion gate:

```bash
./.agents/skills/alignment/scripts/completion-gate.sh
```

Select narrower or escalated routes through the [Project Skill Graph](./.agents/skills/README.md) and [`alignment` route matrix](./.agents/skills/alignment/SKILL.md). Run the broad local audit only when that complete surface is intended:

```bash
./scripts/validate-local.sh --audit-only
```

When tokenomics, formulas, thresholds, or invariants change:

```bash
node ./simulator/tests.js
```

---

## Project State

- [`BACKLOG.md`](./BACKLOG.md) — Open work and external gates
- [`CHANGELOG.md`](./CHANGELOG.md) — Completed delivery history
- [`AGENTS.md`](./AGENTS.md) — Durable architecture and engineering protocol

DEOS is infrastructure for economies whose rules must remain visible. It does not make markets predictable; it makes the protocol's own behavior explicit.
