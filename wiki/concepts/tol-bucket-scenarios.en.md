---
type: concept
title: TOL Bucket Scenarios
description: Configurable treasury-owned-liquidity scenarios, from first-order A/B/C/D to independent second-order BLDR Anchor and BLDR Treasury owners.
locale: en
canonical_page_id: tol-bucket-scenarios
translation_status: source
available_locales:
  - en
  - ru
sources:
  - resource: ../../docs/tmctol.specification.en.md
  - resource: ../../template/pallets/actors/docs/architecture.en.md
  - resource: ../../template/pallets/actors/docs/specification.en.md
  - resource: ../../docs/core.architecture.en.md
  - resource: ../../docs/builder-economy.contract.en.md
  - resource: ../../simulator/README.md
  - resource: ../../AGENTS.md
status: stable
audience: newcomer
tags:
  - concept
  - tmctol
  - liquidity
  - buckets
  - actor
related:
  - TMCTOL Standard
  - End-to-End Flows
  - Architecture Diagrams
  - Actors System
  - Token-Driven Automation
  - Builder Economy
---

# TOL Bucket Scenarios

## Summary

TOL is the asset-scoped topology through which a treasury owns protocol liquidity and related strategic capital. Anchors and buckets preserve liquidity custody and provenance; treasuries realize or spend strategic capital. Their counts are configuration rather than part of the universal definition.

The first-order Native/foreign reference topology instantiates Bucket Anchor (`A`), Bucket Builder (`B`), Bucket Capital (`C`), and Bucket Dormant (`D`). Anchor custody is immutable; B/C/D own optional unwind and treasury lanes. The second-order `$BLDR` topology uses first-order `$NTVE` as collateral and has two independently named owners: BLDR Anchor and BLDR Treasury. Without peer buckets, BLDR Anchor does not inherit the first-order A/B/C/D namespace.

Activation status matters in the first-order runtime topology: Bucket A is a sealed dormant Immutable System identity whose LP balance is frozen against every debit, while Bucket B, C, D and their paired treasury roles start as dormant Mutable System identities without programs. Any later unwind or treasury behavior requires explicit activation with a bounded plan after pool and treasury readiness; balance thresholds do not activate these lanes automatically.

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

Buckets B, C, and D preserve separate policy lanes, but the current genesis configuration keeps each identity dormant and outside scheduler enrollment. Their paired Treasury B/C/D identities also start dormant; Bucket D remains the explicitly dormant reserve.

The architecture provides a production-admissible two-actor unwind family. The Bucket transfers a configured LP percentage; the corresponding Treasury removes all preservable balance of that configured system LP asset in its own cycle, so both underlying assets are born directly in Treasury custody. Treasury does not filter the LP sender: pairing names the reference lane rather than an ingress permission. This capability does not imply genesis activation:

```text
explicit policy and readiness decision
  -> Bucket transfers bounded LP percentage to paired Treasury
  -> Treasury timer observes LP and removes liquidity in a separate admitted cycle
  -> reclaimed Native and foreign assets remain in that Treasury lane
```

No current contract defines automatic threshold-driven wakeups for C or D.

Floor reporting follows live custody rather than historical deposits. A bucket contributes only its current proportional reserve claim while it holds positive LP in an anchor or explicitly active-support state; dormant LP does not count until activation. Once that live position leaves the pool, its claim leaves the reported reserve scope. If live Bucket A anchor support disappears, the report must set `governance_state` to `degraded`.

## TOL Component of Second-Order `$BLDR` TMCTOL

Both reference orders direct approximately one third of total issuance to immutable anchor liquidity. They do not share a collateral percentage: first-order Bucket A receives half of collateral under the four-bucket allocation, while BLDR Anchor receives all `$NTVE`. The `$BLDR` TMC concentrates its retained protocol issuance into two equal policy lanes:

```text
about 1/3 $BLDR -> recipient
about 1/3 $BLDR -> BLDR Anchor
about 1/3 $BLDR -> spendable BLDR Treasury
all $NTVE collateral -> anchor-liquidity lane
```

The parent Bucket B bridge may gradually release LP into Treasury B. Both released reserve assets route into `$BLDR`; half of recipient output burns and half enters BLDR Treasury. Market routes buy existing supply, while a TMC route also creates its ordinary anchor and direct-treasury allocations. The Builder Economy contract defines these route-dependent effects analytically; the current simulator does not model the Builder bridge.

## Why Paired Treasuries Matter

Each non-immediate bucket has a distinct paired treasury account in the reference topology. Those lanes keep provenance and policy intent visible even while their System identities remain dormant:

```text
Bucket B -> Treasury B lane
Bucket C -> Treasury C lane
Bucket D -> Treasury D lane
```

A downstream fork may alter policy, but it should preserve bucket provenance as part of the economic contract rather than accounting decoration. The reference split avoids a same-actor RemoveLiquidity plus dual-transfer plan that exceeds the 50% ProofSize reserve. If a fork activates or changes treasury lanes or actor plans, it must validate TMCTOL math and Actors execution behavior separately.

## Related

- [TMCTOL Standard](tmctol-standard.en.md)
- [End-to-End Flows](end-to-end-flows.en.md)
- [Architecture Diagrams](architecture-diagrams.en.md)
- [Actors System](../overview/actor-system.en.md)
- [Token-Driven Automation](token-driven-automation.en.md)
- [Builder Economy](builder-economy.en.md)
