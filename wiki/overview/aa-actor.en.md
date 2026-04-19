---
page_type: overview
title: AA-Actor
summary: AAA is the Account Abstraction Actors system in DEOS, while an AA-Actor is one concrete bounded execution instance inside that system. Actors let the runtime express recurring protocol flows as typed, schedulable execution plans instead of scattering the same logic across many bespoke pallets.
locale: en
canonical_page_id: aa-actor
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/aaa.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - aaa
  - automation
related:
  - AAA System
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance Overview
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.93
---

# AA-Actor

## Summary

`AAA` stands for `Account Abstraction Actors`, the system and pallet. An `AA-Actor` is one bounded execution instance inside that system.

If you want the system-level view, start with [AAA System](aaa-system.en.md). This page focuses on what one actor is and why DEOS uses this abstraction.

## What an Actor Is

An actor is a configured runtime instance with its own sovereign account, schedule, execution plan, lifecycle rules, and failure behavior.

A good mental model is: `one sovereign account + one trigger surface + one bounded plan`.

Instead of scattering recurring economic logic across many special-purpose pallets, DEOS can express a bounded workflow as an actor with typed steps and explicit runtime rules.

## What Actors Are Used For

In the current reference line, actors are used for flows such as:

- Liquidity provisioning
- Burning and buyback flows
- Treasury split routing
- Bucket hold or unwind behavior
- User-defined bounded task pipelines

Most protocol-owned execution in the current line is realized as System actors.

## Stable Contract

The specification keeps a few guarantees central:

- Deterministic behavior for the same state and block context
- Bounded work under explicit limits
- Stateless execution plans instead of mutable workflow memory between steps
- Predictable failure outcomes
- Destruction in place without automatic refund fan-out

These guarantees matter because actors are meant to be safe runtime infrastructure, not loose scripting.

## Actor Types

The specification distinguishes two broad actor classes:

- `User AAA`, which follows the user fee model and owner-slot rules
- `System AAA`, which is governance-created, mutable, and used for protocol automation

That split keeps the execution engine reusable while still allowing the runtime to ship its own deterministic actor families.

## Triggers and Scheduling

Actors can run from schedules, manual triggers, or balance-ingress-style address events. This is part of the larger token-driven automaton model: state transitions should react to bounded execution conditions instead of relying only on bespoke admin calls.

## Simplified Configuration Examples

The examples below use conceptual pseudo-config, not exact extrinsic syntax. They isolate the trigger-and-plan shape of one actor so the model stays easy to read.

A real actor still lives inside the full AAA contract: deterministic scheduling, cooldown, fee admission, lifecycle rules, and bounded execution all still apply.

### 1. A Timer-Driven Burning Actor

```yaml
actor: Burn collected fees
kind: System
trigger:
  type: Timer
  every_blocks: 10
execution_plan:
  - task: SwapExactIn
    asset_in: ForeignFeeAsset
    asset_out: Native
    amount: AllBalance
  - task: Burn
    asset: Native
    amount: AllBalance
```

This actor wakes up every `10` blocks, converts whatever fee asset it holds into the native asset, and then burns the result.

### 2. A Balance-Triggered Liquidity Actor

```yaml
actor: React to foreign collateral arrival
kind: System
trigger:
  type: OnAddressEvent
  asset_filter: [ForeignAsset]
execution_plan:
  - task: SwapExactIn
    asset_in: ForeignAsset
    asset_out: Native
    amount: PercentageOfCurrent(50%)
  - task: AddLiquidity
    asset_a: Native
    asset_b: ForeignAsset
```

Here the actor does nothing until `ForeignAsset` arrives on its account. The arriving asset is not just value sitting in a wallet. It is also the wake-up message that tells the actor to react.

### 3. One Actor Inside a Graph

```yaml
actor: Treasury lane B
kind: System
trigger:
  type: OnAddressEvent
  asset_filter: [LPToken]
execution_plan:
  - task: RemoveLiquidity
    asset: LPToken
    amount: AllBalance
  - task: SplitTransfer
    outputs:
      - Native -> TreasuryB
      - ForeignAsset -> TreasuryB
```

This actor may stay idle until another actor sends it `LPToken`. In graph terms, the previous actor's output becomes this actor's trigger-message. That is how larger protocol behavior can be assembled from small bounded actors.

## Why Actors Matter in DEOS

Actors are how DEOS turns economic coordination into explicit runtime behavior. They connect minting, routing, buckets, treasury actions, and downstream governance-owned operations without forcing every recurring flow into custom pallet code.

## Related

- [AAA System](aaa-system.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance Overview](governance-overview.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/aaa.specification.en.md`
- `docs/aaa.architecture.en.md`
- `docs/core.architecture.en.md`
