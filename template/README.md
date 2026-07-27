# DEOS Reference Implementation Workspace

This directory is the Rust/FRAME implementation workspace for DEOS.
It contains the reusable parachain runtime components that realize the living contracts described in `../docs/` and, when tokenomics or invariants change, can be cross-checked against the mathematical reference in `../simulator/`. The current flagship economic standard shipped on this runtime kernel is TMCTOL.

## Purpose

`/template` is the runtime kernel of the repository's DEOS reference stack:

- `../docs/` = living contracts and architecture notes
- `/template` = production-oriented runtime implementation
- `../simulator/` = authoritative mathematical reference when tokenomics, formulas, thresholds, or invariants change

The workspace is intentionally runtime-centric.
It does **not** carry a custom node crate in this repository snapshot; deployment is expected through the Polkadot SDK Omni Node flow.

## Workspace Layout

### `runtime/`

The assembled parachain runtime.
It wires pallets together through `Runtime-as-Config` adapters and hosts:

- Pallet configuration in `runtime/src/configs/`
- Runtime tests in `runtime/src/tests/`
- Runtime weight bridges in `runtime/src/weights/`
- Benchmark registry in `runtime/src/benchmarks.rs`

### `pallets/`

Custom DEOS runtime pallets in the current reference configuration:

- [`aaa/`](./pallets/aaa/) — deterministic actor runtime with bounded scheduling, triggers, lifecycle, static execution plans, and sparse progress-preserving Continuation
- [`asset-registry/`](./pallets/asset-registry/) — XCM location to asset-id registry
- [`router/`](./pallets/router/) — Axial Router routing and fee/burn execution gateway
- [`governance/`](./pallets/governance/) — bounded governance reward-memory and proposal lifecycle
- [`oracle/`](./pallets/oracle/README.md) — reusable bounded typed scalar observation and revision truth
- [`staking/`](./pallets/staking/) — share-vault staking, liquid `stXXX` receipts, locked native LP nomination, governance custody, and reward settlement
- [`tmc/`](./pallets/tmc/) — token minting curve logic

See [`pallets/README.md`](./pallets/README.md) for the pallet index.

### [`pallets/aaa/embedding-runtime/`](./pallets/aaa/embedding-runtime/README.md)

External-consumer Cargo fixture owned by the `pallet-deos-aaa` package boundary. It proves the portable host contract in default, DEX, try-runtime, and no-std profiles while starting with zero System actors and using no DEOS helper or topology dependencies.

### `primitives/`

Shared types and constants used across the runtime:

- Asset taxonomy and `AssetKind`
- Ecosystem constants
- Pallet ids and parameter defaults

### `research/`

Repository-local experimental/runtime-adjacent research artifacts when needed.
These are not part of the production runtime contract.

## Current Architecture Notes

The most relevant implementation docs live in `../docs/`:

- [`core.architecture.en.md`](../docs/core.architecture.en.md)
- [`aaa/docs/architecture.en.md`](./pallets/aaa/docs/architecture.en.md)
- [`oracle/docs/specification.en.md`](./pallets/oracle/docs/specification.en.md)
- [`oracle/docs/architecture.en.md`](./pallets/oracle/docs/architecture.en.md)
- [`tmc/docs/architecture.en.md`](./pallets/tmc/docs/architecture.en.md)
- [`router/docs/architecture.en.md`](./pallets/router/docs/architecture.en.md)
- [`asset-registry/docs/architecture.en.md`](./pallets/asset-registry/docs/architecture.en.md)
- [`staking/docs/architecture.en.md`](./pallets/staking/docs/architecture.en.md)
- [`governance/docs/architecture.en.md`](./pallets/governance/docs/architecture.en.md)

For contract/spec-level docs, start at [`../docs/README.md`](../docs/README.md).

## Common Commands

Run from this directory unless noted otherwise.

### Build and test

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

### Targeted runtime checks

```bash
cargo test -p deos-runtime
cargo test -p deos-runtime aaa_integration_tests
cargo test -p deos-runtime staking_integration_tests
cargo check -p deos-runtime --features runtime-benchmarks
```

### Targeted pallet checks

```bash
cargo test -p pallet-deos-aaa
cargo test -p pallet-deos-governance
cargo test -p pallet-deos-staking
```

### Benchmarks

Benchmark orchestration lives at the repository level:

```bash
cd ..
./scripts/benchmarks.sh pallet_governance
./scripts/benchmarks.sh pallet_staking
```

## Implementation Conventions

This workspace follows the repository rules from `../AGENTS.md`, especially:

- Polkadot SDK 2606 patterns
- FRAME v2 pallets and benchmark style
- Runtime weight bridges in `runtime/src/weights/`
- Bounded execution/storage surfaces
- `Runtime-as-Config` instead of hardcoded ecosystem logic inside generic pallets

## What this workspace is not

- Not the living contract/architecture control plane
- Not the mathematical protocol reference surface for tokenomic changes
- Not the deployment node binary layer
- Not the frontend workspace
- Not the canonical backlog or delivery log

Those live respectively in:

- `../docs/`
- `../simulator/`
- External Omni Node flow
- `../web-client/`
- `../BACKLOG.md` and `../CHANGELOG.md`
