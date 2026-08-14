---
page_type: overview
title: Actors System
summary: Actors is the Account Abstraction Actors system in DEOS — the pallet, scheduler, lifecycle rules, fee model, and deterministic execution environment that host individual actors while keeping domain logic in adapters and pallets.
locale: en
canonical_page_id: actor-system
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../template/pallets/actors/docs/specification.en.md
  - ../../template/pallets/actors/docs/architecture.en.md
  - ../../docs/actors.integration.en.md
  - ../../docs/oracle.integration.en.md
  - ../../docs/actors-control-plane.contract.en.md
  - ../../template/pallets/actors/docs/embedding.md
  - ../../docs/core.architecture.en.md
status: active
audience: newcomer
tags:
  - overview
  - actor
  - runtime
  - automation
related:
  - AA-Actor
  - Typed Observations
  - Token-Driven Automation
  - Routing and Minting Loop
  - Governance
  - Core Terms
last_compiled: 2026-07-27
confidence: 0.95
---

# Actors System

## Summary

`Actors` means `Account Abstraction Actors`. In DEOS, it names the whole runtime system: `pallet-deos-actor`, scheduler, lifecycle rules, fee model, actor accounts, and typed execution environment for bounded protocol flows.

An [AA-Actor](actor.en.md) is one concrete instance inside that system. This page explains the system-level contract.

## System Contract

Actors gives the runtime one reusable way to run bounded execution plans instead of hardcoding every recurring workflow into a dedicated pallet.

The normative system contract requires:

- Deterministic scheduling with durable late, paused, cooldown, and pre-window signals; one strict global-ticket FIFO where a live or weight-blocked head holds the only execution offer and light followers keep their exact physical order until it advances. The overdue-wakeup and observation-fanout workers each stop at their own two-dimensional weight ceiling, leaving actor service the remaining budget without guarantee lending. One placement owner maps immediate readiness, cadence/cooldown/window targets, fixed retry backoff, capacity recovery, and terminal expiry to the FIFO or an exact wakeup, and the post-worker cutoff guarantees at most one execution per actor per block. Physical queue occupancy counts tombstones until drained, and namespace overflow fails closed without losing an actor's existing ticket or pointer.
- Balance/event-driven triggering through direct producer-owned adapters without an event scanner or deferred compatibility queue;
- Two-dimensional RefTime and ProofSize admission before each housekeeping, queue, wakeup, close, or cycle operation, including a generated fixed hook base before any `on_idle` storage access;
- Typed tasks such as transfer, swap, liquidity, burn, mint, stake, and unstake;
- Lifecycle rules for identity-only dormancy, atomic activation/deactivation, pause, failure, auto-close, manual close, and mandatory internal terminal transitions;
- Adapter boundaries with runtime-derived worst-case weights so Actors orchestrates mechanics without owning DEX, staking, or asset logic.

Actor balances can function like trigger messages: an asset arriving on an actor account can wake the next bounded execution plan, and that pending signal must retain a bounded path to eventual eligibility. Manual and matched address events coalesce through the single `ActorHot.pending_signal` latch; admitted execution clears it atomically while deferral, pause, and scheduler movement preserve it. `OnObservationChange { feed }` now declares reconsideration against one exact typed feed without threshold, callback, payload, or per-revision execution semantics; Actors derives a duplicate-free bounded subscriber index from that source, and the Oracle hook now coalesces only the latest changed revision into reusable dirty-feed state without reading subscribers. An independently metered deferred worker traverses bounded subscriber pages, binds that state to the same latch, and leaves execution to the existing queue/wakeup scheduler. Observation, Manual, periodic-only, and mixed source sets cannot author `PercentageOfTrigger`; only applicable AddressEvent-only sources establish its exact cycle-start balance snapshot contract.

The Automation Observe view reads the bounded feed registry and one selected feed at a finalized block. It shows exact directional identity, scale-formatted scalar, producer/provenance, aggregation, lifecycle, update block, revision, authored age, and Fresh/Stale/Unavailable/Uninitialized status. It also states latest-state coalescing and warns that local pre-execution pool reserves provide neither external fair price nor MEV/ordering protection; history remains materialized.

Funding uses ordinary inbound transfers rather than a dedicated value-transfer call. Pallet-owned source policy or the default-deny `FundingAuthority` decides whether a tracked transfer accumulates for the actor; rejected or source-less deposits remain spendable balance-only donations, while post-expiry ingress closes the actor inline without recording funding. Each supported producer preflights before value movement and submits one direct fallible notification in the same transaction, so overflow rolls back rather than silently losing funding state. A fresh cycle freezes the accepted accumulator into its opening snapshot, and Continuation preserves that snapshot until the cycle terminates.

## Verifiable Straight-Line Composition

Each plan remains an ordered list of typed steps. A step uses `Preconditions::Unconditional` or bounded `AnyOf` DNF: outer clauses compose with OR and timed predicates inside each clause compose with AND. Every admitted predicate is visited, any predicate error fails the whole expression, and false preconditions only advance to the next fixed step. `Opening` results freeze for the logical cycle and survive Continuation; `Current` reads run immediately before the step and observe successful earlier effects. No precondition introduces jumps, loops, callbacks, arbitrary calls, or authored successors.

Fieldless `StopCycle` provides one explicit successful terminal operation after condition evaluation and ordinary User fee collection. It commits no task-local economic effect and leaves the suffix unreachable. Its pre-execution failures still obey `on_error`: `ContinueNextStep` can bypass the intended stop, execute the suffix, and later reach ordinary success, so authoring and analysis expose that fall-through.

An Active Actor Contract chooses `Persistent` or `CloseAfterProductiveCycle`. The latter closes only after successful logical-cycle completion with at least one committed effectful task, including a committed prefix resumed through Continuation. False latest-state conditions, skips, rolled-back failures, suspension, abort, retry exhaustion, and bare `StopCycle` leave the one-shot policy unconsumed.

Canonical SCALE `ContractInput` remains the source of truth across metadata-bound authoring, structural diff, static analysis, simulation, and governance composition. Visual blocks or neural proposals may project or propose this finite AST, but deterministic validation, human approval, encoding, and runtime execution stay authoritative.

The control-plane corpus models descending buy and ascending sell buckets as independent bounded one-shot actors over directional local-pool observations. It does not mislabel those price feeds as treasury reserve ratios or absolute liquidity depth: only the manual execution cores exist for those reactions until typed producers and meanings ship. A block-height release demonstrates an available non-price scalar strategy using runtime-owned current block truth.

## Progress-Preserving Continuation

A Mutable actor may mark a step `RetryLater { max_attempts }` with a nonzero `u32` limit. Temporary adapter failure or unavailable funding increments the unsuccessful-attempt count at the unresolved cursor; the initial failure counts as `1`, and advancing to a later cursor resets the local count. Actors keeps one sparse Continuation with that count, the logical-cycle-wide attempt, last-attempt block, frozen typed suffix inputs, and cumulative outcomes.

Retries reuse the same logical-cycle nonce and FIFO/wakeup scheduler. They start at the unresolved step instead of replaying the committed prefix. Reaching the local limit closes the actor with `RetryAttemptsExhausted`; a simultaneous actor-wide failure cutoff does not replace that more precise reason.

Permanent and unsupported-adapter failures never create Continuation. Immutable actors cannot use bounded `RetryLater`. Cancellation deletes current progress without compensation, prefix rollback, funding promotion, or balance movement; pause and the global breaker preserve it. Incoming signals during suspension remain latched for the next logical cycle.

`CycleStarted` appears once. `CycleContinued` and `CycleSuspended` identify attempts with `(actor_id, cycle_nonce, attempt)`, while one cumulative `CycleSummary` terminates the logical cycle. Current Continuation is canonical chain state. Long attempt timelines require a materialized event index.

## Operational Observability

Actors keeps current starvation observability sparse. `IdleStarvationState` is absent/Healthy during normal operation. With the breaker inactive it becomes `Starving { consecutive_blocks }` when live FIFO work remains, no attempt commits, and the actor pass stops on Weight, fee collection, or an invariant; it becomes `Alerted { consecutive_blocks }` at the configured threshold. No live work, a committed attempt, pass exhaustion, or breaker activation clears or freezes state as specified, so an empty or tombstone-only queue with exhausted budget does not count as starvation.

`IdleStarvationDetected` and `IdleStarvationRecovered` each emit once per alerted interval. The current phase is canonical chain state; long-term alert history and duration trends belong in an indexed view built from those events. The production-Wasm healthy-empty probe confirms five reads and zero writes. Distinct wakeup blocks live in a paged binary min-heap with exact reverse indices. Insert, pop-min, and exact removal use at most `ceil(log2(MaxActiveActors))` sift steps, covered by maximum-depth generated benchmarks.

Every DEOS System swap also applies a local reference-deviation guard. A nonzero EMA remains eligible through age 100 blocks; zero, missing, or older EMA falls back to the direct-pool reserve ratio, and absence of both references fails Temporary before mutation. The observed pool may already have been manipulated, so this guard proves neither external fair price nor transaction-order protection and does not replace task slippage or output bounds.

Finalized reactive inspection follows the selected dirty feed's exact active-list position and occupied subscriber-page links. Numerical delivery estimates appear only when runtime code, V16 metadata, constants, production weights, descriptors, and topology share one finalized evidence identity. `EvidenceMismatch` withholds those estimates while preserving factual Oracle, Actors, queue, wakeup, and snapshot state.

The read-only `ActorEligibilityApi::actor_eligibility` projection reports current readiness, the scheduler-owned phase, and the next eligible block at one finalized block, reusing the same pure cadence/cooldown/window/backoff/breaker/latch owners as admission. Clients never reimplement that arithmetic; the projection never promises service, because queue position and available Weight decide actual admission.

Off-chain feedback analysis separates observation-caused recurrence from shared account, pool, reserve, or TMC resources. Only evidence-addressable reactive edges can form reactive-cycle findings; resource coupling remains a separate unscored advisory signal with unknown causal and economic significance. This analysis cannot reject plans, alter scheduling, or claim execution.

## Embedding Boundary

External runtimes can reuse `pallet-deos-actor` without inheriting the DEOS/TMCTOL System actor catalog. The host runtime provides bounded adapters for assets, caller-aware DEX quotes, staking shares, liquidity donation, funding authority, atomic fee collection, fallible ingress, and two-dimensional task weights. Actors owns scheduling, lifecycle, policy-aware amount resolution, fee reservation, and task orchestration. After read-only evaluation, each attempted User step calls `FeeCollector` at most once: non-executable outcomes charge evaluation-only, while executable outcomes charge evaluation plus execution together. The collector transfers the full charge into `FeeSink`; downstream allocation remains outside Actors. The DEOS reference Fee Sink currently applies the 50/50 staking/liquidity plan; equal security/staking/liquidity thirds remain gated on permissionless collators and bounded security settlement.

The independent `template/pallets/actors/embedding-runtime` external-consumer fixture makes this boundary executable. It starts with zero System Actors, uses local account/asset types and smaller scheduler pages, and proves direct Executive ingress, fresh-genesis integrity, deterministic unsupported adapters, User/System Continuation, User exact-output swaps, System-only minting, try-state, and no-std operation. It is portability evidence, not a second product or prescribed topology.

The unlaunched reference chain keeps fresh-baseline storage version `1` and ships no historical migration. The independent embedding gate executes `Unconditional`, bounded DNF, `StopCycle`, and Continuation behavior without a DEOS/TMCTOL helper or actor-topology dependency.

The DEOS reference runtime also owns `LpPairByTokenId` outside generic Actors, so liquidity removal resolves one exact LP-to-pair entry instead of scanning pools. Internal adapters and the transaction extension maintain that index when pools are created or first funded. Both authored minimum outputs reach Asset Conversion directly; an outer transactional balance-delta check remains as defense in depth.

The atomicity guarantee is task-scoped, not whole-plan scoped. If an adapter fails after partial mutation, the failed task rolls back its local effects and success event; earlier successful steps remain committed. `SplitTransfer` preflights every non-zero recipient before mutation; one deposit-ineligible leg fails the whole task Temporary, while successful retained value contains only undeclared share and integer-division dust. `ContinueNextStep`, `AbortCycle`, or Mutable-only bounded `RetryLater` then decides whether the attempt proceeds, terminates, or suspends at the same step.

## Control-Plane Boundary

Off-chain tooling binds an executable plan to genesis hash, runtime versions, metadata hash, actor type, mutability, and exact `ContractInput` SCALE bytes. Human JSON is a lossless projection, not runtime truth. A deterministic `contractId` supports review and correlation while metadata changes require explicit decode, validation, and re-encoding.

Plan diffs, forecasts, simulations, governance composition, and long configuration/cycle history remain local or materialized surfaces. They carry block, metadata, and model provenance and never expand consensus state or authorize signing implicitly.

## Portability Boundary

The current staking contract is intentionally generic:

```text
Task::Stake { asset, amount }
Task::Unstake { asset, shares }
```

Actors does not encode DEOS-specific `StakeNative`, collator selection, `stNTVE` naming, or `NTVE/stNTVE` LP custody. Runtime adapters decide what a generic staking position means, expose its share balance, and optionally map it to a transferable share asset for last-funding resolution. That share-asset identity remains stable for the admitted position key; a runtime upgrade must introduce a new key rather than reinterpret an active plan. Execution fails closed if the mapping disappears. In DEOS, the adapter routes native staking into `pallet-staking::stake_native`, while nomination security remains a separate locked-LP staking/governance surface.

This keeps Actors useful outside one tokenomic configuration.

## Current DEOS Role

On the current reference line, Actors is the execution substrate for runtime-side protocol behavior: burning, liquidity provisioning, treasury splitting, bucket handling, BLDR lane flows, and native staking LP provisioning.

The shipped runtime reserves fifteen deterministic System addresses but enrolls only three active Actor Contracts at genesis: Burn Actor, Fee Sink, and BLDR Splitter. These contracts react to verified inbound value rather than periodic polling. Ten Mutable System identities start dormant with no plan, funding, fee, queue, wakeup, or cycle state. Activation accepts one typed active-contract input with an explicit schedule, cycle plan, and funding policy, and validates it before enrollment. The two permanent Bucket A anchors remain custody-only deterministic accounts outside generic actor storage. Native staking LP provisioning can activate only after the receipt asset, staking pool, dormant identity, and non-empty `NTVE/stNTVE` AMM are ready.

Actors does not replace TMC, DEOS Router, DEOS Staking, or DEOS Governance. Those subsystems own math and domain rules. Actors gives them a deterministic way to be orchestrated together.

## Why It Exists

Without Actors, recurring economic workflows would keep becoming bespoke pallet logic. Actors makes those workflows explicit, bounded, governable, and composable as typed actor graphs.

One actor's balance outflow can become another actor's trigger message. Larger protocol behavior can therefore emerge from small bounded parts while still running inside deterministic scheduling and execution limits.

Within the existing task and adapter language, many workflow/topology changes can move from runtime rewrites into on-chain actor-graph configuration. Runtime upgrades remain necessary for new primitives, adapter surfaces, or safety invariants.

## Related

- [AA-Actor](actor.en.md)
- [Typed Observations](typed-observations.en.md)
- [Token-Driven Automation](../concepts/token-driven-automation.en.md)
- [Routing and Minting Loop](../concepts/routing-and-minting-loop.en.md)
- [Governance](governance.en.md)
- [Core Terms](../glossary/core-terms.en.md)
