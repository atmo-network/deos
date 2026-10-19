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

Actor creation admission and machine freeze are reusable bounded controls under the Actors specification §10.6, not a product mandate. The DEOS reference genesis MUST start with `UserCreationEnabled = false` and `GlobalCircuitBreaker = false`: declared System topology may operate while every new User identity remains denied until governance enables creation. Existing User service remains independent of that admission flag. A downstream host owns its own explicit genesis choices.

Protocol-domain `L2ParameterChange` MUST use `PrimaryEligibleSigned`: a signed proposer needs nonzero primary-track governance power, not protection-track power or an Actor identity. This authority covers the complete explicit L2 allowlist, including DEOS Router fee changes and both Actor flag setters. Submission grants only proposal initiation; ordinary witnessed-preimage admission, General capacity, author/maturity bounds, opening fees, voting, protection and enactment delay remain effective. User-creation admission and Actor machine freeze MUST NOT disable DEOS Governance.

The reference runtime MUST admit both Actor flag setters only through the existing protocol-domain `L2ParameterChange` validation/execution allowlist. Both validators MUST recognize the same exact bounded calls, reject other domains/calls and trailing bytes, preserve ordinary governance admission/payment, and emit typed surface receipts. Public pallet calls retain their privileged origin; governance enactment uses only explicitly delegated `DomainParameters` setter logic, never a Root dispatcher or arbitrary Actor authority. The thaw setter remains executable while Actors is frozen.

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

## 9. Repository Ownership Namespaces

`docs/README.md` owns neutral documentation-container navigation. DEOS cross-system contracts and reference composition live in `docs/deos/`, indexed by its own `README.md`. A downstream instance may add its own namespace and reference the base contracts without copying them or registering its identity in upstream configuration. DEOS documents MUST NOT depend on downstream namespaces.

A namespace is an ownership boundary, not an implicit overlay resolver. A fork MUST explicitly describe the applicability and differences of its contracts; directory placement alone neither replaces a DEOS invariant nor establishes compatibility. Instance additions SHOULD remain separate from upstream-owned files. Necessary base changes remain explicit patches that may require semantic reconciliation during upstream updates; this structure does not guarantee conflict-free merges.

Package specifications, implementation architecture and embedding guides remain inside their independently reusable packages. Workspace implementation maps remain with their workspaces. Namespace indexes link directly to those owners instead of relocating, mirroring or overriding them.

`simulators/` contains independently scoped economic models, not one mandatory simulation platform. `simulators/tmctol/` owns the shipped TMCTOL mathematics and tests. Each model MUST declare its assumptions, validation command and proof boundary; economic simulation MUST NOT claim runtime, scheduling, authority, storage, Weight or host guarantees outside that boundary.

Supported parameter changes SHOULD reuse an existing model's configuration and scenarios when sufficient. A distinct mechanism may own a separate model. Shared mathematical code requires demonstrated reuse rather than a speculative common engine. Upstream models MUST NOT depend on downstream model directories; adding a directory does not silently enroll its executable in upstream CI.

Path relocation changes navigation, not mathematical or runtime semantics and not historical evidence authority. Archived manifests, measurements and identity pins remain tied to their original sources. Current-path reconciliation or a new source identity MUST remain separate from those frozen artifacts.
