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
last_compiled: 2026-07-22
confidence: 0.9
---

# Token-Driven Automation

## Summary

DEOS models protocol behavior as a token-driven economic automaton. Instead of centering everything on admin calls, it tries to express recurring economic actions as bounded state transitions triggered by balances, timers, and typed execution plans.

The main execution system for that model is `pallet-aaa`, which hosts both system actors and user actors.

## The Core Coordination Rule

A simple way to read the architecture is:

`Authorized balance ingress -> Admitted deterministic transition -> Observable balance/state result`

AAA separates ordinary balance credit from execution influence. Source and asset filters decide whether ingress triggers an actor, while funding policy decides whether provenance updates funding snapshots. Timers and manual signals provide additional bounded trigger paths.

## Why This Model Exists

The model gives the runtime one reusable way to express flows such as:

- Burning collected fees
- Provisioning liquidity
- Splitting emissions across treasury destinations
- Holding bucket liquidity and activating only production-admissible policies
- Running deterministic task pipelines under bounded scheduler control

In the current reference line, most shipped protocol automation uses System AAA actors. Dedicated pallets still own minting, routing, staking, balances, and AMM mechanics.

## Stable AAA Properties

The specification keeps a few guarantees central:

- Deterministic behavior for identical state and block context
- Bounded work with explicit limits
- Stateless execution plans instead of mutable workflow memory between steps
- Predictable failure modes such as deferred, skipped, failed, or closed
- No automatic refund fan-out when an actor is destroyed

## Why Provenance-Aware Ingress Matters

An actor can accept ordinary transfers without granting every sender trigger or funding authority. Configured source/asset filters and funding policy make that influence explicit rather than assuming every deposit has the same meaning.

Unexpected transfers remain real sovereign-account balances, but they become burns, liquidity, or treasury inputs only when an admitted task and valid trigger consume them. This distinction prevents donation sensitivity from becoming hidden execution authority.

## Relationship to TMCTOL

In the current reference line, TMCTOL uses AAA actors for recurring runtime automation that existing tasks and adapters can express safely. The math lives in the standard, dedicated pallets own economic primitives, and AAA plans own bounded orchestration. Optional Bucket B/C/D unwind now splits LP transfer and Treasury-owned liquidity removal into separate admitted cycles; the lanes remain dormant until governance activates both plans.

## Related

- [DEOS Framework Overview](../overview/deos-framework.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [AA-Actor](../overview/aa-actor.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Read-Model Split](read-model-split.en.md)
- [Core Terms](../glossary/core-terms.en.md)
