---
page_type: concept
title: TOL Bucket Scenarios
summary: Concrete scenarios for the TMCTOL treasury-owned-liquidity bucket model, including bucket A, B, C, D behavior, unwind paths, and actor wakeups.
locale: en
canonical_page_id: tol-bucket-scenarios
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/tmctol.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/aaa.specification.en.md
  - ../../AGENTS.md
status: active
audience: newcomer
tags:
  - concept
  - tmctol
  - liquidity
  - buckets
  - aaa
related:
  - TMCTOL Standard
  - End-to-End Flows
  - Architecture Diagrams
  - AAA System
  - Token-Driven Automation
last_compiled: 2026-06-13
confidence: 0.86
---

# TOL Bucket Scenarios

## Summary

TMCTOL uses treasury-owned-liquidity buckets to keep liquidity capital productive while separating economic intents. The important point is not only that funds enter buckets, but which downstream lane wakes when a bucket becomes actionable.

The reference mental model is a four-bucket split: bucket A is immediate market liquidity, while buckets B/C/D preserve distinct treasury or governance-conditioned lanes.

## Bucket A: Immediate Liquidity

Bucket A is the direct liquidity lane. When minting or routing flows create protocol-owned liquidity, bucket A is the part most closely tied to immediate market depth and the Gravity Well effect.

Scenario:

```text
User demand -> route/mint -> reserves grow
  -> Bucket A receives liquidity share
  -> Market depth improves
  -> Future swaps see stronger protocol-owned liquidity
```

Bucket A is the easiest lane to understand because it behaves like direct reinforcement of the market surface.

## Bucket B: Segmented Treasury Lane

Bucket B preserves a separated treasury intent. It can accumulate liquidity or unwound value without mixing it into every other downstream purpose.

Scenario:

```text
Protocol-owned LP matures or unwinds
  -> Bucket B lane receives value
  -> Paired treasury actor/account accumulates it
  -> Governance can reason about that lane separately
```

The separation matters because governance should not treat every liquidity reserve as one undifferentiated pot.

## Buckets C and D: Wakeup Scenarios

Buckets C and D are easiest to miss because their value is in delayed, segmented action. They matter when an autonomous actor or treasury lane wakes because the bucket contains enough value to justify a bounded operation.

Example C wakeup:

```text
Fees / unwound LP / routed value accumulate
  -> Bucket C crosses actor-specific threshold
  -> System AAA actor wakes
  -> Actor executes bounded plan
  -> Output lands in its paired treasury or liquidity lane
```

Example D wakeup:

```text
Longer-tail accumulation continues
  -> Bucket D remains idle until actionable
  -> Wakeup condition becomes true
  -> Actor attempts execution
  -> Retry/cooldown handles unavailable markets or oracle gaps
```

C/D lanes therefore encode patience and segmentation. They let the protocol avoid forced immediate action while still making accumulated value eventually executable.

## Why Paired Treasuries Matter

Each non-immediate bucket can have a dedicated paired treasury lane. This keeps provenance and governance intent visible:

```text
Bucket B -> Treasury B lane
Bucket C -> Treasury C lane
Bucket D -> Treasury D lane
```

A downstream fork may alter policies, but it should preserve the idea that bucket provenance is part of the economic contract, not just accounting decoration. If bucket policy changes wakeup thresholds, treasury lanes, or actor plans, validate the change against TMCTOL math first and then against AAA execution behavior.

## Related

- [TMCTOL Standard](tmctol-standard.en.md)
- [End-to-End Flows](end-to-end-flows.en.md)
- [Architecture Diagrams](architecture-diagrams.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
