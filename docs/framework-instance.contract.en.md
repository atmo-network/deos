# DEOS Framework / Instance Contract

- **Component**: DEOS framework boundary
- **Status**: Durable contract
- **Audience**: framework contributors, downstream fork authors, reference-client maintainers, partner evaluators

This document defines the boundary between reusable DEOS framework responsibility and downstream instance policy. It prevents one reference economy's choices from becoming accidental framework law while preserving the reusable mechanisms that make DEOS forkable.

## 1. Core Rule

DEOS provides reusable economic mechanisms and safety contracts.

A downstream instance chooses its concrete moral, business, product, launch, and labor policy.

Framework work SHOULD therefore ask two questions before hardening any behavior:

1. Is this a reusable mechanism, invariant, safety boundary, configuration seam, or validation rule?
2. Or is this one instance's brand, product strategy, launch policy, contributor culture, treasury appetite, or demand-generation plan?

Only the first class belongs in the framework by default. The second class belongs in a downstream instance unless repeated implementations reveal a reusable contract.

## 2. Framework-Owned Surfaces

DEOS owns the portable substrate:

- Runtime primitives and pallet contracts.
- Economic invariants and bounded execution rules.
- Protocol-owned-liquidity mechanisms and token-flow safety.
- Actors task language, scheduling, admission, lifecycle, task-scoped atomicity, and observability.
- Governance and protection mechanisms as configurable bounded primitives.
- Read-model provenance rules separating canonical-chain projections from materialized/indexed views.
- Configuration seams that let instances choose parameters without rewriting core logic.
- Validation gates and reference patterns that protect forkability.

A framework feature is healthy when it clarifies or strengthens these reusable surfaces.

## 3. Instance-Owned Surfaces

A DEOS instance owns its concrete economy:

- Brand, token names, public narrative, launch route, and jurisdictional posture.
- dApps, product loops, user acquisition, and demand strategy.
- Founder allocation or no-founder-allocation policy.
- Treasury culture, contributor norms, invoice etiquette, and reward appetite.
- Concrete bucket names, percentages, activation rules, and spend priorities.
- Which governance domains are public, private, advisory, tactical, or protected.
- Which reference mechanisms are enabled, renamed, replaced, or disabled.

A downstream instance may diverge from the reference line without violating DEOS, provided it does not claim guarantees that its chosen configuration no longer preserves.

## 4. Mechanism, Not Mandate

The following DEOS surfaces are mechanisms, not mandatory policies for every fork:

- Builder invoices.
- Bucketed capital flows.
- Protocol-owned liquidity.
- Governance protection / veto surfaces.
- Actor Contracts and System Actor topologies.
- Tactical-domain tokens such as a builder token in the reference line.

The framework may ship reference defaults and examples. Those defaults help a fork start safely, but they do not morally bind every downstream economy.

## 5. Builder Pattern Boundary

The builder pattern is an optional governance-mediated labor-funding primitive. [`builder-economy.contract.en.md`](./builder-economy.contract.en.md) owns the complete DEOS reference composition across second-order TMCTOL, `$BLDR` governance, invoices, BLDR Anchor, BLDR Treasury, and the parent-capital bridge.

DEOS owns safe reusable invoice, governance, treasury, liquidity, and execution mechanisms. An instance decides whether to enable the pattern and owns work norms, invoice etiquette, contributor culture, payout appetite, naming, and demand strategy.

## 6. TOL Capital Boundary

TOL is an asset-scoped treasury-owned liquidity topology, not a synonym for one fixed bucket count. The TMCTOL specification owns the project-independent standard mathematics and reference first-order model; the Builder Economy contract owns the concrete second-order `$BLDR` specialization.

Reusable framework requirements remain limited to explicit capital roles, bounded governance authority, protected custody, honest floor accounting, and declared execution paths. A fork may rename, resize, merge, remove, or add anchors, buckets, and treasuries if it updates its claimed contract and validation surface honestly.

## 7. Actors Extraction Rule

When repeated treasury, vault, drip, buyback, burn, zap, liquidity, or distribution flows appear, prefer lifting the common behavior into bounded configurable Actor Contracts rather than multiplying bespoke pallets.

This does not mean Actors should become a universal workflow engine. Actors remains a deterministic economic actor kernel with bounded tasks, explicit runtime adapters, and predictable weight/fee behavior.

## 8. Primary Risk

The main framework-boundary risk is responsibility confusion:

- Treating optional reference policy as mandatory framework law.
- Treating instance-specific economic or cultural choices as DEOS guarantees.
- Smuggling downstream product logic into pallets, docs, or client surfaces.
- Over-generalizing from imaginary future economies instead of extracted repeated patterns.

The remedy is explicit classification: mechanism vs policy, framework vs instance, contract vs reference topology.
