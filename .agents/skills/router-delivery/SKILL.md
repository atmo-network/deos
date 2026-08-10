---
name: router-delivery
description: Drives DEOS Router route-truth work from specification through bounded preparation, atomic execution, generated evidence, and release handoff.
title: DEOS Router Delivery
status: draft
fss: true
---

# DEOS Router Delivery

Canonical open work: ../../../BACKLOG.md

## Mission and Scope

Guide the active Router route-truth closure without owning Router semantics or duplicating executable commands. Keep route discovery, protection, execution, publication, outcomes, and Weight classification on one bounded runtime-owned truth path.

Exclude arbitrary graph routing, external solvers, new market families, product policy, release publication, and any Actor scheduler expansion beyond adapter synchronization.

## Truth Owners

- `template/pallets/router/docs/specification.en.md` owns intended Router semantics and conformance.
- `template/pallets/router/src/lib.rs` and package tests own reusable executable behavior.
- `template/pallets/router/docs/architecture.en.md` owns shipped package implementation truth.
- `docs/core.architecture.en.md` and affected integration documents own concrete DEOS composition.
- `BACKLOG.md` owns unfinished Router work; `CHANGELOG.md` owns completed outcomes.

## Operating Protocol

- Start with the accepted specification and contract tests before implementation.
- Preserve bounded route families: direct XYK, direct TMC mint, and Native-anchored XYK.
- Keep external quotes as projections; prepare current executable truth inside the transaction.
- Keep fee routing, protection, Oracle publication, Actor ingress, market mutation, balance deltas, and Router events under one rollback boundary.
- Treat Router protection and System Actor reference policy as distinct owners.
- Reconcile the backlog after each validated slice and remove completed detail immediately.

## Knowledge Routing

- Read the specification for route families, intents, comparator, protection, publication, errors, and conformance.
- Read package architecture and `src/lib.rs` for current implementation topology.
- Read `template/runtime/src/configs/deos_router_config.rs` for concrete adapters.
- Read Actors and Oracle integration documents only when their boundaries change.
- Use the benchmarking capability when route classes or affected generated weights move.

## Evidence and Gates

- Focused package tests must falsify each changed route, protection, rollback, and outcome boundary.
- Runtime integration must cover concrete adapters, Oracle ordering, Actor ingress, fee routing, and transaction rollback.
- Every route family needs deterministic vectors and one measured Weight class before release.
- Metadata, descriptors, client projections, documentation, wiki, and generated evidence move only after the ABI stabilizes.
- Finish with workspace Clippy using warnings denied, the owning release gate, and the repository completion gate.

## Evolution and Apoptosis

Keep `status: draft` while Router 0.7.14 work or decisive evidence gates remain. Stabilize after the accepted contract ships and routine support no longer requires reconstruction. Remove this skill only if Router disappears, merges into another canonical feature owner, or host surfaces absorb every unique route, gate, and failure-guidance duty.
