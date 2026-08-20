---
type: status
title: Development Status
description: Current implementation status, roadmap context, and active backlog items for the DEOS framework, focused on shipped baseline, open boundaries, and future-gated work without treating the wiki as a release-note surface.
locale: en
canonical_page_id: status
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../BACKLOG.md
  - resource: ../../CHANGELOG.md
  - resource: ../../web-client/README.md
  - resource: ../../web-client/docs/architecture.en.md
status: stable
audience: newcomer
tags:
  - development
  - status
  - roadmap
related:
  - Three-Layer Validation
  - Reference Client
  - Generated Wiki
---

# Development Status

## Summary

DEOS is in framework-stabilization mode. The runtime, reference client, scripts, docs, and wiki are now being shaped into one coherent reference product rather than a sequence of visible refactor milestones.

This page is a current-state map. It is not the release history and not the full backlog.

## Stable baseline areas

The current framework baseline is best understood by domain:

- **Economic physics**: TMCTOL minting, routing, treasury-owned liquidity, actor-mediated fee burning, and bounded invariants form the core economic loop.
- **Autonomous actors**: Actors provides deterministic Actor Contract execution plus Mutable-only sparse Continuation for Temporary Step failure. Retries preserve committed prefixes on the canonical bounded scheduler without whole-contract rollback.
- **Staking and governance**: staking uses multi-asset share-vault mechanics, while governance uses bounded domain tracks, typed payloads, and protection surfaces.
- **Reference client**: the SvelteKit client exposes on-chain-first wallet, swap, staking, governance, wiki, chart/status, automation, and execution-feedback surfaces.
- **Tooling and validation**: scripts, benchmarks, metadata export, wiki trust checks, client validation, and context gates support local development and release readiness.

Use [Domain Map](../concepts/domain-map.en.md) when you need the conceptual topology instead of the status snapshot.

## Active focus

The current backlog concentrates on closing the contracted model with minimal release machinery:

- Pass the direct pull-request validation gate on the final release tree;
- Publish the runtime Wasm, metadata, descriptors, and five generated semantic/runtime evidence assets;
- Publish one checksum file for that eight-asset payload;
- Keep network assurance as an independently useful local operation rather than a release claim.

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
