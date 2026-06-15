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
- AAA task language, scheduling, admission, lifecycle, task-scoped atomicity, and observability.
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
- AAA execution plans and System AAA topologies.
- Tactical-domain tokens such as a builder token in the reference line.

The framework may ship reference defaults and examples. Those defaults help a fork start safely, but they do not morally bind every downstream economy.

## 5. Builder Pattern Boundary

The builder pattern is an optional governance-mediated labor-funding primitive.

Generic mechanism:

1. Useful completed work is represented as a public invoice payload.
2. Governance evaluates the invoice under explicit domain rules.
3. Governance may approve, reject, reduce, or amplify payout.
4. Execution pays from a declared funding source under bounded caps and transactional rules.

Framework responsibility:

- Define safe invoice payloads, lifecycle, execution, observability, caps, and failure semantics.
- Keep invoice voting bounded and auditable.
- Avoid hardcoding one instance's contributor culture into generic pallets.

Instance responsibility:

- Decide whether builder invoices are used at all.
- Define accepted work norms, individual/team invoice expectations, social review culture, and payout appetite.
- Decide whether contributor funding uses a builder token, a treasury asset, another domain token, or no invoice system.

## 6. Bucketed Capital Boundary

TMCTOL's reference buckets are a canonical standard-level topology, not a requirement that every DEOS fork must preserve identical names or percentages.

Reusable pattern:

- Protocol capital is segmented into explicit roles.
- Each segment has a clear policy boundary.
- Governance authority over each segment is bounded and named.
- Automation moves capital only through declared execution plans.

Reference TMCTOL semantics:

- Bucket A: anchor / protection / floor-support gravity.
- Bucket B: building / contributor-development funding.
- Bucket C: capital / operations.
- Bucket D: dormant reserve or future demand/expansion policy.

A fork may rename, resize, merge, remove, or add buckets if it updates the claimed standard and validation surface honestly.

## 7. AAA Extraction Rule

When repeated treasury, vault, drip, buyback, burn, zap, liquidity, or distribution flows appear, prefer lifting the common behavior into bounded configurable AAA execution plans rather than multiplying bespoke pallets.

This does not mean AAA should become a universal workflow engine. AAA remains a deterministic economic actor kernel with bounded tasks, explicit runtime adapters, and predictable weight/fee behavior.

## 8. Primary Risk

The main framework-boundary risk is responsibility confusion:

- Treating optional reference policy as mandatory framework law.
- Treating instance-specific economic or cultural choices as DEOS guarantees.
- Smuggling downstream product logic into pallets, docs, or client surfaces.
- Over-generalizing from imaginary future economies instead of extracted repeated patterns.

The remedy is explicit classification: mechanism vs policy, framework vs instance, contract vs reference topology.
