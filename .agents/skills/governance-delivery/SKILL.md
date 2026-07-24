---
name: governance-delivery
description: Drives bounded DEOS governance changes across specification, pallet behavior, runtime wiring, generated evidence, client composition, and release handoff.
fmos: true
---

# Governance Delivery

Canonical work: governance items in `BACKLOG.md`, especially `Governance execution expansion policy` and any explicitly activated governance slice.

## Mission and Scope

Move one governance capability at a time from accepted contract through bounded runtime behavior and honest client projection. Preserve domain-scoped primary/protection tracks, typed payloads, explicit origins, bounded reward memory, and fail-closed protection semantics.

Exclude political decisions, concrete proposal advocacy, account actions, signing, deployment, and runtime-upgrade execution. Route upgrade operations through their existing owner.

## Truth Owners

- `docs/governance.specification.en.md` owns intended governance semantics.
- `docs/governance.architecture.en.md` owns shipped implementation mapping.
- `template/pallets/governance/` owns executable pallet behavior, tests, benchmarks, and weights.
- Runtime configuration and integration tests own DEOS payload/origin composition.
- `docs/read-model.contract.en.md` and the governance client slice own provenance and browser realization.
- `BACKLOG.md` owns remaining work; `CHANGELOG.md` owns completed outcomes.

## Delivery Loop

1. Identify the activated governance backlog item and its domain, payload kind, authority, bounds, and read-model class.
2. Reconcile specification before changing non-trivial behavior.
3. Implement the smallest pallet/runtime slice with focused tests and two-dimensional weight evidence when dispatch or bounded work changes.
4. Update metadata, client composition, and canonical/materialized projections only from shipped runtime truth.
5. Run changed-scope checks, escalate across pallet/runtime/client boundaries, then reconcile backlog, architecture, and changelog.

## Evidence and Gates

- Require explicit bounds for collections, windows, expiry, cleanup, and proposal execution.
- Require runtime integration evidence for origins, payload dispatch, protection-track denial, and transactional execution.
- Keep archive search and long ballot timelines materialized; do not grow consensus history.
- Treat signing, proposal submission, governance decisions, deployment, and publication as approval gates.

## Working Memory and Handoff

Report the selected governance item, changed truth surfaces, focused and systemic evidence, metadata/client consequences, and exact external gate. Keep no proposal diary or duplicate task list here.

## Evolution and Retirement

Refine this organ when a recurring governance delivery failure exposes a missing gate. Retire it only if governance delivery becomes fully deterministic tooling or leaves the repository scope.
