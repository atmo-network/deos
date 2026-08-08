# AAA Package Architecture

> Package: `pallet-deos-aaa`; Rust crate: `pallet_aaa`

This document maps the independently reusable crate implementation. A host supplies `PalletId`, account derivation context, origins, adapters, bounds, fees, genesis actors, and production weights. Concrete DEOS namespace, System accounts, TMCTOL plans, runtime adapters, and operational evidence belong in [`docs/aaa.integration.en.md`](../../../../docs/aaa.integration.en.md).

## Executive Summary

> This document records the shipped 0.7.10 package implementation; the standalone specification owns normative semantics and executable tests own conformance.

`pallet-deos-aaa` provides a deterministic scheduler, bounded execution model, typed trigger system, lifecycle state machine, and adapter-driven task runtime for User and System actors.

The crate assigns no economic roles, assets, recipients, routes, actor IDs, or chain policy. Host behavior enters through typed adapters, origins, account derivation, weight/fee conversion, explicit bounds, and genesis actor specifications. External obligations live in the [package-owned embedding guide](./embedding.md).

## Architecture Overview

### Design Principles

1. `Deterministic scheduling`: one monotonic paged FIFO plus exact paged temporal wakeups, deterministic ordering, and explicit per-block caps
2. `Execution safety`: explicit weight and fee admission gates before cycle start
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`FeeBudgetExhausted`, `BalanceExhausted`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `Hot-state decomposition`: `ActorHot` carries only measured scheduler/admission facts needed to avoid cold program or detailed funding decode; the retired synchronized readiness mirror must not return

Canonical writes target `ActorIdentity`, `ActorHot`, `ActorProgram`, and `ActorFunding` directly. `ActiveActorView` is the sole derived read/loaded context and has no write-back path. Scheduler probes load hot state before cold program state; operations that require cross-partition semantics derive one view rather than introducing overlapping purpose-specific aggregates. Further context types would duplicate field ownership without a measured read or interface reduction.

### Host Composition Boundary

AAA executes declarative plans against host-provided adapters. Ledger, market, liquidity, staking, fee, ingress, governance, and genesis policy remain outside the crate. The package never identifies a concrete pallet, asset, actor role, route, or recipient as canonical.

## Execution Model

### Actor Classes

| Class | Ownership | Mint task allowed | Typical usage |
| --- | --- | --- | --- |
| `User` | Signed owner + slot namespace | No | User automation |
| `System` | Governance origin | Yes | Protocol automation |

User recovery now has an explicit slot-targeted surface: the default `create_user_aaa` path allocates the lowest free slot, while `create_user_aaa_at_slot` recreates control for a chosen slot/sovereign deterministically.

Current owner-slot representation is fixed-width and runtime-shaped:

- `OwnerSlotBitmaps` stores one `[u8; 32]` bitmap per User owner; System AAA never consumes it
- `MaxOwnerSlots` is nonzero and at most `255`; every bit at or above the configured bound remains zero
- Slot `s` maps to byte `s / 8` and little-endian bit `s % 8`
- Default allocation scans the 32 bytes in ascending order for the lowest valid free bit, while exact-slot admission changes one validated bit
- Closing the final User actor deletes its all-zero bitmap

### Current Actor-State Shape

The package stores each actor identity once and each active actor across three active-epoch values:

- `ActorIdentities`: durable owner, `ActorClass`, mutability, sovereign account, logical-cycle nonce, and non-optional persistent control-mutation block shared by Active and Dormant lifecycle states
- `ActorHot`: typed Active lifecycle/run state, auto-close target, failure counter, `pending_signal`, queue/wakeup membership, terminal block, `schedule_anchor`, and optional `last_cycle_block`
- `ActorProgram`: trigger/cooldown schedule, optional execution window, cycle plan, and `Persistent | CloseAfterProductiveCycle` completion policy
- `ActorFunding`: canonical funding-source policy, bounded tracked assets, and bounded `funding_accumulated[asset]` checked deltas

`AaaCreated` carries `aaa_id`, owner, `actor_class`, mutability, sovereign account, and `initial_lifecycle`; User slot or System custody locator lives inside `ActorClass`. `aaa_id` exists only as each storage-map key. Dormant identity carries no timestamp. Activation or a pre-first-cycle schedule update derives eligibility from `schedule_anchor`, window start, cadence, and actor-stable jitter. The typed lifecycle forbids contradictory pause state.

Package consumers compose the split state explicitly. Scheduler, execution, lifecycle, liveness, wakeup, ingress, try-state, benchmarks, and tests combine `ActorIdentities + ActorHot + ActorProgram` through private helpers or the public `active_actor_view` Rust query helper. No synchronized compatibility storage mirrors those values.

This is intentionally more concrete than the paired specification: the spec defines the required logical field groups, while this document records the current package storage realization.

### Execution-Plan Structure

Each actor stores a bounded `ExecutionPlan` of ordered `Step`s. One configurable `MaxExecutionPlanSteps` binding applies identically to User and System actors across creation, activation, replacement, simulation, genesis, and benchmark construction. It must remain in `1..=255`; the current DEOS baseline is `8`. Mutable `RetryLater` admits only `1..=MaxRetryAttempts`, with the protocol-fixed metadata constant set to 10; package integrity checks both bounds and their checked composition.

- `conditions: ConditionSet<Condition, MaxConditionsPerStep>`
- `task: Task`
- `on_error: StepErrorPolicy` (`AbortCycle` / `ContinueNextStep` / Mutable-only `RetryLater { max_attempts }`)

`ConditionSet` is one non-nested `Always`, `All`, or `Any` aggregate per step. `Always` owns zero atoms; validated `All` and `Any` own one through `MaxConditionsPerStep`. The executor evaluates every atom without short-circuiting, aggregates once, and either prepares the one task or skips one cursor. Empty groups fail as `EmptyConditionSet`; `Any` never duplicates task execution or introduces successor data.

Evaluation fees derive from the current generated `condition_set_evaluation` and `fee_collection` weights through the host `WeightToFee`; cycle/suffix Weight accounting uses the same `ConditionSet::len()`. Mode, order, and truth position cannot change the configured atomic-count bound. Package helpers map empty vectors to `Always` and non-empty conjunctions to `All`; no parallel pricing, storage, or scheduler surface exists.

`ObservationProvider<FeedId, BlockNumber>` is the generic current-scalar boundary. The host receives `feed`, `now`, and `max_age_blocks`; `Fresh` returns both `value` and `observed_at`. AAA accepts Fresh only when `observed_at <= now` and checked age stays within the authored maximum. Future or over-age Fresh fails condition evaluation Permanently, while explicit Unavailable, Uninitialized, and Stale states produce ordinary `ConditionsNotMet`.

Plan validation rejects zero `max_age_blocks`, fixed-zero and percentage-zero amount resolutions, self-directed Transfer/SplitTransfer recipients, zero absolute input ceilings, zero liquidity minima, and identical swap/liquidity asset pairs. Creation predicts User custody from the first available or explicitly requested slot before fee collection; update and activation validate against the stored sovereign account.

`condition_set_evaluation(c)` charges full non-short-circuit evaluation independently of atom truth or aggregate mode. Package benchmarks cover `Always` plus one through `MaxConditionsPerStep` bounded atoms; production coefficients belong to the host-generated `WeightInfo`.

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

`SwapOut` groups authored output before explicit `InputLimit::{LiveQuote, Absolute(Balance)}` protection. `Absolute(0)` fails before storage; `Absolute(nonzero)` composes its ceiling with live preservable input capacity, while `LiveQuote` intentionally uses that capacity without an authored long-horizon ceiling. `DexOps::swap_exact_out` always receives the resulting finite bound.

Liquidity tasks also carry fixed non-zero outputs. `AddLiquidity.min_lp_out` reaches `LiquidityOps::add_liquidity`; the host adapter must reject a measured LP output below that bound.

`RemoveLiquidity.min_amount_a` and `min_amount_b` pass directly into Asset Conversion. Its two exact withdrawal-minimum errors classify as Temporary; malformed pair identity, missing indexed topology, and unknown downstream failures remain Permanent. The outer adapter transaction retains post-call balance-delta checks as defense in depth, so no success event or partial liquidity mutation survives either enforcement layer.

`StopCycle` executes only after its conditions and ordinary User fee collection succeed. It records `SimulationStepOutcome::Stopped`, emits `CycleStopped { aaa_id, cycle_nonce, step_index }`, and ends the logical cycle successfully at that cursor.

The shared completion path emits the cumulative summary, leaves later funding accumulation untouched, evaluates completion policy and auto-close, clears a resumed Continuation, and leaves the suffix unreachable.

The instruction does not resolve an amount, invoke a runtime adapter, select a successor, or directly mutate actor lifecycle/scheduler state. It increments `executed_steps` but not `committed_effectful_tasks`, so an empty stop cannot close a one-shot productive actor. A false condition advances normally; preparation or fee failure occurs before successful stop admission and follows the step's existing error policy.

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

`attempt_fee_envelope` owns User per-step evaluation/execution/total fees and checked suffix totals. `settle_attempt_fee_step` releases each selected upper reservation before selecting evaluation-only or attempted-step charge; System settlement stays zero.

Admission, execution, retry admission, simulation, and benchmarks derive the selected full-plan or Continuation-suffix fee envelope from the current bounded program. Overflow fails before mutation. The package-owned `fee_envelope_vectors` example emits deterministic User/System suffix, release, rollback-pricing, and protected-floor vectors consumed by browser forecasting tests.

Resolution and charging follow these rules:

- A User run releases the skipped step's unused execution-fee reservation before resolving later steps, matching every non-executable cycle path.
- A multi-amount task resolves every field before dispatch and selects `FundingUnavailable > Skipped > Executable` independently of field order.
- An Unstake last-funding plan fails validation when the runtime adapter cannot expose a transferable share asset.

Pallet boundary tests cover fixed, current/trigger/last-funding percentages, split totals, and `AllAvailable` across native, sufficient-asset, and staking-share surfaces. The embedding fixture binds unrelated host position keys to share assets without DEOS types.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier steps in the same execution plan remain committed.

`src/contract.rs` is the package-owned semantic classification surface. Exhaustive matches derive task adapter/assets/recipients/effects/availability/weight ownership/bounded algorithms, typed task amount roles with dependency and retry behavior, condition observations and purity, and error-policy controls.

`TaskWeightOwner::weight` selects the corresponding method on the runtime's single `WeightInfo`; the module adds no codec, storage, runtime API, or parallel numeric weight authority. Package tests instantiate every current primitive, while a new enum variant makes its owning match non-exhaustive. The package example `semantic_manifest` verifies ordered task and amount coverage against SCALE metadata and emits one deterministic format-neutral contract projection.

The control-flow firewall combines closed types with adversarial evidence. `Step` metadata must expose exactly `conditions`, `task`, and `on_error`; exhaustive variant contracts admit no successor, nested program, callback, generic `RuntimeCall`, or opaque dispatch field. `ConditionSet` classification fixes full atomic observation, whole-group error, one admitted task, false advance, and no nesting. Atomic condition and amount classifiers expose only read dependencies and never a control target.

`resolve_step_control` remains the only runtime transition owner, with the exhaustive policy/mutability/failure matrix and same-cursor Continuation regressions pinning retry identity. Task-local transactions forbid callback-visible partial mutation. Bounded vectors, adapter capability contracts, generated worst-case weights, maximum-plan tests, and circular scheduler stress bound adapter and cross-actor work without interpreting local plans recursively.

### Market Adapter Boundary

`DexOps` owns swap-only host execution; `LiquidityOps` owns add, remove, and donation operations. AAA supplies `ExecutionContext { actor, aaa_type }`, resolved amounts, and authored spend/output bounds without knowing route topology, market identity, or price-source policy.

Adapters return typed task failures. Only explicitly classified Temporary failures may enter Mutable `RetryLater`; every unknown downstream dispatch error remains Permanent. Task-local transactions roll back adapter mutations before step policy runs.

Host-specific quotes, route selection, fees, oracle guards, slippage policy, and failure mapping belong in the integration architecture and embedding evidence.

## Scheduler Architecture

### Hook Separation

- `on_initialize`:
  - bounded scheduler bookkeeping only
  - no cycle execution
- `on_idle`:
  - bounded temporal and queue housekeeping
  - bounded queue-driven cycle execution using remaining `on_idle` weight

### Admission Gates

A cycle is admitted only when all checks pass:

1. actor is ready (`trigger`, cooldown, pause/breaker/window checks)
2. per-block execution cap (`MaxExecutionsPerBlock`) not exceeded
3. a two-dimensional `WeightMeter` can consume the complete attempt plus measured pure-cleanup weight without exceeding RefTime or ProofSize
4. for User AAA: fee preflight covers the opening plan or unresolved retry suffix plus `MinUserBalance`

Attempt Weight and User fees are derived from the current bounded program at every use. Fresh attempts scan the full plan, while suspended attempts scan only the bounded `cursor..plan.len()` suffix and compose generated retry/suffix-admission classes.

Weight or scan deferral remains silent and state-preserving: no candidate identity, event, nonce, attempt, cursor, funding snapshot, or task effect changes. Persistent live-head Weight blockage, fee-collection failure, or invariant stall becomes observable only through sparse starvation transition events.

Deferral/terminal paths:

- insufficient Weight or scan budget → silent state-preserving deferral; actor remains active
- pure terminal cleanup prechecks every fallible identity, funding, count, reverse-index, and User-slot invariant before mutation; no close retry or requeue state exists
- Pre-cycle close precedence is deterministic: `WindowExpired` > `BalanceExhausted` > `FeeBudgetExhausted`
- User fee shortfall at admission → terminal `FeeBudgetExhausted` close
- `CycleResult::Completed` means authored control reached terminal without an abort: skip-only and all-failed-`ContinueNextStep` runs remain Completed, reset `consecutive_failures`, and may satisfy nonce auto-close. Their counters remain factual; only at least one committed non-`StopCycle` task satisfies productive close. Abort emits `Failed`; explicit invalidation emits `Cancelled`.
- Post-failure close is inclusive at `consecutive_failures >= MaxConsecutiveFailures`; an admitted cycle emits its authoritative `CycleSummary` before pure cleanup emits `AaaClosed`, matching post-success `AutoCloseNonceReached` ordering
- Explicit, automatic, lifecycle-touch, dormant, and sweep paths share one pure cleanup routine: no task, condition, fee, funding restoration, sovereign-balance movement, or shared queue/wakeup scan occurs
- Active close prevalidates identity, counts, reverse ownership, slot ownership, funding presence, and subscriptions, then commits cancellation, actor-store deletion, counter/locator release, and the close event in one storage transaction; any residual late error rolls back the complete terminal mutation
- Removing `ActorHot` lazily invalidates its ticket and wakeup pointer; bounded stale records converge through ordinary page draining
- Every bounded window validates checked `end + 1` representability, rejects overflow, and schedules that exact terminal block through `ActorHot.terminal_at`; trigger and terminal readiness share one live wakeup pointer and retain the earlier target
- Paused actors remain hot-only before terminal time and load `ActorProgram` only when closure is due
- With `GlobalCircuitBreaker` active, normal cycles and scheduler-owned terminal cleanup defer; bounded housekeeping plus explicit lifecycle/sweep cleanup remain available

`ActorProgram.completion_policy` defaults to `Persistent`. `CloseAfterProductiveCycle` checks cumulative `committed_effectful_tasks` only after successful logical-cycle completion, including a resumed Continuation. False/latest-state-rejected conditions, skips, rolled-back failures, bare `StopCycle`, suspension, abort, cancellation, and retry exhaustion cannot select `ProductiveCycleCompleted`. The pure close path remains valid for Immutable System actors and preserves their sovereign balances.

Code anchor: `src/execution.rs::execute_single_cycle_traced` increments the cumulative count after task commit and applies productive close after `CycleSummary`. Pallet tests prefixed `close_after_productive_cycle_` falsify false-state, latest-state race, bare stop, retry, exhaustion, balance, and Immutable closure claims.

Lifecycle lease-by-cycles is supported via `auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, actor closes with `AutoCloseNonceReached`. `set_auto_close_at_cycle_nonce` may set, shorten, extend, or clear the target, but every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`; incrementing starts from the existing target or current nonce when unset, rejects zero/overflow, and revalidates the resulting current-relative horizon.

### Fee Collection Boundary

The generic pallet collects creation and per-step User fees through one runtime-supplied `FeeCollector`; terminal cleanup charges no AAA fee. Both User creation calls collect `AaaCreationFee` for Active and Dormant admission before identity, slot, counter, or next-id mutation; failed collection leaves every actor store and owner balance unchanged. System creation remains exempt.

Fee collection is exact-once: one charge performs one read-only ingress preflight, one fee-native ledger movement, and one post-movement notification inside one outer transaction. The ledger-only movement primitive performs no generic transfer/transaction-extension ingress and no native-staking bridge; notifying the same movement twice is impossible, zero/no-op collection emits no ingress, and failure rolls back movement and all AAA state.

- Conditions and task preparation run read-only before collection determines the step outcome.
- Every attempted User step invokes `FeeCollector` at most once: condition/resolution/funding non-execution charges evaluation-only, while an executable step charges evaluation plus generated execution fee together.
- Collection failure rolls back the complete scheduler attempt before step policy, task dispatch, queue consumption, or any persistent fee, event, counter, nonce, cursor, or snapshot mutation. Simulation reports interface-local `FeeCollectionFailed`.
- After successful collection, adapter failure rolls back task-local effects but retains the one combined charge; `ContinueNextStep` and `AbortCycle` never alter that charge or trigger another collection.
- `fee_native_protected_minimum` applies `max(MinUserBalance, asset minimum)` to User fee-native direct preserve-spend capacity and `SwapOut` input capacity after the selected reservation; other assets retain their adapter minimum.

Pallet regressions cover each outcome, collection failure, one-call cardinality, task rollback, release-to-zero, direct User-floor preservation, and exact-output input-cap failure. User fee admission derives each step and complete-plan reserve from host-bound generated weights plus configured step fees; package tests reject execution immediately below that exact reserve.

### Progress-Preserving Continuation

`ActorHot.cycle_state` is the sole discovery marker. `Idle` reads no Continuation value; `Suspended` requires exactly one `ContinuationState[aaa_id]`. The sparse value stores scalar `cursor`, logical-cycle-wide `attempt`, `unsuccessful_attempts_at_cursor`, `last_attempt_block`, frozen typed suffix snapshots, and cumulative outcomes. Try-state enforces marker/store equivalence, cursor bounds, Mutable-only bounded retry policy, a live cursor-local count below its nonzero limit, and snapshot-surface validity.

Attempt `0` opens one logical cycle and increments `cycle_nonce` once. A Temporary `TaskFailure` or `FundingUnavailable` under `RetryLater { max_attempts }` increments both global failure state and the cursor-local count. The first suspension stores `1`; same-cursor suspension increments saturatingly, while a later cursor resets to `1`.

Post-attempt counters use checked addition. Inclusive local exhaustion closes with `RetryAttemptsExhausted`; when both cutoffs land together this reason wins, while an earlier global cutoff closes with `ConsecutiveFailures`. Exhaustion clears Continuation, emits `CycleSummary(Failed)` without `CycleCancelled`, then closes. An already-reached global cutoff closes before another `CycleStarted` or `CycleContinued`. Persisted retry reuses the nonce, omits external cadence, and executes only the suffix.

`scheduler::retry_backoff_blocks` maps persisted attempt `0, 1, 2, ...` to `1, 2, 4, 8, 8...` blocks. Eligibility uses the larger of this delay and schedule cooldown, omits external cadence for the open run, then respects window start. A one-block delay uses the existing next-block FIFO ticket; longer delays use the existing paged wakeup pointer.

`scheduler::timer_jitter_blocks` reads the first eight Blake2-256 bytes of SCALE-encoded `aaa_id` as little-endian `u64` and applies the protocol modulo window. This deterministic value spreads timer admission only; it carries no secrecy, unpredictability, targeting resistance, ordering protection, or MEV property.

`resolve_step_control` is the private exhaustive transition owner for completed, funding-unavailable, Permanent-failure, and Temporary-failure results across both mutability modes and all three error policies. Production execution calls it for adapter and funding decisions, and `simulate_current_program` inherits the same decision because it invokes `execute_single_cycle_traced`; a pure matrix regression pins all combinations.

Task-scoped rollback leaves earlier successful steps committed. The named `SwapIn → AddLiquidity → Transfer` and Burn-prefix regressions prove same-cursor retry, prefix non-replay, cumulative outcomes, and no cancellation compensation. The fixed-seed model `0xDE05_0730` independently checks sparse state, cursor progress, queue/wakeup uniqueness, funding accumulation, frozen cycle snapshots, and cancellation after each transition.

`simulate_current_program` is the package-owned rollback core behind versioned `AaaSimulationApi` runtime metadata. It requires exact stored Active program, actor type, mutability, mode/run-state, readiness, liveness, and User fee budget. Simulation and production admission use the same package-owned suffix envelope and predicate: checked fee-native balance above `MinUserBalance` must cover `attempt_fee_upper`; raw balance alone never admits a User attempt.

The API executes the production path with an optional bounded trace and exposes fresh or Continuation outcomes including `unsuccessful_attempts_at_cursor`. Terminal closure returns `SimulationStatus::Closed(CloseReason)`. The whole attempt runs inside `TransactionOutcome::Rollback`; package tests prove actor state, balances, events, fees, adapter effects, and stored Continuation remain unchanged.

Semantic execution-plan, funding-policy, schedule, window, deactivation, terminal, and close transitions share `cancel_continuation_internal`. Exact encoded plan/completion-policy, funding-policy, schedule/window, auto-close-target, and global active-limit updates return without storage or event mutation; semantic auto-close target changes preserve Continuation.

Every external control that invalidates or reconstructs ordinary membership shares one class-independent actor/block clock across signed-owner and governance origins; exact no-ops, internal transitions, and terminal cleanup remain exempt.

Cancellation emits `CycleCancelled` before one cumulative terminal `CycleSummary(Cancelled)` without compensation or prefix rollback. The closed reason set distinguishes explicit, plan, completion-policy, funding-policy, schedule, deactivation, typed `Closing(CloseReason)`, and runtime-upgrade causes; the last two remain available to terminal cleanup and the pending upgrade manifest.

`CycleStarted` appears once per nonce. `CycleContinued` and `CycleSuspended` carry `(aaa_id, cycle_nonce, attempt)`; suffix step events retain the nonce and their order belongs to the surrounding attempt boundary. Current sparse state is canonical-chain truth. Unbounded attempt history remains materialized.

### Read-Only Eligibility Projection

`aaa_eligibility` is the read-only `AaaEligibilityApi` projection. It mirrors `apply_admission` and reuses the exact cadence, retry, window, failure-limit, breaker, and latch owners, so clients do not reproduce scheduler arithmetic.

The projection reports one phase: `NotRegistered`, `Dormant`, `Ready`, `GlobalCircuitBreaker`, `CloseDue(CloseReason)`, `Paused`, `WaitingSignal`, `WaitingRetry`, or `WaitingTemporal`. `next_eligible_block` is `now` when ready, the next known temporal gate while waiting, or `None` when no future gate is computable.

The projection persists no state, emits no event, and promises no service. Queue position and available Weight still decide actual admission. Arithmetic overflow and malformed Continuation state return typed projection errors rather than an inferred phase.

Code anchor: `src/scheduler.rs::aaa_eligibility`; package tests prefixed `eligibility_projection_` falsify every phase and the `next_eligible_block` contract.

### Queue Execution Model (Monotonic Paged FIFO)

`classify_actor` owns terminal precedence, breaker and pause phase, retry/temporal timing, signal readiness, and cursor-sensitive User viability. Scheduler admission, sweep, simulation, eligibility, and certified ingress project that read-only result; malformed Active partitions and Continuation state map through typed classification errors.

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: a monotonic paged FIFO for work that can execute now and an exact paged temporal layer for later eligibility. Distinct wakeup blocks use a paged minimum heap; same-block actors occupy linked fixed-capacity pages.

1. **Wakeup drain**: the cursor exposes the earliest due block without scanning sparse gaps; one admitted unit consumes one slot, preserves a partial bucket at the same minimum, and either appends live readiness to the active FIFO or lazily discards a stale pointer
2. **Ingress admission**: each matched producer call applies funding independently and sets the unified boolean signal latch; actor-local queue/wakeup membership remains bounded and may join the active run queue in the same `on_idle` pass
3. **Block-start cutoff**: after already-due wakeups materialize, the scheduler snapshots global `NextQueueTicket`; every later append receives a ticket at or beyond that cutoff and cannot execute in the current block
4. **Canonical head**: tri-state discovery returns `Empty`, `Head`, or `Blocked` only for the one global FIFO. It stops on an incomplete probe rather than treating the head as absent.
5. **Strict ticket order**: the scheduler may lazily consume tombstones before the cutoff, but it cannot bypass a live or blocked head. One O(1) preflight owns every FIFO mutation: checked `tail - head == QueueOccupancy`, configured capacity, and canonical current head/tail page-slot layout. Enqueue, live-head consume, and tombstone drain run as closed storage transactions after that preflight. Corruption rolls back exactly and returns an invariant live-head stall rather than `Empty` or progress. The next live ticket receives the only execution offer.
6. **Execution ceiling**: `last_cycle_block`, the shared scan/execution ceilings, and the common cutoff prevent a second scheduler invocation or circular/self-enqueue graph from executing one actor twice in the same block. Untouched suffix entries remain physically in place without reconstruction.

`ActorHot.queue_ticket` is the sole live-membership marker for the canonical FIFO. Replacement, close, pause, and cancellation use actor-local invalidation; stale page entries drain lazily. Scheduler admission reserves the generated hot probe before reading `ActorHot`; paused or negative readiness consumes no `ActorProgram` or `ContinuationState` proof. Only a hot-positive actor reserves the separate program/admission probe.

No actor type, AAA id, execution share, or priority policy changes FIFO service. System and User actors receive bounded service exclusively by their global ticket order.

Wakeup draining, paged queue operations, actor probes, normal cycles, and pure terminal cleanup use two-dimensional `WeightMeter` fit checks. Direct ingress is charged at its originating producer. Close admission uses the generated worst-case User cleanup bound and performs no shared-container scan.

Scheduler accounting stays local unless it controls consensus progress. `scanned` and `executed` enforce independent per-pass ceilings; `QueueDrainStats` and `WakeupDrainStats` expose bounded operation results to callers, tests, and benchmarks without storage writes. Operators derive completed admission from `CycleStarted`/`CycleSummary` and persistent blockage from bounded queue/wakeup state plus starvation detection/recovery events. Loaded-actor and page-touch diagnostics remain stress instrumentation rather than permanent consensus counters.

### Temporal Wakeup Layer

The temporal wakeup layer owns future eligibility and admits it only through the active run queue. Every active actor may own at most one exact pointer; replacement and closure invalidate that slot without scanning actors or blocks. `MaxActiveActors` bounds global live wakeups, while `WakeupPageSize` controls I/O granularity rather than same-block capacity. This makes spillover buckets, placement-drop events, and actor-key retry scans unnecessary.

One placement owner (`schedule_next_work_local`) maps immediate readiness, next-block work, cadence/cooldown/window targets, fixed retry backoff, capacity recovery, and terminal expiry to either the FIFO or an exact wakeup. `defer_wakeup` and `wakeup_substrate_schedule` are its only temporal mutators. Queue saturation falls back to an exact next-block wakeup; fallback failure leaves no false success claim (spec 8.1.4).

Queue consumption, attempt effects/events, and all reachable post-attempt placement share one transaction. Ticket/page/wakeup exhaustion or topology failure rolls back the attempt and preserves the prior queue head. Wakeup removal and queue materialization likewise commit atomically or retain the exact wakeup.

Package coverage proves the strict post-worker cutoff: two manual triggers in one block latch one pending signal and one FIFO ticket, and exactly one `CycleStarted` is emitted (`executions(A, B) <= 1`).

The package uses the paged wakeup substrate and sparse cursor:

- `WakeupPages<(block, page_id)>` and per-block `WakeupBuckets` own the paged topology.
- `WakeupCursorPages` plus `WakeupCursorLen` provide the production paged binary min-heap over distinct wakeup blocks; each bucket owns its exact reverse `cursor_index`.
- Heap insertion, pop-min, and exact removal use at most `ceil(log2(MaxActiveActors))` sift steps, preserve contiguous cursor pages, and avoid scanning empty intermediate blocks; try-state reconciles page shape, uniqueness, ordering, and reverse indices. Maximum-depth generated evidence belongs to `scheduler_wakeup_cursor_insert`, `scheduler_wakeup_cursor_pop_min`, and `scheduler_wakeup_cursor_remove_exact`.
- `ActorHot` owns `WakeupPointer { block, page_id, slot }`.
- Pages use optional slots, a live count, a scan cursor, and bidirectional links.
- Transactional replacement invalidates the prior exact slot, removes an emptied block from the cursor, creates the replacement bucket and cursor entry atomically, and rolls back on reverse-index mismatch; bounded neighboring-page work unlinks empty pages.
- The cursor-driven overdue worker runs before the block-start queue cutoff, peeks sparse blocks, stops before future minima, and processes one slot per admitted unit. It meters cursor lookup, page scan, queue append, and possible full-depth cursor removal before mutation; partial progress keeps the same minimum for later resumption.
- The worker stops at the component-wise minimum of its dedicated `WakeupWeightLimit` and the actual `on_idle` remainder after fixed base and saturated queue cleanup. A complete cursor/drain unit must fit before mutation, returned hook Weight cannot exceed the caller budget, actor service receives the remainder, and there is no guarantee lending.
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

Recovery is governance-operated (circuit breaker or parameter adjustment); no emergency cycle execution occurs in `on_initialize`.

### Cadence

- Cadence readiness is deterministic only; AAA exposes no probability field, entropy provider, secure/insecure branch, hash fallback, probability event, or probability error.
- Delayed cadence derives actor-stable anti-storm jitter from `Blake2_256(aaa_id)`, and schedule validation includes the maximum reachable jitter (`window - 1`) within `MaxExecutionDelayBlocks`.
- Active installation and replacement prevalidate exact next-block, cooldown, maximum cadence-plus-jitter, and window-terminal targets before mutation. Runtime rearm derives cadence gates, retry backoff, and queue-capacity fallback with checked final-type arithmetic; an unrepresentable target fails closed as scheduler-index exhaustion instead of saturating into the current block or another semantic target. Saturation remains only in specification-owned observational elapsed-age calculations.
- `Cadenced::WhenSignalled` latches work immediately but applies cadence before scheduler admission. It retains one cadence wakeup while clean; a missed gate advances arithmetically to the next actor-stable gate, so a later signal cannot execute immediately against a stale first-eligibility anchor. `Cadenced::Always` re-arms from admitted-run cadence without a source.
- Any future probabilistic execution requires a separate append-only admission policy and a concrete financially secure runtime entropy contract rather than an optional field on deterministic cadence.

### Trigger Sources

Manual and AddressEvent ingress share `ActorHot.pending_signal` as one canonical readiness latch. The declared ObservationChange source targets that same latch once AAA-owned deferred fanout binds; no parallel actor signal key, source tag, generation, bitmask, or event-block metadata enters consensus state.

Filter surface:

- Source: `Any` / `OwnerOnly` / `Whitelist`
- Asset: `Any` / `Whitelist`

Each producer event evaluates every configured source atom without short-circuit. Several atoms matching one event fold into one readiness decision; funding/provenance mutation runs once outside that fold. Distinct producer events still apply funding independently, while pending readiness shares one latch and one bounded actor-queue membership without coalescing value effects.
When a signalled cycle starts, the latch is consumed atomically.

`OnObservationChange` ships as an append-only SCALE source atom at index `2`. Its payload contains one typed feed identity only; thresholds remain Condition-owned. `note_observation_changed` enters AAA dirty-feed state without a subscriber walk, amount payload, readiness mutation, or execution path. Deferred fanout owns conversion of latest revisions into the existing readiness latch.

Opening-snapshot surface validation runs at genesis, creation, activation, and plan replacement. It requires every staking-share mapping to exist but remains independent from trigger kind, signal payload, and event amount.

`ActorObservationFeeds` stores each active actor's canonical source-derived feed set. A reusable dense slot has exact `ObservationSubscriptionSlot` and reverse-owner entries. `ObservationFreeSlotPages` provides one-page LIFO allocation bounded by `MaxActiveActors`, so identity churn reuses slots rather than growing consensus keys.

`ObservationSubscriberPages(feed, slot / QueuePageSize)` stores an optional actor at `slot % QueuePageSize`. Each page also stores previous/next occupied page ids, while `ObservationSubscriberPageLists(feed)` stores exact head, tail, and live page count. First insertion appends one page; last removal unlinks it through at most two neighbors. Schedule updates still touch only old/new feed differences, and execution-plan updates touch none.

`ObservationSubscriberCount` and `ObservationSubscriptionCount` expose bounded current cardinality. Try-state reconciles actor/feed ownership, free slots, page cells, reciprocal occupied links, list bounds, and counts. Fanout initializes from the feed-local head and follows exact page links, so an isolated subscriber in a historically high global slot costs one occupied-page unit rather than every lower page id.

`ObservationIngressRevisions` retains the highest accepted nonzero revision while a feed has at least one subscriber. Equal revision is an idempotent no-op, regression fails even after dirty cleanup, and final-subscriber removal deletes the baseline. Feeds without subscribers allocate neither baseline nor dirty state.

`DirtyObservationFeeds` coalesces each subscribed feed to one `latest_revision` equal to its retained baseline, records the first clean-to-dirty block as `dirty_since`, and stores reciprocal previous/next active-dirty links. New state receives zero `fanout_revision` and no subscriber-page cursor. Greater revisions preserve the uninterrupted interval timestamp; clean completion removes dirty state without removing the baseline, while later advancement starts a new interval.

The first fanout unit snapshots the latest revision and exact occupied head; later units retain the next linked page id.

`DirtyObservationListState` stores exact head, tail, fair cursor, and live count. First insertion initializes the list; later insertion appends through the current tail. Completion or last-subscriber removal unlinks at most two neighbors and repairs the cursor. Dirty ingress runs transactionally and reports one conservative Weight independent of subscriber count. Try-state walks the bounded list and reconciles reciprocal links, cursor membership, map ownership, and count.

Ingress rejects capacity exhaustion as `DirtyObservationCapacityExceeded` and broken reciprocal ownership as `DirtyObservationInvariant`. Its transaction restores every touched dirty link and list field on failure. The producer decides whether and how to surface rejection; recovery is an explicit later retry after host/operator cleanup, with no package-owned replay queue.

The independently metered fanout worker runs before actor execution in `on_idle`. One admitted unit selects the exact active-dirty cursor and processes at most one occupied `QueuePageSize` subscriber page. It sets only `ActorHot.pending_signal`, preserves existing queue/wakeup membership, and requests canonical paged-queue admission only when neither exists. A capacity failure retains the exact page for retry after ordinary queue cleanup; the scheduler remains the sole execution owner.

The fair cursor advances to the selected feed's next active neighbor and wraps to the exact head without empty probes. Per-feed `fanout_revision` and `next_subscriber_page` preserve bounded progress. Completion deletes state only when `latest_revision == fanout_revision`; a newer revision resets the same state to the current occupied head, so an older pass cannot erase a newer change.

Fanout admits one complete worst-case page Weight in RefTime and ProofSize before mutation. Host bounds set the independent per-block page ceiling and weight limit. One unit short leaves dirty state, actor latches, queue membership, and cursor unchanged. Package benchmarks cover empty, completing dense, and queue-blocked branches; production coefficients and delivery envelopes belong to the host integration.

The typed `AddressEventIngress::preflight`/`notify` boundary owns signal, filtering, and funding-accumulator effects. Producers preflight before movement, notify exactly once in the originating transaction, propagate rejection, and never mutate `ActorHot` or funding storage directly.

`IngressFailure { error, retry }` classifies recoverable queue/wakeup capacity or placement unavailability as Temporary. Monotonic ticket/index exhaustion, topology corruption, invalid provenance, and invariant failure are Permanent. AAA tasks preserve the classification through `TaskFailure`; non-AAA producers map it to their outer dispatch error.

The package never scans host events, fingerprints value transfers, or defers ingress correctness to `on_idle`. Trigger filtering consumes only the independently supplied source; funding authorization consumes source and typed provenance without inferring either from the other. `OwnerOnly` and signed allowlists require Signed provenance plus a matching source, `AnyVerifiedIngress` requires at least one verified field, and all-None context remains funding-ineligible.

Host-decided `RuntimePolicy` receives both optional fields unchanged. Every accepted tracked transfer checked-adds into `funding_accumulated`; preflight rejects overflow before supported movement. Fresh-run opening takes and clears the accumulator atomically only after all fallible admission checks. Retry retains the frozen funding snapshot, later ingress accumulates for the next run, and completion, failure, or cancellation neither promotes nor restores funding.

### Manual

`manual_trigger` sets the shared `ActorHot.pending_signal` latch only when the canonical source set includes Manual. Missing Manual fails with `ManualSourceDisabled`, paused calls fail with `AaaPaused`, and System Immutable calls fail with `ImmutableAaa`. Immediate policies request the active FIFO; Cadenced policies retain one future wakeup and do not permit Manual to bypass cadence. The latch clears when a signalled cycle starts and survives deferrals.

---

## Storage Topology

Primary storage follows explicit owners. Section 13's stable behavioral stores constrain compatibility, while bounded scheduler and ingress machinery remains replaceable implementation state. No synchronized readiness mirror remains.

- `NextAaaId`: monotonic AAA id allocator
- `ActorIdentities`: one durable identity map for Active and Dormant actors, retaining owner, class/custody locator, mutability, sovereign account, `cycle_nonce`, and non-optional `last_control_mutation_block`
- `ActorHot`: active/paused lifecycle, counters, pending readiness, eligibility anchors, live queue ticket, exact paged wakeup pointer, and direct `terminal_at`
- `ActorProgram`: active schedule/window plus the bounded cycle plan; the metadata maximum is 7,144 bytes
- `ActorFunding`: active-only canonical funding-source policy, bounded tracked-asset set, and `funding_accumulated[asset] = amount`; authorized ingress adds checked deltas, fresh cycle opening atomically takes the map as its frozen snapshot, and later ingress remains accumulated for the next cycle
- `ActorIdentityCount`: transactionally maintained O(1) `ActorIdentities` cardinality bounded by `MaxActorIdentities`
- `ActiveAaaCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against `ActorHot`, `ActorProgram`, and `ActorFunding`
- `NextQueueTicket`: shared monotonic global age allocator and common block-start cutoff source
- `QueueHead` / `QueueTail` / `QueueOccupancy` / `QueuePages`: one global physical FIFO with a shared O(1) topology preflight, transactional append and exact-live-head consume, exact unconsumed physical-entry capacity, actor-local live-ticket dedup/invalidation, transactional tombstone drain, full-page reclamation, and checked empty partial-tail alignment. Entries store global `ticket` plus `aaa_id`; ticket publication/clearing commits with its physical mutation.
- `QueueOccupancy` includes tombstones, so physical tail gaps never weaken capacity; package coverage proves occupancy counts invalidated entries until the tombstone drain releases exactly their share, and the wakeup cursor's `MaxActiveActors` capacity overflow fails closed transactionally, preserving the actor's exact existing pointer and bucket.
- `WakeupPages` / `WakeupBuckets` / `WakeupCursorPages` / `WakeupCursorLen`: exact paged temporal topology and sparse minimum cursor; `ActorHot.wakeup_pointer` is the sole ordinary temporal-membership authority, while `terminal_at + aaa_id` owns terminal membership/removal. Try-state rejects pointer/slot disagreement, terminal drift, and pointers beyond the terminal block, but accepts bounded stale physical wakeup tombstones until normal drain converges.
- Wakeup replacement is one closed storage transaction across existing pointer validation, page-slot removal, checked page/bucket live-count release, reciprocal neighbor unlinking, cursor removal, new cursor/page insertion, checked live-count acquisition, and the actor pointer rewrite. Missing pages or slots, non-reciprocal links, cursor disagreement, count underflow/overflow, or insertion exhaustion rolls back the exact existing schedule and returns topology corruption; intentional actor-local invalidation on terminal cleanup remains the only lazy stale-entry path.
- Cross-path falsifiers compare complete storage roots and event vectors around manual-trigger capacity fallback, schedule replacement, fresh post-attempt rearm, and Continuation retry rejection. Terminal-only wakeup plus live-ticket coexistence runs try-state, while the fixed-seed corruption corpus injects queue occupancy/page and wakeup cursor/slot/live-count contradictions and requires exact rollback on every rejected transition.
- `ActiveActorLimit`: explicit nonzero governance-configurable active cap bounded by `min(MaxActiveActors, MaxQueueLength)` and never below `ActiveAaaCount`; zero has no fallback meaning and fails try-state
- `OwnerSlotBitmaps`: one fixed 256-bit User owner-slot bitmap per owner; all-zero values are absent and System AAA never consumes it
- `SovereignIndex`: reverse index from sovereign account to active or dormant `aaa_id`; vacant custody locators intentionally have no entry
- `SystemSovereigns`: bounded lifetime registry from `SystemSovereignId` to `Vacant | Occupied(aaa_id)`; close changes only occupancy, while reattachment creates a fresh actor id against the retained locator
- `SystemSovereignCount`: exact O(1) registry cardinality bounded by `MaxSystemSovereigns`; vacant locators remain capacity-consuming so their deterministic custody accounts stay recoverable
- `GlobalCircuitBreaker`: global scheduler halt flag
- `IdleStarvationState`: sparse `Healthy | Starving { consecutive_blocks } | Alerted { consecutive_blocks }` starvation transition state

### Pre-fork storage baseline

The package ships a fresh-genesis storage baseline and no historical `OnRuntimeUpgrade` bridge. Pallet genesis writes the current storage version; package and independent-runtime tests reconcile current/on-chain versions with `try_state`. A live downstream host owns any later bounded migration.

The independent zero-topology runtime proves exact `ConditionSet` SCALE round trips, metadata names, nonempty aggregate `try_state`, and one Executive-submitted `Always → All → Any` plan. The package test suite uses a names-and-order SCALE contract instead of isolated numeric pins; the metadata-derived AAA ABI manifest plus PAPI descriptors own variant indices, and the pallet error surface matches the corrected spec §12.2 list in both directions. Default, try-runtime, no-std, and runtime-benchmark profiles remain independent of DEOS types.

## Lifecycle State Machine

The implementation separates identity-only dormancy from active execution:

```text
Created Dormant ⇄ Active → Ready → Admitted → Running ⇄ Suspended → Completed/Deferred/Failed/Cancelled → TerminalPending → Closed
```

Lifecycle calls preserve the split-store boundary:

- `activate_aaa` accepts typed `ProgramInput::Active(ActiveProgramInput)` and validates schedule/window, cycle plan, funding policy, optional auto-close target, tracked assets, cached bounds, class restrictions, active capacity, and the host-configured idle envelope. It then creates matching `ActorHot`, `ActorProgram`, and `ActorFunding` entries for a Mutable identity; `ProgramInput::Dormant` is rejected.
- `deactivate_aaa` clears queues, wakeups, pending signal, funding, cycle, and fee state while preserving identity, owner slot, sovereign address, and balances.

Active and dormant creation normalize into one typed internal boundary. Every creation path consumes `ProgramInput`; no lineage/reopen call or explicit actor-id creation path remains.

Package lifecycle interpretation:

- `Normal cycle`: scheduler-owned `execution_plan` run; checked-increments the stored nonce before events, so a new actor's first run emits nonce `1` and the run from `u64::MAX - 1` emits and executes nonce `u64::MAX`; a later Active installation or run at stored exhaustion executes no normal steps or cycle events and closes either class with `CycleNonceExhausted`
- `Pure close`: prechecked actor-local state/index deletion; executes no cycle or task and emits `AaaClosed` exactly once
- `Lifecycle touch`: extrinsics such as `manual_trigger`, `pause_aaa`, `permissionless_sweep`, and plan/schedule updates may detect terminal state before their normal mutation path; ordinary deposits into expired/closed sovereign addresses remain balance-only

Creation and mutability rules are explicit:

- Lowest-free-slot and exact-slot User creation accept complete typed `ProgramInput`: Active programs carry one `ActiveProgramInput` with schedule/window, cycle plan, completion/funding policy, and optional auto-close target; Dormant identities carry no program.
- Fresh System creation allocates matching actor and custody-locator ids. `create_system_aaa_at_sovereign_id` requires an allocated vacant locator, creates a fresh actor id with nonce zero, and accepts complete Active or Dormant input without inheriting lineage state.
- Mutable actors may replace the cycle plan through `update_execution_plan`; Immutable actors fix it for actor lifetime.
- User actors cannot admit `Mint` in the cycle plan.
- Immutable System actors reject Manual sources at admission; no runtime extrinsic, including governance/root, can mutate, pause, manually trigger, or close one. Reattachment after terminal close creates a distinct identity and does not mutate the former actor.

Mandatory runtime-owned terminal transitions remain distinct from the control guard. Immutable System actors may use an execution window or another internal terminal condition; an actor with none may remain Active indefinitely under the current dispatch contract. Failure threshold and window expiry use pure cleanup. Only a runtime upgrade can replace this immutability contract.

Scheduler hygiene follows the specification's bounded liveness matrix:

- One `next_eligible_at` calculation combines admitted-run cooldown, deterministic cadence plus actor-stable jitter, and window start.
- Execution-created late enqueues join next-block queue state only when eligibility reaches that block; later eligibility receives one wakeup.
- Immediate sources omit cadence but retain cooldown and window gates; sources under `Cadenced::WhenSignalled` retain cadence as well.
- Paused Cadenced actors consume no continuation after a due wakeup; resume re-primes from effective eligibility. Pending signalled actors re-prime under their configured Immediate or Cadenced gate.
- Closed or missing stale queue and wakeup entries are ignored deterministically.

Pallet regressions cover paused-pop-resume, cooldown, and pre-window ordering. Runtime integration proves actor-to-actor ingress remains queued across the `on_idle` boundary.

## AAA Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](../../../../docs/read-model.contract.en.md) split.

### Canonical on-chain AAA projections

The current pallet already provides chain-native bounded reads for live actor and scheduler truth through:

- `actor_hot(aaa_id)` for lifecycle, identity/control, queue membership, cycle state, and cached bounds
- `actor_program(aaa_id)` for schedule/window and bounded cycle plan
- `actor_funding(aaa_id)` for funding policy, tracked assets, and the bounded accumulated-delta map
- `owner_slot_bitmap(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` recovery and `sovereign_index(sovereign)` lookup for bounded per-owner discovery/recovery
- Deterministic `sovereign_account_id_system(aaa_id)` for System AAA addressing against the known runtime catalog
- Bounded scheduler / readiness / breaker surfaces such as `ActorHot.pending_signal`, `ActorHot.queue_ticket`, `ActorHot.wakeup_pointer`, `NextQueueTicket`, `QueueHead`, `QueueTail`, `QueueOccupancy`, `QueuePages`, paged wakeup stores, `ActiveActorLimit`, `GlobalCircuitBreaker`, and `IdleStarvationState`
- Live execution-side effects and bounded operational events

User custody derives from `SCALE(AaaPalletId, b"user", owner, owner_slot)` while System custody derives from `SCALE(AaaPalletId, b"system", sovereign_id)`. The explicit tags separate the two deterministic account domains before hashing.

These are the authoritative bounded surfaces for known-actor inspection, per-owner recovery, scheduler state, and current operator observability.

### Indexed / materialized AAA views

The pallet intentionally does **not** promise these as canonical on-chain surfaces:

- Long-lived per-actor execution history
- Per-step timeline replay across many cycles
- Fleet-wide dashboards, rankings, and operator analytics across arbitrary actor sets
- Archived run logs or forensic traces beyond bounded recent on-chain observability

Those belong to events plus external indexing/materialization rather than permanent in-kernel storage.

### Current boundary for actor discovery

AAA discovery is intentionally split by use case:

- User-facing recovery/discovery is chain-native only within the bounded owner-slot space: read `owner_slot_bitmap(owner)`, derive occupied sovereign accounts, and resolve them through `sovereign_index`
- System AAA discovery is chain-native for the known runtime catalog because `aaa_id` values and sovereign derivation are deterministic
- Arbitrary fleet-wide discovery across all actors is still an indexed/materialized view unless a future bounded runtime projection is added

## Extrinsics (Implementation Surface)

| Call | Extrinsic | Notes |
| --- | --- | --- |
| `0` | `create_user_aaa` | fee; complete Active or Dormant program input; no User `Mint` |
| `1` | `create_user_aaa_at_slot` | exact slot; same complete Active or Dormant input |
| `2` | `create_system_aaa` | governance origin; explicit mutability and complete Active or Dormant program input |
| `3` | `create_system_aaa_at_sovereign_id` | attach a fresh System identity to an allocated vacant custody locator with a complete Active or Dormant program input |
| `4` | `pause_aaa` | mutable actors only |
| `5` | `resume_aaa` | mutable actors only |
| `6` | `manual_trigger` | set flag and enqueue/schedule |
| `7` | `update_funding_source_policy` | mutable actors; keeps existing batches |
| `8` | `close_aaa` | prechecked pure destruction in place |
| `9` | `update_schedule` | mutable actors only |
| `10` | `set_global_circuit_breaker` | breaker control |
| `11` | `permissionless_sweep` | liveness touchpoint, no normal cycle |
| `12` | `update_execution_plan` | mutable actors only, re-derive assets |
| `13` | `set_active_actor_limit` | governance operational cap tuning |
| `14` | `permissionless_sweep_many` | bounded batch touchpoint, no direct enqueue |
| `15` | `set_auto_close_at_cycle_nonce` | set/clear cycle lease with horizon checks |
| `16` | `increment_auto_close_nonce` | extend cycle lease, checked and bounded |
| `17` | removed | retired pre-launch close-plan mutation |
| `18..=20` | reserved | retired transitional dormant creation calls; canonical User/System creation accepts `ProgramInput::Dormant` |
| `21` | `activate_aaa` | typed Active program with schedule, cycle plan, funding policy, and admission validation |
| `22` | `deactivate_aaa` | remove program/scheduler state while preserving identity and balances |

Calls `4`, `5`, `6`, `7`, `8`, `9`, `12`, `15`, `16`, `21`, and `22` use the class-specific control authority: signed owner for User actors, signed owner or governance for System actors. Active-only calls reject dormant identities; `close_aaa` handles either lifecycle.

---

## Validation Coverage

Package validation lives in `src/tests.rs`, `src/benchmarking.rs`, the independent `embedding-runtime`, and compile-time exhaustive semantic contracts. Tests pin SCALE indices, storage names and types, actor-state decomposition, scheduler/trigger/lifecycle invariants, task atomicity, retry transitions, funding conservation, subscription topology, and try-state reconciliation.

Replayable state-machine traces cover suspend, continuation, cancellation, queue/wakeup uniqueness, owner slots, funding snapshot opening, balances, and observation churn. The generated transition model drives create/activate/deactivate/fund/signal/trigger/pause/resume/program-update/enqueue/wakeup/execute/close/slot-round-trip/suspend/continue/cancel sequences against cross-store invariants and try-state.

Mandatory reactive falsifiers cover partial fanout followed by subscriber deactivation, subscriber removal and late re-addition during fanout, stale close entries draining as tombstones before a recreated slot runs, newer revision during page progress, queue saturation, protected User fee-native floor, fee-collector failure after admission, invalid `Fresh`, and nonce exhaustion for both classes, all ending in try-state.

FRAME benchmarks construct bounded worst-case package branches; every production host must generate and bind runtime-specific weights.

External-consumer profiles prove that the crate composes without DEOS types. Concrete runtime adapters, generated artifacts, stress SLOs, and operational gates belong to the integration architecture.

## Integration Handoff

A production host must bind generated `WeightInfo`, concrete adapters and origins, defensible queue/wakeup/actor bounds, fee conversion and collection, ingress producers, genesis actors, and independent runtime evidence. The package embedding guide owns that checklist; [`docs/aaa.integration.en.md`](../../../../docs/aaa.integration.en.md) records the DEOS realization.

Implementation mirror for [Specification](./specification.en.md).
