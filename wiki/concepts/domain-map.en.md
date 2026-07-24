---
page_type: concept
title: Domain Map
summary: A self-contained map of the major DEOS knowledge domains and how they link together inside the wiki.
locale: en
canonical_page_id: domain-map
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
  - ../../docs/core.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/web-client.architecture.en.md
status: active
audience: newcomer
tags:
  - domain-map
  - onboarding
  - wiki
related:
  - DEOS Framework Overview
  - TMCTOL Standard
  - AAA System
  - Governance Overview
  - $BLDR Builder Economy
  - Reference Client
last_compiled: 2026-07-20
confidence: 0.9
---

# Domain Map

## Summary

DEOS is easiest to understand as a set of linked domains rather than as a list of pallets, files, or UI widgets. Pallets and modules are implementation shapes. Wiki domains are meaning-bearing contours: each one explains a force in the system and points to the other forces it depends on.

Use this page as the map of the wiki knowledge graph.

## The main domains

### Framework identity

The framework domain explains what DEOS is: a forkable deterministic economic operating system for protocol economies. It separates DEOS, the framework, from TMCTOL, the current tokenomic standard.

Start with [DEOS Framework Overview](../overview/deos-framework.en.md), then use [Core Terms](../glossary/core-terms.en.md) when vocabulary becomes dense.

### Economic physics

The economic domain explains the managed token economy: minting curves, treasury-owned liquidity, floor/ceiling behavior, burn liveness, and compression claims.

Read [TMCTOL Standard](tmctol-standard.en.md), [Token Surfaces](token-surfaces.en.md), [TOL Bucket Scenarios](tol-bucket-scenarios.en.md), [Economic Thresholds](economic-thresholds.en.md), [TMCTOL Formulas](../math/tmctol-formulas.en.md), [Token Minting Curve](../overview/token-minting-curve.en.md), and [Routing and Minting Loop](routing-and-minting-loop.en.md).

### Autonomous actors

The actor domain explains how protocol-owned accounts run bounded tasks. System AAA actors burn, route, provide liquidity, split flows, hold buckets, and execute treasury policies without making the protocol depend on bespoke manager pallets.

Read [AAA System](../overview/aaa-system.en.md), [AA-Actor](../overview/aa-actor.en.md), and [Token-Driven Automation](token-driven-automation.en.md).

### Routing and asset identity

The routing domain connects user intent, mint paths, AMM liquidity, fees, and registered assets. It is where the framework decides whether a trade should go through market liquidity or protocol liquidity, and how foreign assets become local runtime citizens.

Read [Axial Router](../overview/axial-router.en.md), [Asset Identity](../overview/asset-identity.en.md), and [Read-Model Split](read-model-split.en.md).

### Governance and protection

The governance domain explains who may change what. DEOS governance is domain-scoped: each governed area has a primary power surface, a protection surface, typed payloads, bounded execution authority, and explicit limits.

Read [Governance Overview](../overview/governance-overview.en.md), [Governance Domains](governance-domains.en.md), and [Physics-First vs Politics-First](../comparisons/physics-vs-politics.en.md).

### Builder economy and useful work

The builder domain sits where tactical governance, labor funding, protocol-owned liquidity, and downstream product demand meet. It explains how public invoices can reward completed work without turning founder status into a framework entitlement, while Native protection keeps the tactical domain inside its delegated boundary.

Read [$BLDR Builder Economy](builder-economy.en.md), then use [Token Surfaces](token-surfaces.en.md) and [Governance Domains](governance-domains.en.md) for the token and authority details.

### Staking and rewards

The staking domain explains share-vault receipts, native liquid staking, LP nomination, reward memory, and protocol donation into liquidity. It bridges economic security, user positions, and governance-conditioned rewards.

Read [Staking Pools](staking-pools.en.md) and [Three-Layer Validation](../development/three-layer-validation.en.md).

### Client and read model

The client domain explains how the browser product shows the system without pretending to be the source of truth. It distinguishes direct on-chain state, session-derived projections, and future materialized/indexed providers.

Read [Reference Client](../overview/reference-client.en.md) and [Read-Model Split](read-model-split.en.md).

### Tooling and validation

The tooling domain explains how contributors and agents keep the system honest: simulator math, runtime checks, wiki trust validation, Domain DAG validation, and release gates.

Read [Three-Layer Validation](../development/three-layer-validation.en.md), [Scripts Layer](../usage/scripts-layer.en.md), and [Development Status](../development/status.en.md).

### Future gates

The future-gates domain explains what is intentionally not part of the current shipped baseline: permissionless collators, relay-beacon randomness, full indexed portfolio discovery, and richer materialized archives.

Read [Randomness Strategy](../overview/randomness-strategy.en.md) and [Development Status](../development/status.en.md).

## How the domains connect

A useful traversal is:

1. Framework identity defines the product boundary.
2. Economic physics defines the token laws.
3. Autonomous actors execute recurring economic flows.
4. Routing and asset identity connect users, assets, and protocol liquidity.
5. Governance and protection decide which changes are allowed.
6. Builder economy turns delegated capital into evaluated useful work.
7. Staking and rewards connect users to security and incentives.
8. Client and read model expose the system without inventing truth.
9. Tooling and validation keep the graph synchronized.
10. Future gates prevent speculative work from masquerading as shipped reality.

## Related

- [End-to-End Flows](end-to-end-flows.en.md)
- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Governance Overview](../overview/governance-overview.en.md)
- [$BLDR Builder Economy](builder-economy.en.md)
- [Reference Client](../overview/reference-client.en.md)
