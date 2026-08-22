---
type: overview
title: Typed Observations
description: Typed observations provide bounded current scalar truth while producers retain samples, Actors owns reactions, DEOS Router owns routing, and indexed providers own history.
locale: en
canonical_page_id: typed-observations
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/oracle/docs/specification.en.md
  - resource: ../../template/pallets/oracle/docs/architecture.en.md
  - resource: ../../docs/oracle.integration.en.md
status: stable
audience: newcomer
tags:
  - overview
  - oracle
  - observations
related:
  - DEOS Router
  - Actors System
  - Token-Driven Automation
  - Read-Model Split
---

# Typed Observations

## Summary

Typed observations are the domain contract for bounded current scalar truth. DEOS Oracle is the current bounded owner. A feed fixes its producer, semantic meaning, scale, aggregation rule, zero policy, freshness contract, and provenance when governance registers it.

The subsystem does not own raw sample history, routing decisions, actor execution, or unbounded analytics. Those responsibilities remain with producers, DEOS Router, Actors, and indexed providers.

## Current Truth Contract

Each admitted feed exposes bounded current state:

- immutable feed semantics;
- one current scalar value when initialized;
- the block of the latest accepted publication;
- a change-only revision;
- explicit freshness and availability;
- bounded producer and feed indexes.

Equal aggregate output refreshes `updated_at` without increasing the revision or invoking the change hook. Semantic changes require a new feed identity rather than mutation of an existing feed.

## Directional Pool Observations

The DEOS reference runtime registers forward and reverse pool observations as distinct feeds. Canonical pool admission creates both directions transactionally, and DEOS Router publishes the direction it executes before direct execution.

A direction is never inferred from its reverse. The feed records pre-execution reserves with Router provenance; it does not claim a universal fair price, manipulation immunity, or complete market history.

## Reactive Actors Boundary

A changed revision invokes one atomic Actors transition-ingress hook carrying its exact previous and current scalar values. Broad `ObservationChange` remains latest-state reconsideration: it coalesces dirty state and later traverses exact occupied subscriber pages. Sparse `ObservationCrossing` instead retains revision-ordered transition obligations and visits only occupied thresholds crossed by that transition. If Actors cannot retain the required obligation, Oracle publication rolls back with it.

Both paths converge on the existing Actors pending latch, queue, wakeup, and scheduler; DEOS Oracle never executes subscribers synchronously. Predicates still own attempt-time conditions, while Crossing owns declarative fire/rearm hysteresis and cannot fire twice before a qualifying rearm.

## Read-Model Boundary

Current feed configuration, status, value, revision, and update block are canonical bounded chain truth. Historical samples, long timelines, search, and analytics belong to indexed or materialized providers.

The reference client reads finalized current state and labels provenance. It must not reconstruct history from a session cache or present provider data as direct runtime truth.

## Related

- [DEOS Router](router.en.md)
- [Actors System](actor-system.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Read-Model Split](../concepts/read-model-split.en.md)
