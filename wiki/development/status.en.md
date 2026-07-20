---
page_type: status
title: Development Status
summary: Current implementation status, roadmap context, and active backlog items for the DEOS framework, focused on shipped baseline, open boundaries, and future-gated work without treating the wiki as a release-note surface.
locale: en
canonical_page_id: status
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../BACKLOG.md
  - ../../CHANGELOG.md
  - ../../web-client/README.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: newcomer
tags:
  - development
  - status
  - roadmap
related:
  - Three-Layer Validation
  - Reference Client
  - Generated Wiki
last_compiled: 2026-07-20
confidence: 0.9
---

# Development Status

## Summary

DEOS is in framework-stabilization mode. The runtime, reference client, scripts, docs, and wiki are now being shaped into one coherent reference product rather than a sequence of visible refactor milestones.

This page is a current-state map. It is not the release history and not the full backlog.

## Stable baseline areas

The current framework baseline is best understood by domain:

- **Economic physics**: TMCTOL minting, routing, treasury-owned liquidity, actor-mediated fee burning, and bounded invariants form the core economic loop.
- **Autonomous actors**: AAA provides deterministic actor execution for burning, liquidity provisioning, treasury/bucket flows, and staking-related protocol automation.
- **Staking and governance**: staking uses multi-asset share-vault mechanics, while governance uses bounded domain tracks, typed payloads, and protection surfaces.
- **Reference client**: the SvelteKit client exposes on-chain-first wallet, swap, staking, governance, wiki, chart/status, automation, and execution-feedback surfaces.
- **Tooling and validation**: scripts, benchmarks, metadata export, wiki trust checks, client validation, and context gates support local development and release readiness.

Use [Domain Map](../concepts/domain-map.en.md) when you need the conceptual topology instead of the status snapshot.

## Active focus

The current backlog concentrates on explicit framework boundaries:

- Derive time-sensitive runtime constants from a configurable cadence profile;
- Define a non-enabled V3 scheduling and block-bundling readiness profile;
- Separate staking reward distribution from reward origin and define typed budget recipients;
- Make unclaimed-reward policy explicit;
- Preserve Phase 2 LP nomination as a runtime-upgrade boundary;
- Allow client structure to grow only when concrete product pressure exposes a named seam.

## Open boundaries

The important unfinished areas are intentionally gated:

- Wallet expansion waits for a materialized/indexed asset-discovery surface;
- Archive/search UX waits for a materialized provider contract;
- Permissionless collators and advanced randomness wait for an upstream relay-beacon path;
- Client composition and provider growth wait for concrete ownership pressure;
- Block-reward routing waits for a concrete subsidy source and amount policy.

## Where to look next

For active tasks, use the root backlog. For completed delivery history, use the root changelog. For how to validate a change, use [Three-Layer Validation](three-layer-validation.en.md).

## Related

- [Domain Map](../concepts/domain-map.en.md)
- [Three-Layer Validation](three-layer-validation.en.md)
- [Reference Client](../overview/reference-client.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
