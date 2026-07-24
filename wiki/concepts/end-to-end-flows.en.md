---
page_type: concept
title: End-to-End Flows
summary: Concrete walkthroughs that connect user actions, runtime routing, AAA actor wakeups, buckets, read-model surfaces, and validation choices inside DEOS.
locale: en
canonical_page_id: end-to-end-flows
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/axial-router.architecture.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/tmctol.specification.en.md
  - ../../docs/staking.architecture.en.md
status: active
audience: newcomer
tags:
  - concept
  - flows
  - routing
  - aaa
  - onboarding
related:
  - Domain Map
  - Routing and Minting Loop
  - AAA System
  - AA-Actor
  - TMCTOL Standard
  - Staking Pools
  - Read-Model Split
last_compiled: 2026-07-24
confidence: 0.85
---

# End-to-End Flows

## Summary

This page fills the gap between concept pages and implementation files. It shows how a user action or protocol event moves through DEOS as a concrete flow.

The examples are simplified, but each step names the responsible domain and the page that owns deeper explanation.

## Swap Through Axial Router

1. A user asks the reference client for a swap preview.
2. The client reads bounded route and asset data, then labels the result as live on-chain truth rather than archive analytics.
3. Axial Router compares available market-liquidity and protocol-liquidity paths.
4. If the TMC path is better, the route mints through the curve. If the XYK path is better, it swaps through market liquidity.
5. Router fees enter the configured Burn Actor flow; the burn occurs only when that actor remains funded, schedulable, and completes execution.
6. The client shows execution progress through centralized feedback instead of each widget inventing its own transaction log.

Owner pages: [Routing and Minting Loop](routing-and-minting-loop.en.md), [Axial Router](../overview/axial-router.en.md), [Read-Model Split](read-model-split.en.md), [Reference Client](../overview/reference-client.en.md).

## Actor Wakeup Chain

1. A System actor reaches its configured trigger: balance ingress for omnivorous actors or a bounded schedule for timer-driven actors.
2. AAA scheduler admits the actor only if lifecycle, cooldown, fee, and bounded-execution rules allow it.
3. The actor executes a typed plan such as swap, burn, add/remove liquidity, split transfer, stake, or unstake.
4. Its output may land on another ingress-driven actor account and wake that actor.
5. A larger protocol behavior emerges from small bounded steps, but remains inspectable as an actor graph.

Owner pages: [AAA System](../overview/aaa-system.en.md), [AA-Actor](../overview/aa-actor.en.md), [Token-Driven Automation](token-driven-automation.en.md).

## Temporary Middle-Step Failure

1. A Mutable plan executes `SwapExactIn`, then attempts `AddLiquidity`, then intends to `Transfer` the result.
2. The swap succeeds and commits. The liquidity adapter reports an explicitly Temporary failure.
3. `RetryLater` rolls back only the failed task and stores a sparse Continuation at the `AddLiquidity` cursor.
4. Cooldown re-enters the same FIFO/wakeup scheduler without a new external trigger. Retry charges and admits only `AddLiquidity → Transfer`.
5. Success removes Continuation and emits one cumulative summary for the original logical-run nonce. Cancellation instead keeps the swap committed and performs no compensation.

Current cursor and attempt are bounded chain truth. A long attempt timeline belongs to a materialized event index.

## TOL Bucket and Treasury Lane

1. Mint-side reserve flow increases protocol-owned liquidity.
2. TOL bucket policy segments that liquidity so not every reserve performs the same job.
3. A bucket or LP-unwind actor can wake when the right balance appears or the right schedule arrives.
4. Unwound assets move toward paired treasury lanes instead of collapsing all reserves into one undifferentiated account.
5. Governance can reason about segmented treasury surfaces without owning the launch physics of the curve.

This is intentionally a domain-level walkthrough. Bucket ratios and formulas belong to [TMCTOL Standard](tmctol-standard.en.md) and [TMCTOL Formulas](../math/tmctol-formulas.en.md).

## Native Staking and Collator Security

1. A user stakes `$NTVE` and receives liquid `stNTVE` receipt shares.
2. Collator security is not inferred from wallet `stNTVE` balances.
3. The security path uses explicit locked `NTVE/stNTVE` LP custody.
4. Native nomination reward paths stay separate from generic same-asset staking rewards.
5. Governance-conditioned participation can influence reward coefficients, but governance and staking remain separate subsystems.

Owner page: [Staking Pools](staking-pools.en.md).

## Validation Rule

When changing a flow, validate the highest affected layer:

- Formula or invariant changed -> simulator first;
- Pallet behavior changed -> targeted Rust tests/benchmarks;
- Runtime interaction changed -> integration checks;
- Client presentation changed -> web-client validation and read-model honesty;
- Wiki explanation changed -> trusted wiki validation.

See [Three-Layer Validation](../development/three-layer-validation.en.md).

## Related

- [Domain Map](domain-map.en.md)
- [Routing and Minting Loop](routing-and-minting-loop.en.md)
- [AAA System](../overview/aaa-system.en.md)
- [AA-Actor](../overview/aa-actor.en.md)
- [TMCTOL Standard](tmctol-standard.en.md)
- [Staking Pools](staking-pools.en.md)
- [Read-Model Split](read-model-split.en.md)
