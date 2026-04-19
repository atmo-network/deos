# DEOS Documentation Hub

> `Comprehensive Knowledge Base` | From Mathematical Specifications to Production Deployment.

This directory serves as the central navigation hub for the DEOS ecosystem. It unifies the theoretical economic specifications (like the TMCTOL standard) with the concrete technical architecture of the Polkadot SDK Parachain implementation.

Throughout these docs, `DEOS` names the deterministic economic operating substrate, while `TMCTOL` names the current flagship tokenomic standard running on top of that substrate. The root [`README.md`](../README.md) carries the naming rationale for the acronym itself.

These docs are also the current conceptual control plane for day-to-day maintenance: start here before touching runtime, frontend, or tooling surfaces. The simulator remains the authoritative executable math reference when tokenomics, formulas, thresholds, or invariants change, but it is no longer the default entrypoint for unrelated work.

These docs describe the protocol and its reference implementation, not a finished end-user product. DEOS's product layer is expected to emerge in downstream forks that pair this framework with a concrete ecosystem thesis, product philosophy, and real dApps.

## Documentation Structure

### 1. Essential Foundation

`START HERE`: Before contributing code or designing features, understanding the underlying framework patterns is mandatory.

- `[Polkadot SDK 2603 Best Practices](./polkadot-sdk-2603.insights.en.md)`
  _! REQUIRED READING !_
  Modern architecture patterns for the Polkadot SDK 2603 standard. Covers unified dependency management, `frame::v2` macros, `Omni Node` utilization, the newer session-key ownership/runtime-API flow, and the `Runtime-as-Config` pattern.

### 2. Philosophy & Vision

The strategic context defining "Why" the system exists.

- `The Fractal-Cybernetic Manifesto`
  Defines the "Real DAO" philosophy: a transition from Subjective Policy (Politics) to Objective Mechanism (Cybernetics). Outlines the separation of `L1 Strategy` (Mathematical Sovereignty) and `L2 Tactics` (Fractal Federation).
  - [English](./manifesto.en.md) | [Russian](./manifesto.ru.md)

### 3. Core Specifications & Contracts

Normative source-of-truth documents defining subsystem concepts, rationale, invariants, and public contracts. These docs are not post-hoc code notes; implementation and tests are expected to follow them.

#### TMCTOL Standard

The flagship economic standard combining minting curves with automatic liquidity generation on top of DEOS.

- [English](./tmctol.specification.en.md) | [Russian](./tmctol.specification.ru.md)

#### Runtime / Product Contracts

- `AAA Specification`
  Deterministic Account Abstraction Actors contract. Defines the actor model, scheduler semantics, execution-plan/task rules, event-driven trigger semantics, circuit breakers, lifecycle, and safety invariants, including balance-ingress triggers and the reconfigurable actor-graph behavior surface as part of a broader bounded execution contract.
  - [English](./aaa.specification.en.md) | [Russian](./aaa.specification.ru.md)

- `[Staking Specification](./staking.specification.en.md)`
  Multi-asset share-vault staking contract: sovereign backing channels, share-based ownership, receipt direction, native-special-case rules, and the dual-inflow reward contract.

- `[DEOS Governance Specification](./governance.specification.en.md)`
  DEOS's bounded dual-track alternative to OpenGov for the current TMCTOL standard: domains, cadence, primary/protection hierarchy, typed payload kinds, invoice voting, bounded observability, and runtime-upgrade authority.

- `[Read-Model Contract](./read-model.contract.en.md)`
  Project-wide data-surface rule. Defines the split between bounded authoritative on-chain projections and externally indexed/materialized views.

### 4. Architecture & Shipped Implementation Maps

Implementation-specific documents describing how the current runtime realizes the contracts above.

- `[AAA Architecture](./aaa.architecture.en.md)`
  Code-anchored implementation map of `pallet-aaa`: scheduler queues, admission/fee gates, lifecycle transitions, adapter boundaries, balance-ingress event wiring, the current TMCTOL reference topology for System AAA, runtime bindings, telemetry surface, and release-mode scheduler performance baseline.

- `[Core Architecture](./core.architecture.en.md)`
  _! SYSTEM BACKBONE !_
  The token-driven design foundation. Covers system accounts structure, "Omnivorous" balance monitoring, Bitmask Asset Taxonomy, separation of Abstract Actors from Concrete Pallets, and the operational token lifecycle checkpoint runbook.

- `[Axial Router Architecture](./axial-router.architecture.en.md)`
  The economic coordination actor. Details mechanism-over-policy design, EMA oracle, fee burning flows, and integration with Asset Conversion.

- `[Token Minting Curve Architecture](./tmc.architecture.en.md)`
  The unidirectional minting engine. Covers the current runtime realization of the linear bonding curve, integral-based minting, read surfaces, and conservation invariants.

- `[Asset Registry Architecture](./asset-registry.architecture.en.md)`
  The foreign asset gateway. Documents the Hybrid Registry pattern: deterministic hashing at registration, persistent Location→AssetId mapping, and XCM version migration.

- `[Randomness Strategy](./randomness.strategy.en.md)`
  Secondary operational note for the current launch line. Documents the retirement of the local `pallet-vrf` line, the trusted team-operated collator posture plus previous-block-hash fallback used for the first mainnet, and the gate for any future relay-beacon replacement: only a new parachain-consumable per-block protocol beacon qualifies.

- `[Staking Architecture](./staking.architecture.en.md)`
  Code-anchored implementation map of `pallet-staking`: deterministic pool/reward account derivation, receipt lifecycle, legacy `Positions -> stXXX` bridge, native binding surface, sparse reward snapshot ingress, same-asset auto-compound claims, runtime bindings, and the current operational watchpoints.

- `[Governance Architecture](./governance.architecture.en.md)`
  Code-anchored implementation map of `pallet-governance`: bounded winning-vote memory, resolution-once dedup, active proposal lifecycle, weighted vote policy wiring, auto-finalization buckets, recent finalized-outcome retention, and the current policy/watchpoint surface.

### 5. Roadmap

- `[Roadmap / BACKLOG](../BACKLOG.md)`
  Canonical open backlog for infrastructure hardening, testing expansion, and upcoming framework evolution tasks.

### 6. Local Tooling

- `[Template Workspace README](../template/README.md)`
  Repository-local Rust runtime-kernel workspace entrypoint for `runtime/`, `pallets/`, `primitives/`, and benchmark/test commands.

- `[Simulator README](../simulator/README.md)`
  Repository-local executable math reference for tokenomic validation, formulas, thresholds, and invariants.

- `[Web Client README](../web-client/README.md)`
  Repository-local SvelteKit workspace for the browser-facing reference client.

- `[Generated Wiki Index](../wiki/index.en.md)`
  Repo-local onboarding and navigation layer derived from `/docs`, also consumed by the browser-facing wiki surface.

- `[Scripts Layer Map](../scripts/README.md)`
  Canonical map of atomic scripts, named orchestrators, and local admin utilities.

- `[Web Client Architecture](./web-client.architecture.en.md)`
  UI vocabulary, widget-vs-layout boundary, DRY/KISS presentation rules, and the current client refactor direction.
