---
page_type: overview
title: AAA System
summary: AAA is the Account Abstraction Actors system in DEOS — the pallet, scheduler, lifecycle rules, fee model, and deterministic execution environment that host individual actors. It combines deterministic scheduling, bounded execution, and event-driven reactions, including balance-ingress triggers and larger behavior emerging from composed actor graphs.
locale: en
canonical_page_id: aaa-system
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
  - runtime
  - automation
related:
  - AA-Actor
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance Overview
  - Core Terms
last_compiled: 2026-04-16
confidence: 0.94
---

# AAA System

## Summary

`AAA` means `Account Abstraction Actors`. In DEOS, that names the whole runtime system: `pallet-aaa`, its scheduler, its lifecycle and fee rules, and the typed execution environment for bounded protocol flows.

An `AA-Actor` is one concrete instance inside that system. This page explains the system-level contract; the separate [AA-Actor](aa-actor.en.md) page explains the single-instance view.

## What the System Provides

AAA gives the runtime one reusable way to run bounded execution plans instead of hardcoding every recurring flow into a dedicated pallet.

The full picture has several parts at once: deterministic scheduling, bounded execution, explicit lifecycle and fee rules, and bounded reactions to events. In the DEOS model, actor balances can work like trigger messages: the arrival of specific assets on a specific actor account can be the thing that decides which execution plan should wake up and what economic action should happen next.

At a high level, the system provides:

- Deterministic scheduling
- Event-driven triggering, especially through balance ingress
- Typed tasks such as transfer, swap, liquidity actions, burn, mint, and staking
- Explicit lifecycle rules for pause, failure, auto-close, and manual close
- A split between user-owned actors and governance-owned system actors
- Adapter boundaries so AAA can orchestrate runtime mechanics without embedding DEX or asset logic directly

## System vs Actor

The distinction matters:

- `AAA` = the system, pallet, scheduler, and all actors together
- `AA-Actor` = one bounded runtime instance inside that system

That is why DEOS can talk about AAA as infrastructure while still talking about many different actors with different jobs.

## Current Role in DEOS

On the current reference line, AAA is the execution substrate for runtime-side protocol behavior. It is how DEOS expresses burning, liquidity provisioning, treasury splitting, bucket handling, and other bounded economic reactions.

Some of those reactions are timer-driven. Others are balance-driven: an asset arriving on an actor account can function like a message that triggers the next bounded step. That is one of the most important ideas in the system.

The math and domain rules live in subsystem contracts such as TMC, the Axial Router, staking, and governance. AAA does not replace those subsystems. It gives them a deterministic way to be orchestrated together.

## Current System Topology

The shipped DEOS runtime currently provisions a fixed set of System actors at genesis, plus one reserved deterministic fee-sink address.

In newcomer terms, that means the current line already uses AAA as real protocol infrastructure, not as a future concept. The runtime ships named actor lanes for burning, liquidity, bucket handling, treasury flows, and the current BLDR lane.

## Why the System Exists

Without AAA, the runtime would have to keep adding more bespoke pallet logic for every recurring economic workflow. AAA makes those workflows explicit, bounded, and governable through execution plans, scheduler semantics, lifecycle rules, and trigger semantics.

One important effect appears when actor graphs are composed. One actor's balance outflow can become another actor's trigger message, and chains of such reactions can produce larger protocol behavior from small bounded parts. But that graph composition still runs inside the same deterministic scheduler and bounded execution contract.

Within the existing task and adapter language, this shifts a large class of protocol evolution from runtime rewrites into on-chain actor-graph reconfiguration. Runtime upgrades are still needed for new primitives, adapter surfaces, or safety invariants, but many workflow and topology changes can stay at the configuration layer.

That keeps the kernel smaller and keeps more protocol behavior visible as typed automation instead of hidden glue code.

## Related

- [AA-Actor](aa-actor.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance Overview](governance-overview.en.md)
- [Core Terms](../glossary/core-terms.en.md)

## Sources

- `docs/aaa.specification.en.md`
- `docs/aaa.architecture.en.md`
- `docs/core.architecture.en.md`
