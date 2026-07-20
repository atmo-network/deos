---
page_type: concept
title: Architecture Diagrams
summary: Compact text diagrams for the main DEOS subsystem relationships, including the domain map, routing loop, AAA actor graph, read-model split, and governance/staking boundary.
locale: en
canonical_page_id: architecture-diagrams
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/governance.architecture.en.md
  - ../../docs/staking.architecture.en.md
status: active
audience: newcomer
tags:
  - concept
  - diagrams
  - architecture
  - onboarding
related:
  - Domain Map
  - End-to-End Flows
  - Routing and Minting Loop
  - AAA System
  - Read-Model Split
last_compiled: 2026-07-20
confidence: 0.85
---

# Architecture Diagrams

## Summary

This page gives compact visual maps for readers who need shape before detail. The diagrams are intentionally textual so they stay readable in the repository, the web client, and agent contexts.

Use [Domain Map](domain-map.en.md) for domain ownership and [End-to-End Flows](end-to-end-flows.en.md) for walkthroughs.

## Core Domain Loop

```text
User intent
  -> Reference Client
  -> Read-model classification
  -> Runtime surface
  -> Axial Router / TMC / Staking / Governance
  -> Events, balances, and bounded projections
  -> Reference Client feedback
```

The client is not the source of truth. It reads bounded chain truth directly when possible and labels session or materialized data when it is not direct protocol state.

## Routing and Minting

```text
Swap request
  -> Axial Router
      -> compare XYK market path
      -> compare TMC protocol path
      -> choose best bounded route
  -> execute swap or mint
  -> collect fee
  -> route fee toward the configured Burn Actor flow
```

The router coordinates market liquidity and protocol liquidity. TMC owns deterministic mint-side pricing. Long-range analytics stay outside consensus state.

## AAA Actor Graph

```text
Configured trigger becomes due
  -> balance ingress for omnivorous actors
     or bounded schedule for timer-driven actors
  -> AAA scheduler checks lifecycle / cooldown / limits
  -> actor executes typed plan
  -> output asset lands elsewhere
  -> a downstream ingress actor may wake
```

AAA is the reusable execution system. An AA-Actor is one bounded instance inside it. Larger protocol behavior can be assembled from small actor steps.

## Governance and Staking Boundary

```text
Governance domain
  -> primary track + protection track
  -> typed payload
  -> bounded execution authority
  -> optional participation-quality signal
  -> staking reward coefficient

Staking pool
  -> share-vault accounting
  -> receipt supply
  -> reward settlement
```

Governance and staking interact, but they do not collapse into one subsystem. Governance can produce bounded reward signals; staking owns pool math and settlement.

## Read-Model Split

```text
Public datum
  -> bounded canonical on-chain projection
  -> or indexed / materialized view

Browser realization
  -> direct read
  -> session cache
  -> session-derived view
  -> provider-backed materialized data
```

The first split is the protocol contract. The second explains how the browser currently obtains a value.

## Related

- [Domain Map](domain-map.en.md)
- [End-to-End Flows](end-to-end-flows.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Read-Model Split](read-model-split.en.md)
