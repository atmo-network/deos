---
name: oracle-delivery
description: Drives typed observation delivery from standalone bounded pallet through Router extraction, AAA reaction ingress, generated evidence, and release handoff.
type: FSS Skill
title: Typed Observation Delivery
status: draft
fss: true
---

# Typed Observation Delivery

Canonical open work: ../../../BACKLOG.md

## Mission and Scope

Deliver the DEOS typed-observation vertical without merging truth ownership across the oracle, producers, AAA, Router, or indexed history. This skill owns delivery sequencing and evidence interpretation, not runtime semantics, task state, or shared validation implementations.

## Truth Owners

- `BACKLOG.md` owns remaining 0.7.7 observation work and dependency order.
- `template/pallets/oracle/docs/specification.en.md` owns standalone oracle semantics.
- `template/pallets/aaa/docs/specification.en.md` owns reactive AAA semantics.
- `template/pallets/router/docs/architecture.en.md` owns current Router behavior.
- Code and tests own executable behavior; architecture documents map only shipped integration.
- `CHANGELOG.md` receives outcomes only after the complete 0.7.7 release gate.

## Operating Protocol

- Start with the earliest open 0.7.7 dependency and refuse shadow state introduced for later compatibility.
- Keep `pallet-oracle` independently reusable: generic typed identities, bounded admission, O(1) current truth, no AAA/Router/DEOS topology dependency, no history, network, or off-chain correctness.
- Require immutable feed meaning, producer, provenance, scale, aggregation, zero policy, and EMA parameters; semantic change creates a new feed.
- Preserve Router arithmetic with exact regression vectors before deleting Router-owned EMA storage.
- Keep oracle change notification O(1) and subscriber-independent; AAA alone owns bounded subscriptions, dirty coalescing, fanout, and execution.
- Update canonical work immediately when evidence closes, narrows, splits, or gates a slice.

## Knowledge Routing

- Load the oracle specification for package types, state, transitions, read semantics, and hook atomicity.
- Load Router architecture and its implementation only for extraction/parity work.
- Load AAA specification, architecture, embedding guide, and `aaa-delivery` public contract only when the slice crosses into reactive AAA.
- Load read-model and control-plane contracts only when exposing observation data beyond runtime consumers.
- Use the benchmarking capability for generated RefTime/ProofSize evidence; do not duplicate benchmark commands here.

## Evidence and Gates

- Package foundation requires default, no-std, runtime-benchmark, independent embedding, unit, metadata/storage-contract, try-state, and generated-weight evidence.
- Every state collection and producer index needs an explicit bound and maximum-density test.
- Revision tests must distinguish accepted sample, changed published value, equal published value, refresh-only update, overflow, and transactional hook failure.
- Extraction cannot delete Router storage before exact EMA vectors and failed-swap rollback prove parity.
- Reactive delivery cannot iterate subscribers in producer context or add another scheduler.
- Release handoff requires the repository validation matrix, artifact equality, context synchronization, and no unresolved correctness blocker.

## Evolution and Retirement

Refine this skill only when observation delivery exposes a recurring route, gate, or failure mode not owned by canonical docs or shared capabilities. Remove mutable implementation detail after promotion. Mature or retire the delivery emphasis once 0.7.7 ships and the remaining support loop no longer requires cross-slice reconstruction.
