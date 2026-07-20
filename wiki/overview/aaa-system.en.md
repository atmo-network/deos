---
page_type: overview
title: AAA System
summary: AAA is the Account Abstraction Actors system in DEOS — the pallet, scheduler, lifecycle rules, fee model, and deterministic execution environment that host individual actors while keeping domain logic in adapters and pallets.
locale: en
canonical_page_id: aaa-system
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/aaa.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/aaa.embedding.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - aaa
  - runtime
  - automation
related:
  - AA-Actor
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance Overview
  - Core Terms
last_compiled: 2026-07-20
confidence: 0.95
---

# AAA System

## Summary

`AAA` means `Account Abstraction Actors`. In DEOS, it names the whole runtime system: `pallet-aaa`, scheduler, lifecycle rules, fee model, actor accounts, and typed execution environment for bounded protocol flows.

An [AA-Actor](aa-actor.en.md) is one concrete instance inside that system. This page explains the system-level contract.

## System Contract

AAA gives the runtime one reusable way to run bounded execution plans instead of hardcoding every recurring workflow into a dedicated pallet.

At system level it provides:

- Deterministic scheduling;
- Balance/event-driven triggering;
- Typed tasks such as transfer, swap, liquidity, burn, mint, stake, and unstake;
- Lifecycle rules for pause, failure, auto-close, and manual close;
- Separation between user-owned actors and governance-owned System actors;
- Adapter boundaries so AAA orchestrates runtime mechanics without owning DEX, staking, or asset logic.

Actor balances can function like trigger messages: an asset arriving on an actor account can wake the next bounded execution plan.

## Embedding Boundary

External runtimes can reuse `pallet-aaa` without inheriting the DEOS/TMCTOL System actor catalog. The host runtime provides bounded adapters for assets, DEX, staking, liquidity donation, fee conversion, ingress, entropy, and task weights. AAA owns scheduling, lifecycle, amount resolution, fee reservation, and task orchestration.

The atomicity guarantee is task-scoped, not whole-plan scoped. If an adapter fails after partial mutation, the failed task rolls back its local effects and success event; earlier successful steps remain committed. `ContinueNextStep` or `AbortCycle` then decides whether the cycle proceeds or stops.

## Portability Boundary

The current staking contract is intentionally generic:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

AAA does not encode DEOS-specific `StakeNative`, collator selection, `stNTVE` naming, or `NTVE/stNTVE` LP custody. Runtime adapters decide what a generic stake means for the chain. In DEOS, the adapter routes native staking into `pallet-staking::stake_native`, while nomination security remains a separate locked-LP staking/governance surface.

This keeps AAA useful outside one tokenomic configuration.

## Current DEOS Role

On the current reference line, AAA is the execution substrate for runtime-side protocol behavior: burning, liquidity provisioning, treasury splitting, bucket handling, BLDR lane flows, and native staking LP provisioning.

The shipped runtime provisions System actors at genesis, plus one deterministic fee-sink address. Native staking LP provisioning starts dormant and can activate only after the native staking receipt, staking pool, actor, and non-empty `NTVE/stNTVE` AMM are ready.

AAA does not replace TMC, Axial Router, staking, or governance. Those subsystems own math and domain rules. AAA gives them a deterministic way to be orchestrated together.

## Why It Exists

Without AAA, recurring economic workflows would keep becoming bespoke pallet logic. AAA makes those workflows explicit, bounded, governable, and composable as typed actor graphs.

One actor's balance outflow can become another actor's trigger message. Larger protocol behavior can therefore emerge from small bounded parts while still running inside deterministic scheduling and execution limits.

Within the existing task and adapter language, many workflow/topology changes can move from runtime rewrites into on-chain actor-graph configuration. Runtime upgrades remain necessary for new primitives, adapter surfaces, or safety invariants.

## Related

- [AA-Actor](aa-actor.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance Overview](governance-overview.en.md)
- [Core Terms](../glossary/core-terms.en.md)
