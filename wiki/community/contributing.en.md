---
page_type: process
title: Contributing Guidelines
summary: How to contribute to DEOS by choosing the right domain, preserving physics-first constraints, and running the correct validation layer.
locale: en
canonical_page_id: contributing
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
status: active
audience: developer
tags:
  - community
  - contributing
  - workflow
related:
  - Domain Map
  - Three-Layer Validation
  - DEOS Framework Overview
  - Scripts Layer
last_compiled: 2026-05-17
confidence: 0.95
---

# Contributing Guidelines

## Summary

Contributing to DEOS means changing a domain without weakening the rest of the graph. A contribution may touch runtime code, economic formulas, client UX, wiki pages, scripts, or release metadata, but it should preserve the same core discipline: explicit contracts, deterministic economic behavior, honest read models, and validated boundaries.

## Before writing code

### 1. Find the domain

Use [Domain Map](../concepts/domain-map.en.md) first. Decide whether the change belongs to economic physics, autonomous actors, routing, governance, staking, read models, client UX, tooling, or future-gated work.

### 2. Respect physics-first design

DEOS prefers bounded protocol-managed reactions over manual intervention. Governance can steer and authorize changes, but survival-critical economic behavior should remain explicit, deterministic, and validated.

See [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md).

### 3. Keep forkability in mind

DEOS is a framework intended for downstream ecosystems. Upstream contributions should harden reusable framework surfaces: runtime reliability, read models, validation, tooling, docs/wiki clarity, and reference-client honesty. Product-specific business logic should normally stay downstream.

## Development workflow

### Pick the right validation layer

Use [Three-Layer Validation](../development/three-layer-validation.en.md):

1. simulation for economic formulas and invariants;
2. runtime behavior checks for pallets and integrations;
3. systemic validation for cross-domain behavior.

Client work should also run the web-client validation stack, and wiki work should run the trusted wiki validation gate.

### Preserve zero-warning hygiene

Runtime and client code should not accumulate warnings, debug leakage, unsafe type escapes, or stale terminology. If a shortcut becomes necessary, document why it is local and bounded.

### Keep knowledge synchronized

When behavior changes, update the right knowledge surface:

- Wiki pages for newcomer-facing explanation and cross-links;
- Backlog for open work;
- Changelog for completed delivery;
- Durable context when a reusable rule or pattern changes.

## Contribution boundaries

Good contributions usually:

- Close a concrete backlog item or a clear discovered slice;
- Improve validation or reduce ambiguity;
- Remove repeated UI, docs, or terminology drift;
- Strengthen runtime/client honesty;
- Keep future-gated work gated until the external condition exists.

Risky contributions usually:

- Add feature growth without an identified domain pressure;
- Move unbounded history into consensus state;
- Make indexers a silent dependency for canonical flows;
- Reintroduce manager/farmer wording for current System AAA actors;
- Turn wiki pages into release notes or duplicate docs.

## Related

- [Domain Map](../concepts/domain-map.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Scripts Layer](../usage/scripts-layer.en.md)
