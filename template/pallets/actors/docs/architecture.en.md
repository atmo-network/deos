# DEOS Actors Package Architecture

> Package: `pallet-deos-actors`; Rust crate: `pallet_deos_actors`

This document maps the independently reusable crate implementation. A host supplies `PalletId`, account derivation context, origins, adapters, bounds, fees, genesis actors, and production weights. Concrete DEOS namespace, System accounts, TMCTOL plans, runtime adapters, and operational evidence belong in [`docs/actors.integration.en.md`](../../../../docs/actors.integration.en.md).

## Executive Summary

> This document records the shipped package implementation; the standalone specification owns normative semantics and executable tests own conformance.

`pallet-deos-actors` provides a deterministic scheduler, bounded execution model, typed trigger system, lifecycle state machine, and adapter-driven task runtime for User and System actors.

The crate assigns no economic roles, assets, recipients, routes, actor IDs, or chain policy. Host behavior enters through typed adapters, origins, account derivation, weight/fee conversion, explicit bounds, and genesis actor specifications. External obligations live in the [package-owned embedding guide](./embedding.md).

## Architecture Overview

### Design Principles

1. `Deterministic scheduling`: one monotonic paged FIFO plus exact paged temporal wakeups, deterministic ordering, and explicit per-block caps
2. `Execution safety`: exact current-Step control/effect Weight admission, complete Pipeline charging at Opening, and Action-only fee settlement around invocation
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`CycleAdmissionInsufficient`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `Hot-state decomposition`: `ActorHot` carries only measured scheduler/admission facts needed to avoid cold contract or detailed funding decode; the retired synchronized readiness mirror must not return

Canonical writes target `ActorIdentity`, `ActorHot`, `ActorContractHead`, `ActorAdmissionCertificate`, bounded `ActorContractTailChunk`, and `ActorFunding` directly. `ActorFunding` stores only bounded tracked-asset funding state; no Trigger-family amount or fee reserve is persisted there. `ActorStateHolds` records one named six-component refundable geometry quote per User Actor under `HoldReason::ActorState`; its run component reserves the type-derived maximum for the Active installed lifetime, so autonomous Opening never needs a new owner hold. Dormant Actors release that component, and System Actors remain host-capacity-backed and hold-exempt.

`TriggerTransitionPlan` is the shared read-only preflight owner for Crossing membership and broad observation subscriptions; it distinguishes genesis installation, Active creation, Dormant activation, Active replacement, deactivation, and close. `crossing.rs` and `subscriptions.rs` return bounded commit inputs, while lifecycle code commits both inside the enclosing control transaction before canonical Contract/Hot replacement. Schedule and placement planning remain separate implementation work.

One crate-private loader reconstructs the certified Contract from those partitions, reads optional `ActorRunState`, and classifies the actor as `NotRegistered`, identity-only `Dormant`, exact `Active`, or `Corrupt`; exact Active requires coherent Identity, Hot, C6 Contract geometry, Funding, and run state exactly while `cycle_state` is `Running` or `Suspended`. Its loaded state has no write-back path, and the public `ActiveActorState` returns the canonical partitions without flattening. No derived context owns authored equality or mutation.

### Host Composition Boundary

Actors executes declarative plans against host-provided adapters. Ledger, market, liquidity, staking, fee, ingress, governance, and genesis policy remain outside the crate. The package never identifies a concrete pallet, asset, actor role, route, or recipient as canonical.

### Type Ownership

`src/types.rs` is the canonical package facade and contains only public re-exports. `src/lib.rs` declares the four owner modules privately, and each owner preserves the existing `pallet_deos_actors::types::*` metadata namespace and crate-root re-export surface; no compatibility alias or second semantic owner exists.

| Module | Owned type families |
| --- | --- |
| `src/types/contract.rs` | Actor Contract, triggers, Steps, tasks, predicates, funding policy, and certified address ingress |
| `src/types/lifecycle.rs` | Identity/class, lifecycle, Continuation, outcomes, classification, simulation, and active read views |
| `src/types/scheduler.rs` | FIFO tickets/pages, wakeup topology, drain statistics, and starvation phase |
| `src/types/observation.rs` | Subscriber pages, observation revisions, and dirty-fanout ownership |

Execution logic remains in `src/execution.rs`, `src/scheduler.rs`, `src/reactions.rs`, and `src/subscriptions.rs`; the type split does not move algorithms or create a mirrored model. `src/contract.rs` remains the semantic classifier and is distinct from the Actor Contract type owner at `src/types/contract.rs`.

## Execution Model

### Actor Classes

| Class | Ownership | Mint task allowed | Typical usage |
| --- | --- | --- | --- |
| `User` | Signed owner + slot namespace | No | User automation |
| `System` | Governance origin | Yes | Protocol automation |

User recovery has an explicit slot-targeted surface: the default `create_user_actor` path allocates the lowest free slot, while `create_user_actor_at_slot` recreates fresh Mutable control for a released slot and therefore derives the same sovereign account. Close never moves custody, and the recovery Contract can use residual native or asset balances without a rescue subsystem.

Current owner-slot representation is fixed-width and runtime-shaped:

- `OwnerSlotBitmaps` stores one `[u8; 32]` bitmap per User owner; System Actors never consume it
- `MaxOwnerSlots` is nonzero and at most `255`; every bit at or above the configured bound remains zero
- Slot `s` maps to byte `s / 8` and little-endian bit `s % 8`
- Default allocation scans the 32 bytes in ascending order for the lowest valid free bit, while exact-slot admission changes one validated bit
- Closing the final User actor deletes its all-zero bitmap

### Current Actor-State Shape

The package stores each actor identity once and decomposes each active epoch into bounded hot, certified Contract, funding, and optional run owners:

- `ActorIdentities`: durable owner, `ActorClass`, mutability, sovereign account, logical-cycle nonce, and non-optional persistent control-mutation block shared by Active and Dormant lifecycle states
- `ActorHot`: typed Active lifecycle/run state, auto-close target, failure counter, `pending_signal`, one queue membership, independent Pipeline-service and Trigger-temporal wakeup pointers, terminal block, `schedule_anchor`, and optional `last_cycle_block`
- `ActorContractHeads`: schedule/completion header, semantic/body/admission commitments, Step count, optional inline Step 0, and its optional control/effect envelope
- `ActorAdmissionCertificates`: compact runtime semantics/layout/Weight and lifecycle identity; encoding is independent of the configured Step/resource ceiling
- `ActorContractTailChunks`: authority-bound gap-free Steps 1..N in chunks of at most four, with one aligned control/effect envelope per Step
- `ActorActivationAuthorities`: fixed ObservationChange activation projections bound to feed, semantic/body/admission identity, cooldown, window, and auto-close nonce
- `ActorFunding`: canonical funding-source policy, bounded tracked assets, and bounded `funding_accumulated[asset]` checked deltas
- `ActorRunHeads`: mutable semantic/body/admission authority, current Step and Opening-predicate cursors, cumulative outcomes, causal commit block, eligibility, suspension facts, and immutable-payload commitment/count
- `ActorRunPayloads`: immutable Opening/funding snapshots retained only while a multi-block Cycle is open

`ActorCreated` carries `actor_id`, owner, `actor_class`, mutability, sovereign account, and `initial_lifecycle`; User slot or System custody locator lives inside `ActorClass`. `actor_id` exists only as each storage-map key. Dormant identity carries no timestamp. Activation or schedule replacement derives block eligibility from `schedule_anchor` and window start, while `ActorHot.trigger_runtime_state::Cadenced` owns the optional timestamp-ceiled anchor tick. The typed lifecycle forbids contradictory pause state.

Package internals reconstruct the generic execution view from `ActorIdentities + ActorHot + certified C6 Contract geometry + ActorFunding + optional ActorRunHeads/ActorRunPayloads`. Observation fanout instead loads the fixed activation authority and mutable run head needed for placement classification; it does not decode generic funding, immutable run payload, admission resource vectors, or unreached Steps. External consumers use `active_actor_state`, which returns canonical semantic partitions without synchronized mirrors.

This is intentionally more concrete than the paired specification: the spec defines the required logical field groups, while this document records the current package storage realization.

### Contract Steps Structure

Each actor admits `0..=MaxContractSteps` ordered Steps and stores them as C6 geometry: optional Step 0 and its optional resource envelope live in the head, while Steps 1..N and aligned envelopes occupy gap-free authority-bound chunks of at most four. A zero-Step Contract has neither inline body nor tail fragments and reconstructs from its certified header alone. One configurable `MaxContractSteps` binding applies identically to User and System actors across creation, activation, replacement, simulation, genesis, and benchmark construction. The configured maximum must remain in `1..=255`; the DEOS production binding is `32`. For nonempty Contracts, the Step-0 control context charges exactly the authored Opening tail-chunk count, from zero for one Step through eight at the production ceiling; later cursors charge no Opening reconstruction. Mutable `RetryLater` admits only `2..=MaxRetryAttempts`, with the protocol-fixed metadata constant set to 10.

- `precondition: Option<Precondition<Predicate, MaxPreconditionClauses, MaxPredicatesPerClause>>`
- `task: Task`
- `on_error: StepErrorPolicy` (`AbortCycle` / `ContinueNextStep` / Mutable-only `RetryLater { max_attempts }`)

`None` is the sole unconditional Step form. `Some(Precondition { clauses })` stores one bounded DNF: outer clauses are OR and each inner clause is AND. Runtime admission rejects empty outer and inner vectors, caps each dimension at four and each step at four total predicates, evaluates every admitted predicate without short-circuit, and executes or skips the step exactly once.

Each `TimedPredicate` names `ObservationTiming::Opening` or `Current`. Fresh-cycle opening evaluates and stores one `Result<bool, PredicateError>` per Opening predicate before any task; ActorRunState retains the full bounded result vector and reuses it by canonical linear position. Current predicates evaluate immediately before their Step and therefore observe committed earlier-Step effects from the same logical multi-block cycle.

Admission sorts predicates and clauses by canonical typed SCALE, removes repeated predicates within a clause, rejects clauses that become semantically identical, absorbs exact predicate-superset clauses, and stores only the canonical form. `update_contract` canonicalizes before equality and returns an exact no-op before rate limiting, cancellation, writes, placement reconstruction, or events when the resulting contract is unchanged.

Current-Step control fees use `evaluation_units = total predicates + Opening predicates`, then chunk the units by `MaxPredicatesPerStep` before calling the benchmarked component. This accounts for opening capture plus full expression visitation without relying on the generated component clamp. A false DNF expression emits `StepSkipped(PreconditionFalse)` and advances one fixed cursor; evaluation errors remain task-independent failures routed through the authored step policy.

`ObservationProvider<FeedId, BlockNumber>` is the generic current-scalar boundary. The host receives `feed`, `now`, and `max_age_blocks`; `Fresh` returns both `value` and `observed_at`. Actors accepts Fresh only when `observed_at <= now` and checked age stays within the authored maximum. Future or over-age Fresh maps to `PredicateError::InvalidObservation`, while explicit Unavailable, Uninitialized, and Stale states produce ordinary false results.

Plan validation rejects zero `max_age_blocks`, fixed-zero and percentage-zero amount resolutions, self-directed Transfer/SplitTransfer recipients, zero absolute input ceilings, zero liquidity minima, and identical swap/liquidity asset pairs. Creation predicts User custody from the first available or explicitly requested slot before fee collection; update and activation validate against the stored sovereign account.

Task set in implementation:

- `Transfer`
- `SplitTransfer`
- `SwapIn`
- `SwapOut`
- `AddLiquidity`
- `RemoveLiquidity`
- `Burn`
- `Mint` (System only)
- `Stake`
- `DonateLiquidity`
- `Unstake`
- `StopCycle` (User/System, fieldless, adapter-free)

### Public Inventory Evidence

`public_reachability_inventory_is_closed_and_canonical` freezes the reviewed SCALE names and order for the listed Actors public families. Semantic-contract tests exhaustively interpret Task, Predicate, amount, and policy families, while focused production, simulation, embedding, metadata, and ABI tests cover the named seams below. This evidence does not claim universal call-graph reachability from every public variant to a production constructor.

`public_api_error_signatures_use_shared_typed_cores` compiler-checks that the eligibility runtime API returns `ActorClassificationError` directly and simulation wraps that core once in `SimulationError`. Its exhaustive classification-to-dispatch match fails compilation when the shared core grows without a mapping. Focused eligibility and simulation tests cover the cited public roots; metadata/spec drift checks freeze Event and pallet Error inventories without asserting universal constructor reachability.

| Family | Retained variants | Reviewed executable evidence |
| --- | --- | --- |
| Predicate | `BalanceAbove`, `BalanceBelow`, `BalanceEquals`, `BalanceNotEquals`, `BlockNumberAbove`, `BlockNumberBelow`, `ObservationAbove`, `ObservationBelow`, `ObservationEquals`, `ObservationNotEquals` | Active contract calls; `every_predicate_is_pure_and_bounded`, predicate evaluator and observation tests |
| Task | `Transfer`, `SplitTransfer`, `SwapIn`, `SwapOut`, `AddLiquidity`, `RemoveLiquidity`, `Burn`, `Mint`, `Stake`, `DonateLiquidity`, `Unstake`, `StopCycle` | Active contract calls; `every_task_has_one_exhaustive_semantic_contract`, task tests, and independent runtime profiles |
| Amount and exact-output bound | `Fixed`, `PercentageOfCurrent`, `PercentageAtOpening`, `PercentageOfLastFunding`, `AllAvailable`; `LiveQuote`, `Absolute` | Task constructors; amount classifier/resolution tests and independent exact-output evidence |
| Trigger | Exactly one of `Manual`, `AddressEvent`, `ObservationChange`, `ObservationCrossing`, `AtTime`, or `Cadenced`; source `Any`, `OwnerOnly`, `Whitelist`; asset `Any`, `Whitelist` | Actor Contract constructors plus raw typed calls; exhaustive Active replacement and Dormant lifecycle matrices; manual, certified-ingress, observation, timestamp-cadence, and embedding tests |
| Funding | `OwnerOnly`, `SignedAllowlist`, `RuntimePolicy`, `AnyVerifiedIngress`; provenance `Signed`, `InternalProtocol`, `Xcm` | Active contract and certified producer constructors; funding-policy package tests and DEOS producer inventory |
| Completion and step policy | `Persistent`, `CloseAfterProductiveCycle`; `AbortCycle`, `ContinueNextStep`, `RetryLater` | Active contract constructors; productive-close and exhaustive transition-matrix tests |
| Attempt and step views | `AttemptDisposition::{Completed, Continued, Failed, Suspended, Closed}`; `StepOutcome::{Executed, Stopped, Skipped, FundingUnavailable, Failed}` | Shared production evaluator; production/simulation parity tests |
| Eligibility view | `NotRegistered`, `Dormant`; Active phases `Ready`, `Paused`, `GlobalCircuitBreaker`, `WaitingSignal`, `WaitingRetry`, `WaitingBlock`, `WaitingCadenceTick` plus terminal reason | `actor_eligibility`; `eligibility_projection_*` tests |
| Cost quote | Named Creation, family-specific Trigger, upfront Pipeline Machine/cleanup, current maximum Action, and refundable state-hold components with independent Weight/admission identities | `actor_cost_quote`; `actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate` |
| Simulation | `FreshCurrentPlan`, `CurrentRun`; every `SimulationError` in specification Section 7.2 | `simulate_current_contract`; package simulation and independent runtime tests |
| Adapter result | `RetryClass::{Permanent, Temporary}`; scalar observation `Unavailable`, `Uninitialized`, `Fresh`, `Stale` | Host adapters and fail-closed unit implementations; runtime Oracle mapping, retry matrix, and embedding tests |
| Event | Every variant in specification Section 8 and runtime metadata | Production `deposit_event` sites; event-order, task, lifecycle, ingress, scheduler, and generated ABI tests |
| Error | Every variant in specification Section 9.2 and runtime metadata | Production `ensure!`/error branches; rejection, rollback, exact metadata/spec, package, and embedding tests |

`CancellationReason::RuntimeUpgrade` and semantic-manifest `ContextDependency::None` were removed because neither had a production constructor. Runtime upgrades remain migration-specific work under specification Section 9.4 rather than a permanently encoded placeholder. Every amount classifier now reports its actual task-policy dependency.

`SwapOut` groups authored output before explicit `InputLimit::{LiveQuote, Absolute(Balance)}` protection. `Absolute(0)` fails before storage; `Absolute(nonzero)` composes its ceiling with live preservable input capacity, while `LiveQuote` intentionally uses that capacity without an authored long-horizon ceiling. `DexOps::swap_exact_out` always receives the resulting finite bound.

Liquidity tasks also carry fixed non-zero outputs. `AddLiquidity.min_lp_out` reaches `LiquidityOps::add_liquidity`; the host adapter must reject a measured LP output below that bound.

`RemoveLiquidity.min_amount_a` and `min_amount_b` pass directly into Asset Conversion. Its two exact withdrawal-minimum errors classify as Temporary; malformed pair identity, missing indexed topology, and unknown downstream failures remain Permanent. The outer adapter transaction retains post-call balance-delta checks as defense in depth, so no success event or partial liquidity mutation survives either enforcement layer.

`StopCycle` executes only after its precondition and ordinary User fee collection succeed. The canonical evaluator produces `StepOutcome::Stopped`, production emits `CycleStopped { actor_id, cycle_nonce, step_index }`, and the shared policy interpreter ends the logical cycle successfully at that cursor.

The shared completion path emits the cumulative summary, leaves later funding accumulation untouched, evaluates completion policy and auto-close, clears the persisted run, and leaves the suffix unreachable.

The instruction does not resolve an amount, invoke a runtime adapter, select a successor, or directly mutate actor lifecycle/scheduler state. It increments `executed_steps` but not `committed_effectful_tasks`, so an empty stop cannot close a one-shot productive actor. A false Precondition advances normally; Predicate-evaluation or fee-collection failure occurs before successful stop admission and follows its owning runtime boundary.

### Amount Resolution

The pallet resolves dynamic amounts through `AmountResolution`:

- `Fixed`
- `PercentageOfCurrent`
- `PercentageAtOpening`
- `PercentageOfLastFunding`
- `AllAvailable`

Resolution policy is task-bound in code:

- `PreserveSpend`: applies to Transfer, SplitTransfer, Burn, exact-input swap, liquidity add/remove, Stake, and DonateLiquidity; computes one spend ceiling as adapter-visible balance minus reserved future User fees for the native fee asset and, for User fee-native direct debits, `max(MinUserBalance, asset minimum)`; other assets retain their adapter minimum.
- `DonateLiquidity` resolves only declared `asset_a` as `max_amount_a` and passes the current preservable `asset_b` balance as `max_amount_b`; the host adapter must keep the paired debit within both caps and report exact used amounts. `Fixed`, every percentage basis, `SplitTransfer` total, and `AllAvailable` must stay within that ceiling.
- `ExpendableSpend`: consume available amount where task allows
- `Mint`: amount interpreted in mint context
- `Unstake share spend`: `Fixed`, `PercentageOfCurrent`, `PercentageAtOpening`, and `AllAvailable` resolve against `StakingOps::share_balance(position_asset)` with full share withdrawal allowed; `PercentageOfLastFunding` reads the snapshot keyed by `StakingOps::share_asset(position_asset)`

Resolution outcomes are deterministic:

- `Resolved(value)`
- `Skipped`
- `FundingUnavailable`

`FundingUnavailable` is a deterministic resolution outcome for both actor classes. It advances as a non-terminal skip under `AbortCycle` and `ContinueNextStep`, while valid Mutable `RetryLater { max_attempts }` suspends at the current cursor. It covers missing or zero tracked snapshots, tracked-balance overspend, staking-share overspend, and preserve-spend resolution that would cross the minimum-balance ceiling. Untracked assets remain `SnapshotUnavailable`.

`PipelineMachineEnvelope` stores the complete bounded User control/cleanup quote in the certified Contract head; scheduler admission reads that O(1) authority only when Idle paid readiness is consumable. Zero-Step machine work uses generated `scheduler_inner_zero_step_complete`. Nonempty geometry checked-sums each Step's generated maximum control owner across its authored total attempt count; `StopCycle` folds its control-only effect into machine work and remains Action-fee-free. The cleanup component uses generated `close_actor`, not the broader certificate lifecycle maximum. The production Weight identity commits zero-Step, Opening, retry/error, continuation/RunFrame, completion, placement, and cleanup owners.

Insufficient `MinUserBalance + Pipeline total` selects `CycleAdmissionInsufficient` before Opening, while Running/Suspended never reclassify economically. `StepFeeBreakdown` owns Action effect fees only: non-invoked effects, false Precondition, skipped resolution, and `FundingUnavailable` charge zero; every invoked success or typed failure settles valid actual effect Weight. System settlement stays zero.

`attempt_fee_envelope` and `settle_attempt_fee_step` remain bounded forecast-vector utilities for comparative client evidence; they do not authorize runtime admission or settlement. The package-owned `fee_envelope_vectors` example emits deterministic User/System forecast, release, rollback-pricing, and protected-floor vectors consumed by browser tests.

The User state hold is paid by the identity owner through the runtime's `StateHoldCurrency` under the aggregate `RuntimeHoldReason::Actors(ActorState)` reason. Each present component prices one configured base plus a configured per-byte rate. Identity uses its concrete locator authority; the Active header combines fixed `ActorHot` capacity with the actual C6 head and admission certificate; body uses only present tail chunks; detector uses actor-owned activation/subscription/Crossing/temporal records without shared-page slack; funding and run use their current encoded state. Create, update, ingress, Opening/progress/suspension, cancellation, deactivation, and close reconcile exact per-Actor deltas in the owning storage transaction. Positive-delta failure rolls back semantic state, while close releases the record without touching sovereign custody. TryRuntime rederives every record and compares each owner's aggregate dedicated currency hold.

Resolution and charging follow these rules:

- A User run releases the skipped step's unused execution-fee reservation before resolving later steps, matching every non-executable cycle path.
- A multi-amount task resolves every field before dispatch and selects `FundingUnavailable > Skipped > Executable` independently of field order.
- An Unstake last-funding plan fails validation when the runtime adapter cannot expose a transferable share asset.

Pallet boundary tests cover fixed, current/trigger/last-funding percentages, split totals, and `AllAvailable` across native, sufficient-asset, and staking-share surfaces. The embedding fixture binds unrelated host position keys to share assets without DEOS types.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier Steps in the same Actor Contract remain committed.

`src/contract.rs` is the package-owned semantic classification surface. Exhaustive matches derive task adapter/assets/recipients/effects/availability/weight ownership/bounded algorithms, typed task amount roles with dependency and retry behavior, predicate observations and purity, and error-policy controls.

`TaskWeightOwner::weight` selects the corresponding method on the runtime's single `WeightInfo`; the module adds no codec, storage, runtime API, or parallel numeric weight authority. Package tests instantiate every current primitive, while a new enum variant makes its owning match non-exhaustive. The package example `semantic_manifest` verifies ordered task and amount coverage against SCALE metadata and emits one deterministic format-neutral contract projection.

The control-flow firewall combines closed types with adversarial evidence. `Step` metadata exposes exactly `precondition`, `task`, and `on_error`; exhaustive contracts admit no successor, nested contract, callback, generic `RuntimeCall`, or opaque dispatch field. Optional `Precondition` classification fixes full bounded visitation, whole-expression error, one admitted task, and false advance. Predicates and amount classifiers expose read dependencies and never a control target.

The scheduler service loads canonical Identity, Hot, Funding, Run, admission certificate, hot Contract head, and exactly one certificate-gated current C6 Step; it does not reconstruct an unreached tail for classification or planning. Admission consumes the resulting `CurrentStepPlanOf<T>` control/effect envelope, and FIFO head consumption atomically revalidates ticket/topology authority immediately before the evaluator executes the carried Step and fee authority. [actors/EXP-0013](../../../../.agents/skills/architecture-experiments/tracks/actors/EXP-0013.md) selected loaded-state reuse after reducing cheap-Step marginal RefTime by 17.87% without changing ProofSize or storage-key models.

Fresh multi-Step Opening reconstructs the full body only after admission because immutable Opening snapshots require every authored dependency; one-Step Opening and Running/Suspended service remain head/current-fragment only. Post-placement, enqueue/invalidation, tombstone drain, and wakeup schedule/drain also use the current service state, so unreached-tail corruption cannot block an otherwise authoritative running prefix. General activation, eligibility, detector, funding, and liveness helpers remain separate hot-header conversion work.

The execution kernel visits at most one current Step per actor per block after exact ticket, admission-certificate, resource, fee, and same-block revalidation. A non-terminal commit persists `CycleState::Running`, cumulative outcomes, `last_committed_step_block`, and `eligible_at >= now + 1`; scheduler placement emits one exact successor ticket, while final, abort, suspension, and close branches commit atomically without replaying the prefix.

The execution kernel produces one `StepOutcome` for each visited Step after canonical precondition, amount, fee, and task evaluation. `StepOutcome::Failed(TaskFailure)` retains the concrete `DispatchError` cause and orthogonal `RetryClass`; `resolve_step_control` interprets that outcome with the authored policy without a simulation-only failure vocabulary.

One `AttemptDisposition::{Completed, Continued, Failed, Suspended, Closed}` owns production and simulation meaning; `Continued` commits exactly one non-terminal Step and persists the causal successor cursor. Production emits events and commits state from it; simulation returns the same disposition and final counters while its transaction rolls back. Bounded trace records wrap shared Step outcomes and do not reconstruct task, predicate, amount, fee, failure, or finalization semantics.

`resolve_step_control` remains the only runtime transition owner, with the exhaustive policy/mutability/failure matrix and same-cursor Continuation regressions pinning retry identity. Task-local transactions forbid callback-visible partial mutation. Bounded vectors, adapter capability contracts, generated worst-case weights, maximum-plan tests, and circular scheduler stress bound adapter and cross-actor work without interpreting local plans recursively.

### Market Adapter Boundary

`DexOps` owns swap-only host execution; `LiquidityOps` owns add, remove, and donation operations. Actors supplies `ExecutionContext { actor, actor_type }`, resolved amounts, and authored spend/output bounds without knowing route topology, market identity, or price-source policy.

DEX adapters return `DexSwapOutcome { total_amount_in, recipient_amount_out }`. Actors validates those committed facts against the authored exact-input or exact-output bound before emitting its task event.

The DEOS runtime benchmark helper prepares two Local/Native pools so both DEX benchmarks execute the maximum Native-anchored Router class rather than a cheaper direct route.

Adapters return typed task failures. Only explicitly classified Temporary failures may enter Mutable `RetryLater`; unknown downstream errors remain Permanent. Task-local transactions roll back adapter mutations before step policy runs.

Host-specific quotes, route selection, fees, oracle guards, slippage policy, and failure mapping belong in the integration architecture and embedding evidence.

Under `runtime-benchmarks`, the opaque `MaximumContextInherent` helper fixture is prepared outside measurement, dispatched inside `maximum_context_inherent`, and verified afterward. This gives the host runtime one complete maximum-context falsification owner without importing Cumulus payload or relay-proof types into the reusable package contract.

## Scheduler Architecture

### Hook Separation

- `on_initialize` performs no Actor work; the mandatory inherent is the sole pre-external phase owner.
- `actor_prepass` inherent:
  - is payload-free, versioned at the provider-data boundary, Mandatory, fee-free, and required under the runtime inherent contract
  - consults the host `PrepassContext` and rejects before mutation unless required Timestamp and parachain consensus context are present
  - opens and settles the generated cutoff-capture owner against Actor Control
  - freezes `PrepassExecutionCutoff`, performs one bounded saturated-FIFO stale-cleanup quantum, services the rotated wakeup/Crossing/fanout materialization families against reserved Actor Control, and keeps every newly materialized ticket behind that cutoff
  - executes the mandatory strict-FIFO base pass against remaining Actor Control and `ActorBaseTurn`, then advances to `ExternalPhase`
  - rejects duplicate or stale execution without moving the accepted cutoff
- `on_idle`:
  - derives finalization and Drain capacity from current-block Actor Control remaining after Prepass
  - pre-reserves the remaining Drain-control maximum and settles generated actual control before final reconciliation
  - rejects stale, wrong-phase, or optional-halted resource state before housekeeping mutation
  - advances `ExternalPhase -> FreshDrain`, meters queue service against remaining Actor Control and Shared Economic effect capacity, writes the latest non-authoritative finalized snapshot, and finishes successful reconciliation in `Finalizable`
- `on_finalize`:
  - consumes and requires the current block's one-pass `Finalizable` marker, zero outstanding reservations, and matching telemetry block tag
  - makes any incomplete resource protocol consensus-invalid rather than silently carrying state into the next block

### Admission Gates

A cycle is admitted only when all checks pass:

1. actor is ready (`trigger`, cooldown, pause/breaker/window checks)
2. per-block execution cap (`MaxExecutionsPerBlock`) not exceeded
3. a two-dimensional `WeightMeter` can consume the complete attempt plus measured pure-cleanup weight without exceeding RefTime or ProofSize
4. for a User Actor: fee preflight covers the opening plan or unresolved retry suffix plus `MinUserBalance`

Attempt Weight and User fees are derived from the current bounded contract at every use. Fresh attempts scan the full plan, while suspended attempts scan only the bounded `cursor..plan.len()` suffix and compose generated retry/suffix-admission classes.

Weight or scan deferral remains silent and state-preserving: no candidate identity, event, nonce, cursor, funding snapshot, or task effect changes. Persistent live-head Weight blockage, fee-collection failure, or invariant stall becomes observable only through sparse starvation transition events.

Deferral/terminal paths:

- insufficient Weight or scan budget → silent state-preserving deferral; actor remains active
- pure terminal cleanup prechecks every fallible identity, funding, count, reverse-index, and User-slot invariant before mutation; no close retry or requeue state exists
- Pre-cycle close precedence is deterministic: `WindowExpired` precedes `CycleAdmissionInsufficient`, then independently owned nonce/failure/lifecycle terminals
- Paid readiness that cannot preserve `MinUserBalance` while paying complete Pipeline Machine/cleanup → terminal `CycleAdmissionInsufficient` process cleanup with custody untouched
- `CycleResult::Completed` means authored control reached terminal without an abort: skip-only and all-failed-`ContinueNextStep` runs remain Completed, reset `unsuccessful_attempt_streak`, and may satisfy nonce auto-close. Their counters remain factual; only at least one committed non-`StopCycle` task satisfies productive close. Abort emits `Failed`; explicit invalidation emits `Cancelled`.
- Post-failure close is inclusive at `unsuccessful_attempt_streak >= MaxConsecutiveFailures`; an admitted cycle emits its authoritative `CycleSummary` before pure cleanup emits `ActorClosed`, matching post-success `AutoCloseNonceReached` ordering
- Explicit, automatic, lifecycle-touch, dormant, and sweep paths share one pure cleanup routine: no Task, Precondition, fee, funding restoration, sovereign-balance movement, or shared queue/wakeup scan occurs
- Signed User owner close requires `Mutable`; User Immutable rejects pause, resume, semantic Contract replacement, auto-close replacement, deactivation, cancellation, and close while still admitting its authored Manual source. Scheduler, sweep, authored completion, auto-close, window, and Pipeline-admission terminals remain runtime-owned.
- Active close prevalidates identity, counts, reverse ownership, slot ownership, funding presence, and subscriptions, then commits cancellation, actor-store deletion, counter/locator release, and the close event in one storage transaction; any residual late error rolls back the complete terminal mutation
- Package and runtime tests preserve native and non-native residual custody across Mutable owner close, productive close, User Immutable zero-Step AtTime auto-close, and Pipeline-admission apoptosis; exact-slot recreation receives a fresh actor id and nonce but executes against the same sovereign account
- Pipeline-admission insufficiency meters generated `pipeline_admission_apoptosis` cleanup at `161,616,000 / 5,736` with 15 reads and 15 writes for maximum Contract deletion without Crossing, ObservationChange, temporal, or Run topology. Other terminal paths and the economically collected cleanup upper remain bound to conservative `close_actor`; the narrow owner changes scheduler admission only.
- Removing `ActorHot` lazily invalidates its ticket and wakeup pointer; bounded stale records converge through ordinary page draining
- Every bounded window validates checked `end + 1` representability, rejects overflow, and schedules that exact terminal block through `ActorHot.terminal_at`; trigger and terminal readiness share one live wakeup pointer and retain the earlier target
- Paused actors remain hot-only before terminal time and load `ActorContract` only when closure is due
- With `GlobalCircuitBreaker` active, normal cycles and scheduler-owned terminal cleanup defer; bounded housekeeping plus explicit lifecycle/sweep cleanup remain available

`ActorContract.completion_policy` defaults to `Persistent`. `CloseAfterProductiveCycle` checks cumulative `committed_effectful_tasks` only after successful logical-cycle completion, including a resumed Continuation. False latest-state Precondition results, skipped Steps, rolled-back failures, bare `StopCycle`, suspension, abort, cancellation, and retry exhaustion cannot select `ProductiveCycleCompleted`. The pure close path remains valid for Immutable System actors and preserves their sovereign balances.

Code anchor: `src/execution.rs::execute_single_cycle_traced` increments the cumulative count after task commit and applies productive close after `CycleSummary`. Pallet tests prefixed `close_after_productive_cycle_` falsify false-state, latest-state race, bare stop, retry, exhaustion, balance, and Immutable closure claims.

Lifecycle lease-by-cycles is authored by `ActorContract.auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, the actor closes with `AutoCloseNonceReached`. Complete Mutable Contract replacement may set, shorten, extend, or clear the target; every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`. No field-specific setter or increment call exists.

### Fee Collection Boundary

The generic pallet collects creation, successfully processed Manual, AddressEvent, ObservationChange, and ObservationCrossing-fire Trigger occurrences, per-admitted-cycle Pipeline Machine/cleanup, and per-invoked-Action User fees through one runtime-supplied `FeeCollector`; Crossing rearm and terminal cleanup charge no Actors fee. Both User creation calls collect `ActorCreationFee` for Active and Dormant admission before identity, slot, counter, or next-id mutation; failed collection leaves every actor store and owner balance unchanged. System creation remains exempt.

Actors invokes the runtime-supplied collector exactly once only when one Trigger changes `pending_signal` from false to true. Manual, matching AddressEvent, ObservationChange, fired ObservationCrossing, AtTime, and Cadenced use their generated family owner and emit `TriggerOccurrenceProcessed` only for that useful transition. An already-latched occurrence performs no Actor-specific fee, event, activation, or causal-history update. Trigger underfunding rolls back queue/latch mutation without closing the process, except that a consumed one-shot User `AtTime` closes through prepaid fee-free minimal apoptosis with `TriggerAdmissionInsufficient`. Fee-collector infrastructure failure rolls back consumption and retains the wakeup. Collection is ledger-only: it performs no Actors ingress preflight, funding accumulation, readiness mutation, or scheduler placement. Zero collection is a no-op, and collector failure rolls back movement and all Actors state.

- Predicate and Task preparation run read-only before collection determines the Step outcome.
- Idle readiness consumption first charges the fixed-size certified Pipeline Machine/cleanup total and emits `PipelineFeeCharged`; collection failure rolls back queue/latch mutation and preserves the live head. Current-Step admission still meters control/effect Weight component-wise but reserves only the Action-effect maximum economically. Before a Suspended User retry, the scheduler proves the current maximum Action liability above `MinUserBalance`; insufficiency closes through fee-free minimal apoptosis before invocation, while the enclosing Step transaction settles valid actual cost and releases the remainder. False Precondition, skipped resolution, `FundingUnavailable`, and `StopCycle` charge no Action fee; an invoked success or typed failure settles valid actual effect Weight and appends `ActionFeeCharged` after semantic boundary events as the attempt's final economic receipt.
- Collection failure rolls back the complete scheduler transaction, including provisional task dispatch, placement, close, queue consumption, and every fee, event, counter, nonce, cursor, or snapshot mutation. Missing or excessive actual evidence fails before collection and rolls back the same atom. Simulation reports interface-local `FeeCollectionFailed`.
- An invoked adapter failure reports `Invoked` effect evidence and retains its valid actual effect fee when the enclosing attempt and collection commit; `ContinueNextStep` and `AbortCycle` never alter that charge or trigger another collection. Zero actual total fee produces no collector call.
- `TaskEffectWeightProvider` owns maximum admission and one closed post-dispatch branch: `NotInvoked` returns zero, while `Invoked` returns the canonical generated Task-family Weight whether the operation commits or returns typed failure. Scheduler service rejects absent or component-wise greater-than-reserved evidence and rolls back the complete queue/Step/effect transaction. Successful non-invocation consumes control only and releases the effect reservation inside the pass budget.
- `StepControlWeightProvider` receives the starting phase, committed outcome, and canonical post-placement class (`None`, `Queue`, or `Wakeup`) after Step mutation and successor placement share one transaction. Missing or component-wise greater-than-reserved actual control evidence rolls back that transaction. The production maximum benchmark materializes eight current/Opening evaluation units, eight C6 tail chunks, 64 Opening snapshot entries, 128 Opening results, and 40 funding entries. That exact context conservatively selects the measured full-cycle owner so the maximum remains dispatchable, even though scheduler queue service is also metered outside it. Independently estimated ProofSize cannot be subtracted soundly; a direct inner owner must replace this temporary overlap.

Running, Suspended, and fresh-Opening progress select direct inner owners. Fresh-Opening progress has separate minimal and maximum realizable geometry owners at each authored C6 tail count; minimal paths retain only a negligible in-memory slope instead of inheriting maximum Opening capture. Execution reuses the transaction-prevalidated full Contract, hot state, run authority, and admission identity for run persistence and FIFO successor placement without reloading the next tail fragment. Opening-heavy Suspended owners use the worst realizable one-Opening/three-Current composition rather than a non-monotonic mixed-unit axis. Fresh-Opening completion selects separate minimal and maximum realizable direct owners; ordinary completion no longer reconstructs cold tails merely to prove that no terminal close condition applies.

The minimal completion owner keeps fixed `5,696` ProofSize and `3/3` reads/writes across `0..=8` tail chunks, while its `524,747` RefTime slope accounts only for bounded in-memory authored geometry. Fresh-Opening retry and non-closing failure likewise select minimal and maximum-realizable direct owners; wakeup placement reuses the loaded hot authority and adds no Contract fragment read to the minimal path. Actor close preserves the logical completed/failed control outcome, while `close_actor` remains the separately reserved, non-fee terminal-cleanup owner. Full minimal/maximum close benchmarks are composition assurance only and are never substituted for actual Step control or charged again through the Step fee.
- `fee_native_protected_minimum` applies `max(MinUserBalance, asset minimum)` to User fee-native direct preserve-spend capacity and `SwapOut` input capacity after the selected reservation; other assets retain their adapter minimum.

Pallet regressions cover one useful charge and no redundant latched charge for every Trigger family, detector disable/re-arm, automatic underfunding, independent temporal pointers, Crossing batch-to-scalar progress, Pipeline collection rollback, exact admission boundaries, zero-Step service, each Action outcome, task rollback, valid-actual release to zero, missing/excessive evidence, direct User-floor preservation, and exact-output input-cap failure. Package tests reject Pipeline service one unit below `MinUserBalance + Pipeline total`, preserve committed Trigger fees, and prove Running/Suspended service has no renewed machine-solvency owner.

### Progress-Preserving Continuation

`ActorHot.cycle_state` is the sole discovery marker. `Idle` reads no Actor run value; `Suspended` requires exactly one `ActorRunState[actor_id]`. The sparse value stores scalar `cursor`, `unsuccessful_attempts_at_cursor`, `last_attempt_block`, frozen typed suffix snapshots, and cumulative outcomes. `Running` is present in metadata for the accepted block-paced contract but is not persisted by the current suffix executor; its implementation remains open in I1/I3. Try-state enforces marker/store equivalence, cursor bounds, Mutable-only bounded retry policy, a live cursor-local count below its nonzero limit, and snapshot-surface validity.

Attempt `0` opens one logical cycle and increments `cycle_nonce` once. A Temporary `TaskFailure` or `FundingUnavailable` under `RetryLater { max_attempts }` increments both global failure state and the cursor-local count. The first suspension stores `1`; same-cursor suspension uses checked increment after admission proves both retry bounds, while a later cursor resets to `1`.

`transition_failure_streak` is the sole mutation formula: an unsuccessful attempt checked-increments, while completed execution or semantic Step replacement resets to zero. Suspension and terminal failure call that owner once, classification only reads its result, and simulation inherits the same transition through canonical execution.

Post-attempt counters use checked addition. Inclusive local exhaustion closes with `RetryAttemptsExhausted`; when both cutoffs land together this reason wins, while an earlier global cutoff closes with `ConsecutiveFailures`. Exhaustion clears ActorRunState, emits `CycleSummary(Failed)` without `CycleCancelled`, then closes. An already-reached global cutoff closes before another `CycleStarted` or `CycleContinued`. Persisted retry reuses the nonce, omits external cadence, and executes only the suffix.

`scheduler::retry_backoff_blocks` uses checked capped exponentiation to map persisted attempt `0, 1, 2, ...` to `1, 2, 4, 8, 8...` blocks.

`Attempt identity proof`: `cycle_nonce` identifies the logical cycle, and the scheduler's one-execution-per-actor-per-block invariant makes `(actor_id, cycle_nonce, block_number, event_index)` unique for every opening or resumed-run attempt. Cursor and `unsuccessful_attempts_at_cursor` state the semantic retry position. No cycle-global attempt ordinal is stored, emitted, simulated, or projected; executable evidence covers opening, repeated suspension, and completion coordinates.

Eligibility uses the larger of this delay and schedule cooldown, omits external cadence for the open run, then respects window start. A one-block delay uses the existing next-block FIFO ticket; longer delays use the existing paged wakeup pointer.

`Backoff decision evidence`: over 64 unavailable blocks at the 10,000-actor bound, capped exponential creates 100,000 due retry obligations and recovery-wait sum 210. Fixed delays of 1, 4, and 8 cause `640,000/0`, `160,000/96`, and `80,000/224` respectively. Every policy retains the same 10,000 wakeup cohort and FIFO order, and each serviced retry has the same Weight class. No fixed delay improves aggregate pressure and recovery together, so capped exponential remains canonical.

`Timer phase decision`: `tests/fixtures/timer-jitter-decision.v1.json` preserves the 10,000-actor comparison. Historical jitter expanded two targets to 128 while leaving tail service, `PassExhausted`, serviced Weight, FIFO, suspension, and retry recovery unchanged; peak queue improved by less than 2.5%. The implementation therefore uses exact cadence from `schedule_anchor` with no phase constant, hash, arithmetic, metadata, or host configuration.

`resolve_step_control` is the private exhaustive transition owner for executed, stopped, skipped, funding-unavailable, Permanent-failure, and Temporary-failure outcomes across all three error policies. Production execution and `simulate_current_contract` both invoke `execute_single_cycle_traced`.

`canonical_step_transition_matrix_has_production_simulation_parity` runs every Section 3.4 row, Actor type, mutability, error-policy variant, Step-outcome variant, local/global bound, and fresh/resumed-run attempt. Variant counts fail closed when either canonical enum grows.

The matrix compares rollback-only simulation with independently observed production events, counters, cursors, failure streaks, fees, balances, custody, suffix effects, and final disposition. The protocol-coherence audit rejects specification/matrix and enum-inventory drift, and Actors assurance executes the focused matrix.

Task-scoped rollback leaves earlier successful steps committed. The named `SwapIn → AddLiquidity → Transfer` and Burn-prefix regressions prove same-cursor retry, prefix non-replay, cumulative outcomes, and no cancellation compensation. The fixed-seed model `0xDE05_0730` independently checks sparse state, cursor progress, queue/wakeup uniqueness, funding accumulation, frozen cycle snapshots, and cancellation after each transition.

`simulate_current_contract` is the package-owned rollback core behind versioned `ActorSimulationApi` runtime metadata. It requires exact stored Active Actor Contract, actor type, mutability, mode/run-state, readiness, liveness, and User fee budget. Simulation and production admission use the same package-owned suffix envelope and predicate: checked fee-native balance above `MinUserBalance` must cover `attempt_fee_upper`; raw balance alone never admits a User attempt.

The API repeatedly executes the one-Step production path under rollback, advances a virtual causal block argument for `Continued`, and exposes fresh or persisted-run outcomes including `unsuccessful_attempts_at_cursor`. Terminal closure returns `AttemptDisposition::Closed(CloseReason)`. The whole attempt runs inside `TransactionOutcome::Rollback`; package tests prove actor state, balances, events, fees, adapter effects, and stored ActorRunState remain unchanged.

Semantic Contract Steps, funding-policy, schedule, window, deactivation, terminal, and close transitions share `cancel_run_internal`. Cancellation validates and mutates `ActorHot` through one fallible storage owner; missing or contradictory state returns `ActorRunInvariant`. Actor run writes load all five canonical partitions, return `ActorNotFound` for absent or Dormant identity, and return `ActorInvariant` for malformed partition ownership, without panic-only mutation assumptions. Continuation attempt opening returns a state-preserving failed disposition when its canonical state disappears, and checked fee-envelope disagreement does the same before cycle mutation or events. Exact encoded plan/completion-policy, funding-policy, schedule/window, auto-close-target, and global active-limit updates return without storage or event mutation; semantic auto-close target changes preserve Continuation.

Every external control that invalidates or reconstructs ordinary membership shares one class-independent actor/block clock across signed-owner and governance origins; its identity write and Contract replacement hot-state/reset write are fallible transaction-owned mutations returning typed invariant errors rather than panic-only post-prevalidation assumptions. Exact no-ops, internal transitions, and terminal cleanup remain exempt.

Cancellation emits `CycleCancelled` before one cumulative terminal `CycleSummary(Cancelled)` without compensation or prefix rollback. The reason set distinguishes explicit, Steps, completion, funding, schedule, deactivation, and typed `Closing(CloseReason)` causes. Runtime-upgrade cancellation is not a public placeholder; a deployed host that needs one must ship the concrete bounded migration and its executable constructor together.

`CycleStarted` appears once per nonce. `CycleContinued` and `CycleSuspended` carry `(actor_id, cycle_nonce, cursor)`; suffix step events retain the nonce and their order belongs to the surrounding attempt boundary. Current sparse state is canonical-chain truth. Unbounded attempt history remains materialized.

### Read-Only Eligibility Projection

Version 6 `ActorEligibilityApi` owns `actor_eligibility` plus the bounded named `materialization_faults` and `crossing_capacity` projections. `actor_eligibility` is the read-only semantic Actor projection. It mirrors `apply_admission` and reuses the exact cadence, retry, window, failure-limit, breaker, and latch owners, so clients do not reproduce scheduler arithmetic.

The projection is one algebra: `NotRegistered`, `Dormant`, or `Active(ActorClassification)`. Active Crossing includes canonical phase and installation revision plus bounded pending/processing revision facts; the companion fault method returns only current Crossing, broad-fanout, and wakeup faults, while `crossing_capacity` returns per-feed User/total policy and exact counts without raw physical topology. Active eligibility preserves terminal reason and the exact `ActorExecutionPhase`, including `WaitingRetry(block)`, `WaitingBlock(block)`, and `WaitingCadenceTick(tick)` payloads; no parallel phase or next-block field exists.

The projection persists no state, emits no event, and promises no service. Queue position and available Weight still decide actual admission. Arithmetic overflow and malformed Actor run state return typed projection errors rather than an inferred phase. Nonce exhaustion is projected as the same terminal close as execution without attempting an overflowing successor nonce.

Authoritative amount arithmetic uses checked accumulation/subtraction: normalized split legs plus retained remainder exactly conserve the resolved total, including `u128::MAX`. Fee reservations, existential protection, and percentage amount resolution intentionally floor unavailable spendable balance at zero. Saturating `Weight` composition remains a conservative upper-bound cap, scheduler pass statistics and starvation counters remain bounded telemetry caps, and try-state topology/accounting counters fail on overflow rather than masking malformed state.

Code anchor: `src/scheduler.rs::actor_eligibility`; package tests prefixed `eligibility_projection_` falsify absence, dormancy, every Active phase, terminal coexistence, and exact temporal payloads.

### Read-Only Cost Projection

`ActorCostApi::actor_cost_quote` returns one bounded current quote without combining economic owners. Active Actors expose the exact family-specific Trigger occurrence Weight and fee, the stored upfront Pipeline Machine/cleanup amounts and admission/production Weight identities, and the current Step's maximum Action-effect Weight and fee. Dormant Actors have no prospective Trigger or Pipeline charge, while a zero-Step or `StopCycle` current branch reports zero Action maximum.

The quote returns the configured Creation Fee separately from the current refundable state hold. User hold provenance preserves identity, Contract head, actual retained tail body, detector, funding, and run components plus base/per-byte pricing and checked total; System Actors report explicit exemption and zero held components. The method reads bounded canonical state only, mutates nothing, exposes no remaining machine budget, and returns typed errors for absence, partition corruption, arithmetic overflow, or missing Weight authority.

Code anchor: `src/lib.rs::actor_cost_quote`; `actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate` falsifies User/Dormant/System separation, generated identities, current Action ownership, and hold totals.

### Queue Execution Model (Monotonic Paged FIFO)

`classify_actor` owns non-economic terminal precedence, breaker and pause phase, retry/temporal timing, and signal readiness. Scheduler admission alone checks User Pipeline capacity when Idle paid readiness is consumable; sweep, Running/Suspended classification, eligibility, and certified ingress never predict future affordability. Simulation mirrors the separate Pipeline boundary; malformed Active partitions and Continuation state map through typed classification errors.

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: a monotonic paged FIFO for work that can execute now and one typed paged temporal layer for later eligibility. Block deadlines and timestamp-tick deadlines have separate paged minimum heaps under one coordinator; actors sharing one typed deadline occupy linked fixed-capacity pages.

1. **Wakeup drain**: the coordinator fairly alternates block and tick domains when both are due; each cursor exposes its earliest deadline without scanning sparse gaps, and one admitted unit consumes one slot, preserves a partial bucket at the same minimum, and either appends live readiness to the active FIFO or lazily discards a stale pointer
2. **Ingress admission**: each matched producer call applies funding independently and sets the unified boolean signal latch; actor-local queue/wakeup membership remains bounded and may join the active run queue in the same `on_idle` pass
3. **Block-start cutoff**: `on_initialize` stores `(block, NextQueueTicket)` in `PrepassExecutionCutoff` before ordinary external causes; `NextBlock` Actor Drain consumes that cutoff only when its block tag matches the current block, while direct physical service helpers without a current hook-owned cutoff use their explicit current frontier. Every later append receives a ticket at or beyond the cutoff and cannot execute in that block
4. **Canonical head**: tri-state discovery returns `Empty`, `Head`, or `Blocked` only for the one global FIFO. It stops on an incomplete probe rather than treating the head as absent.
5. **Strict ticket order**: the scheduler may lazily consume tombstones before the cutoff, but it cannot bypass a live or blocked head. One O(1) preflight owns every FIFO mutation: checked `tail - head == QueueOccupancy`, configured capacity, and canonical current head/tail page-slot layout. Enqueue, live-head consume, and tombstone drain run as closed storage transactions after that preflight. Corruption rolls back exactly and returns an invariant live-head stall rather than `Empty` or progress. The next live ticket receives the only execution offer.
6. **Execution ceiling**: `last_cycle_block` records both Opening and terminal disposition blocks, while Running owns `last_committed_step_block`; these markers, shared scan/execution ceilings, and the common cutoff prevent another Step or retained-signal Cycle from executing for that actor in the same block. Untouched suffix entries remain physically in place without reconstruction.

`ActorHot.queue_ticket` is the sole live-membership marker for the canonical FIFO. Replacement, close, pause, and cancellation use actor-local queue invalidation; stale queue entries drain lazily. FIFO enqueue, invalidation, liveness, tombstone drain, and scheduler admission load the exact five-partition actor state before interpreting the ticket, same-block marker, pause state, or Contract; corrupt state stalls without queue mutation. Live-head service carries that validated hot state into transactional consumption, which rechecks the exact physical actor/ticket pair before clearing membership; post-execution placement reloads canonical state because execution may change lifecycle, Continuation, funding, or terminal ownership.

Wakeup schedule, invalidation, drain, and materialization use the same canonical boundary; only NotRegistered, Dormant, or Active-with-no-pointer entries drain as lazy temporal tombstones, while another live pointer or corrupt partitions roll back. Continuation cancellation exactly removes the retained Active actor's current wakeup slot before re-priming a latched next run, so an old slot cannot conflict with the replacement pointer. The production-Wasm `scheduler_actor_state_probe` benchmark at 100 steps and 50 repeats measures `38,413,000 / 12,200` with five reads. The reference runtime regenerated every changed scheduler and ingress branch at 50 steps and 20 repeats against the same loader; branch weights now carry their complete read sets without a separate partial-probe surcharge.

No actor type, actor ID, execution share, or priority policy changes FIFO service. System and User actors receive bounded service exclusively by their global ticket order.

The production Actor Drain `execute_cycle_to_cutoff_with_resources` path pre-reserves the complete pass Actor Control maximum before queue scan/probe, reserves each Action's phase-owned effect maximum before semantic mutation, settles committed actual effects transactionally, and settles the full reconciled pass control afterward. Coverage includes successful Action, zero-Step Opening, precondition skip, paused/tombstone consumption, and proof-budget probe stall as exact control-only or split-domain outcomes. Missing actual evidence retains both authorities and halts optional Actor work, preventing final resource reconciliation from publishing a false domain split.

`BlockResourceState::reserve_actor_step` atomically reserves one Step's Actor Control and phase-specific Actor Base/Drain effect maxima; a failed second reservation restores the complete prior state. `settle_actor_step` similarly commits both valid actual values or restores state and both one-shot reservation authorities.

`CyclePass` separately accumulates committed actual Task-effect Weight and derives Actor Control as checked `consumed - effect`. A rolled-back Action-bearing attempt whose exact effect execution cannot be recovered marks domain reconciliation uncertain rather than fabricating actual evidence; resource integration can therefore halt before publishing a false split.

Wakeup draining, paged queue operations, actor probes, normal cycles, and pure terminal cleanup use two-dimensional `WeightMeter` fit checks. Direct ingress is charged at its originating producer. Close admission uses the generated worst-case User cleanup bound and performs no shared-container scan.

Scheduler accounting stays local unless it controls consensus progress. `scanned` and `executed` enforce independent per-pass ceilings; `QueueDrainStats` and `WakeupDrainStats` expose bounded operation results to callers, tests, and benchmarks without storage writes. Operators derive completed admission from `CycleStarted`/`CycleSummary` and persistent blockage from bounded queue/wakeup state plus starvation detection/recovery events. Loaded-actor and page-touch diagnostics remain stress instrumentation rather than permanent consensus counters.

### Temporal Wakeup Layer

The temporal wakeup layer owns future eligibility without conflating detection and execution. Every Active Actor may own at most one exact block-keyed Pipeline-service pointer and one independent tick-keyed Trigger pointer; each clock-domain replacement invalidates only its own slot without scanning actors or deadlines. `MaxActiveActors` bounds live memberships per clock, while `WakeupPageSize` controls I/O granularity rather than same-block capacity. This makes spillover buckets, placement-drop events, and actor-key retry scans unnecessary.

One placement owner (`schedule_next_work_local`) maps signalled readiness, timestamp cadence, block cooldown/window/retry targets, capacity recovery, and terminal expiry to either the FIFO or one typed wakeup. Placement owners normalize `AlreadyLive` to success; a missed normalization at a defensive `map_err` boundary returns `QueueCapacityUnavailable` rather than panicking. `defer_wakeup` and `defer_tick_wakeup` share the same transactional substrate. Queue saturation falls back to an exact next-block wakeup; fallback failure leaves no false success claim (spec 8.1.4).

Queue consumption, attempt effects/events, and all reachable post-attempt placement share one transaction. Ticket/page/wakeup exhaustion or topology failure rolls back the attempt and preserves the prior queue head. Wakeup removal and queue materialization likewise commit atomically or retain the exact wakeup.

Package coverage proves the hook-ordered `NextBlock` cutoff: a block-N Manual occurrence allocates one ticket strictly after the block-N prepass frontier, remains pending without `CycleStarted` through block-N Drain, and executes in block N+1. Direct physical FIFO tests remain independent of runtime hook timing.

The package uses the paged wakeup substrate and sparse cursor:

- `WakeupPages<(WakeupKey, page_id)>` and per-key `WakeupBuckets` own the paged topology, where `WakeupKey` is `Block(number)` or `Tick(number)`.
- `WakeupCursorPages<(clock, page_id)>` plus `WakeupCursorLen<clock>` provide separate production paged binary min-heaps for block and tick keys; each bucket owns its exact reverse `cursor_index`.
- Heap insertion, pop-min, and exact removal use at most `ceil(log2(MaxActiveActors))` sift steps, preserve contiguous per-clock cursor pages, and avoid scanning empty intermediate keys; try-state reconciles each clock's page shape, uniqueness, ordering, and reverse indices. Maximum-depth generated evidence belongs to `scheduler_wakeup_cursor_insert`, `scheduler_wakeup_cursor_pop_min`, and `scheduler_wakeup_cursor_remove_exact`.
- `ActorHot.wakeup_pointer` owns `WakeupPointer { block: WakeupKey::Block, page_id, slot }` for Pipeline service, while `trigger_wakeup_pointer` owns `TriggerWakeupPointer { tick, page_id, slot }` for AtTime/Cadenced detection; `trigger_runtime_state::Cadenced` owns the optional anchor tick.
- Pages use optional slots, a live count, a scan cursor, and bidirectional links.
- Transactional replacement invalidates the prior exact slot, removes an emptied key from its clock cursor, creates the replacement bucket and cursor entry atomically, and rolls back on reverse-index mismatch; bounded neighboring-page work unlinks empty pages.
- The cursor-driven overdue worker runs after timestamp inherent application and before the queue cutoff. It floors consensus milliseconds into the current tick, reselects after every admitted unit, and round-robins simultaneously due block/tick clocks while preserving FIFO order inside each clock. It meters each cursor lookup, page scan, queue append, and possible full-depth cursor removal before mutation; partial progress keeps each clock's same minimum for later resumption.
- The coordinator gives the rotated first family its generated maximum-unit minimum plus the shared lendable remainder, while reserving one complete maximum-unit minimum for each later family. The wakeup worker consumes only that grant and remains independently bounded by `MaxWakeupsPerBlock`; a complete cursor/drain unit must fit before mutation, and returned hook Weight cannot exceed the caller budget.
- The production drain primitive bounds work by slots scanned, preserves a partial head cursor, crosses linked page boundaries, deletes exhausted pages, clears only matching live pointers, discards stale slots, and removes an exhausted bucket from the cursor in the same transaction.
- Try-state reconciles links, counts, slots, unique pointers, and active-actor capacity.

Package benchmarks isolate append, exact replacement, middle-page unlink, partial/full/stale drain, cursor insert/pop/removal, one-slot worker progress, full-depth repair, and future-minimum stop. Every branch reserves complete RefTime and ProofSize before mutation. Hosts select page size and generate production coefficients against their configured depth and database schedule.

### Starvation Safeguard

The scheduler first admits `scheduler_on_idle_base`. If that fixed two-dimensional envelope cannot fit, `on_idle` returns zero without storage or telemetry work.

Starvation handling then follows these rules:

- The base reads paged occupancy.
- A physically saturated queue reserves and attempts one generated tombstone-drain unit before breaker return.
- A stale head makes bounded progress even while the breaker remains active; a live head stays in place.
- The actor pass distinguishes empty work, a known head, weight stall, pass exhaustion, and invariant stall. With the breaker inactive, weight or invariant stalls over a live head and no admitted attempt saturating-increment `Starving { consecutive_blocks }`; scan/count exhaustion and same-block boundaries remain silent pass deferrals.
- No live work, an admitted attempt, pass exhaustion, or breaker activation clears state once; alerted recovery emits once, and Healthy blocks perform no telemetry write. Weight-blocked, fee-collection-stalled, and invariant-stalled live heads all advance starvation; an empty or tombstone-only queue with exhausted budget does not.

Package coverage retains actor-local readiness through ProofSize-only exhaustion, verifies telemetry without beginning an inadmissible drain unit, and proves that an empty queue with an exhausted budget never enters `Starving` while a weight-blocked live head does.

| Live-head stop | Valid-state reachability | Maximum persistence and recovery owner | Falsification evidence |
| --- | --- | --- | --- |
| Weight | A conforming maximum admitted attempt may consume one complete block reserve; a smaller residual meter may reject the next complete unit. | At most the next conforming actor-service pass because every admitted Contract fits the guaranteed reserve; runtime Weight configuration owns recovery. | `proof_size_exhaustion_counts_as_idle_starvation`; `scheduler_starvation_freedom` |
| Fee collection | A User attempt reaches this stop only when the host's ledger-only `FeeCollector` fails after admission; authored policy, Fee Sink Actor state, and scheduler pressure cannot cause it. | A conforming host proves the reserved debit and deposit-capable sink before execution, so valid state cannot sustain the failure; runtime ledger configuration owns recovery from invariant failure. | `stop_cycle_fee_failure_rolls_back_before_step_policy_or_stop`; runtime `actor_fee_collector_ignores_malformed_actor_and_scheduler_state` |
| Same-block suppression | Internal re-entry or a second actor-service pass may encounter an actor already attempted in the current block; a post-cutoff authored signal cannot cross the cutoff. | One block at most; the consensus block clock and next ordinary pass recover it without reticketing. | `repeated_trigger_same_block_yields_one_ticket_and_one_execution` |
| Placement failure | Queue saturation defers to one exact next-block wakeup. Transient or corrupt post-attempt placement failure rolls back the whole attempt; permanent queue or wakeup index exhaustion closes the actor without bypassing its head. | Capacity pressure recovers when bounded stale work drains. Index exhaustion commits prior attempt effects, or closes before a due attempt, under `SchedulerIndexExhausted`; neither path can retain the global head. | `post_attempt_wakeup_index_exhaustion_commits_then_closes`; `wakeup_materialization_index_exhaustion_closes_without_an_attempt` |
| Invariant failure | Not reachable from valid admitted Actor state; malformed partitions, ownership, topology, or transaction structure stall before unsafe mutation. | Unbounded by design until fresh-genesis correction or a deployed-lineage migration; try-state, starvation telemetry, and operators own detection and repair. | `mixed_fifo_stops_at_corrupt_actor_without_touching_valid_suffix`; `mixed_wakeup_bucket_rolls_back_valid_neighbors_around_corruption` |

Recovery is governance-operated (circuit breaker or parameter adjustment); no emergency cycle execution occurs in `on_initialize`.

### Temporal Triggers

- Temporal readiness is deterministic only; Actors exposes no probability field, entropy provider, secure/insecure branch, hash fallback, probability event, or probability error.
- `AtTime` and Cadenced use `temporal_anchor_tick = ceil(timestamp_millis / CadenceTickMillis)` as their exact origin and derive no actor-specific phase. `MaxTemporalDelayTicks` bounds this timestamp domain independently from block-domain `MaxExecutionDelayBlocks`; neither horizon is converted or reused across clocks.
- Genesis records an uninitialized temporal anchor and one tick-zero bootstrap wakeup because no consensus timestamp exists during construction. The first ordinary wakeup service anchors from the applied timestamp and installs one full-delay deadline without latching readiness or entering FIFO.
- Active installation and replacement anchor directly because consensus time is available. Eligibility projects authored AtTime/Cadenced geometry and one-shot consumption, while canonical Trigger runtime state and `TriggerWakeupPointer` own the exact deadline.
- AtTime admission prevalidates nonzero bounded `after_ticks`, zero cooldown, no window, and checked `anchor_tick + after_ticks` arithmetic. A due occurrence marks `consumed = true` and never rearms or catches up.
- Cadenced admission applies the same policy to `every_ticks`. Runtime rearm selects the first aligned tick strictly after the observed tick, so missed periods coalesce without catch-up bursts.
- A due AtTime occurrence clears its Trigger pointer and permanently marks the one-shot opportunity consumed. A useful Cadenced occurrence clears its Trigger pointer and leaves temporal detection disabled while readiness remains latched. Already-latched due work performs no Actor-specific processing.
- Opening atomically rearms Cadenced to the first aligned tick strictly after current consensus time before Pipeline charging. Failure rolls back readiness consumption and fee movement. Only Pipeline-service FIFO membership executes the Actor; any future probabilistic execution requires separate admission policy and secure runtime entropy.

### Trigger Sources

Every Trigger family reaches activation only for an unset readiness latch. Direct families reject redundancy before family-specific evaluation or charging. Indexed families retain stable subscription/range topology and use `IndexedTriggerDetectionDisabled` as detector-local authority, allowing fanout and Crossing traversal to skip a latched identity before loading Actor state. Opening clears ObservationChange authority directly; Crossing Opening first relocates membership and resets phase/revision from the current observation, discarding latched-period transitions. Canonical FIFO append separates read-only `QueueAppendPlan` authority from mutation, and bounded Crossing cohort preflight validates queue topology once before commit.

Successful temporal enqueue carries single-Actor authority into activation commit, while queue saturation retains the exact next-block wakeup fallback. Ordinary planning records no placement, immediate enqueue, or one exact block wakeup before commit. Source-specific code owns only detection and source-state progression; no parallel signal key or source tag enters consensus state.

The scheduler carries one loaded Active state through live-ticket verification, same-block and lifecycle gates, classification, attempt admission, fee admission, and loaded-head consumption. Control preflight, ingress preflight/consequence, eligibility, simulation, sweep, and activation likewise classify with the Continuation returned by their complete canonical probe instead of rereading it through a view-only helper. A transaction boundary or adapter/task effect that may mutate actor state terminates that borrow and requires an explicit canonical reload before later placement.

Crossing phase and installation revision live only in `ActorHot.trigger_runtime_state`; generation-checked exact-threshold memberships store only key/page/offset/generation physical back-pointers in bounded dense pages under a sparse radix path. `MaxCrossingMembersPerFeed` is the independent total host bound, while `MaxUserCrossingMembersPerFeed` and its exact reconciled count reject User admission before topology writes and leave the difference exclusively available to System Actors. The reference runtime binds these to 10,000 total and 9,000 User memberships per feed; global Active capacity remains a separate aggregate ceiling. Dense-page swap compaction rewinds an active range cursor when an unprocessed tail member moves behind it, preventing close or replacement of a skipped newly installed member from erasing older transition work.

If activation terminally closes the final feed member, ordinary close cleanup removes the membership, queue, cursor, and pending-feed link atomically; the worker recognizes the resulting zero membership count as completed cleanup and does not recreate source state. A non-terminal placement failure rolls the complete actor unit back.

`classify_crossing_work` is a bounded read-only projection over the current pending-feed head, transition, range cursor, sparse radix result, generation-checked member, canonical Actor state, and prospective activation shape. Its production bounded contiguous-prefix authority is obtained through the internal `CrossingCohortSnapshot` helper bounded by Crossing-owned `CrossingPageSize` and `MaxCrossingActorsPerBlock`. Each snapshot binds the source leaf key, page, half-open `[start_offset, end_offset)` range, and contiguous member/generation/locator authorities; it validates only selected entries, clamps to page remainder, grants no authority outside that selection, and performs no mutation. Its production-used count authority also accepts an optional tail-refill limit, allowing non-tail selection to preserve packed pages with one bounded suffix.

Production Weight owns the required maximum encoded P128 tail-page probe at `20,464,000 / 11,729` plus one database read. Compatibility pair fallback retains separate cursor and tail snapshots because sequential single-member compaction defines that execution shape; production larger batches use one admitted contiguous prefix and, for non-tail sources, one exact physical-tail refill suffix. First- and tail-candidate preflight use one branch-directed read-only cohort classifier for post-installation skip, rearm, pending fire, placed fire, coalesced fire, and terminal fire. Placed fire retains scheduler-owned prospective Hot state after the phase changes to `WaitingForRearm`, including its pending latch and exact immediate-FIFO versus block-wakeup action; aggregate queue authority therefore cannot restore the pre-fire phase.

The first authority establishes the expected branch; later skip mismatches stop after Hot state, rearm mismatches stop after Hot plus Contract, and fire activation reads the full canonical shape only when that expected branch requires it. The bounded preflight admits only the homogeneous leading branch before mutation, leaving the first differing authority unadmitted without exceeding the generated branch probe. Internal work classification is distinct from the candidate-preflight result and carries branch, exact current admitted candidate count, and optional `(tail page, available suffix)` refill authority through probe fallback, full classification, and pair-to-single downgrade. Tail-page classification leaves refill authority absent; non-tail admission populates it only after reserving and charging the generated tail-page probe.

Service admission fails closed if that count disagrees with the selected branch components, so it does not reconstruct count from mutable state after preflight. Production Weight now owns placed-fire, coalesced-fire, terminal-fire, post-installation-skip, and rearm preflight as parameterized `c ∈ [1, 4]` functions with generated RefTime, ProofSize, and respective `2 + 7c`, `2 + 7c`, `2 + 7c`, `2c`, and `3c` database-read formulas; mutation branches use generated single, pair, maximum tail-page, and trimmed/emptied non-tail owners.

Current pair admission is work-conserving at component and two-dimensional Weight boundaries: one remaining transition/leaf/page/Actor unit selects one candidate, an unaffordable pair probe falls back to the corresponding single preflight, and an affordable pair probe with an unaffordable pair branch executes its already classified first candidate without rereading state. The production cohort extends this authority boundary without a second snapshot path. Its `CrossingWorkPlan` distinguishes empty, completion, seek, leaf/page advancement, installation skip, rearm, coalesced/placed/closed fire, and structural-fault branches without mutating state. Service admits the generated common probe before preliminary classification, conditionally admits the fire-only classification extension, and then selects one generated execution owner.

Completion/seek miss use transition Weight; open/advance use the ordinary leaf/page maximum; skip, rearm, coalesced fire, placed fire, and terminal close each use their dedicated owner; structural fault reserves the terminal envelope. It never adds independently benchmarked whole-branch results as though they were disjoint components. The committing transaction revalidates canonical and physical ownership. Each pass also retains exact bounded counters for transition units, leaves, pages, candidates, canonical five-partition activation probes, activation calls, terminal closes, and rolled-back faults; the transition/leaf/page/candidate dimensions enforce existing component ceilings, while probe/activation/close/fault dimensions support exact branch evidence without entering consensus storage.

The release load profile installs 10,000 members on one feed and proves a zero-match transition finishes in one configured pass with zero candidate, canonical-probe, activation, close, fault, or FIFO-placement work. Its following eight-member crossed cohort records exactly eight candidates, canonical probes, activations, and FIFO placements, with zero closes or faults; the other 9,992 members remain unprobed. Latched members retain physical range authority but later transitions take the generated disabled-skip branch without Actor-state loading, fee, event, activation, placement, or phase accumulation.

Pure unlatched rearm retains a generated owner and performs no occurrence collection, activation probe, placement, or close. Classification is staged: every nonempty unit first admits the generated common probe, and only a useful fire-pending plan may admit the single or pair fire extension. No-match, structural search, disabled skip, and unlatched rearm never pay that extension. A latched identity selects disabled skip before Actor-state classification; compatibility coalesced owners remain generated branch envelopes but no longer collect a redundant occurrence fee.

A newly latched nonterminal fire uses generated `crossing_placed_unit` Weight (`558,181,000 / 162,782`, 89 reads and 80 writes) for phase movement, one User occurrence collection, and exactly one canonical FIFO placement without terminal cleanup. The separately generated economic owner `observation_crossing_trigger_occurrence` is `503,983,000 / 164,107` with 89 reads and 80 writes; it begins from an already published transition and retained threshold, so Oracle publication/signaller work is excluded. A post-installation transition skip uses generated `crossing_skip_unit` Weight (`171,882,000 / 81,886`, 40 reads and two writes), preserving phase and placement while advancing the stale candidate. The first cohort slice recognizes two homogeneous installation-skip, pure-rearm, coalesced-fire, or nonterminal placed-fire candidates on the same current tail page before mutation, admits a placed pair only when both scheduler plans require immediate FIFO and one read-only aggregate queue plan validates exact pages, consecutive tickets, final tail/occupancy, duplicates, capacity, and index arithmetic; otherwise it downgrades to one candidate.

Dedicated pair probe and execution owners charge and commit both in one transaction. Placed-batch accounting and Weight admission carry exact bounded candidate counts: two uses the generated atomic pair owner, while homogeneous larger cohorts use the generated maximum-batch owner.

Placed-batch production reconstructs one coherent bounded cohort authority. Count two uses pair movement; a homogeneous contiguous P128 tail prefix may admit up to 128 candidates and uses descending physical movement followed by stable retained-suffix rewrite and locator repair. Both paths commit one queue plan plus one cursor/feed advancement. Exact-root tests reject corrupted generation or compact authority before unsafe grouped mutation. The fixed-depth radix seek has a separate generated owner charged only while `current_threshold` is absent, so candidate service reuses the retained threshold without sparse-work doubling.

Production non-tail batch classification charges the tail-refill probe, binds one generation-checked physical-tail suffix to the placed authority, preserves retained source order, groups equal destination obligations so each touched destination page is written once, repairs source/tail/destination locators, handles trimmed and emptied tail pages under separate generated mutation owners, meters their component-wise maximum, commits the aggregate queue and cursor once, and rejects stale authority transactionally, and a route-heterogeneous third candidate truncates a four-candidate prefix after its first two homogeneous immediate-FIFO authorities, consumes it through two movement-without-Hot operations with swapped-tail generation validation, commits one aggregate queue plan, installs final phase/tickets, and advances cursor/feed once.

Exact instrumentation and rollback evidence cover this path, while separate movement evidence covers split destinations. Compatibility coalesced/rearm/skip pair execution still restores its preflighted feed between scalar units so pending-list rotation cannot substitute another feed. Post-installation exclusion uses `crossing_skip_pair_probe` (`46,515,000 / 6,112`, nine reads) plus `crossing_skip_pair_unit` (`70,960,000 / 6,112`, 10 reads and two writes) and leaves both phases and placements unchanged. Pure rearm uses `crossing_rearm_pair_probe` (`58,249,000 / 23,410`, 11 reads) plus `crossing_rearm_pair_unit` (`430,439,000 / 162,782`, 82 reads and 76 writes) and performs zero canonical activation probes.

Latched homogeneous members use generated disabled-skip service and retain phase, membership, and placement unchanged. Generated atomic pair and maximum-cohort owners charge every useful User authority before committing one bounded membership/queue transaction; generation, locator, compact admission identity, canonical state, phase-before-activation, and rollback checks remain per candidate. A first compact-authority mismatch receives one exact generic scalar fallback; a later mismatch truncates before that candidate and permits only the preceding coherent prefix. A terminal peer is never absorbed into a nonterminal placed owner.

A candidate whose installation revision equals the pending revision truncates a homogeneous prefix before mutation: earlier valid Actors may commit through their coherent prefix, while the excluded candidate advances through the dedicated installation-skip owner without latching, placement, retrofire, or phase change. A newly found threshold is retained before candidate service. Homogeneous immediate-FIFO preflight and tail cohorts admit up to 128 candidates; exact non-tail source/refill cohorts admit up to 64 because both source and physical-tail reverse locators must be proven. Mixed routes truncate before mutation. If any batch member cannot complete its User occurrence collection, the aggregate attempt rolls back and the admitted branch executes one scalar candidate: its Crossing phase/membership/cursor progression commits without fee/readiness/apoptosis when underfunded, then later members continue without a free retry loop.

| Crossing capacity dimension | Reference value | Current evidence |
| --- | --- | --- |
| Actor Control envelope | `304,686,077,576 / 676,150` | Every generated complete branch fits component-wise after fixed-context composition |
| Physical page ownership | P128 Crossing / P64 broad fanout | Separate bounds prevent Crossing geometry from taxing broad-page proof; regenerated broad maximum is 304,734 |
| Tail/preflight candidate cap | 128 per admitted cohort | Generated P128 preflight and maximum tail owners fit at most 346,462 ProofSize |
| Non-tail candidate cap | 64 per admitted cohort | Exact source plus physical-tail reverse-locator validation fits at 327,262 ProofSize |
| User memberships per feed | 9,000 | At least 71 homogeneous tail cohorts or 141 exact non-tail cohorts; not a block-latency promise |
| Total memberships per feed | 10,000 | At least 79 homogeneous tail cohorts or 157 exact non-tail cohorts; reserves 1,000 System positions |
| Canonical FIFO capacity | 10,000 | One maximum first-fire herd can retain one placement path per Actor |
| Pending transitions per feed | 64 | Explicit producer backpressure; not a sustainable publication-rate promise |

The 10,000-member single-threshold release profile exhibits no failure mode distinct from the total per-feed horizon, so the runtime intentionally adds no separate leaf cap; exact page and worker bounds already constrain physical service. These are bounded capacity facts, not a latency SLA. The cohort counts assume one homogeneous placed-fire head transition, sufficient Actor Control, solvent Actors, available placement, and no competing materialization pressure. Alternating revisions may occupy all 64 transition slots; admission then fails atomically until service creates capacity.

`MaterializationFamilyCursor` rotates the block-start family across overdue wakeups, Crossing, and broad fanout as `0 → 1 → 2 → 0`. The coordinator services every family in rotated order, may service only the rotated first family once more through the bounded lending pass, preserves each worker's internal deadline/revision order and counters across grants, advances the cursor before the execution cutoff, and continues materialization while the global breaker suppresses Actor execution. The sum of the three configured ceilings is the single canonical materialization envelope. Each family also has one canonical maximum-unit minimum derived from generated branch Weight, and production integrity requires all three minima to fit together with a positive two-dimensional lendable remainder.

The shared envelope plus fixed base, cleanup, and coordinator Weight is subtracted once to derive the exact guaranteed Actor execution floor, so materialization cannot borrow from execution. A cursor outside `0..3` returns after fixed charged work without running or rewriting a family, and TryRuntime rejects it. Workers consume only the coordinator grant and retain per-block wakeup-scan, Crossing-component, and broad-page counters across grants. The rotated first family receives the production-proven lendable remainder after all three maximum-unit minima. After the first pass, generated coordinator Weight owns bounded due-wakeup, pending-Crossing, and dirty-fanout classification. When the rotated first family still has serviceable work, one bounded second pass offers it all trailing unused reservations without resetting counters, including when its first branch cost less than the maximum-unit minimum.

A permanent Crossing materialization error rolls back the attempted unit, stores one bounded current fault with feed, revision, threshold, and typed class, and halts later Crossing service after base admission instead of retrying unchanged corrupt state every block. Observation fanout applies the same contract with feed, latest revision, available subscriber-page cursor, and class. Wakeup materialization records its temporal key, page cursor, and class after rolling back the unit, then stops at the next charged cursor probe. All three faults are exposed through bounded pallet projection. Their `GlobalBreakerOrigin` clear calls remove exactly the current record, emit its identity, and permit the preserved obligation to resume; callers repair the underlying invariant before clearing or the deterministic worker records the fault again.

Active System installation and replacement invoke the narrow `SystemActorContractValidator` host port before committing the Contract. Generic Actors neither derives a graph nor stores ranks or edges. The unit implementation accepts all contracts; the reference runtime binds its bounded topology policy independently from the portable package.

Filter surface:

- Source: `Any` / `OwnerOnly` / `Whitelist`
- Asset: `Any` / `Whitelist`

Each producer event evaluates every configured source atom without short-circuit. Several atoms matching one event fold into one readiness decision; funding/provenance mutation runs once outside that fold. Distinct producer events still apply funding independently, while pending readiness shares one latch and one bounded actor-queue membership without coalescing value effects.
When a signalled cycle starts, the latch is consumed atomically.

`ObservationChange` is the scalar SCALE trigger at index `2`. Its payload contains one typed feed identity only; thresholds remain Predicate-owned. `note_observation_changed` enters Actors dirty-feed state without a subscriber walk, amount payload, readiness mutation, or execution path. Deferred fanout owns conversion of latest revisions into the existing readiness latch.

Opening-snapshot surface validation runs at genesis, creation, activation, and plan replacement. It requires every staking-share mapping to exist but remains independent from trigger kind, signal payload, and event amount.

`ActorObservationFeeds` stores each active actor's canonical source-derived feed set. A reusable dense slot has exact `ObservationSubscriptionSlot` and reverse-owner entries. `ObservationFreeSlotPages` provides one-page LIFO allocation bounded by `MaxActiveActors`, so identity churn reuses slots rather than growing consensus keys.

`ObservationSubscriberPages(feed, slot / QueuePageSize)` stores an optional actor at `slot % QueuePageSize`. Each page also stores previous/next occupied page ids, while `ObservationSubscriberPageLists(feed)` stores exact head, tail, and live page count. First insertion appends one page; last removal unlinks it through at most two neighbors. Schedule updates still touch only old/new feed differences, and Contract Steps updates touch none.

`ObservationSubscriberCount` and `ObservationSubscriptionCount` expose bounded current cardinality. Try-state reconciles actor/feed ownership, free slots, page cells, reciprocal occupied links, list bounds, and counts. Fanout initializes from the feed-local head and follows exact page links, so an isolated subscriber in a historically high global slot costs one occupied-page unit rather than every lower page id.

`ObservationIngressRevisions` retains the highest accepted nonzero revision while a feed has at least one subscriber. Equal revision is an idempotent no-op, regression fails even after dirty cleanup, and final-subscriber removal deletes the baseline. Feeds without subscribers allocate neither baseline nor dirty state.

`DirtyObservationFeeds` coalesces each subscribed feed to one `latest_revision` plus its exact `latest_cause_provenance` and `latest_cause_block`, records the first clean-to-dirty block as `dirty_since`, and stores reciprocal previous/next active-dirty links. Progress binds `fanout_revision` plus the independently frozen `fanout_cause_provenance` and `fanout_cause_block`, occupied page, exact subscriber position, `Ordinary | Terminal` branch, and optional `retry_after`. Starting or restarting fanout copies the matching latest revision and cause together; a newer revision may replace only the latest pair while the active pair finishes. The retained source block lets ordinary cohort and scalar/terminal fallback consume the same causal authority without inferring age from worker phase. The protocol-fixed prepass cutoff prevents every current-block Observation cause from executing before N+1; aged work receives no repeated floor and remains ordinary FIFO work. Greater revisions preserve the uninterrupted interval timestamp; clean completion removes dirty state without removing the baseline, while later advancement starts a new interval.

The first fanout turn snapshots the latest revision and exact occupied head. Later turns resume from the retained subscriber position without replaying a committed prefix. Temporary queue or wakeup capacity persists `retry_after = current_block + 1`, so neither direct service nor the outer worker retries the same cursor in the same block.

`DirtyObservationListState` stores exact head, tail, fair cursor, and live count. First insertion initializes the list; later insertion appends through the current tail. Completion or last-subscriber removal unlinks at most two neighbors and repairs the cursor. Dirty ingress runs transactionally and reports one conservative Weight independent of subscriber count. Try-state walks the bounded list and reconciles reciprocal links, cursor membership, map ownership, and count.

Ingress rejects capacity exhaustion as `DirtyObservationCapacityExceeded` and broken reciprocal ownership as `DirtyObservationInvariant`. Its transaction restores every touched dirty link and list field on failure. The producer decides whether and how to surface rejection; recovery is an explicit later retry after host/operator cleanup, with no package-owned replay queue.

The independently metered fanout worker runs before actor execution in `on_idle`. It checks detector-local disabled authority before loading compact Actor state. Useful ordinary service classifies exact non-economic terminal precedence and prepares canonical queue or wakeup placement without an affordability probe. Contiguous queue candidates commit through one bounded append plan; candidates sharing one wakeup key commit under one rollback boundary. Mixed branches split into dense cohorts without creating a second readiness topology.

Terminal classification persists `TerminalDeferred`; a later scalar terminal turn owns cleanup separately from successful ordinary pages. Crossing transitions retain their source classification and block as causal evidence, while the fixed prepass execution cutoff uniformly prevents every current-block placement from executing in that block. Temporal occurrences remain `Deferred`; paid readiness and terminal substitution use the same FIFO/wakeup owners without a separate timing mode. The fair cursor advances only after its retained work commits. Completion deletes state only when `latest_revision == fanout_revision`; a newer revision restarts at the occupied head, so an older pass cannot erase a newer change.

Fanout first pays a branch probe, then admits the component-wise maximum of generated disabled-skip, queue, and wakeup page owners plus separately reserved fault-record Weight. Base, ordinary branches, scalar terminal cleanup, fault record, and fault clear retain disjoint owners. Exact fault identity is loaded only after transactional rollback and binds feed, revision, page, subscriber position, actor, semantic/body/admission authority, branch, and class. Host bounds set the independent page ceiling and Weight limit.

The typed `AddressEventIngress::preflight`/`notify` boundary owns signal, filtering, and funding-accumulator effects. Producers perform literal read-only preflight and invoke exactly one consequence under their declared post-movement or transactional-precommit atomicity protocol. They propagate rejection and never mutate `ActorHot` or funding storage directly; the host integration owns protocol inventory and rollback evidence.

`IngressFailure { error, retry }` classifies recoverable queue/wakeup capacity or placement unavailability as Temporary. Monotonic ticket/index exhaustion, topology corruption, invalid provenance, and invariant failure are Permanent. Actor tasks preserve the classification through `TaskFailure`; non-Actor producers map it to their outer dispatch error.

The package never scans host events, fingerprints value transfers, or defers ingress correctness to `on_idle`. Trigger filtering consumes only the independently supplied source; funding authorization consumes source and typed provenance without inferring either from the other. `OwnerOnly` and signed allowlists require Signed provenance plus a matching source, `AnyVerifiedIngress` requires at least one verified field, and all-None context remains funding-ineligible.

Host-decided `RuntimePolicy` receives both optional fields unchanged. Every accepted tracked transfer checked-adds into `funding_accumulated`; preflight rejects overflow before supported movement. Fresh-run opening takes and clears the accumulator atomically only after all fallible admission checks. Retry retains the frozen funding snapshot, later ingress accumulates for the next run, and completion, failure, or cancellation neither promotes nor restores funding.

### Manual

`manual_trigger` sets `ActorHot.pending_signal` only when the Actor's scalar trigger is `Manual`. Every other trigger fails with `ManualSourceDisabled`; paused calls fail with `ActorPaused`, and System Immutable calls fail with `ImmutableActor`. Manual readiness requests the FIFO under block cooldown/window gates. The latch clears when a signalled cycle starts and survives deferrals.

---

## Storage Topology

Primary storage follows explicit owners. Section 13's stable behavioral stores constrain compatibility, while bounded scheduler and ingress machinery remains replaceable implementation state. No synchronized readiness mirror remains.

- `NextActorId`: monotonic actor ID allocator
- `ActorIdentities`: one durable identity map for Active and Dormant actors, retaining owner, class/custody locator, mutability, sovereign account, `cycle_nonce`, and non-optional `last_control_mutation_block`
- `ActorHot`: active/paused lifecycle, closed Trigger runtime state, counters, pending readiness, block anchor, live queue ticket, exact typed wakeup pointer, and direct `terminal_at`
- `ActorContractHeads`: scalar trigger, schedule/window/completion authority, commitments, Step count, optional inline Step 0, and its optional resource envelope; the P32 generated `MaxEncodedLen` ceiling is 2,237 bytes
- `ActorAdmissionCertificates`: compact runtime/layout/Weight and lifecycle identity; generated descriptor maximum is 208 bytes under both B8 and P32
- `ActorContractTailChunks`: gap-free authority-bound Steps 1..N plus aligned envelopes in chunks of at most four; generated descriptor maximum is 4,070 bytes per chunk
- `ActorActivationAuthorities`: ObservationChange-only compact placement authority; generated descriptor maximum is 159 bytes
- `ActorFunding`: active-only canonical funding-source policy, bounded tracked-asset set, and `funding_accumulated[asset] = amount`; authorized ingress adds checked deltas, fresh cycle opening atomically takes the map as its frozen snapshot, and later ingress remains accumulated for the next cycle
- `ActorRunHeads` / `ActorRunPayloads`: mutable cursor/outcome/authority state separated from immutable Opening and funding snapshots; payload commitment and predicate-result count bind the pair
- `ActorIdentityCount`: transactionally maintained O(1) `ActorIdentities` cardinality bounded by `MaxActorIdentities`
- `ActiveActorCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against hot state, certified C6 geometry, compact activation authority, funding, and optional split run state
- `NextQueueTicket`: shared monotonic global age allocator and common block-start cutoff source
- `PrepassExecutionCutoff`: optional `(block, ticket frontier)` authority frozen by `on_initialize`; matching-block `NextBlock` Drain consumes it, TryRuntime rejects a frontier beyond `NextQueueTicket`, and absence/staleness is reserved for direct physical service fixtures rather than runtime timing conformance
- `QueueHead` / `QueueTail` / `QueueOccupancy` / `QueuePages`: one global physical FIFO with a shared O(1) topology preflight, transactional append and exact-live-head consume, exact unconsumed physical-entry capacity, actor-local live-ticket dedup/invalidation, transactional tombstone drain, full-page reclamation, and checked empty partial-tail alignment. Entries store global `ticket` plus `actor_id`; ticket publication/clearing commits with its physical mutation.
- `QueueOccupancy` includes tombstones, so physical tail gaps never weaken capacity; package coverage proves occupancy counts invalidated entries until the tombstone drain releases exactly their share, and the wakeup cursor's `MaxActiveActors` capacity overflow fails closed transactionally, preserving the actor's exact existing pointer and bucket.
- `WakeupPages` / `WakeupBuckets` / `WakeupCursorPages` / `WakeupCursorLen`: typed paged temporal topology with separate sparse block/tick cursors; `NextWakeupClock` fairly selects the first domain when both are due. `ActorHot.wakeup_pointer` is the sole temporal-membership authority. Try-state rejects key/clock/pointer/slot disagreement and invalid terminal membership, while accepting bounded stale physical tombstones until normal drain converges.
- Wakeup replacement is one closed storage transaction across existing pointer validation, page-slot removal, checked page/bucket live-count release, reciprocal neighbor unlinking, cursor removal, new cursor/page insertion, checked live-count acquisition, and the actor pointer rewrite. Missing pages or slots, non-reciprocal links, cursor disagreement, count underflow/overflow, or insertion exhaustion rolls back the exact existing schedule and returns topology corruption; intentional actor-local invalidation on terminal cleanup remains the only lazy stale-entry path.
- Cross-path falsifiers compare complete storage roots and event vectors around manual-trigger capacity fallback, schedule replacement, fresh post-attempt rearm, and Continuation retry rejection. Terminal-only wakeup plus live-ticket coexistence runs try-state, while the fixed-seed corruption corpus injects queue occupancy/page and wakeup cursor/slot/live-count contradictions and requires exact rollback on every rejected transition.
- `ActiveActorLimit`: explicit nonzero governance-configurable active cap bounded by `min(MaxActiveActors, MaxQueueLength)` and never below `ActiveActorCount`; zero has no fallback meaning and fails try-state
- `OwnerSlotBitmaps`: one fixed 256-bit User owner-slot bitmap per owner; all-zero values are absent and System Actors never consume it
- `SovereignIndex`: reverse index from sovereign account to active or dormant `actor_id`; vacant custody locators intentionally have no entry. Try-state reconciles every key/value against the identity and requires index cardinality to equal identity cardinality.
- `SystemSovereigns`: bounded lifetime registry from `SystemSovereignId` to `Vacant | Occupied(actor_id)`; close changes only occupancy, while reattachment creates a fresh actor id against the retained locator. Try-state rejects duplicate derived custody accounts, identity ownership on a vacant locator, and occupied locator/id/account/reverse-index disagreement.
- `SystemSovereignCount`: exact O(1) registry cardinality bounded by `MaxSystemSovereigns`; vacant locators remain capacity-consuming so their deterministic custody accounts stay recoverable. Try-state reconciles the count against the complete registry.
- `GlobalCircuitBreaker`: global scheduler halt flag
- `IdleStarvationState`: sparse `Healthy | Starving { consecutive_blocks } | Alerted { consecutive_blocks }` starvation transition state

### Pre-fork storage baseline

The package ships a fresh-genesis storage baseline and no historical `OnRuntimeUpgrade` bridge. Pallet genesis writes the current storage version; package and independent-runtime tests reconcile current/on-chain versions with `try_state`. A live downstream host owns any later bounded migration.

The alignment auditor maintains exact allowlists for remaining assertions. Execution sites are owned by canonical loading, bounded admission, exhaustive control mapping, integrity checks, or transactional finalization; pallet sites are owned by genesis-construction failure, integrity checks, or admitted helper preconditions. Any differently worded Actors pallet or execution panic site fails the full-tree audit.

The independent zero-topology runtime proves exact bounded-DNF SCALE round trips, metadata names, nonempty present-Precondition `try_state`, and Executive-submitted absent and present Precondition plans. The package test suite uses a names-and-order SCALE contract instead of isolated numeric pins; the metadata-derived Actors ABI manifest plus PAPI descriptors own variant indices, and the pallet error surface matches the corrected spec §12.2 list in both directions. Default, try-runtime, no-std, and runtime-benchmark profiles remain independent of DEOS types.

## Lifecycle State Machine

The implementation separates identity-only dormancy from active execution:

```text
Created Dormant ⇄ Active → Ready → Admitted → Running ⇄ Suspended → Completed/Deferred/Failed/Cancelled → TerminalPending → Closed
```

Lifecycle calls preserve the split-store boundary:

- `activate_actor` accepts one typed `ActorContract` and validates trigger/cooldown/window, Contract Steps, funding policy, optional auto-close target, tracked assets, resource bounds, class restrictions, active capacity, and the host-configured idle envelope. It then writes matching hot state, certified C6 fragments, optional activation authority, and funding for a Mutable identity.
- `deactivate_actor` transactionally clears queues, wakeups, trigger topology, hot state, every C6 head/certificate/tail fragment, activation authority, funding, and both run partitions while preserving identity, owner slot, sovereign address, nonce, and balances. Reactivation derives a fresh complete active epoch, so Dormant identity cannot own or ambiguously reference orphan body fragments.

Creation expresses dormancy by Contract absence: every creation path accepts `Option<ActorContract>`, while activation and simulation accept a direct Contract because their Dormant branch is impossible. No lineage/reopen call or explicit actor-id creation path remains.

Package lifecycle interpretation:

- `Normal cycle`: scheduler-owned `contract_steps` run; checked-increments the stored nonce before events, so a new actor's first run emits nonce `1` and the run from `u64::MAX - 1` emits and executes nonce `u64::MAX`; a later Active installation or run at stored exhaustion executes no normal steps or cycle events and closes either class with `CycleNonceExhausted`
- `Pure close`: prechecked actor-local state/index deletion; executes no cycle or task and emits `ActorClosed` exactly once
- `Lifecycle touch`: extrinsics such as `manual_trigger`, `pause_actor`, `permissionless_sweep`, and plan/schedule updates may detect terminal state before their normal mutation path; ordinary deposits into expired/closed sovereign addresses remain balance-only

Creation and mutability rules are explicit:

- Lowest-free-slot and exact-slot User creation accept `Option<ActorContract>`: `Some` installs the complete authored Contract and `None` creates identity-only dormancy.
- Fresh System creation allocates matching actor and custody-locator ids. `create_system_actor_at_sovereign_id` requires an allocated vacant locator, creates a fresh actor id with nonce zero, and accepts complete Active or Dormant input without inheriting lineage state.
- Mutable actors may replace the authored contract through `update_contract`; Immutable actors fix it for actor lifetime.
- User actors cannot admit a `Mint` Task in Contract Steps.
- Immutable System actors reject the `Manual` trigger at admission; no runtime extrinsic, including governance/root, can mutate, pause, manually trigger, or close one. Reattachment after terminal close creates a distinct identity and does not mutate the former actor.

Mandatory runtime-owned terminal transitions remain distinct from the control guard. Immutable System actors may use an execution window or another internal terminal condition; an actor with none may remain Active indefinitely under the current dispatch contract. Failure threshold and window expiry use pure cleanup. Only a runtime upgrade can replace this immutability contract.

Scheduler hygiene follows the specification's bounded liveness matrix:

- `next_eligible_at` combines block cooldown and window start for signalled Actors; timestamp cadence has its separate aligned-tick owner.
- Execution-created late enqueues join next-block queue state only when block eligibility reaches that point; later eligibility receives one typed wakeup.
- `Manual`, `AddressEvent`, and `ObservationChange` retain block cooldown/window gates. `Cadenced` admits neither and owns one timestamp Trigger deadline throughout its Active epoch.
- A suspended Cadenced Actor may own its ordinary block retry while one useful due occurrence consumes the Trigger deadline into deferred readiness. No timestamp deadline exists while latched; the next Opening rearms it from current consensus time.
- Closed or missing stale queue and wakeup entries are ignored deterministically.

Pallet regressions cover paused-pop-resume, cooldown, and pre-window ordering. Runtime integration proves actor-to-actor ingress remains queued across the `on_idle` boundary.

## Actors Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](../../../../docs/read-model.contract.en.md) split.

### Canonical on-chain Actor projections

The current pallet already provides chain-native bounded reads for live actor and scheduler truth through:

- `actor_hot(actor_id)` for lifecycle, identity/control, queue membership, cycle state, and cached bounds
- `actor_contract(actor_id)` for schedule/window and bounded `ContractSteps`
- `actor_funding(actor_id)` for funding policy, tracked assets, and the bounded accumulated-delta map
- `owner_slot_bitmap(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` recovery and `sovereign_index(sovereign)` lookup for bounded per-owner discovery/recovery
- Deterministic `sovereign_account_id_system(actor_id)` for System Actor addressing against the known runtime catalog
- Bounded scheduler / readiness / breaker surfaces such as `ActorHot.pending_signal`, `ActorHot.queue_ticket`, `ActorHot.wakeup_pointer`, `NextQueueTicket`, `QueueHead`, `QueueTail`, `QueueOccupancy`, `QueuePages`, paged wakeup stores, `ActiveActorLimit`, `GlobalCircuitBreaker`, and `IdleStarvationState`
- Live execution-side effects and bounded operational events

`SovereignAccountDeriver` maps User custody from `SCALE(ActorsPalletId, b"user", owner, owner_slot)` and System custody from `SCALE(ActorsPalletId, b"system", sovereign_id)`. The host owns the concrete infallible `AccountId` mapping; explicit tags separate the two deterministic account domains before hashing, and the DEOS runtime preserves its established 32-byte identities.

These are the authoritative bounded surfaces for known-actor inspection, per-owner recovery, scheduler state, and current operator observability.

### Indexed / materialized Actor views

The pallet intentionally does **not** promise these as canonical on-chain surfaces:

- Long-lived per-actor execution history
- Per-step timeline replay across many cycles
- Fleet-wide dashboards, rankings, and operator analytics across arbitrary actor sets
- Archived run logs or forensic traces beyond bounded recent on-chain observability

Those belong to events plus external indexing/materialization rather than permanent in-kernel storage.

### Current boundary for actor discovery

Actor discovery is intentionally split by use case:

- User-facing recovery/discovery is chain-native only within the bounded owner-slot space: read `owner_slot_bitmap(owner)`, derive occupied sovereign accounts, and resolve them through `sovereign_index`
- System Actor discovery is chain-native for the known runtime catalog because `actor_id` values and sovereign derivation are deterministic
- Arbitrary fleet-wide discovery across all actors is still an indexed/materialized view unless a future bounded runtime projection is added

## Extrinsics (Implementation Surface)

| Call | Extrinsic | Notes |
| --- | --- | --- |
| `0` | `create_user_actor` | fee; complete Active or Dormant contract input; no User `Mint` |
| `1` | `create_user_actor_at_slot` | exact slot; same complete Active or Dormant input |
| `2` | `create_system_actor` | governance origin; explicit mutability and complete Active or Dormant contract input |
| `3` | `create_system_actor_at_sovereign_id` | attach a fresh System identity to an allocated vacant custody locator with a complete Active or Dormant contract input |
| `4` | `pause_actor` | mutable actors only |
| `5` | `resume_actor` | mutable actors only |
| `6` | `manual_trigger` | set flag and enqueue/schedule |
| `7` | removed | retired separate funding mutation; `update_contract` owns authored replacement |
| `8` | `close_actor` | prechecked pure destruction in place |
| `9` | `update_contract` | mutable actors; atomically replace the complete authored Actor Contract |
| `10` | `set_global_circuit_breaker` | breaker control |
| `11` | `permissionless_sweep` | liveness touchpoint, no normal cycle |
| `12` | removed | retired separate steps/completion mutation; `update_contract` owns authored replacement |
| `13` | `set_active_actor_limit` | governance operational cap tuning |
| `14` | `permissionless_sweep_many` | bounded batch touchpoint, no direct enqueue |
| `15..=16` | reserved | retired field-specific auto-close mutations; `update_contract` owns authored replacement |
| `17` | removed | retired close-plan mutation |
| `18..=20` | reserved | retired transitional dormant creation calls; canonical User/System creation expresses dormancy as absent Contract |
| `21` | `activate_actor` | typed Active Actor Contract with schedule, `ContractSteps`, funding policy, and admission validation |
| `22` | `deactivate_actor` | remove contract/scheduler state while preserving identity and balances |

Calls `4`, `5`, `6`, `8`, `9`, `21`, and `22` use the class-specific control authority: signed owner for User actors, signed owner or governance for System actors. Active-only calls reject dormant identities; `close_actor` handles either lifecycle.

---

## Validation Coverage

Package validation lives in the `src/tests.rs` fixture/module root, domain suites under `src/tests/`, `src/benchmarking.rs`, the independent `embedding-runtime`, and compile-time exhaustive semantic contracts. Tests pin SCALE indices, storage names and types, actor-state decomposition, scheduler/trigger/lifecycle invariants, task atomicity, retry transitions, funding conservation, subscription topology, and try-state reconciliation.

Replayable state-machine traces cover suspend, continuation, cancellation, queue/wakeup uniqueness, owner slots, funding snapshot opening, balances, and observation churn. The seeded transition model drives create/activate/deactivate/fund/signal/trigger/pause/resume/contract-update/enqueue/wakeup/execute/close/slot-round-trip/suspend/continue/cancel sequences, installs Crossing contracts, publishes alternating observations, and materializes ordered Crossing work against conservation, cross-store invariants, and try-state after every operation. Temporary DEX failure and recovery exercise randomized fault suspension and repair. Each control operation admits only its typed lifecycle event family, including ordered cancellation and summary events when replacing, deactivating, or closing a suspended cycle; observation ingress and deferred Crossing materialization remain event-silent.

The ignored release profiles add 10,000-scale zero-match and homogeneous-herd evidence, maximum mixed wakeup/Crossing/broad-fanout materialization under the breaker, and a mixed Crossing branch profile. The mixed profile combines dense and sparse thresholds, distinct rearm values, both directions, paused and already-latched Actors, and an insolvent User Actor; it proves bounded multi-pass drain and canonical pending placement before permissionless insolvency cleanup.

Mandatory reactive falsifiers cover partial fanout followed by subscriber deactivation, subscriber removal and late re-addition during fanout, stale close entries draining as tombstones before a recreated slot runs, newer revision during page progress, queue saturation, protected User fee-native floor, fee-collector failure after admission, invalid `Fresh`, and nonce exhaustion for both classes, all ending in try-state.

FRAME benchmarks construct bounded worst-case package branches; every production host must generate and bind runtime-specific weights.

External-consumer profiles prove that the crate composes without DEOS types. Concrete runtime adapters, generated artifacts, stress SLOs, and operational gates belong to the integration architecture.

## Integration Handoff

A production host must bind generated `WeightInfo`, concrete adapters and origins, defensible queue/wakeup/actor bounds, fee conversion and collection, ingress producers, genesis actors, and independent runtime evidence. The package embedding guide owns that checklist; [`docs/actors.integration.en.md`](../../../../docs/actors.integration.en.md) records the DEOS realization.

Implementation mirror for [Specification](./specification.en.md).
