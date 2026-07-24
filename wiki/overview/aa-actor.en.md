---
page_type: overview
title: AA-Actor
summary: AAA is the Account Abstraction Actors system in DEOS, while an AA-Actor is one concrete bounded execution instance inside that system. Actors express recurring protocol flows as typed, schedulable execution plans instead of bespoke pallet logic.
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
last_compiled: 2026-07-24
confidence: 0.9
---

# AA-Actor

## Summary

`AAA` is the Account Abstraction Actors system. An `AA-Actor` is one bounded execution instance inside that system.

Use [AAA System](aaa-system.en.md) for the system-level view. This page focuses on the single actor abstraction.

## Actor Contract

A useful mental model is:

```text
one sovereign account + one trigger surface + one bounded plan
```

An actor has its own account, schedule or trigger, execution plan, lifecycle rules, and failure behavior. Instead of scattering recurring economic logic across special-purpose pallets, DEOS can express a bounded workflow as typed actor steps under explicit runtime limits.

The stable contract emphasizes:

- Deterministic behavior for the same state and block context;
- Bounded work;
- Static execution plans without task-authored workflow memory;
- Sparse scheduler-owned progress only while a Mutable actor is suspended;
- Predictable failure outcomes;
- Destruction in place without automatic refund fan-out.

Actors are runtime infrastructure, not loose scripting.

A Mutable actor can assign `RetryLater` to a step whose adapter may report a Temporary failure. AAA then stores only the unresolved cursor and bounded attempt state, preserving successful earlier steps without turning the plan into mutable code. Permanent failure terminates; cancellation deletes progress without compensating committed effects. Immutable actors cannot use this policy.

## Actor Classes and Uses

The specification distinguishes two broad classes:

- `User AAA`: user-fee model and owner-slot rules;
- `System AAA`: governance-created actors used for protocol automation.

In the current reference line, actors support liquidity provisioning, burning/buyback flows, treasury split routing, bucket hold or unwind behavior, and user-defined bounded task pipelines. Most protocol-owned execution is realized as System actors.

## Triggers and Plan Shapes

Actors can run from schedules, manual triggers, or balance-ingress address events. Balance ingress is the key token-driven shape: an asset arriving on an actor account can also be the wake-up message.

Common plan shapes include:

- Timer-driven burning: swap collected fees into Native, then burn;
- Balance-triggered liquidity: react to foreign collateral arrival, swap part of it, then add liquidity;
- Graph node: receive an LP token from another actor, unwind it, then split outputs to treasury accounts.

In all cases the actor remains inside the full AAA contract: deterministic scheduling, cooldowns, fee admission, lifecycle rules, and bounded execution.

## Why Actors Matter

Actors turn economic coordination into explicit runtime behavior. They connect minting, routing, buckets, treasury actions, and governance-owned operations without forcing every recurring flow into custom pallet code.

They also make actor graphs possible: one actor's balance outflow can become another actor's trigger message. Larger protocol behavior can be composed from small bounded pieces while staying inspectable as typed automation.

## Related

- [AAA System](aaa-system.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance Overview](governance-overview.en.md)
- [Core Terms](../glossary/core-terms.en.md)
