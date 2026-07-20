---
page_type: concept
title: Token-Driven Automation
summary: DEOS expresses recurring protocol behavior as bounded execution plans triggered by balances, schedules, and typed runtime tasks. AAA is the main execution system for these flows.
locale: en
canonical_page_id: token-driven-automation
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/core.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../README.md
status: active
audience: newcomer
tags:
  - concept
  - aaa
  - automation
related:
  - DEOS Framework Overview
  - AAA System
  - AA-Actor
  - TMCTOL Standard
  - Read-Model Split
  - Core Terms
last_compiled: 2026-07-20
confidence: 0.9
---

# Token-Driven Automation

## Summary

DEOS models protocol behavior as a token-driven economic automaton. Instead of centering everything on admin calls, it tries to express recurring economic actions as bounded state transitions triggered by balances, timers, and typed execution plans.

The main execution system for that model is `pallet-aaa`, which hosts both system actors and user actors.

## The Core Coordination Rule

A simple way to read the architecture is:

`Balance in -> Deterministic state transition -> Balance out`

That rule pushes the system toward origin-agnostic handling. The important question is not who sent the tokens. The important question is what arrived and which bounded plan should react.

## Why This Model Exists

The model gives the runtime one reusable way to express flows such as:

- Burning collected fees
- Provisioning liquidity
- Splitting emissions across treasury destinations
- Unwinding or holding bucket liquidity
- Running deterministic task pipelines under bounded scheduler control

In the current reference line, most shipped protocol flows use System AAA actors.

## Stable AAA Properties

The specification keeps a few guarantees central:

- Deterministic behavior for identical state and block context
- Bounded work with explicit limits
- Stateless execution plans instead of mutable workflow memory between steps
- Predictable failure modes such as deferred, skipped, failed, or closed
- No automatic refund fan-out when an actor is destroyed

## Why Origin-Agnostic Handling Matters

Origin-agnostic execution makes the system more robust. An actor can react to balances or configured triggers without depending on one privileged caller path.

The core architecture also frames this as graceful degradation: even an unexpected transfer to a system account may still become an economically meaningful input instead of turning into dead state.

## Relationship to TMCTOL

In the current reference line, TMCTOL uses AAA actors to execute its runtime-side economics. The math lives in the standard, while the recurring operational behavior lives in bounded execution plans.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [AA-Actor](../overview/aa-actor.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Read-Model Split](read-model-split.en.md)
- [Core Terms](../glossary/core-terms.en.md)
