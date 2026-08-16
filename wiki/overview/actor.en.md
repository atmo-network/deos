---
type: overview
title: AA-Actor
description: Actors is the Account Abstraction Actors system in DEOS, while an AA-Actor is one concrete bounded execution instance. Each instance follows one typed Actor Contract with ordered Steps.
locale: en
canonical_page_id: actor
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../template/pallets/actors/docs/specification.en.md
  - resource: ../../template/pallets/actors/docs/architecture.en.md
  - resource: ../../docs/actors.integration.en.md
  - resource: ../../docs/core.architecture.en.md
status: stable
audience: newcomer
tags:
  - overview
  - actor
  - automation
related:
  - Actors System
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance
  - Core Terms
last_compiled: 2026-08-14
confidence: 0.9
---

# AA-Actor

## Summary

`Actors` is the Account Abstraction Actors system. An `AA-Actor` is one bounded execution instance inside that system.

Use [Actors System](actor-system.en.md) for the system-level view. This page focuses on the single actor abstraction.

## Actor Contract

A useful mental model is:

```text
one sovereign account + one trigger surface + one bounded Actor Contract
```

An actor has its own account, schedule or trigger, Actor Contract, lifecycle rules, and failure behavior. Instead of scattering recurring economic logic across special-purpose pallets, DEOS can express bounded behavior as typed Steps under explicit runtime limits.

The stable contract emphasizes:

- Deterministic behavior for the same state and block context;
- Bounded work;
- Static ordered Steps without Task-authored workflow memory;
- Sparse scheduler-owned progress only while a Mutable actor is suspended;
- Predictable failure outcomes;
- Destruction in place without automatic refund fan-out.

Actors are runtime infrastructure, not loose scripting.

A Mutable actor can assign `RetryLater` to a step whose adapter may report a Temporary failure. Actors then stores only the unresolved cursor and bounded attempt state, preserving successful earlier steps without turning the Actor Contract into mutable code. Permanent failure terminates; cancellation deletes progress without compensating committed effects. Immutable actors cannot use this policy.

## Actor Classes and Uses

The specification distinguishes two broad classes:

- `User Actors`: user-fee model and owner-slot rules;
- `System Actors`: governance-created actors used for protocol automation.

In the current reference line, actors support liquidity provisioning, burning/buyback flows, treasury split routing, bucket hold or unwind behavior, and user-defined bounded task pipelines. Most protocol-owned execution is realized as System actors.

## Triggers and Actor Contract Shapes

Actors can run from schedules, manual triggers, or balance-ingress address events. Balance ingress is the key token-driven shape: an asset arriving on an actor account can also be the wake-up message.

Common Actor Contract shapes include:

- Timer-driven burning: swap collected fees into Native, then burn;
- Balance-triggered liquidity: react to foreign collateral arrival, swap part of it, then add liquidity;
- Graph node: receive an LP token from another actor, unwind it, then split outputs to treasury accounts.

In all cases the actor remains inside the full Actors contract: deterministic scheduling, cooldowns, fee admission, lifecycle rules, and bounded execution.

## Why Actors Matter

Actors turn economic coordination into explicit runtime behavior. They connect minting, routing, buckets, treasury actions, and governance-owned operations without forcing every recurring flow into custom pallet code.

They also make actor graphs possible: one actor's balance outflow can become another actor's trigger message. Larger protocol behavior can be composed from small bounded pieces while staying inspectable as typed automation.

## Related

- [Actors System](actor-system.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance](governance.en.md)
- [Core Terms](../glossary/core-terms.en.md)
