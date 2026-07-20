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
last_compiled: 2026-07-20
confidence: 0.85
---

# TOL Bucket Scenarios

## Summary

TMCTOL uses treasury-owned-liquidity buckets to separate economic intents and preserve reserve provenance. The current reference topology distinguishes immutable Bucket A custody from optional B/C/D unwind and treasury lanes.

Activation status matters: Bucket A is an immutable custody sink, while Bucket B, C, D and their paired treasury actors start as `Noop`. Any later unwind or treasury behavior requires an explicit bounded plan after pool and treasury readiness; balance thresholds do not activate these lanes automatically.

## Bucket A: Immediate Liquidity

Bucket A is the direct liquidity lane. When minting or routing flows create protocol-owned liquidity, bucket A is the part most closely tied to immediate market depth and the Gravity Well effect.

Scenario:

```text
User demand -> route/mint -> protocol reserve reaches Liquidity Actor
  -> actor adds balanced pool liquidity after pool activation
  -> resulting LP moves to immutable Bucket A custody
  -> market depth reflects the completed liquidity operation
```

Bucket A holds the resulting LP; it does not itself add liquidity or execute a follow-on plan.

## Optional Buckets B, C, and D

Buckets B, C, and D preserve separate policy lanes, but the current genesis configuration assigns each a timer schedule with a `Noop` execution plan. Their paired Treasury B/C/D actors also start as `Noop`; Bucket D remains the explicitly dormant reserve.

The architecture provides a bounded unwind-plan family that can remove a configured LP percentage and send reclaimed assets to the paired treasury after readiness. This capability does not imply current activation:

```text
explicit policy and readiness decision
  -> install bounded timer-driven unwind plan
  -> actor removes configured LP percentage
  -> reclaimed assets move to the paired treasury lane
```

No current contract defines automatic threshold-driven wakeups for C or D.

## Why Paired Treasuries Matter

Each non-immediate bucket has a distinct paired treasury account in the reference topology. Those lanes keep provenance and policy intent visible even while their actors remain `Noop`:

```text
Bucket B -> Treasury B lane
Bucket C -> Treasury C lane
Bucket D -> Treasury D lane
```

A downstream fork may alter policy, but it should preserve bucket provenance as part of the economic contract rather than accounting decoration. If it activates or changes treasury lanes or actor plans, it must validate TMCTOL math and AAA execution behavior separately.

## Related

- [TMCTOL Standard](tmctol-standard.en.md)
- [End-to-End Flows](end-to-end-flows.en.md)
- [Architecture Diagrams](architecture-diagrams.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
