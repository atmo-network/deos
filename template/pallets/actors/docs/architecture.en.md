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
2. `Execution safety`: explicit weight and fee admission gates before cycle start
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`FeeBudgetExhausted`, `BalanceExhausted`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `Hot-state decomposition`: `ActorHot` carries only measured scheduler/admission facts needed to avoid cold contract or detailed funding decode; the retired synchronized readiness mirror must not return

Canonical writes target `ActorIdentity`, `ActorHot`, `ActorContract`, and `ActorFunding` directly. `ActorFunding.trigger_state_bond` stores the exact runtime-policy amount for every Active User and zero for every System Actor. The host `TriggerStateBond` adapter adjusts refundable owner custody inside the same storage transaction before Trigger topology commits; Active creation and Dormant activation move zero to the family amount, Active replacement moves the stored amount to the prospective family amount, and deactivation or any terminal close moves it to zero before partition removal. TryRuntime recomputes the exact class/Trigger amount; the formula depends only on authored Trigger semantics, never current index occupancy. `TriggerTransitionPlan` is the shared read-only preflight owner for Crossing membership and broad observation subscriptions; it distinguishes genesis installation, Active creation, Dormant activation, Active replacement, deactivation, and close. `crossing.rs` and `subscriptions.rs` return bounded commit inputs, while lifecycle code commits both inside the enclosing control transaction before canonical Contract/Hot replacement. Schedule, placement, and bond planning remain separate implementation work. One crate-private loader reads those partitions plus `ContinuationState` and classifies the actor as `NotRegistered`, identity-only `Dormant`, exact `Active`, or `Corrupt`; exact Active requires all four canonical partitions and Continuation if and only if `cycle_state == Suspended`. Its loaded state has no write-back path, and the public `ActiveActorState` returns the canonical partitions without flattening. No derived context owns authored equality or mutation.

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

User recovery now has an explicit slot-targeted surface: the default `create_user_actor` path allocates the lowest free slot, while `create_user_actor_at_slot` recreates control for a chosen slot/sovereign deterministically.

Current owner-slot representation is fixed-width and runtime-shaped:

- `OwnerSlotBitmaps` stores one `[u8; 32]` bitmap per User owner; System Actors never consume it
- `MaxOwnerSlots` is nonzero and at most `255`; every bit at or above the configured bound remains zero
- Slot `s` maps to byte `s / 8` and little-endian bit `s % 8`
- Default allocation scans the 32 bytes in ascending order for the lowest valid free bit, while exact-slot admission changes one validated bit
- Closing the final User actor deletes its all-zero bitmap

### Current Actor-State Shape

The package stores each actor identity once and each active actor across three active-epoch values:

- `ActorIdentities`: durable owner, `ActorClass`, mutability, sovereign account, logical-cycle nonce, and non-optional persistent control-mutation block shared by Active and Dormant lifecycle states
- `ActorHot`: typed Active lifecycle/run state, auto-close target, failure counter, `pending_signal`, queue/wakeup membership, terminal block, `schedule_anchor`, and optional `last_cycle_block`
- `ActorContract`: trigger/cooldown schedule, optional execution window, ordered `ContractSteps`, and `Persistent | CloseAfterProductiveCycle` completion policy
- `ActorFunding`: canonical funding-source policy, bounded tracked assets, and bounded `funding_accumulated[asset]` checked deltas

`ActorCreated` carries `actor_id`, owner, `actor_class`, mutability, sovereign account, and `initial_lifecycle`; User slot or System custody locator lives inside `ActorClass`. `actor_id` exists only as each storage-map key. Dormant identity carries no timestamp. Activation or schedule replacement derives block eligibility from `schedule_anchor` and window start, while `ActorHot.trigger_runtime_state::Cadenced` owns the optional timestamp-ceiled anchor tick. The typed lifecycle forbids contradictory pause state.

Package internals classify `ActorIdentities + ActorHot + ActorContract + ActorFunding + optional ContinuationState` through one crate-private loader before deriving an execution view. External consumers use `active_actor_state`, which returns those canonical partitions without flattening or synchronized mirrors.

This is intentionally more concrete than the paired specification: the spec defines the required logical field groups, while this document records the current package storage realization.

### Contract Steps Structure

Each actor stores a bounded `ContractSteps` of ordered `Step`s. One configurable `MaxContractSteps` binding applies identically to User and System actors across creation, activation, replacement, simulation, genesis, and benchmark construction. It must remain in `1..=255`; the current DEOS baseline is `8`. Mutable `RetryLater` admits only `2..=MaxRetryAttempts`, with the protocol-fixed metadata constant set to 10; package integrity checks both bounds and their checked composition.

- `precondition: Option<Precondition<Predicate, MaxPreconditionClauses, MaxPredicatesPerClause>>`
- `task: Task`
- `on_error: StepErrorPolicy` (`AbortCycle` / `ContinueNextStep` / Mutable-only `RetryLater { max_attempts }`)

`None` is the sole unconditional Step form. `Some(Precondition { clauses })` stores one bounded DNF: outer clauses are OR and each inner clause is AND. Runtime admission rejects empty outer and inner vectors, caps each dimension at four and each step at four total predicates, evaluates every admitted predicate without short-circuit, and executes or skips the step exactly once.

Each `TimedPredicate` names `ObservationTiming::Opening` or `Current`. Fresh-cycle opening evaluates and stores one `Result<bool, PredicateError>` per Opening predicate before any task; Continuation retains the full bounded result vector and reuses it by canonical linear position. Current predicates evaluate immediately before their step and therefore observe successful earlier-step effects in the same attempt.

Admission sorts predicates and clauses by canonical typed SCALE, removes repeated predicates within a clause, rejects clauses that become semantically identical, absorbs exact predicate-superset clauses, and stores only the canonical form. `update_contract` canonicalizes before equality and returns an exact no-op before rate limiting, cancellation, writes, placement reconstruction, or events when the resulting contract is unchanged.

Evaluation fees and cycle/suffix Weight use `evaluation_units = total predicates + Opening predicates`, then chunk the units by `MaxPredicatesPerStep` before calling the benchmarked component. This accounts for opening capture plus full expression visitation without relying on the generated component clamp. A false DNF expression emits `StepSkipped(PreconditionFalse)` and advances one fixed cursor; evaluation errors remain task-independent failures routed through the authored step policy.

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
| Trigger | Exactly one of `Manual`, `AddressEvent`, `ObservationChange`, `ObservationCrossing`, or `Cadenced`; source `Any`, `OwnerOnly`, `Whitelist`; asset `Any`, `Whitelist` | Actor Contract constructors plus raw typed calls; exhaustive Active replacement and Dormant lifecycle matrices; manual, certified-ingress, observation, timestamp-cadence, and embedding tests |
| Funding | `OwnerOnly`, `SignedAllowlist`, `RuntimePolicy`, `AnyVerifiedIngress`; provenance `Signed`, `InternalProtocol`, `Xcm` | Active contract and certified producer constructors; funding-policy package tests and DEOS producer inventory |
| Completion and step policy | `Persistent`, `CloseAfterProductiveCycle`; `AbortCycle`, `ContinueNextStep`, `RetryLater` | Active contract constructors; productive-close and exhaustive transition-matrix tests |
| Attempt and step views | `AttemptDisposition::{Completed, Failed, Suspended, Closed}`; `StepOutcome::{Executed, Stopped, Skipped, FundingUnavailable, Failed}` | Shared production evaluator; production/simulation parity tests |
| Eligibility view | `NotRegistered`, `Dormant`; Active phases `Ready`, `Paused`, `GlobalCircuitBreaker`, `WaitingSignal`, `WaitingRetry`, `WaitingBlock`, `WaitingCadenceTick` plus terminal reason | `actor_eligibility`; `eligibility_projection_*` tests |
| Simulation | `FreshCurrentPlan`, `CurrentContinuation`; every `SimulationError` in specification Section 7.2 | `simulate_current_contract`; package simulation and independent runtime tests |
| Adapter result | `RetryClass::{Permanent, Temporary}`; scalar observation `Unavailable`, `Uninitialized`, `Fresh`, `Stale` | Host adapters and fail-closed unit implementations; runtime Oracle mapping, retry matrix, and embedding tests |
| Event | Every variant in specification Section 8 and runtime metadata | Production `deposit_event` sites; event-order, task, lifecycle, ingress, scheduler, and generated ABI tests |
| Error | Every variant in specification Section 9.2 and runtime metadata | Production `ensure!`/error branches; rejection, rollback, exact metadata/spec, package, and embedding tests |

`CancellationReason::RuntimeUpgrade` and semantic-manifest `ContextDependency::None` were removed because neither had a production constructor. Runtime upgrades remain migration-specific work under specification Section 9.4 rather than a permanently encoded placeholder. Every amount classifier now reports its actual task-policy dependency.

`SwapOut` groups authored output before explicit `InputLimit::{LiveQuote, Absolute(Balance)}` protection. `Absolute(0)` fails before storage; `Absolute(nonzero)` composes its ceiling with live preservable input capacity, while `LiveQuote` intentionally uses that capacity without an authored long-horizon ceiling. `DexOps::swap_exact_out` always receives the resulting finite bound.

Liquidity tasks also carry fixed non-zero outputs. `AddLiquidity.min_lp_out` reaches `LiquidityOps::add_liquidity`; the host adapter must reject a measured LP output below that bound.

`RemoveLiquidity.min_amount_a` and `min_amount_b` pass directly into Asset Conversion. Its two exact withdrawal-minimum errors classify as Temporary; malformed pair identity, missing indexed topology, and unknown downstream failures remain Permanent. The outer adapter transaction retains post-call balance-delta checks as defense in depth, so no success event or partial liquidity mutation survives either enforcement layer.

`StopCycle` executes only after its precondition and ordinary User fee collection succeed. The canonical evaluator produces `StepOutcome::Stopped`, production emits `CycleStopped { actor_id, cycle_nonce, step_index }`, and the shared policy interpreter ends the logical cycle successfully at that cursor.

The shared completion path emits the cumulative summary, leaves later funding accumulation untouched, evaluates completion policy and auto-close, clears a resumed Continuation, and leaves the suffix unreachable.

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

`attempt_fee_envelope` owns User per-step evaluation/execution/total fees and checked suffix totals. `settle_attempt_fee_step` releases each selected upper reservation before selecting evaluation-only or attempted-step charge; System settlement stays zero.

Admission, execution, retry admission, simulation, and benchmarks derive the selected full-plan or Continuation-suffix fee envelope from the current bounded contract. Overflow fails before mutation. The package-owned `fee_envelope_vectors` example emits deterministic User/System suffix, release, rollback-pricing, and protected-floor vectors consumed by browser forecasting tests.

Resolution and charging follow these rules:

- A User run releases the skipped step's unused execution-fee reservation before resolving later steps, matching every non-executable cycle path.
- A multi-amount task resolves every field before dispatch and selects `FundingUnavailable > Skipped > Executable` independently of field order.
- An Unstake last-funding plan fails validation when the runtime adapter cannot expose a transferable share asset.

Pallet boundary tests cover fixed, current/trigger/last-funding percentages, split totals, and `AllAvailable` across native, sufficient-asset, and staking-share surfaces. The embedding fixture binds unrelated host position keys to share assets without DEOS types.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier Steps in the same Actor Contract remain committed.

`src/contract.rs` is the package-owned semantic classification surface. Exhaustive matches derive task adapter/assets/recipients/effects/availability/weight ownership/bounded algorithms, typed task amount roles with dependency and retry behavior, predicate observations and purity, and error-policy controls.

`TaskWeightOwner::weight` selects the corresponding method on the runtime's single `WeightInfo`; the module adds no codec, storage, runtime API, or parallel numeric weight authority. Package tests instantiate every current primitive, while a new enum variant makes its owning match non-exhaustive. The package example `semantic_manifest` verifies ordered task and amount coverage against SCALE metadata and emits one deterministic format-neutral contract projection.

The control-flow firewall combines closed types with adversarial evidence. `Step` metadata exposes exactly `precondition`, `task`, and `on_error`; exhaustive contracts admit no successor, nested contract, callback, generic `RuntimeCall`, or opaque dispatch field. Optional `Precondition` classification fixes full bounded visitation, whole-expression error, one admitted task, and false advance. Predicates and amount classifiers expose read dependencies and never a control target.

The execution kernel produces one `StepOutcome` for each visited Step after canonical precondition, amount, fee, and task evaluation. `StepOutcome::Failed(TaskFailure)` retains the concrete `DispatchError` cause and orthogonal `RetryClass`; `resolve_step_control` interprets that outcome with the authored policy without a simulation-only failure vocabulary.

One `AttemptDisposition::{Completed, Failed, Suspended, Closed}` owns final production and simulation meaning. Production emits events and commits state from it; simulation returns the same disposition and final counters while its transaction rolls back. Bounded trace records wrap shared Step outcomes and do not reconstruct task, predicate, amount, fee, failure, or finalization semantics.

`resolve_step_control` remains the only runtime transition owner, with the exhaustive policy/mutability/failure matrix and same-cursor Continuation regressions pinning retry identity. Task-local transactions forbid callback-visible partial mutation. Bounded vectors, adapter capability contracts, generated worst-case weights, maximum-plan tests, and circular scheduler stress bound adapter and cross-actor work without interpreting local plans recursively.

### Market Adapter Boundary

`DexOps` owns swap-only host execution; `LiquidityOps` owns add, remove, and donation operations. Actors supplies `ExecutionContext { actor, actor_type }`, resolved amounts, and authored spend/output bounds without knowing route topology, market identity, or price-source policy.

DEX adapters return `DexSwapOutcome { total_amount_in, recipient_amount_out }`. Actors validates those committed facts against the authored exact-input or exact-output bound before emitting its task event.

The DEOS runtime benchmark helper prepares two Local/Native pools so both DEX benchmarks execute the maximum Native-anchored Router class rather than a cheaper direct route.

Adapters return typed task failures. Only explicitly classified Temporary failures may enter Mutable `RetryLater`; unknown downstream errors remain Permanent. Task-local transactions roll back adapter mutations before step policy runs.

Host-specific quotes, route selection, fees, oracle guards, slippage policy, and failure mapping belong in the integration architecture and embedding evidence.

## Scheduler Architecture

### Hook Separation

- `on_initialize`:
  - no Actors work
  - zero Actors Weight
- `on_idle`:
  - bounded temporal and queue housekeeping
  - bounded queue-driven cycle execution using remaining `on_idle` weight

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
- Pre-cycle close precedence is deterministic: `WindowExpired` > `BalanceExhausted` > `FeeBudgetExhausted`
- User fee shortfall at admission → terminal `FeeBudgetExhausted` close
- `CycleResult::Completed` means authored control reached terminal without an abort: skip-only and all-failed-`ContinueNextStep` runs remain Completed, reset `unsuccessful_attempt_streak`, and may satisfy nonce auto-close. Their counters remain factual; only at least one committed non-`StopCycle` task satisfies productive close. Abort emits `Failed`; explicit invalidation emits `Cancelled`.
- Post-failure close is inclusive at `unsuccessful_attempt_streak >= MaxConsecutiveFailures`; an admitted cycle emits its authoritative `CycleSummary` before pure cleanup emits `ActorClosed`, matching post-success `AutoCloseNonceReached` ordering
- Explicit, automatic, lifecycle-touch, dormant, and sweep paths share one pure cleanup routine: no Task, Precondition, fee, funding restoration, sovereign-balance movement, or shared queue/wakeup scan occurs
- Active close prevalidates identity, counts, reverse ownership, slot ownership, funding presence, and subscriptions, then commits cancellation, actor-store deletion, counter/locator release, and the close event in one storage transaction; any residual late error rolls back the complete terminal mutation
- Removing `ActorHot` lazily invalidates its ticket and wakeup pointer; bounded stale records converge through ordinary page draining
- Every bounded window validates checked `end + 1` representability, rejects overflow, and schedules that exact terminal block through `ActorHot.terminal_at`; trigger and terminal readiness share one live wakeup pointer and retain the earlier target
- Paused actors remain hot-only before terminal time and load `ActorContract` only when closure is due
- With `GlobalCircuitBreaker` active, normal cycles and scheduler-owned terminal cleanup defer; bounded housekeeping plus explicit lifecycle/sweep cleanup remain available

`ActorContract.completion_policy` defaults to `Persistent`. `CloseAfterProductiveCycle` checks cumulative `committed_effectful_tasks` only after successful logical-cycle completion, including a resumed Continuation. False latest-state Precondition results, skipped Steps, rolled-back failures, bare `StopCycle`, suspension, abort, cancellation, and retry exhaustion cannot select `ProductiveCycleCompleted`. The pure close path remains valid for Immutable System actors and preserves their sovereign balances.

Code anchor: `src/execution.rs::execute_single_cycle_traced` increments the cumulative count after task commit and applies productive close after `CycleSummary`. Pallet tests prefixed `close_after_productive_cycle_` falsify false-state, latest-state race, bare stop, retry, exhaustion, balance, and Immutable closure claims.

Lifecycle lease-by-cycles is authored by `ActorContract.auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, the actor closes with `AutoCloseNonceReached`. Complete Mutable Contract replacement may set, shorten, extend, or clear the target; every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`. No field-specific setter or increment call exists.

### Fee Collection Boundary

The generic pallet collects creation and per-step User fees through one runtime-supplied `FeeCollector`; terminal cleanup charges no Actors fee. Both User creation calls collect `ActorCreationFee` for Active and Dormant admission before identity, slot, counter, or next-id mutation; failed collection leaves every actor store and owner balance unchanged. System creation remains exempt.

Actors invokes the runtime-supplied collector exactly once per selected charge and assigns no trigger semantics to that movement. Collection is ledger-only: it performs no Actors ingress preflight or notification, funding accumulation, readiness latch, or scheduler placement. Zero collection is a no-op, and any collector failure rolls back movement and all Actors state.

- Predicate and Task preparation run read-only before collection determines the Step outcome.
- Every attempted User Step invokes `FeeCollector` at most once: false Precondition or resolution/funding non-execution charges evaluation-only, while an executable Step charges evaluation plus generated execution fee together.
- Collection failure rolls back the complete scheduler attempt before step policy, task dispatch, queue consumption, or any persistent fee, event, counter, nonce, cursor, or snapshot mutation. Simulation reports interface-local `FeeCollectionFailed`.
- After successful collection, adapter failure rolls back task-local effects but retains the one combined charge; `ContinueNextStep` and `AbortCycle` never alter that charge or trigger another collection.
- `fee_native_protected_minimum` applies `max(MinUserBalance, asset minimum)` to User fee-native direct preserve-spend capacity and `SwapOut` input capacity after the selected reservation; other assets retain their adapter minimum.

Pallet regressions cover each outcome, collection failure, one-call cardinality, task rollback, release-to-zero, direct User-floor preservation, and exact-output input-cap failure. User fee admission derives each step and complete-plan reserve from host-bound generated weights plus configured step fees; package tests reject execution immediately below that exact reserve.

### Progress-Preserving Continuation

`ActorHot.cycle_state` is the sole discovery marker. `Idle` reads no Continuation value; `Suspended` requires exactly one `ContinuationState[actor_id]`. The sparse value stores scalar `cursor`, logical-cycle-wide `attempt`, `unsuccessful_attempts_at_cursor`, `last_attempt_block`, frozen typed suffix snapshots, and cumulative outcomes. Try-state enforces marker/store equivalence, cursor bounds, Mutable-only bounded retry policy, a live cursor-local count below its nonzero limit, and snapshot-surface validity.

Attempt `0` opens one logical cycle and increments `cycle_nonce` once. A Temporary `TaskFailure` or `FundingUnavailable` under `RetryLater { max_attempts }` increments both global failure state and the cursor-local count. The first suspension stores `1`; same-cursor suspension uses checked increment after admission proves both retry bounds, while a later cursor resets to `1`.

`transition_failure_streak` is the sole mutation formula: an unsuccessful attempt checked-increments, while completed execution or semantic Step replacement resets to zero. Suspension and terminal failure call that owner once, classification only reads its result, and simulation inherits the same transition through canonical execution.

Post-attempt counters use checked addition. Inclusive local exhaustion closes with `RetryAttemptsExhausted`; when both cutoffs land together this reason wins, while an earlier global cutoff closes with `ConsecutiveFailures`. Exhaustion clears Continuation, emits `CycleSummary(Failed)` without `CycleCancelled`, then closes. An already-reached global cutoff closes before another `CycleStarted` or `CycleContinued`. Persisted retry reuses the nonce, omits external cadence, and executes only the suffix.

`scheduler::retry_backoff_blocks` uses checked capped exponentiation to map persisted attempt `0, 1, 2, ...` to `1, 2, 4, 8, 8...` blocks.

`Attempt identity proof`: `cycle_nonce` identifies the logical cycle, and the scheduler's one-execution-per-actor-per-block invariant makes `(actor_id, cycle_nonce, block_number, event_index)` unique for every opening or Continuation attempt. Cursor and `unsuccessful_attempts_at_cursor` state the semantic retry position. No cycle-global attempt ordinal is stored, emitted, simulated, or projected; executable evidence covers opening, repeated suspension, and completion coordinates.

Eligibility uses the larger of this delay and schedule cooldown, omits external cadence for the open run, then respects window start. A one-block delay uses the existing next-block FIFO ticket; longer delays use the existing paged wakeup pointer.

`Backoff decision evidence`: over 64 unavailable blocks at the 10,000-actor bound, capped exponential creates 100,000 due retry obligations and recovery-wait sum 210. Fixed delays of 1, 4, and 8 cause `640,000/0`, `160,000/96`, and `80,000/224` respectively. Every policy retains the same 10,000 wakeup cohort and FIFO order, and each serviced retry has the same Weight class. No fixed delay improves aggregate pressure and recovery together, so capped exponential remains canonical.

`Timer phase decision`: `tests/fixtures/timer-jitter-decision.v1.json` preserves the 10,000-actor comparison. Historical jitter expanded two targets to 128 while leaving tail service, `PassExhausted`, serviced Weight, FIFO, suspension, and retry recovery unchanged; peak queue improved by less than 2.5%. The implementation therefore uses exact cadence from `schedule_anchor` with no phase constant, hash, arithmetic, metadata, or host configuration.

`resolve_step_control` is the private exhaustive transition owner for executed, stopped, skipped, funding-unavailable, Permanent-failure, and Temporary-failure outcomes across all three error policies. Production execution and `simulate_current_contract` both invoke `execute_single_cycle_traced`.

`canonical_step_transition_matrix_has_production_simulation_parity` runs every Section 3.4 row, Actor type, mutability, error-policy variant, Step-outcome variant, local/global bound, and fresh/Continuation attempt. Variant counts fail closed when either canonical enum grows.

The matrix compares rollback-only simulation with independently observed production events, counters, cursors, failure streaks, fees, balances, custody, suffix effects, and final disposition. The protocol-coherence audit rejects specification/matrix and enum-inventory drift, and Actors assurance executes the focused matrix.

Task-scoped rollback leaves earlier successful steps committed. The named `SwapIn → AddLiquidity → Transfer` and Burn-prefix regressions prove same-cursor retry, prefix non-replay, cumulative outcomes, and no cancellation compensation. The fixed-seed model `0xDE05_0730` independently checks sparse state, cursor progress, queue/wakeup uniqueness, funding accumulation, frozen cycle snapshots, and cancellation after each transition.

`simulate_current_contract` is the package-owned rollback core behind versioned `ActorSimulationApi` runtime metadata. It requires exact stored Active Actor Contract, actor type, mutability, mode/run-state, readiness, liveness, and User fee budget. Simulation and production admission use the same package-owned suffix envelope and predicate: checked fee-native balance above `MinUserBalance` must cover `attempt_fee_upper`; raw balance alone never admits a User attempt.

The API executes the production path with an optional bounded trace and exposes fresh or Continuation outcomes including `unsuccessful_attempts_at_cursor`. Terminal closure returns `AttemptDisposition::Closed(CloseReason)`. The whole attempt runs inside `TransactionOutcome::Rollback`; package tests prove actor state, balances, events, fees, adapter effects, and stored Continuation remain unchanged.

Semantic Contract Steps, funding-policy, schedule, window, deactivation, terminal, and close transitions share `cancel_continuation_internal`. Cancellation validates and mutates `ActorHot` through one fallible storage owner; missing or contradictory state returns `ContinuationInvariant`. Continuation writes load all five canonical partitions, return `ActorNotFound` for absent or Dormant identity, and return `ActorInvariant` for malformed partition ownership, without panic-only mutation assumptions. Continuation attempt opening returns a state-preserving failed disposition when its canonical state disappears, and checked fee-envelope disagreement does the same before cycle mutation or events. Exact encoded plan/completion-policy, funding-policy, schedule/window, auto-close-target, and global active-limit updates return without storage or event mutation; semantic auto-close target changes preserve Continuation.

Every external control that invalidates or reconstructs ordinary membership shares one class-independent actor/block clock across signed-owner and governance origins; its identity write and Contract replacement hot-state/reset write are fallible transaction-owned mutations returning typed invariant errors rather than panic-only post-prevalidation assumptions. Exact no-ops, internal transitions, and terminal cleanup remain exempt.

Cancellation emits `CycleCancelled` before one cumulative terminal `CycleSummary(Cancelled)` without compensation or prefix rollback. The reason set distinguishes explicit, Steps, completion, funding, schedule, deactivation, and typed `Closing(CloseReason)` causes. Runtime-upgrade cancellation is not a public placeholder; a deployed host that needs one must ship the concrete bounded migration and its executable constructor together.

`CycleStarted` appears once per nonce. `CycleContinued` and `CycleSuspended` carry `(actor_id, cycle_nonce, cursor)`; suffix step events retain the nonce and their order belongs to the surrounding attempt boundary. Current sparse state is canonical-chain truth. Unbounded attempt history remains materialized.

### Read-Only Eligibility Projection

Version 4 `ActorEligibilityApi` owns `actor_eligibility` plus the bounded `materialization_faults` projection. `actor_eligibility` is the read-only semantic Actor projection. It mirrors `apply_admission` and reuses the exact cadence, retry, window, failure-limit, breaker, and latch owners, so clients do not reproduce scheduler arithmetic.

The projection is one algebra: `NotRegistered`, `Dormant`, or `Active(ActorClassification)`. Active Crossing includes canonical phase and installation revision plus bounded pending/processing revision facts; the companion fault method returns only current Crossing, broad-fanout, and wakeup faults, `crossing_capacity` returns per-feed User/total policy and exact counts without raw physical topology, and `trigger_state_bond` quotes the prospective refundable User amount from the host policy used by lifecycle dispatch. Active eligibility preserves terminal reason and the exact `ActorExecutionPhase`, including `WaitingRetry(block)`, `WaitingBlock(block)`, and `WaitingCadenceTick(tick)` payloads; no parallel phase or next-block field exists.

The projection persists no state, emits no event, and promises no service. Queue position and available Weight still decide actual admission. Arithmetic overflow and malformed Continuation state return typed projection errors rather than an inferred phase. Nonce exhaustion is projected as the same terminal close as execution without attempting an overflowing successor nonce.

Authoritative amount arithmetic uses checked accumulation/subtraction: normalized split legs plus retained remainder exactly conserve the resolved total, including `u128::MAX`. Fee reservations, existential protection, and percentage amount resolution intentionally floor unavailable spendable balance at zero. Saturating `Weight` composition remains a conservative upper-bound cap, scheduler pass statistics and starvation counters remain bounded telemetry caps, and try-state topology/accounting counters fail on overflow rather than masking malformed state.

Code anchor: `src/scheduler.rs::actor_eligibility`; package tests prefixed `eligibility_projection_` falsify absence, dormancy, every Active phase, terminal coexistence, and exact temporal payloads.

### Queue Execution Model (Monotonic Paged FIFO)

`classify_actor` owns terminal precedence, breaker and pause phase, retry/temporal timing, signal readiness, and cursor-sensitive User viability. Scheduler admission, sweep, simulation, eligibility, and certified ingress project that read-only result; malformed Active partitions and Continuation state map through typed classification errors.

Scheduler execution is queue-first and deterministic:

It uses two scheduler layers: a monotonic paged FIFO for work that can execute now and one typed paged temporal layer for later eligibility. Block deadlines and timestamp-tick deadlines have separate paged minimum heaps under one coordinator; actors sharing one typed deadline occupy linked fixed-capacity pages.

1. **Wakeup drain**: the coordinator fairly alternates block and tick domains when both are due; each cursor exposes its earliest deadline without scanning sparse gaps, and one admitted unit consumes one slot, preserves a partial bucket at the same minimum, and either appends live readiness to the active FIFO or lazily discards a stale pointer
2. **Ingress admission**: each matched producer call applies funding independently and sets the unified boolean signal latch; actor-local queue/wakeup membership remains bounded and may join the active run queue in the same `on_idle` pass
3. **Block-start cutoff**: after already-due wakeups materialize, the scheduler snapshots global `NextQueueTicket`; every later append receives a ticket at or beyond that cutoff and cannot execute in the current block
4. **Canonical head**: tri-state discovery returns `Empty`, `Head`, or `Blocked` only for the one global FIFO. It stops on an incomplete probe rather than treating the head as absent.
5. **Strict ticket order**: the scheduler may lazily consume tombstones before the cutoff, but it cannot bypass a live or blocked head. One O(1) preflight owns every FIFO mutation: checked `tail - head == QueueOccupancy`, configured capacity, and canonical current head/tail page-slot layout. Enqueue, live-head consume, and tombstone drain run as closed storage transactions after that preflight. Corruption rolls back exactly and returns an invariant live-head stall rather than `Empty` or progress. The next live ticket receives the only execution offer.
6. **Execution ceiling**: `last_cycle_block`, the shared scan/execution ceilings, and the common cutoff prevent a second scheduler invocation or circular/self-enqueue graph from executing one actor twice in the same block. Untouched suffix entries remain physically in place without reconstruction.

`ActorHot.queue_ticket` is the sole live-membership marker for the canonical FIFO. Replacement, close, pause, and cancellation use actor-local queue invalidation; stale queue entries drain lazily. FIFO enqueue, invalidation, liveness, tombstone drain, and scheduler admission load the exact five-partition actor state before interpreting the ticket, same-block marker, pause state, or Contract; corrupt state stalls without queue mutation. Live-head service carries that validated hot state into transactional consumption, which rechecks the exact physical actor/ticket pair before clearing membership; post-execution placement reloads canonical state because execution may change lifecycle, Continuation, funding, or terminal ownership.

Wakeup schedule, invalidation, drain, and materialization use the same canonical boundary; only NotRegistered, Dormant, or Active-with-no-pointer entries drain as lazy temporal tombstones, while another live pointer or corrupt partitions roll back. Continuation cancellation exactly removes the retained Active actor's current wakeup slot before re-priming a latched next run, so an old slot cannot conflict with the replacement pointer. The production-Wasm `scheduler_actor_state_probe` benchmark at 100 steps and 50 repeats measures `38,413,000 / 12,200` with five reads. The reference runtime regenerated every changed scheduler and ingress branch at 50 steps and 20 repeats against the same loader; branch weights now carry their complete read sets without a separate partial-probe surcharge.

No actor type, actor ID, execution share, or priority policy changes FIFO service. System and User actors receive bounded service exclusively by their global ticket order.

Wakeup draining, paged queue operations, actor probes, normal cycles, and pure terminal cleanup use two-dimensional `WeightMeter` fit checks. Direct ingress is charged at its originating producer. Close admission uses the generated worst-case User cleanup bound and performs no shared-container scan.

Scheduler accounting stays local unless it controls consensus progress. `scanned` and `executed` enforce independent per-pass ceilings; `QueueDrainStats` and `WakeupDrainStats` expose bounded operation results to callers, tests, and benchmarks without storage writes. Operators derive completed admission from `CycleStarted`/`CycleSummary` and persistent blockage from bounded queue/wakeup state plus starvation detection/recovery events. Loaded-actor and page-touch diagnostics remain stress instrumentation rather than permanent consensus counters.

### Temporal Wakeup Layer

The temporal wakeup layer owns future eligibility and admits it only through the active run queue. Every active actor may own at most one exact pointer; replacement and closure invalidate that slot without scanning actors or blocks. `MaxActiveActors` bounds global live wakeups, while `WakeupPageSize` controls I/O granularity rather than same-block capacity. This makes spillover buckets, placement-drop events, and actor-key retry scans unnecessary.

One placement owner (`schedule_next_work_local`) maps signalled readiness, timestamp cadence, block cooldown/window/retry targets, capacity recovery, and terminal expiry to either the FIFO or one typed wakeup. Placement owners normalize `AlreadyLive` to success; a missed normalization at a defensive `map_err` boundary returns `QueueCapacityUnavailable` rather than panicking. `defer_wakeup` and `defer_tick_wakeup` share the same transactional substrate. Queue saturation falls back to an exact next-block wakeup; fallback failure leaves no false success claim (spec 8.1.4).

Queue consumption, attempt effects/events, and all reachable post-attempt placement share one transaction. Ticket/page/wakeup exhaustion or topology failure rolls back the attempt and preserves the prior queue head. Wakeup removal and queue materialization likewise commit atomically or retain the exact wakeup.

Package coverage proves the strict post-worker cutoff: two manual triggers in one block latch one pending signal and one FIFO ticket, and exactly one `CycleStarted` is emitted (`executions(A, B) <= 1`).

The package uses the paged wakeup substrate and sparse cursor:

- `WakeupPages<(WakeupKey, page_id)>` and per-key `WakeupBuckets` own the paged topology, where `WakeupKey` is `Block(number)` or `Tick(number)`.
- `WakeupCursorPages<(clock, page_id)>` plus `WakeupCursorLen<clock>` provide separate production paged binary min-heaps for block and tick keys; each bucket owns its exact reverse `cursor_index`.
- Heap insertion, pop-min, and exact removal use at most `ceil(log2(MaxActiveActors))` sift steps, preserve contiguous per-clock cursor pages, and avoid scanning empty intermediate keys; try-state reconciles each clock's page shape, uniqueness, ordering, and reverse indices. Maximum-depth generated evidence belongs to `scheduler_wakeup_cursor_insert`, `scheduler_wakeup_cursor_pop_min`, and `scheduler_wakeup_cursor_remove_exact`.
- `ActorHot` owns `WakeupPointer { block: WakeupKey, page_id, slot }` and closed `trigger_runtime_state`; its Cadenced variant owns the optional anchor tick. The retained pointer field name `block` is SCALE representation, while its typed value owns the clock domain.
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

### Cadence

- Cadence readiness is deterministic only; Actors exposes no probability field, entropy provider, secure/insecure branch, hash fallback, probability event, or probability error.
- Cadence uses `cadence_anchor_tick = ceil(timestamp_millis / CadenceTickMillis)` as its exact origin and derives no actor-specific phase. `MaxCadenceDelayTicks` bounds this timestamp domain independently from the block-domain `MaxExecutionDelayBlocks`; neither numeric horizon is converted or reused across clocks. Genesis records an uninitialized anchor and one tick-zero bootstrap wakeup because no consensus timestamp exists during construction.
- The first ordinary wakeup service after timestamp inherent application replaces that marker with the ceiled anchor and one full-period deadline without latching readiness or entering FIFO. Active installation and replacement anchor directly because consensus time is then available.
- Eligibility and simulation project the exact actor-owned tick pointer. A pointer at or before the current scheduler tick remains `WaitingCadenceTick(exact_tick)` until wakeup materialization latches readiness; projection never advances it to a later cadence period.
- Admission prevalidates nonzero bounded `every_ticks`, zero cooldown, no window, and checked anchor/deadline arithmetic. Runtime rearm selects the first aligned tick strictly after the observed tick, so missed periods coalesce without catch-up bursts.
- A due initialized tick clears the typed temporal pointer, sets the existing pending latch, and appends one FIFO opportunity. Only FIFO service executes the Actor; after an attempt terminates, cadence rearms from the immutable anchor.
- Any future probabilistic execution requires a separate append-only admission policy and a concrete financially secure runtime entropy contract rather than an optional field on deterministic cadence.

### Trigger Sources

`Manual`, `AddressEvent`, `ObservationChange`, `ObservationCrossing`, and due `Cadenced` detection all enter `request_activation`. That sink loads canonical Active state once and builds one private prospective-state `ActivationPlan`: it latches `ActorHot.pending_signal` in memory, classifies with the loaded Continuation, and records exact close, coalesced, cadenced-enqueue, or non-cadenced placement action before commit. Canonical FIFO append likewise separates read-only `QueueAppendPlan` authority from mutation. One production `preflight_paged_enqueue_cohort` admits a nonempty vector bounded by `MaxCrossingActorsPerBlock`, validates queue topology once, and accumulates consecutive Actor tickets, final tail/occupancy, and each touched physical page in memory; over-cap, duplicate/live, capacity, and index failure retain no writes. Successful cadenced enqueue carries single-Actor authority into activation commit, while queue saturation retains the exact next-block wakeup fallback. Non-cadenced planning records no placement, immediate enqueue, or one exact block wakeup before commit. Source-specific code owns only detection and source-state progression; no parallel signal key or source tag enters consensus state.

The scheduler carries one loaded Active state through live-ticket verification, same-block and lifecycle gates, classification, attempt admission, fee admission, and loaded-head consumption. Control preflight, ingress preflight/consequence, eligibility, simulation, sweep, and activation likewise classify with the Continuation returned by their complete canonical probe instead of rereading it through a view-only helper. A transaction boundary or adapter/task effect that may mutate actor state terminates that borrow and requires an explicit canonical reload before later placement.

Crossing phase and installation revision live only in `ActorHot.trigger_runtime_state`; generation-checked exact-threshold memberships store only key/page/offset/generation physical back-pointers in bounded dense pages under a sparse radix path. `MaxCrossingMembersPerFeed` is the independent total host bound, while `MaxUserCrossingMembersPerFeed` and its exact reconciled count reject User admission before topology writes and leave the difference exclusively available to System Actors. The reference runtime binds these to 10,000 total and 9,000 User memberships per feed; global Active capacity remains a separate aggregate ceiling. Dense-page swap compaction rewinds an active range cursor when an unprocessed tail member moves behind it, preventing close or replacement of a skipped newly installed member from erasing older transition work. If activation terminally closes the final feed member, ordinary close cleanup removes the membership, queue, cursor, and pending-feed link atomically; the worker recognizes the resulting zero membership count as completed cleanup and does not recreate source state. A non-terminal placement failure rolls the complete actor unit back.

`classify_crossing_work` is a bounded read-only projection over the current pending-feed head, transition, range cursor, sparse radix result, generation-checked member, canonical Actor state, and prospective activation shape. Its production bounded contiguous-prefix authority is obtained through the internal `CrossingCohortSnapshot` helper bounded by `ObservationPageSize` and `MaxCrossingActorsPerBlock`. Each snapshot binds the source leaf key, page, half-open `[start_offset, end_offset)` range, and contiguous member/generation/locator authorities; it validates only selected entries, clamps to page remainder, grants no authority outside that selection, and performs no mutation. Its production-used count authority also accepts an optional tail-refill limit, allowing non-tail selection to preserve packed pages with one bounded suffix. Production Weight owns the required maximum encoded tail-page probe at `17,530,000 / 4,561` plus one database read. Compatibility pair fallback retains separate cursor and tail snapshots because sequential single-member compaction defines that execution shape; production larger batches use one admitted contiguous prefix and, for non-tail sources, one exact physical-tail refill suffix. First- and tail-candidate preflight use one branch-directed read-only cohort classifier for post-installation skip, rearm, pending fire, placed fire, coalesced fire, and terminal fire. Placed fire retains scheduler-owned prospective Hot state after the phase changes to `WaitingForRearm`, including its pending latch and exact immediate-FIFO versus block-wakeup action; aggregate queue authority therefore cannot restore the pre-fire phase. The first authority establishes the expected branch; later skip mismatches stop after Hot state, rearm mismatches stop after Hot plus Contract, and fire activation reads the full canonical shape only when that expected branch requires it. The bounded preflight admits only the homogeneous leading branch before mutation, leaving the first differing authority unadmitted without exceeding the generated branch probe. Internal work classification is distinct from the candidate-preflight result and carries branch, exact current admitted candidate count, and optional `(tail page, available suffix)` refill authority through probe fallback, full classification, and pair-to-single downgrade. Tail-page classification leaves refill authority absent; non-tail admission populates it only after reserving and charging the generated tail-page probe. Service admission fails closed if that count disagrees with the selected branch components, so it does not reconstruct count from mutable state after preflight. Production Weight now owns placed-fire, coalesced-fire, terminal-fire, post-installation-skip, and rearm preflight as parameterized `c ∈ [1, 4]` functions with generated RefTime, ProofSize, and respective `2 + 7c`, `2 + 7c`, `2 + 7c`, `2c`, and `3c` database-read formulas; mutation branches use generated single, pair, maximum tail-page, and trimmed/emptied non-tail owners. Current pair admission is work-conserving at component and two-dimensional Weight boundaries: one remaining transition/leaf/page/Actor unit selects one candidate, an unaffordable pair probe falls back to the corresponding single preflight, and an affordable pair probe with an unaffordable pair branch executes its already classified first candidate without rereading state. The production cohort extends this authority boundary without a second snapshot path. Its `CrossingWorkPlan` distinguishes empty, completion, seek, leaf/page advancement, installation skip, rearm, coalesced/placed/closed fire, and structural-fault branches without mutating state. Service admits the generated common probe before preliminary classification, conditionally admits the fire-only classification extension, and then selects one generated execution owner. Completion/seek miss use transition Weight; open/advance use the ordinary leaf/page maximum; skip, rearm, coalesced fire, placed fire, and terminal close each use their dedicated owner; structural fault reserves the terminal envelope. It never adds independently benchmarked whole-branch results as though they were disjoint components. The committing transaction revalidates canonical and physical ownership. Each pass also retains exact bounded counters for transition units, leaves, pages, candidates, canonical five-partition activation probes, activation calls, terminal closes, and rolled-back faults; the transition/leaf/page/candidate dimensions enforce existing component ceilings, while probe/activation/close/fault dimensions support exact branch evidence without entering consensus storage. The release load profile installs 10,000 members on one feed and proves a zero-match transition finishes in one configured pass with zero candidate, canonical-probe, activation, close, fault, or FIFO-placement work. Its following eight-member crossed cohort records exactly eight candidates, canonical probes, activations, and FIFO placements, with zero closes or faults; the other 9,992 members remain unprobed. The same profile then converges a 10,000-member single-threshold fire/rearm/fire sequence in exactly 10,002 work units per transition, preserves one placement per Actor across oscillation, fills the 1,024-entry mock FIFO, defers the remaining 8,976 through canonical wakeups, loses no transition, and creates no duplicate placement. Pure rearm has a generated `crossing_rearm_unit` owner (`397,683,000 / 162,782`, 78 reads and 74 writes) and performs no activation probe, placement, or close during execution. Classification is staged: every nonempty unit first admits the eight-read `crossing_work_probe` (`46,305,000 / 12,200`), and only a fire-pending plan may admit the single 14-read `crossing_fire_probe` (`72,427,000 / 12,200`) or queue-page-boundary atomic-authority pair 25-read `crossing_fire_pair_probe` (`241,375,000 / 23,410` before execution. No-match, structural search, skip, and rearm never pay the five-partition fire extension. Already-latched and canonically placed fire uses generated `crossing_coalesced_unit` Weight (`419,403,000 / 162,782`, 82 reads and 74 writes), so it never pays queue placement or terminal cleanup. A newly latched nonterminal fire uses generated `crossing_placed_unit` Weight (`439,379,000 / 162,782`, 87 reads and 78 writes) for exactly one canonical FIFO placement without terminal cleanup. A post-installation transition skip uses generated `crossing_skip_unit` Weight (`165,038,000 / 81,886`, 40 reads and two writes), preserving phase and placement while advancing the stale candidate. The first cohort slice recognizes two homogeneous installation-skip, pure-rearm, coalesced-fire, or nonterminal placed-fire candidates on the same current tail page before mutation, admits a placed pair only when both scheduler plans require immediate FIFO and one read-only aggregate queue plan validates exact pages, consecutive tickets, final tail/occupancy, duplicates, capacity, and index arithmetic; otherwise it downgrades to one candidate. Dedicated pair probe and execution owners charge and commit both in one transaction. Placed-batch accounting and Weight admission carry exact bounded candidate counts: two uses the generated atomic pair owner, while three/four use the generated maximum-batch owner. Placed-batch production reconstructs one coherent bounded cohort authority; count two uses pair movement while a homogeneous contiguous tail-page prefix of three/four uses descending physical movement followed by stable retained-suffix rewrite and locator repair, and both commit one queue plan plus one cursor/feed advancement; a transactional full-tail-page prototype consumes four authorities in descending physical order with live-locator checks, rejects a corrupted third-candidate generation with exact-root rollback, and commits one aggregate queue plan on success; the fixed-depth radix seek has a separate generated owner charged only while `current_threshold` is absent, so candidate service reuses the retained threshold without sparse-work doubling; production non-tail batch classification charges the tail-refill probe, binds one generation-checked physical-tail suffix to the placed authority, preserves retained source order, groups equal destination obligations so each touched destination page is written once, repairs source/tail/destination locators, handles trimmed and emptied tail pages under separate generated mutation owners, meters their component-wise maximum, commits the aggregate queue and cursor once, and rejects stale authority transactionally, and a route-heterogeneous third candidate truncates a four-candidate prefix after its first two homogeneous immediate-FIFO authorities, consumes it through two movement-without-Hot operations with swapped-tail generation validation, commits one aggregate queue plan, installs final phase/tickets, and advances cursor/feed once; exact instrumentation and rollback evidence cover this path, while separate movement evidence covers split destinations. Compatibility coalesced/rearm/skip pair execution still restores its preflighted feed between scalar units so pending-list rotation cannot substitute another feed. Post-installation exclusion uses `crossing_skip_pair_probe` (`46,515,000 / 6,112`, nine reads) plus `crossing_skip_pair_unit` (`70,960,000 / 6,112`, 10 reads and two writes) and leaves both phases and placements unchanged. Pure rearm uses `crossing_rearm_pair_probe` (`58,249,000 / 23,410`, 11 reads) plus `crossing_rearm_pair_unit` (`430,439,000 / 162,782`, 82 reads and 76 writes) and performs zero canonical activation probes. Already-latched homogeneous fire uses `crossing_coalesced_pair_unit` (`478,630,000 / 162,782`, 89 reads and 76 writes) with no new FIFO placement. The generated atomic `crossing_placed_pair_unit` (`510,967,000 / 162,782`, 94 reads and 80 writes) and maximum four-candidate `crossing_placed_maximum_unit` (`712,602,000 / 162,782`, 108 reads and 84 writes) read/write bounded membership-page keys—the one source fire page and one shared destination rearm page—rather than reloading either key per Actor; generation, locator, canonical state, phase-before-activation, and rollback checks remain per candidate. A malformed second locator classifies fail-closed before execution and proves zero probes, activations, closes, or FIFO placements for its otherwise-valid first peer; a terminal peer is never absorbed into the nonterminal pair owner. A second candidate whose installation revision equals the pending revision truncates the pair before mutation: the valid first Actor fires singly, the second then advances through the dedicated installation-skip owner without latching, placement, retrofire, or phase change. Pair fallback snapshots the cursor member and physical tail member because its first single-member removal uses swap compaction; the tail then becomes the next cursor member for the second execution. Replacing the tail with the encoded adjacent member without replacing single-member compaction would classify one Actor and execute another. A larger cohort must therefore land bounded contiguous-prefix authority, complete preflight, and one-pass source compaction together rather than treating adjacency as a local classifier edit. A newly found threshold is retained before candidate service, homogeneous immediate-FIFO cohorts admit up to four candidates, and mixed routes truncate before mutation.

| Crossing capacity dimension | Reference value | Current evidence |
| --- | --- | --- |
| Worker reserve | 10% maximum block Weight | Generated maximum placed-fire batch plus staged probes admits four candidates by both Weight dimensions after base |
| Candidate component cap | 4 per block | Exactly reachable by one admitted homogeneous maximum batch; prevents cheaper branches from escaping the production service contract |
| User memberships per feed | 9,000 | 2,250 uncontended blocks, or 3h45m at six-second blocks |
| Total memberships per feed | 10,000 | 2,500 uncontended blocks, or 4h10m; reserves 1,000 System positions |
| Canonical FIFO capacity | 10,000 | One maximum first-fire herd can retain one placement path per Actor |
| Pending transitions per feed | 64 | Explicit producer backpressure; not a sustainable publication-rate promise |

The 10,000-member single-threshold release profile exhibits no failure mode distinct from the total per-feed horizon, so the runtime intentionally adds no separate leaf cap; exact page and worker bounds already constrain physical service. These are bounded capacity facts, not a latency SLA. The horizon assumes one maximum homogeneous placed-fire head transition, the configured Crossing reserve, solvent Actors, available placement, no competing materialization family pressure, and six-second blocks. Alternating revisions may occupy all 64 transition slots; admission then fails atomically until service creates capacity. Final release evidence must regenerate this ledger from the accepted production tree.

`MaterializationFamilyCursor` rotates the block-start family across overdue wakeups, Crossing, and broad fanout as `0 → 1 → 2 → 0`. The coordinator services every family in rotated order, may service only the rotated first family once more through the bounded lending pass, preserves each worker's internal deadline/revision order and counters across grants, advances the cursor before the execution cutoff, and continues materialization while the global breaker suppresses Actor execution. The sum of the three configured ceilings is the single canonical materialization envelope. Each family also has one canonical maximum-unit minimum derived from generated branch Weight, and production integrity requires all three minima to fit together with a positive two-dimensional lendable remainder. The shared envelope plus fixed base, cleanup, and coordinator Weight is subtracted once to derive the exact guaranteed Actor execution floor, so materialization cannot borrow from execution. A cursor outside `0..3` returns after fixed charged work without running or rewriting a family, and TryRuntime rejects it. Workers consume only the coordinator grant and retain per-block wakeup-scan, Crossing-component, and broad-page counters across grants. The rotated first family receives the production-proven lendable remainder after all three maximum-unit minima. After the first pass, generated coordinator Weight owns bounded due-wakeup, pending-Crossing, and dirty-fanout classification. When the rotated first family still has serviceable work, one bounded second pass offers it all trailing unused reservations without resetting counters, including when its first branch cost less than the maximum-unit minimum.

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

`DirtyObservationFeeds` coalesces each subscribed feed to one `latest_revision` equal to its retained baseline, records the first clean-to-dirty block as `dirty_since`, and stores reciprocal previous/next active-dirty links. New state receives zero `fanout_revision` and no subscriber-page cursor. Greater revisions preserve the uninterrupted interval timestamp; clean completion removes dirty state without removing the baseline, while later advancement starts a new interval.

The first fanout unit snapshots the latest revision and exact occupied head; later units retain the next linked page id.

`DirtyObservationListState` stores exact head, tail, fair cursor, and live count. First insertion initializes the list; later insertion appends through the current tail. Completion or last-subscriber removal unlinks at most two neighbors and repairs the cursor. Dirty ingress runs transactionally and reports one conservative Weight independent of subscriber count. Try-state walks the bounded list and reconciles reciprocal links, cursor membership, map ownership, and count.

Ingress rejects capacity exhaustion as `DirtyObservationCapacityExceeded` and broken reciprocal ownership as `DirtyObservationInvariant`. Its transaction restores every touched dirty link and list field on failure. The producer decides whether and how to surface rejection; recovery is an explicit later retry after host/operator cleanup, with no package-owned replay queue.

The independently metered fanout worker runs before actor execution in `on_idle`. One admitted unit selects the exact active-dirty cursor and processes at most one occupied `QueuePageSize` subscriber page. It sets only `ActorHot.pending_signal`, preserves existing queue/wakeup membership, and requests canonical paged-queue admission only when neither exists. A capacity failure retains the exact page for retry after ordinary queue cleanup; the scheduler remains the sole execution owner.

The fair cursor advances to the selected feed's next active neighbor and wraps to the exact head without empty probes. Per-feed `fanout_revision` and `next_subscriber_page` preserve bounded progress. Completion deletes state only when `latest_revision == fanout_revision`; a newer revision resets the same state to the current occupied head, so an older pass cannot erase a newer change.

Fanout admits one complete worst-case page Weight in RefTime and ProofSize before mutation. Host bounds set the independent per-block page ceiling and weight limit. One unit short leaves dirty state, actor latches, queue membership, and cursor unchanged. Package benchmarks cover empty, completing dense, and queue-blocked branches; production coefficients and delivery envelopes belong to the host integration.

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
- `ActorContract`: one scalar trigger, optional block window, and bounded ordered `ContractSteps`; the generated storage descriptor maximum is 8,735 bytes
- `ActorFunding`: active-only canonical funding-source policy, bounded tracked-asset set, and `funding_accumulated[asset] = amount`; authorized ingress adds checked deltas, fresh cycle opening atomically takes the map as its frozen snapshot, and later ingress remains accumulated for the next cycle
- `ActorIdentityCount`: transactionally maintained O(1) `ActorIdentities` cardinality bounded by `MaxActorIdentities`
- `ActiveActorCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against `ActorHot`, `ActorContract`, and `ActorFunding`
- `NextQueueTicket`: shared monotonic global age allocator and common block-start cutoff source
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

- `activate_actor` accepts one typed `ActorContract` and validates trigger/cooldown/window, Contract Steps, funding policy, optional auto-close target, tracked assets, cached bounds, class restrictions, active capacity, and the host-configured idle envelope. It then creates matching `ActorHot`, `ActorContract`, and `ActorFunding` entries for a Mutable identity.
- `deactivate_actor` clears queues, wakeups, pending signal, funding, cycle, and fee state while preserving identity, owner slot, sovereign address, and balances.

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
- `Manual`, `AddressEvent`, and `ObservationChange` retain block cooldown/window gates. `Cadenced` admits neither and owns one timestamp deadline while Idle.
- A suspended Cadenced Actor owns only its ordinary block retry. After the cycle terminates, it rearms one aligned timestamp deadline strictly after the observed tick.
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
