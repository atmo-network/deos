---
page_type: overview
title: AAA System
summary: AAA is the Account Abstraction Actors system in DEOS — the pallet, scheduler, lifecycle rules, fee model, and deterministic execution environment that host individual actors while keeping domain logic in adapters and pallets.
locale: en
canonical_page_id: aaa-system
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../docs/aaa.specification.en.md
  - ../../docs/aaa.architecture.en.md
  - ../../docs/aaa-control-plane.contract.en.md
  - ../../template/pallets/aaa/EMBEDDING.md
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
last_compiled: 2026-07-24
confidence: 0.95
---

# AAA System

## Summary

`AAA` means `Account Abstraction Actors`. In DEOS, it names the whole runtime system: `pallet-aaa`, scheduler, lifecycle rules, fee model, actor accounts, and typed execution environment for bounded protocol flows.

An [AA-Actor](aa-actor.en.md) is one concrete instance inside that system. This page explains the system-level contract.

## System Contract

AAA gives the runtime one reusable way to run bounded execution plans instead of hardcoding every recurring workflow into a dedicated pallet.

The normative system contract requires:

- Deterministic scheduling with durable late, paused, cooldown, and pre-window signals;
- Balance/event-driven triggering through direct producer-owned adapters without an event scanner or deferred compatibility queue;
- Two-dimensional RefTime and ProofSize admission before each housekeeping, queue, wakeup, close, or cycle operation, including a generated fixed hook base before any `on_idle` storage access;
- Typed tasks such as transfer, swap, liquidity, burn, mint, stake, and unstake;
- Lifecycle rules for identity-only dormancy, atomic activation/deactivation, pause, failure, auto-close, manual close, and mandatory internal terminal transitions;
- Adapter boundaries with runtime-derived worst-case weights so AAA orchestrates mechanics without owning DEX, staking, or asset logic.

Actor balances can function like trigger messages: an asset arriving on an actor account can wake the next bounded execution plan, and that pending signal must retain a bounded path to eventual eligibility. Manual and matched address events coalesce through the single `ActorHot.pending_signal` latch; admitted execution clears it atomically while deferral, pause, and scheduler movement preserve it.

Funding uses ordinary inbound transfers rather than a dedicated value-transfer call. Pallet-owned source policy or the default-deny `FundingAuthority` decides whether a tracked transfer activates or accumulates a two-stage funding batch; rejected, source-less, and post-expiry deposits remain spendable balance-only donations. Each supported producer preflights before value movement and submits one direct fallible notification in the same transaction, so overflow rolls back rather than silently losing funding state. Armed funding stays frozen for the logical run, pending funding promotes only after full success, and bounded events expose activation, accumulation, promotion, and policy updates.

## Progress-Preserving Continuation

A Mutable actor may mark a step `RetryLater`. When that step reports an explicitly Temporary adapter failure, AAA keeps one sparse Continuation with the unresolved step cursor, attempt number, last-attempt block, frozen typed suffix inputs, and cumulative outcomes. Retries reuse the same logical-run nonce and the existing FIFO/wakeup scheduler. They start at the unresolved step instead of replaying the committed prefix.

Permanent and unsupported-adapter failures never create Continuation. Immutable actors cannot use `RetryLater`. Cancellation deletes current progress without compensation, prefix rollback, funding promotion, or balance movement; pause and the global breaker preserve it. Incoming signals during suspension remain latched for the next logical run.

`CycleStarted` appears once. `CycleContinued` and `CycleSuspended` identify attempts with `(aaa_id, cycle_nonce, attempt)`, while one cumulative `CycleSummary` terminates the logical run. Current Continuation is canonical chain state. Long attempt timelines require a materialized event index.

## Operational Observability

AAA keeps current starvation observability sparse. `IdleStarvationState` is absent/Healthy during normal operation, becomes `Starving { since }` on the first exhausted post-housekeeping budget, and becomes `Alerted { since }` at the configured threshold. Duration derives from block number, so unchanged starving or alerted blocks do not rewrite a counter.

`IdleStarvationDetected` and `IdleStarvationRecovered` each emit once per alerted interval. The current phase is canonical chain state; long-term alert history and duration trends belong in an indexed view built from those events. The production-Wasm healthy-empty probe confirms five reads and zero writes.

## Embedding Boundary

External runtimes can reuse `pallet-aaa` without inheriting the DEOS/TMCTOL System actor catalog. The host runtime provides bounded adapters for assets, caller-aware DEX quotes, staking shares, liquidity donation, funding authority, atomic fee collection, fallible ingress, and two-dimensional task weights. AAA owns scheduling, lifecycle, policy-aware amount resolution, fee reservation, and task orchestration. After read-only evaluation, each attempted User step calls `FeeCollector` at most once: non-executable outcomes charge evaluation-only, while executable outcomes charge evaluation plus execution together. The collector transfers the full charge into `FeeSink`; downstream allocation remains outside AAA. The DEOS reference Fee Sink currently applies the 50/50 staking/liquidity plan; equal security/staking/liquidity thirds remain gated on permissionless collators and bounded security settlement.

The independent `template/pallets/aaa/embedding-runtime` external-consumer fixture makes this boundary executable. It starts with zero System AAAs, uses local account/asset types and smaller scheduler pages, and proves direct Executive ingress, fresh-genesis integrity, deterministic unsupported adapters, User/System Continuation, User exact-output swaps, System-only minting, try-state, and no-std operation. It is portability evidence, not a second product or prescribed topology.

The `0.7.3` line keeps the unlaunched reference chain at fresh-baseline storage version `1`; it ships no historical migration. The independent Continuation embedding gate passes without a DEOS/TMCTOL helper or actor-topology dependency.

The DEOS reference runtime also owns `LpPairByTokenId` outside generic AAA, so liquidity removal resolves one exact LP-to-pair entry instead of scanning pools. Internal adapters and the transaction extension maintain that index when pools are created or first funded.

The atomicity guarantee is task-scoped, not whole-plan scoped. If an adapter fails after partial mutation, the failed task rolls back its local effects and success event; earlier successful steps remain committed. `ContinueNextStep`, `AbortCycle`, or Mutable-only `RetryLater` then decides whether the attempt proceeds, terminates, or suspends at the same step.

## Control-Plane Boundary

Off-chain tooling binds an executable plan to genesis hash, runtime versions, metadata hash, actor type, mutability, and exact `ProgramInput` SCALE bytes. Human JSON is a lossless projection, not runtime truth. A deterministic `planId` supports review and correlation while metadata changes require explicit decode, validation, and re-encoding.

Plan diffs, forecasts, simulations, governance composition, and long configuration/cycle history remain local or materialized surfaces. They carry block, metadata, and model provenance and never expand consensus state or authorize signing implicitly.

## Portability Boundary

The current staking contract is intentionally generic:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

AAA does not encode DEOS-specific `StakeNative`, collator selection, `stNTVE` naming, or `NTVE/stNTVE` LP custody. Runtime adapters decide what a generic staking position means, expose its share balance, and optionally map it to a transferable share asset for last-funding resolution. In DEOS, the adapter routes native staking into `pallet-staking::stake_native`, while nomination security remains a separate locked-LP staking/governance surface.

This keeps AAA useful outside one tokenomic configuration.

## Current DEOS Role

On the current reference line, AAA is the execution substrate for runtime-side protocol behavior: burning, liquidity provisioning, treasury splitting, bucket handling, BLDR lane flows, and native staking LP provisioning.

The shipped runtime reserves fifteen deterministic System addresses but enrolls only three active programs at genesis: Burn Actor, Fee Sink, and BLDR Splitter. These programs react to verified inbound value rather than periodic polling. Ten Mutable System identities start dormant with no plan, funding, fee, queue, wakeup, or cycle state. Activation accepts one typed active-program input with an explicit schedule, run plan, and funding policy, and validates it before enrollment. The two permanent Bucket A anchors remain custody-only deterministic accounts outside generic actor storage. Native staking LP provisioning can activate only after the receipt asset, staking pool, dormant identity, and non-empty `NTVE/stNTVE` AMM are ready.

AAA does not replace TMC, Axial Router, staking, or governance. Those subsystems own math and domain rules. AAA gives them a deterministic way to be orchestrated together.

## Why It Exists

Without AAA, recurring economic workflows would keep becoming bespoke pallet logic. AAA makes those workflows explicit, bounded, governable, and composable as typed actor graphs.

One actor's balance outflow can become another actor's trigger message. Larger protocol behavior can therefore emerge from small bounded parts while still running inside deterministic scheduling and execution limits.

Within the existing task and adapter language, many workflow/topology changes can move from runtime rewrites into on-chain actor-graph configuration. Runtime upgrades remain necessary for new primitives, adapter surfaces, or safety invariants.

## Related

- [AA-Actor](aa-actor.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance Overview](governance-overview.en.md)
- [Core Terms](../glossary/core-terms.en.md)
