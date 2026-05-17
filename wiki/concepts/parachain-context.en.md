---
page_type: concept
title: Parachain Context
summary: How DEOS relates to Polkadot, parachains, XCM, collators, Omni Node, and upstream relay-chain dependencies.
locale: en
canonical_page_id: parachain-context
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/core.architecture.en.md
  - ../../docs/polkadot-sdk-2603.insights.en.md
  - ../../template/README.md
status: active
audience: newcomer
tags:
  - concept
  - polkadot
  - parachain
  - xcm
  - collators
related:
  - DEOS Framework Overview
  - Runtime Patterns
  - Asset Identity
  - Tech Stack
  - Randomness Strategy
last_compiled: 2026-05-17
confidence: 0.84
---

# Parachain Context

## Summary

DEOS is a parachain-oriented framework. It does not try to be a standalone blockchain stack with a custom node in this repository. The reference runtime is built for Omni Node deployment, Polkadot SDK conventions, local assets, XCM asset identity, and a controlled collator phase.

The short model: DEOS owns economic logic; Polkadot supplies the broader parachain execution environment.

## Main Layers

```text
Relay / ecosystem layer
  provides shared security, parachain context, upstream SDK constraints

Omni Node
  runs the parachain without an in-repo custom node crate

DEOS runtime
  owns pallets, assets, routing, staking, governance, and AAA actors

Reference client / indexers
  read bounded chain state directly and materialized history externally
```

## XCM and Asset Identity

DEOS treats foreign assets as local registered assets after governance-controlled registration. The stable identity is not “whatever the current XCM location serializes to.” The registry persists bidirectional `Location <-> AssetId` mappings so later location updates do not break balances or local economic logic.

This keeps cross-chain asset identity compatible with the local bitmask-based asset model used by DEOS runtime primitives.

## Collators and Randomness

The current launch line uses a trusted-collator simplification phase. Native binding targets stay restricted to trusted collators until a production-ready relay/protocol beacon is available for parachain-consumable per-block randomness.

That means local pseudo-random fallbacks can support bounded reference behavior, but they are not a replacement for a future protocol-grade randomness source.

## Costs and Operations

A downstream ecosystem still has operator concerns:

- Collator infrastructure and monitoring;
- Endpoint and bootnode configuration;
- Runtime upgrade procedure;
- XCM asset registration and location maintenance;
- Archive/indexer infrastructure for unbounded history;
- Client endpoint defaults and provider reliability.

These are deployment responsibilities, not hidden product assumptions inside the runtime.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [Runtime Patterns](../overview/runtime-patterns.en.md)
- [Asset Identity](../overview/asset-identity.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [Randomness Strategy](../overview/randomness-strategy.en.md)
