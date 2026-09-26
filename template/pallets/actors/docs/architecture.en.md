# DEOS Actors Package Architecture

> Package: `pallet-deos-actors`; Rust crate: `pallet_deos_actors`

This document maps the independently reusable crate implementation. A host supplies `PalletId`, account derivation context, origins, adapters, bounds, fees, genesis actors, and production weights. Concrete DEOS namespace, System accounts, TMCTOL plans, runtime adapters, and operational evidence belong in [`docs/actors.integration.en.md`](../../../../docs/actors.integration.en.md).

## Executive Summary

> This document records the shipped package implementation; the standalone specification owns normative semantics and executable tests own conformance.

`pallet-deos-actors` provides a deterministic scheduler, bounded execution model, typed trigger system, lifecycle state machine, and adapter-driven task runtime for User and System actors.

The crate assigns no economic roles, assets, recipients, routes, actor IDs, or chain policy. Host behavior enters through typed adapters, origins, account derivation, weight/fee conversion, explicit bounds, and genesis actor specifications. External obligations live in the [package-owned embedding guide](./embedding.md).

## Architecture Overview

### Design Principles

1. `Deterministic scheduling`: one persistent generation-bound Service ring plus exact paged Deadline carriers, deterministic encounter order, and explicit per-block caps
2. `Execution safety`: exact current-Step control/effect Weight admission, complete Pipeline charging at Opening, and Action-only fee settlement around invocation
3. `Lifecycle correctness`: pause/close transitions are deterministic and reasoned (`CycleAdmissionInsufficient`, `WindowExpired`, etc.)
4. `Adapter isolation`: pallet never embeds DEX pricing logic or asset implementation specifics
5. `State decomposition`: semantic, certified Contract, process, Service/Deadline/Parked residence, and optional run owners remain independently bounded.

Canonical writes target one generation-bound `ActorSemanticStates` record, `ActorProcesses` record, exact process residence, and certified Contract geometry. Funding authorization remains in Contract policy, while dynamic amounts read current sovereign Available balance at each execution attempt. `ActorStateHolds` records a refundable User geometry quote for installed-lifetime capacity; Dormant Actors release that component, while System Actors remain host-capacity-backed and hold-exempt.

One crate-private loader reconstructs the certified Contract from those partitions, reads optional `ActorRunState`, and classifies the actor as `NotRegistered`, identity-only `Dormant`, exact `Active`, or `Corrupt`; exact Active requires coherent Identity, Hot, C6 Contract geometry, generation-bound process residence, and run state exactly while `cycle_state` is `Running` or `Suspended`. Its loaded state has no write-back path, and the public `ActiveActorState` returns the canonical partitions without flattening. No derived context owns authored equality or mutation.

### Host Composition Boundary

Actors executes declarative plans against host-provided adapters. Ledger, market, liquidity, staking, fee, ingress, governance, and genesis policy remain outside the crate. The package never identifies a concrete pallet, asset, actor role, route, or recipient as canonical.

### Type Ownership

`src/types.rs` is the canonical package facade and contains only public re-exports. `src/lib.rs` declares the four owner modules privately, and each owner preserves the existing `pallet_deos_actors::types::*` metadata namespace and crate-root re-export surface; no compatibility alias or second semantic owner exists.

| Module | Owned type families |
| --- | --- |
| `src/types/contract.rs` | Actor Contract, triggers, Steps, tasks, predicates, funding policy, and certified address ingress |
| `src/types/lifecycle.rs` | Identity/class, lifecycle, Continuation, outcomes, classification, simulation, and active read views |
| `src/types/scheduler.rs` | Service ring, process and Trigger Deadline topology, compatibility carriers, drain statistics, and starvation phase |
Execution logic remains in `src/execution.rs`, `src/scheduler.rs`, and `src/reactions.rs`; the type split does not move algorithms or create a mirrored model. `src/contract.rs` remains the semantic classifier and is distinct from the Actor Contract type owner at `src/types/contract.rs`.

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

The package stores each actor identity once and decomposes each active epoch into bounded semantic, certified Contract, process, residence, and optional run owners:

- `ActorSemanticStates`: canonical lifecycle semantic and Contract-generation owner; stores dormant identity plus last generation or active Identity/generation/Hot/admission independently of physical placement. Initial Contract publication uses generation one, replacement checked-increments it, dormancy preserves it, and ordinary Hot updates do not change it
- `ActorIdentities`: dormant physical identity partition only; it must equal the dormant semantic state and cannot coexist with active partitions
- `ActorProcesses`: canonical generation-bound process status and primary-residence owner
- `ServiceHeader` / `ServiceNodes`: canonical persistent service ring with one generation-bound Pending or Live node per resident Actor; the persistent cursor, reciprocal links, eligibility, and same-round consideration markers determine class-neutral encounter order
- `DeadlineHeaders` / `DeadlinePages` / `DeadlineHandles`: canonical C32 Block/Tick deadline buckets, linked pages, and exact generation-bound reverse handles for suspended processes and independent temporal Triggers; vacancy authority reuses fragmented slots and empty-page removal updates bounded key indexes transactionally
- `ActorControlCell` / `ActorControlLocators` and paged Ready/Waiting stores: retained pre-cutover compatibility and benchmark fixtures only; fresh-genesis production publication, block service, simulation, and TryRuntime reject them as scheduler authority
- `ActorContractHeads`: schedule/completion header, optional canonical parked-balance activation, semantic/body/admission commitments, Step count, optional inline Step 0, and its optional control/effect envelope
- `ActorControlCell.admission`: compact runtime semantics/layout/Weight and lifecycle identity, independent of the configured Step/resource ceiling
- `ActorContractTailChunks`: authority-bound gap-free Steps 1..N in chunks of at most four, with one aligned control/effect envelope per Step
- `ActorRunState`: sparse bounded semantic/body/admission authority, current Step cursor, cumulative outcomes, eligibility, retry, and suspension facts retained only while a multi-block Cycle is open

`ActorCreated` carries `actor_id`, owner, `actor_class`, mutability, sovereign account, and `initial_lifecycle`; User slot or System custody locator lives inside `ActorClass`. `actor_id` identifies the semantic owner and custody locator; dormant identity retains its last control-mutation block. Public creation admits Dormant only as Mutable, while host genesis may install a sealed Immutable Dormant System identity that generic activation and owner-close control reject. Activation or schedule replacement derives block eligibility from `schedule_anchor` and window start, while canonical Active semantic state owns the optional timestamp-ceiled cadence anchor. Replacement releases the superseded Trigger deadline before planning its successor, so no replaced cadence or AtTime tick survives. The typed lifecycle forbids contradictory pause state.

Package internals reconstruct execution state from the canonical semantic owner, process residence, certified Contract geometry, current sovereign ledger state, and optional `ActorRunState`. `active_actor_state` exposes canonical semantic partitions without synchronized mirrors.

This is intentionally more concrete than the paired specification: the spec defines the required logical field groups, while this document records the current package storage realization.

### Contract Steps Structure

Each actor admits `0..=MaxContractSteps` ordered Steps and stores them as C6 geometry: optional Step 0 and its optional resource envelope live in the head, while Steps 1..N and aligned envelopes occupy gap-free authority-bound chunks of at most four. A zero-Step Contract has neither inline body nor tail fragments and reconstructs from its certified header alone. One host-configured `MaxContractSteps` in `1..=255` applies identically to User and System actors across creation, activation, replacement, simulation, genesis, and benchmark construction. For a nonempty N-Step Contract, execution loads only the current Step head or one exact tail chunk; unreached tail fragments remain cold. Mutable `RetryLater` admits only `2..=MaxRetryAttempts`, with the protocol-fixed metadata constant set to 10.

- `precondition: Option<Precondition<Predicate, MaxPreconditionClauses, MaxPredicatesPerClause>>`
- `task: Task`
- `on_error: StepErrorPolicy` (`AbortCycle` / `ContinueNextStep` / Mutable-only `RetryLater { max_attempts }`)

`None` is the sole unconditional Step form. `Some(Precondition { clauses })` stores one bounded DNF: outer clauses are OR and each inner clause is AND. Admission rejects empty outer and inner vectors and enforces the configured `MaxPreconditionClauses`, `MaxPredicatesPerClause` and total `MaxPredicatesPerStep` bounds. Evaluation visits every admitted predicate without short-circuit and executes or skips the Step exactly once.

Each admitted predicate is current-state only. The evaluator reads it immediately before its Step, so it observes committed earlier-Step effects from the same logical multi-block cycle plus intervening authoritative state. No predicate result is captured at cycle admission or retained in `ActorRunState`.

Admission sorts predicates and clauses by canonical typed SCALE, removes repeated predicates within a clause, rejects clauses that become semantically identical, absorbs exact predicate-superset clauses, and stores only the canonical form. `update_contract` canonicalizes before equality and returns an exact no-op before rate limiting, cancellation, writes, placement reconstruction, or events when the resulting contract is unchanged.

`Precondition::evaluation_units` supplies the total predicate count to the host's `StepControlWeight` resource-envelope provider. It is a pricing hint and equals the bounded count of current predicate evaluations. `evaluate_step_precondition` visits each predicate once. The prepaid Pipeline Machine envelope sums admitted Step control bounds and cleanup without a separate predicate-capture component.

A false DNF expression emits `StepSkipped(PreconditionFalse)` and advances one fixed cursor; evaluation errors remain task-independent failures routed through the authored Step policy.

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
| Amount and exact-output bound | `Fixed`, `Percent`; `LiveQuote`, `Absolute` | Task constructors; amount classifier/resolution tests, removed-form decoding tests and independent exact-output evidence |
| Trigger | Exactly one of `Manual`, `AddressEvent`, `AtTime`, or `Cadenced`; source `Any`, `OwnerOnly`, `Whitelist`; asset `Any`, `Whitelist` | Actor Contract constructors plus raw typed calls; exhaustive Active replacement and Dormant lifecycle matrices; manual, certified-ingress, timestamp-cadence, and embedding tests |
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

The shared completion path emits the cumulative summary, evaluates completion policy and auto-close, clears the persisted run, derives the complete Idle residence from current state, and leaves the suffix unreachable.

The instruction does not resolve an amount, invoke a runtime adapter, select a successor, or directly mutate actor lifecycle/scheduler state. It increments `executed_steps` but not `committed_effectful_tasks`, so an empty stop cannot close a one-shot productive actor. A false Precondition advances normally; Predicate-evaluation or fee-collection failure occurs before successful stop admission and follows its owning runtime boundary.

### Amount Resolution

The pallet resolves amounts through `AmountResolution`:

- `Fixed`
- `Percent`

Resolution policy is task-bound in code:

- `PreserveSpend`: applies to Transfer, SplitTransfer, Burn, exact-input swap, liquidity add/remove, Stake, and DonateLiquidity; computes one spend ceiling as adapter-visible balance minus reserved future User fees for the native fee asset and, for User fee-native direct debits, `max(MinUserBalance, asset minimum)`; other assets retain their adapter minimum.
- `DonateLiquidity` resolves only declared `asset_a` as `max_amount_a` and passes the current preservable `asset_b` balance as the cap on pre-existing B debit; the host adapter must enforce both debit caps and report total donated amounts. An adapter may atomically acquire and donate new B above that debit cap, so generic Actors verifies returned A spend while adapter tests falsify B-cap violations. `Fixed`, every percentage basis, and `SplitTransfer` total must stay within their applicable ceiling.
- `ExpendableSpend`: consume available amount where task allows
- `Mint`: amount interpreted in mint context
- `Unstake share spend`: `Fixed` and `Percent` resolve against `StakingOps::share_balance(position_asset)`; 100% of current shares permits full withdrawal.

Resolution outcomes are deterministic:

- `Resolved(value)`
- `Skipped`
- `FundingUnavailable`

`FundingUnavailable` is a deterministic current-capacity resolution outcome for both actor classes. It advances as a non-terminal skip under `AbortCycle` and `ContinueNextStep`, while valid Mutable `RetryLater { max_attempts }` suspends at the current cursor. It covers current source/share overspend and preserve-spend resolution that would cross the minimum-balance ceiling. Actors retain no funding accumulator or Opening snapshot.

`PipelineMachineEnvelope` stores the User control and cleanup quote derived from current host Weight bindings in the certified Contract head. Scheduler admission reads that O(1) authority only when Idle paid readiness is consumable. Generated owners distinguish zero-Step, Opening, retry, continuation, completion and cleanup work; the benchmark ownership qualifications below separate priced fixtures from complete physical containment. Mutually exclusive owners combine by component-wise maximum rather than subtraction or additive overlap.

Breaker refusal preserves exact Service residence. Temporal Opening charges no second Trigger fee.

Nonempty geometry checked-sums each Step's generated maximum control owner across its authored total attempt count; `StopCycle` folds its control-only effect into machine work and remains Action-fee-free. The cleanup component uses generated `close_actor`, not the broader certificate lifecycle maximum. The production Weight identity commits zero-Step, Opening, retry/error, continuation/RunFrame, completion, placement, and cleanup owners.

Insufficient `MinUserBalance + Pipeline total` selects `CycleAdmissionInsufficient` before Opening; Running/Suspended reuse prepaid machine authority. A Suspended User separately checks current Action liability before effect-capacity deferral: insolvency closes fee-free with custody unchanged, while a solvent deferred retry retains its complete Run and Service or Deadline authority (`user_retry_insolvency_closes_before_effect_capacity_deferral`). `StepFeeBreakdown` owns Action effect fees only: non-invoked effects, false Precondition, skipped resolution, and `FundingUnavailable` charge zero; every invoked success or typed failure settles valid actual effect Weight. System settlement stays zero.

`attempt_fee_envelope` and `settle_attempt_fee_step` remain bounded forecast-vector utilities for comparative client evidence; they do not authorize runtime admission or settlement. The package-owned `fee_envelope_vectors` example emits deterministic User/System forecast, release, rollback-pricing, and protected-floor vectors consumed by browser tests.

The User state hold is paid by the identity owner through the runtime's `StateHoldCurrency` under the aggregate `RuntimeHoldReason::Actors(ActorState)` reason. Each present component prices one configured base plus a configured per-byte rate. Identity uses its concrete locator authority; the Active header prices bounded hot/process capacity with the actual Contract head and admission certificate; body uses only present tail chunks; detector uses actor-owned temporal Trigger records without shared-page slack; run uses its current encoded state. Create, update, Opening/progress/suspension, cancellation, deactivation, and close reconcile exact per-Actor deltas in the owning storage transaction. Positive-delta failure rolls back semantic state, while close releases the record without touching sovereign custody. TryRuntime rederives every record and compares each owner's aggregate dedicated currency hold.

`user_lifecycle_reconciles_exact_state_hold_and_preserves_exact_slot_custody` falsifies dormant/active hold deltas, deactivation shrinkage, close release, custody neutrality, and exact-slot reuse through real User lifecycle calls.

Resolution and charging follow these rules:

- A User run releases the skipped step's unused execution-fee reservation before resolving later steps, matching every non-executable cycle path.
- A multi-amount task resolves every field before dispatch and selects `FundingUnavailable > Skipped > Executable` independently of field order.

Pallet boundary tests cover fixed and current percentages across native, sufficient-asset, split-total, and staking-share surfaces. Decoding regressions reject retired amount discriminants rather than reinterpreting them. The embedding fixture binds unrelated host position keys to share assets without DEOS types.

Task execution is wrapped in a task-scoped storage transaction. If an adapter fails after an intermediate mutation, the task-local storage effects and success event are rolled back before `StepErrorPolicy` handling decides whether the cycle aborts or continues to the next step. Successful earlier Steps in the same Actor Contract remain committed.

`src/contract.rs` is the package-owned semantic classification surface. Exhaustive matches derive task adapter/assets/recipients/effects/availability/weight ownership/bounded algorithms, typed task amount roles with dependency and retry behavior, predicate observations and purity, and error-policy controls.

`TaskWeightOwner::weight` selects the corresponding method on the runtime's single `WeightInfo`; the module adds no codec, storage, runtime API, or parallel numeric weight authority. Package tests instantiate every current primitive, while a new enum variant makes its owning match non-exhaustive. The package example `semantic_manifest` verifies ordered task and amount coverage against SCALE metadata and emits one deterministic format-neutral contract projection.

The control-flow firewall combines closed types with adversarial evidence. `Step` metadata exposes exactly `precondition`, `task`, and `on_error`; exhaustive contracts admit no successor, nested contract, callback, generic `RuntimeCall`, or opaque dispatch field. Optional `Precondition` classification fixes full bounded visitation, whole-expression error, one admitted task, and false advance. Predicates and amount classifiers expose read dependencies and never a control target.

Service discovery validates the exact header cursor, reciprocal node links, Actor generation, process residence, semantic record, admission certificate, and current Step before execution. Future eligibility, insufficient RefTime or ProofSize, effect-capacity refusal, fee insolvency, breaker deferral, and terminal precedence retain their distinct owners. Negative resource preflight cannot authorize execution and leaves the complete Service head unchanged.

Suspended service validates the Run head, current certified Step, retry bound, and current Action liability before another effect attempt. Solvent refusal preserves the same cursor and exact Service or Deadline authority; insolvency selects custody-neutral cleanup. Current-state amount checks need no funding partition or historical snapshot, and execution loads only the certificate-gated current Step rather than reconstructing unreached tails.

Destination planning receives the loaded Identity, Hot state, optional coherent Run, admission certificate, and current Step resources. It selects Disabled, Service, or Deadline before mutation. The effectful core returns the exact successor Run for progress/suspension or no Run for a terminal outcome; publication consumes that result without reloading Run backing.

Opening prepares Trigger rearm in the same transaction that consumes Pending Service. Cadenced installs a fresh Tick `TriggerDeadlineHandle`; AtTime marks its one-shot source consumed; Manual and AddressEvent require no rearm mutation. Any rearm, hold, fee, or effect failure restores the paid latch and all source authority. Rearm reads no observation, so observation availability cannot refuse Opening.

`try_store_service_control_state` commits canonical semantic state, in-place Pending-to-Live promotion, and the User state-hold delta together. Promotion changes neither generation nor ring links. The dedicated regressions prove peer-order preservation, stale-kind refusal, complete rollback on hold refusal, and one committed effect after recovery.

Activation, cancellation, Contract replacement, pause/resume, and occurrence publication carry the validated admission identity and exact source residence into one transactional publication owner. A surviving destination is fully planned before source detachment; no scalar/canonical mode selector or fallback carrier exists.

Fresh multi-Step Opening loads only Step 0. One-Step Opening and Running/Suspended service likewise load only the head/current fragment, so unreached-tail corruption cannot block an authoritative committed prefix. The execution kernel visits at most one current Step per Actor per block after generation, process, Service, admission, resource, fee, and same-block revalidation. A nonterminal commit persists the advanced Run and retains Live Service with `eligible_at >= now + 1`; retry may move to Deadline, while completion, abort, and close commit atomically without replaying the prefix.

The execution kernel produces one `StepOutcome` for the current Step after canonical precondition, amount, fee, and task evaluation. `StepOutcome::Failed(TaskFailure)` retains the concrete `DispatchError` cause and orthogonal `RetryClass`; `execute_loaded_single_step_core` applies the authored policy without a simulation-only failure vocabulary.

One `AttemptDisposition::{Completed, Continued, Failed, Suspended, Closed}` owns production and simulation meaning; `Continued` commits exactly one non-terminal Step and persists the causal successor cursor. Production emits events and commits state from it; simulation returns the same disposition and final counters while its transaction rolls back. Bounded trace records wrap shared Step outcomes and do not reconstruct task, predicate, amount, fee, failure, or finalization semantics.

Production and simulation share current-Step transition owners, with the exhaustive policy/mutability/failure matrix and same-cursor Continuation regressions pinning retry identity. Task-local transactions forbid callback-visible partial mutation. Bounded vectors, adapter capability contracts, generated worst-case weights, maximum-plan tests, and circular scheduler stress bound adapter and cross-actor work without interpreting local plans recursively.

### Market Adapter Boundary

`DexOps` owns swap-only host execution; `LiquidityOps` owns add, remove, and donation operations. Actors supplies `ExecutionContext { actor, actor_type }`, resolved amounts, and authored spend/output bounds without knowing route topology, market identity, or price-source policy.

DEX adapters return `DexSwapOutcome { total_amount_in, recipient_amount_out }`. Liquidity adapters return actual add debits and LP output, remove outputs, or donation debits. Actors validates every returned committed fact against the authored input cap or output minimum before emitting its task event; an out-of-bound success is a Permanent failure inside the task-local rollback boundary.

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
  - admits the complete fixed Prepass, independent Block/Tick Deadline, dependency-scan and finalization envelope before mutation
  - reserves and settles one Block-then-Tick Deadline quantum before the bounded parked-balance dependency scan
  - runs the canonical Service base pass against remaining Actor Control and `ActorBaseTurn`, retaining the idle-base/finalization envelope
  - advances to `ExternalPhase` and rejects duplicate or stale execution without publishing alternate scheduler authority
- `on_idle`:
  - derives finalization and Drain capacity from current-block Actor Control remaining after Prepass
  - pre-reserves the remaining Drain-control maximum and settles generated actual control before final reconciliation
  - rejects stale, wrong-phase, or optional-halted resource state before housekeeping mutation
  - advances `ExternalPhase -> FreshDrain`, services the canonical Service ring against remaining Actor Control and Shared Economic effect capacity without another Deadline quantum, writes the latest non-authoritative finalized snapshot, and finishes successful reconciliation in `Finalizable`
- `on_finalize`:
  - consumes and requires the current block's one-pass `Finalizable` marker, zero outstanding reservations, and matching telemetry block tag
  - makes any incomplete resource protocol consensus-invalid rather than silently carrying state into the next block

### Ordinary Drain Authority

The canonical persistent Service ring is the sole ordinary execution drain. Prepass and `on_idle` share its persistent next-encounter cursor and current round identity; an empty ring stops immediately, while an authoritative blocked head prevents bypass.

Mixed System/User and lower-id regressions compare each Actor's first serviced attempt in ring order. Recurring later Cycles remain independently legal, and current `Percent` resolution or fees may change later effects. Mandatory-prepass and finalized-frontier tests derive eligibility and next-attempt capacity only from `ServiceHeader`, `ServiceNodes`, `ActorProcesses`, and current semantic state.

### Admission Gates

A cycle is admitted only when all checks pass:

1. actor is ready (`trigger`, cooldown, pause/breaker/window checks)
2. per-block execution cap (`MaxExecutionsPerBlock`) not exceeded
3. a two-dimensional `WeightMeter` can consume the complete attempt plus measured pure-cleanup weight without exceeding RefTime or ProofSize
4. For an Idle User Actor consuming paid readiness: complete Pipeline Machine payment preserves `MinUserBalance`; each invoked Action independently admits its current effect fee

The certified Contract head stores the complete bounded `PipelineMachineEnvelope`; Idle admission reads that authority without rescanning the full Contract. Opening and every later Attempt read declared predicates and dynamic amounts from current authoritative state. Running/Suspended service reuses paid machine authority, loads only the current Step, and independently admits its control, Task effect, and current Action liability.

Weight or scan deferral remains silent and state-preserving: no identity, event, nonce, cursor, Run, residence, or Task effect changes. Persistent head Weight blockage, fee-collection failure, or invariant refusal becomes observable through sparse starvation transitions; starvation does not itself identify the cause.

Deferral/terminal paths:

- insufficient Weight or scan budget → silent state-preserving deferral; actor remains active
- Pure terminal cleanup prechecks every fallible identity, count, hold, residence, reverse-index, and User-slot invariant before mutation; no close retry or requeue state exists
- After circuit-breaker refusal, admission handles the classified terminal reason before testing capacity for a new User Pipeline. Already-due nonce/failure/lifecycle closure therefore cannot be relabelled as `CycleAdmissionInsufficient`; terminal cleanup retains its own Weight admission.
- Paid readiness that cannot preserve `MinUserBalance` while paying complete Pipeline Machine/cleanup → terminal `CycleAdmissionInsufficient` process cleanup with custody untouched
- `CycleResult::Completed` means authored control reached terminal without an abort: skip-only and all-failed-`ContinueNextStep` runs remain Completed, reset `unsuccessful_attempt_streak`, and may satisfy nonce auto-close. Their counters remain factual; only at least one committed non-`StopCycle` task satisfies productive close. Abort emits `Failed`; explicit invalidation emits `Cancelled`.
- Post-failure close selects retry-local exhaustion before the inclusive global bound `unsuccessful_attempt_streak >= MaxConsecutiveFailures` when both are reached by one Attempt. The shared finalizer emits the authoritative `CycleSummary` before pure cleanup emits `ActorClosed`, matching post-success `AutoCloseNonceReached` ordering.
- Explicit, automatic, lifecycle-touch, dormant, and sweep paths share one pure cleanup routine: no Task, Precondition, fee, funding restoration, sovereign-balance movement, or population-wide scheduler scan occurs
- Owner-initiated Active or Dormant close requires `Mutable`. User Immutable rejects pause, resume, semantic Contract replacement, auto-close replacement, deactivation, cancellation, and close while still admitting its authored Manual source. A host-genesis Immutable Dormant System identity has no authored source and rejects activation. Scheduler, sweep, authored completion, auto-close, window, and Pipeline-admission terminals remain runtime-owned.
- Active close prevalidates identity, generation, process residence, counts, reverse ownership, slot ownership, and hold authority, then commits cancellation, Actor-state deletion, counter/slot release, and the close event in one storage transaction; any residual late error rolls back the complete terminal mutation
- Package and runtime tests preserve native and non-native residual custody across Mutable owner close, productive close, User Immutable zero-Step AtTime auto-close, and Pipeline-admission apoptosis; exact-slot recreation receives a fresh actor id and nonce but executes against the same sovereign account. `user_lifecycle_reconciles_exact_state_hold_and_preserves_exact_slot_custody` covers real dormant/active User hold deltas, and `user_close_reclaims_park_and_pending_without_custody_or_hold_leak` closes real User Parked/Pending residence without retained authority.
- Pure close paths, including insufficient Pipeline admission, reserve `close_dispatch_weight_upper()` through `close_cleanup_weight_upper()` and the generated `close_actor` owner. `pipeline_admission_apoptosis` benchmarks a direct minimal-finalizer fixture but has no separate production Weight consumer. The complete Service-admission refusal is measured by `scheduler_service_minimal_apoptosis` and belongs to the outer Service selector; adding the standalone finalizer Weight would double-count cleanup. These owners do not establish effectful-Step terminal containment; Service admits Step and lifecycle control separately.
- Non-attempt close removes exact Service/Deadline/Parked and Trigger authority transactionally through `remove_actor_publication_and_finalize`, releases the generation-bound process exactly once, and converges on the custody-neutral finalizer.
- Post-Attempt Service close uses `finalize_actor_from_consumed_state` followed by `retire_service_member` inside the same attempt transaction. Retirement validates generation and ring authority, relinks peers, then deletes the process rather than publishing a tombstone. Late refusal restores finalization and the exact source publication.
- Deactivation uses `detach_actor_publication` to release Service/Deadline/Parked and Trigger authority and remove `ActorProcesses` while preserving the durable Dormant identity. `remove_actor_publication_and_finalize` reuses the same bounded detach owner before finalizing.
- Every bounded window validates checked `end + 1` representability and stores that exact terminal block in primary hot state. Block service/terminal reasons share one exact Block pointer and retain the earlier target; AtTime/Cadenced detection has its independent Tick pointer.
- Paused actors remain hot-only before terminal time and load `ActorContract` only when closure is due
- With `GlobalCircuitBreaker` active, normal cycles and scheduler-owned terminal cleanup defer; bounded housekeeping plus explicit lifecycle/sweep cleanup remain available

The reference implementation selects complete synchronous reclamation rather than incremental retirement. `do_deactivate_actor`, `finalize_actor_loaded_inner`, and `close_inactive_actor` preflight exact generation-bound references and reclaim every bounded Contract, Run, detector, residence, hold, slot, and identity owner in one storage transaction. Production writes no `Retired` process and stores no cleanup debt or reclamation cursor; replacement-during-cleanup and debt-progress states therefore do not exist. The maximum User `close_actor` benchmark owns retained Run, maximum Contract, and state-hold release; integrity checks require that Weight to fit guaranteed Actor service. Full Deadline removal fixtures prove carrier occupancy cannot block removal. Introducing incremental cleanup would be a new specification, storage, hold, and Weight contract.

`terminal_service_reserves_cleanup_before_any_step_mutation` checks absent process/Service/Run authority after User/System terminal outcomes under both Control-ownership seams. `service_retirement_atomically_unlinks_each_topology_and_reclaims_the_process` covers interior/cursor/pair/singleton removal and exact-root refusal for stale generations, mismatched residence and corrupt rings. Try-state validates every process key against semantic identity ownership; `try_state_rejects_orphan_canonical_processes` rejects orphan tombstones that forward-only checks cannot see.

`ActorContract.completion_policy` defaults to `Persistent`. `CloseAfterProductiveCycle` checks cumulative `committed_effectful_tasks` only after successful logical-cycle completion, including a resumed Continuation. False latest-state Precondition results, skipped Steps, rolled-back failures, bare `StopCycle`, suspension, abort, cancellation, and retry exhaustion cannot select `ProductiveCycleCompleted`. The pure close path remains valid for Immutable System actors and preserves their sovereign balances.

Code anchors: `src/execution.rs::execute_loaded_single_step_core` owns cumulative outcomes; canonical Service settlement in `src/scheduler.rs` applies productive close after cycle completion. Pallet tests prefixed `close_after_productive_cycle_` falsify false-state, latest-state race, bare stop, retry, exhaustion, balance, and Immutable closure claims.

Lifecycle lease-by-cycles is authored by `ActorContract.auto_close_at_cycle_nonce`: after a successful cycle reaches the configured target, the actor closes with `AutoCloseNonceReached`. Complete Mutable Contract replacement may set, shorten, extend, or clear the target; every non-empty target must remain strictly ahead of current `cycle_nonce` and within `MaxAutoCloseNonceHorizon`. No field-specific setter or increment call exists.

### Fee Collection Boundary

The generic pallet collects creation, successfully processed Manual, AddressEvent, AtTime, and Cadenced Trigger occurrences, per-admitted-cycle Pipeline Machine/cleanup, and per-invoked-Action User fees through one runtime-supplied `FeeCollector`; terminal cleanup charges no Actors fee. Both User creation calls collect `ActorCreationFee` for Active and Dormant admission before identity, slot, counter, or next-id mutation; failed collection leaves every actor store and owner balance unchanged. System creation remains exempt.

Actors invokes the runtime-supplied collector exactly once only when one Trigger changes `pending_signal` from false to true. Manual, matching AddressEvent, AtTime, and Cadenced use their generated family owner and emit `TriggerOccurrenceProcessed` only for that useful transition. An already-latched occurrence performs no Actor-specific fee, event, activation, or causal-history update. Trigger underfunding rolls back readiness publication without closing the process, except that a consumed one-shot User AtTime closes through prepaid fee-free minimal apoptosis with `TriggerAdmissionInsufficient`. Collector rejection rolls back the complete occurrence and restores exact source authority. Collection is ledger-only: it performs no Actors ingress preflight, readiness mutation, or scheduler placement. Zero collection is a no-op.

`manual_trigger()` is both the signed dispatch Weight and one useful User Manual Trigger fee basis. A paid maximum-Step signed-funding/watch User latch exceeds the retained short-header dispatch coefficient in complete RefTime and even in measured PoV bytes versus retained charged ProofSize (EXP-0164). This selected result does not qualify all Manual placements or justify charging later Pipeline/Action effects at Trigger time.

A completed Cycle leaves its Actor as a retained Idle Service resident in canonical `ServiceNodes` with `pending_signal` false and no open Run. A later occurrence re-latches that resident as a fresh B+1 Pending member by releasing and re-admitting the exact ring slot, so the consumed useful-readiness member is never left labelled pending; a duplicate occurrence in the same block coalesces before fee admission. Disabled, Deadline and Parked Idle residents publish a new Pending member through the same occurrence planner.

- Predicate and Task preparation run read-only before collection determines the Step outcome.
- Idle readiness consumption first charges the fixed-size certified Pipeline Machine/cleanup total and emits `PipelineFeeCharged`; collection failure rolls back latch/residence mutation and preserves the Service head. Current-Step admission still meters control/effect Weight component-wise but reserves only the Action-effect maximum economically. Before a Suspended User retry, the scheduler proves the current maximum Action liability above `MinUserBalance`; insufficiency closes through fee-free minimal apoptosis before invocation, while the enclosing Step transaction settles valid actual cost and releases the remainder. False Precondition, skipped resolution, `FundingUnavailable`, and `StopCycle` charge no Action fee; an invoked success or typed failure settles valid actual effect Weight and appends `ActionFeeCharged` after semantic boundary events as the attempt's final economic receipt.
- Collection failure rolls back the complete scheduler transaction, including provisional Task dispatch, residence, close, and every fee, event, counter, nonce, cursor, or Run mutation. Missing or excessive actual evidence fails before collection and rolls back the same atom. Simulation reports interface-local `FeeCollectionFailed`.
- An invoked adapter failure reports `Invoked` effect evidence and retains its valid actual effect fee when the enclosing attempt and collection commit; `ContinueNextStep` and `AbortCycle` never alter that charge or trigger another collection. Zero actual total fee produces no collector call.
- The `task_split_transfer(l)` effect benchmark targets saturated Actor sovereign recipients so each transfer exercises ingress hooks. The full positive User Step diagnostic uses ordinary non-Actor recipients and asserts that boundary in `capture_user_split_header`. Their measurements cannot be subtracted to infer pure Control cost or treated as one simultaneous worst-case geometry; complete Control/effect containment requires its own admissible composition.
- `TaskEffectWeightProvider` owns maximum admission and one closed post-dispatch branch: `NotInvoked` returns zero, while `Invoked` returns the canonical generated Task-family Weight whether the operation commits or returns typed failure. Scheduler service rejects absent or component-wise greater-than-reserved evidence and rolls back the complete process/Step/effect transaction. Successful non-invocation consumes control only and releases the effect reservation inside the pass budget.
- `StepControlWeightProvider` receives the starting phase, committed outcome, canonical post-placement class (`None`, `Queue`, or `Wakeup`, where the retained code names map to no residence, Service, or Deadline), typed Task invocation evidence, and independently whether nonzero Action fee collection is required for the attempt to commit. The runtime keeps invocation-receipt Control separate from fee collection. A receipt-free estimate adds the generated receipt owner for general-path invocation, including System zero-fee attempts; a direct estimate already containing receipt deposition adds nothing. Maximum composition compares complete envelopes, and conservative maximum fallback retains its included receipt allowance. `StopCycle` emits no receipt, with or without predicates; a host quote's conservative receipt allowance does not prove emission. These additions are staged components; complete generated Weight rebinding remains required.
- Missing or component-wise greater-than-reserved actual control evidence rolls back the shared Step/placement transaction. The host binds generated owners to the admitted Contract geometry; independent storage maxima do not prove a jointly reachable context. Independently estimated ProofSize cannot be subtracted soundly to remove overlapping owners.

Admission and resumed control contexts price current-state work and current authored geometry. Host models distinguish Fresh Opening, Running progress/completion, retry and non-closing failure over legal Contract geometry. Persistent completion does not certify destruction: Service retains its Step maximum plus a separate lifecycle allowance on Close. No owner reconstructs unreached tails or derives a coefficient by subtracting another branch.

The final release binding must regenerate those owners from the converged source tree and reconcile runtime selectors, metadata, vectors, Wasm, and exact replay together. Package architecture deliberately carries no intermediate hashes or coefficients.
- `fee_native_protected_minimum` accepts `FeeAssetClass::{FeeNative, Other}` and applies `max(MinUserBalance, asset minimum)` to User fee-native direct preserve-spend capacity and `SwapOut` input capacity after the selected reservation; other assets retain their adapter minimum. Public callers cannot invert this safety distinction through an unlabeled boolean.

Pallet regressions cover one useful charge and no redundant latched charge for every Trigger family, re-arm, automatic underfunding, independent temporal pointers, Pipeline collection rollback, exact admission boundaries, zero-Step service, each Action outcome, task rollback, valid-actual release to zero, missing/excessive evidence, direct User-floor preservation, and exact-output input-cap failure. Package tests reject Pipeline service one unit below `MinUserBalance + Pipeline total`, preserve committed Trigger fees, and prove Running/Suspended service has no renewed machine-solvency owner.

### Progress-Preserving Continuation

`ActorSemanticStates.hot.cycle_state` selects sparse Run state. Idle has no Run; Running and Suspended retain one `ActorRunState` record carrying Contract/admission identity, nonce, cursor, retry count, causal block, eligibility, cumulative outcomes, latest outcome, and optional suspension reason. It carries no historical amount or predicate state. TryRuntime checks phase/store equivalence, cursor/retry bounds, identity bindings, and run coherence.

Opening derives one checked successor nonce. A nonterminal Step persists Running with the next cursor and `eligible_at >= now + 1`, preserving Q1 and the committed prefix. Temporary `TaskFailure` or `FundingUnavailable` under valid Mutable `RetryLater` suspends only the current cursor. The first unsuccessful attempt stores one; same-cursor retry checked-increments, while progress to another cursor resets cursor-local attempts.

`transition_failure_streak` is the sole mutation formula for the global streak: unsuccessful attempts checked-increment, while completed execution or semantic Step replacement resets it. Inclusive local exhaustion selects `RetryAttemptsExhausted` before a simultaneous global threshold; an earlier global threshold selects `ConsecutiveFailures`. Exhaustion emits `CycleSummary(Failed)`, clears the Run, and closes without `CycleCancelled`.

`scheduler::retry_backoff_blocks` maps persisted attempts through checked capped exponentiation to `1, 2, 4, 8, 8...` blocks. A one-block successor stays Live in Service with next-block eligibility; a later target moves to the exact Block Deadline. Retry reuses the Cycle nonce and executes only the suffix. Cadence remains an independent Trigger deadline and creates no second Cycle.

`Backoff decision evidence`: `tests/fixtures/retry-backoff-decision.v1.json` retains capped exponential after bounded maximum-occupancy comparison; implementation and tests enforce checked arithmetic and the canonical sequence without a lookup table.

`Attempt identity proof`: `cycle_nonce`, cursor, block number, and event order identify each Opening or resumed attempt without storing a cycle-global attempt ordinal. The one-Step-per-Actor-per-block invariant is enforced by Service round/process markers, not per-Step tickets.

Canonical Service dispatches zero-Step, StopCycle, and effectful attempts through the generation-bound semantic process. Task-scoped rollback leaves earlier committed Steps intact while rolling back only the current failed Task transaction; same-cursor retry and cancellation never replay or compensate the committed prefix.

`simulate_current_contract` is the rollback core behind `ActorSimulationApi`. It requires an exact Active Contract, generation/process/residence, mode/Run state, current liveness, User fee capacity, and `SimulationBudget { actor_control, shared_economic }`. CurrentRun accepts Running or Suspended authority only when semantically ready; waiting work returns `NotReady`. Insufficient component-wise resources return `ResourceDeferred` without fabricating an attempt.

Simulation uses the production current-Step evaluator, resource admission, fee bounds, and terminal finalizer, returns at most one bounded Step record, and wraps the operation in `TransactionOutcome::Rollback`. Actor state, custody, fees, events, and adapter effects therefore remain unchanged. Synthetic budgets prove transition parity only, not production block capacity.

`canonical_step_transition_matrix_has_production_simulation_parity` covers every transition-table row, Actor type, mutability, error policy, outcome, local/global bound, and fresh/resumed attempt. Independent production evidence checks events, counters, cursor, failure streak, fees, balances, untouched suffixes, final disposition, and both Weight dimensions.

Contract replacement, schedule/window change, deactivation, terminal cleanup, and explicit cancellation share `cancel_run_internal`. It requires coherent semantic, Contract, process, residence, admission, and Run authority; absence or contradiction returns typed `ActorNotFound`, `ActorRunInvariant`, or `ActorInvariant` before mutation. Exact no-op updates return without state or event changes.

`cancel_actor_publication` releases exact Service/Deadline/Parked and Trigger authority, clears the Run, and republishes one complete Idle successor when the Actor remains Active. Close instead detaches publication and invokes synchronous finalization. Destination planning precedes source mutation, and any invariant, capacity, hold, or fee refusal rolls the complete transaction back.

Temporal progression reloads canonical authority after source extraction and hold reconciliation. Typed terminal classification closes through the ordinary finalizer; underfunded one-shot User AtTime selects minimal apoptosis; a nonterminal occurrence uses the shared Trigger publication owner.

Every external control uses one class-independent actor/block mutation clock. Identity, generation, Hot state, process, residence, Contract, and hold writes remain transaction-owned fallible mutations rather than panic-only assumptions.

Cancellation emits `CycleCancelled` before `CycleSummary(Cancelled)` without compensation or prefix rollback. `CycleStarted` appears once per nonce; `CycleContinued` and `CycleSuspended` carry Actor, nonce, and cursor. Current sparse state is canonical-chain truth, while unbounded attempt history remains materialized.

### Read-Only Eligibility Projection

Version 6 `ActorEligibilityApi` owns only `actor_eligibility`, the read-only semantic Actor projection. It mirrors `apply_admission` and reuses the exact cadence, retry, window, failure-limit, breaker, and latch owners, so clients do not reproduce scheduler arithmetic.

The projection is one algebra: `NotRegistered`, `Dormant`, or `Active(ActorClassification)`. Active Trigger activation exposes only the Manual, AddressEvent, AtTime (delay and consumption), or Cadenced (period) shape. Active eligibility preserves terminal reason and the exact `ActorExecutionPhase`, including `WaitingRetry(block)`, `WaitingBlock(block)`, and `WaitingCadenceTick(tick)` payloads; no parallel phase or next-block field exists.

The projection persists no state, emits no event, and promises no service. Service-ring position and available Weight still decide actual admission. Arithmetic overflow and malformed Actor run state return typed projection errors rather than an inferred phase. Nonce exhaustion is projected as the same terminal close as execution without attempting an overflowing successor nonce.

Authoritative amount arithmetic uses checked accumulation/subtraction: normalized split legs plus retained remainder exactly conserve the resolved total, including `u128::MAX`. Fee reservations, existential protection, and percentage amount resolution intentionally floor unavailable spendable balance at zero. Saturating `Weight` composition remains a conservative upper-bound cap, scheduler pass statistics and starvation counters remain bounded telemetry caps, and try-state topology/accounting counters fail on overflow rather than masking malformed state.

Code anchor: `src/scheduler.rs::actor_eligibility`; package tests prefixed `eligibility_projection_` falsify absence, dormancy, every Active phase, terminal coexistence, and exact temporal payloads.

### Read-Only Cost Projection

`ActorCostApi::actor_cost_quote` returns one bounded current quote without combining economic owners. Active Actors expose the exact family-specific Trigger occurrence Weight and fee, the stored upfront Pipeline Machine/cleanup amounts and admission/production Weight identities, and the current Step's maximum Action-effect Weight and fee. Dormant Actors have no prospective Trigger or Pipeline charge, while a zero-Step or `StopCycle` current branch reports zero Action maximum.

The quote returns the configured Creation Fee separately from the current refundable state hold. User hold provenance separates identity, Contract head, actual retained tail body, detector, and run components with base/per-byte pricing and a checked total; System Actors are explicitly exempt. Active identity uses the canonical semantic identity, while Dormant identity uses its registry value. The head component includes actual certified head/admission bytes and type-derived maximum mutable control, cursor, eligibility and resource-envelope capacity; it is not a claim about the current SCALE payload length. Ordinary reconciliation and the direct benchmark path use the same quote, and detector accounting follows the resulting reference/re-arm state. Zero/one-Step Contracts reserve no maximum tail body. The bounded read-only method exposes no remaining machine budget and returns typed absence, corruption, overflow or missing-Weight errors.

Code anchor: `src/lib.rs::actor_cost_quote`; `actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate` falsifies User/Dormant/System separation, generated identities, current Action ownership, and hold totals.

### Persistent Service Execution

`classify_actor_loaded` owns non-economic terminal precedence, breaker and pause phase, current run timing, and useful readiness. Pipeline affordability remains an Opening boundary: sweep, Running/Suspended classification, eligibility, and certified ingress do not predict a future Pipeline payment. Simulation uses the same separation and rejects malformed semantic, process, Contract, or run authority.

`ServiceHeader` and `ServiceNodes` form one persistent circular ring. The header stores the next Actor generation to encounter, exact occupancy, and current round block. Each node stores reciprocal neighbors, `Pending` or `Live` residence, `eligible_from`, and `last_considered`. `ActorProcesses` binds the node to the same generation and `ProcessResidence::Service` kind.

Each admitted canonical encounter calls `begin_service_round` to bind the block identity and `consider_service_head` without moving a blocked or closed head. A successful retained turn records `last_attempted`, marks the node considered, and advances to the captured successor. An Idle resident with no admitted work advances through the dedicated no-attempt transition. Removal, Deadline transfer, Park transfer, close, and insertion preserve the cyclic order of surviving members.

Fresh publication in block `B` sets `eligible_from >= B + 1`. `last_considered` and process `last_attempted` prevent a remove/reinsert, second pass, or internal callback from granting a second same-block turn. A successful nonterminal Step remains Live in the ring with its advanced cursor and next-block eligibility; it does not allocate a successor ticket. A useful Idle Trigger publishes Pending Service only once, while busy Running/Suspended source activity creates no deferred Cycle.

The current Service head is authoritative. User/System class, Actor id, Task shape, and fee status create no alternate priority. If the complete transition cannot fit RefTime, ProofSize, effect capacity, or the admitted economic boundary, later residents do not bypass it. Corrupt generation, links, residence, Contract authority, or run state fail closed without fabricating absence.

`execute_cycle_to_cutoff_with_resources` reserves the pass Actor Control maximum and derives remaining Control by checked subtraction of `consumed - charged effects`. The caller-owned seam admits discovery, eligible-Actor loading and the complete selected branch in stages, independently of the combined hook meter. It suppresses nested Control booking, never measured actual Control; Action effects retain their own reservation. Direct callers reserve both domains. `pass_owned_control_matches_direct_service_actual_accounting` and the independent-dimensional no-borrow regression bind this distinction for ordinary service.

Discovery reserves `service_round_begin_populated + service_round_probe_eligible` before topology access or round-marker mutation. Only an eligible frontier admits `scheduler_actor_state_probe` before semantic/Contract/Run loading. Empty, closed and already-attempted encounters pay discovery alone. Prepass and Drain each use one bounded pass with no preliminary uncounted attempt or outer peek; a concluding empty/closed probe is paid too.

Each Step, its round opening and its resulting Service, Deadline, Parked, Disabled, or close transition share one storage transaction. Actual Task effects and Action fees settle only with the complete transition. Pipeline/Action collector rejection, placement failure, or resource refusal preserves the current head and prior authority. A due terminal reason closes through the synchronous finalizer rather than advancing an Idle Actor past its terminal boundary.

Prepass admission retains the generated `scheduler_on_idle_base + block_resource_finalize` envelope. Optional Service or Deadline work may halt, but the mandatory ExternalPhase-to-Finalizable transition remains admissible and publishes only reconciled resource evidence. Missing actual effect evidence retains its admitted effect maximum and halts optional work; that charge is conservative accounting, not a measurement.

The production block path and focused regressions cover effectful Steps, zero-Step Opening, current precondition skip, Pipeline apoptosis, proof-only refusal, same-block suppression, terminal close, fee rollback, and exact finalization. `QueueDrainStats`, `WakeupDrainStats`, and finalized resource projections are bounded diagnostics; they do not create scheduler authority.

The deterministic production-resource collector separates base and Drain dispositions, checks Q1, records lifecycle boundaries and failures, reconciles Actor Control versus Task effect Weight, and emits bounded latency and stop-reason measurements. Release measurements belong to the generated binding and assurance artifacts for the exact source tree; historical profile values are not package architecture.

Profile-specific throughput, saturation, latency, and counterfactual conclusions remain experiment and release-assurance evidence. They are regenerated only after the final canonical Weight binding; package architecture retains only the scheduler owners and invariants that those measurements falsify.

Cadenced materialization, Deadline extraction, dependency review, Service execution, and completion remain independent measured owners. Their component-wise totals must come from the current generated binding; removed cursor and paged-FIFO coefficients are not architecture inputs.

`service_terminal_control_upper` reserves `close_dispatch_weight_upper` before Step mutation when authored completion/nonce policy or the current retry/global-failure frontier can close the Actor. Effectful and zero-Step branches admit this allowance independently of effect capacity and in both Weight dimensions. A Closed attempt retains the complete admitted Control maximum, not a narrower persistent-completion profile; nonterminal success releases the allowance, while late rollback retains it. These conservative charges do not certify complete generated containment.

`terminal_service_reserves_cleanup_before_any_step_mutation` proves one-unit RefTime/ProofSize refusal, exact admission and unchanged source authority for User/System Actors through direct and pass-owned service. The late-collector regression covers both persistent and closing continuations, preserving prior commits and the exact Run on rejection. Pre-Step terminal classification retains its separate pure-cleanup admission; minimal Pipeline apoptosis remains custody-neutral and charges no Pipeline or Action execution fee.

`BlockResourceState::reserve_actor_step` atomically reserves one Step's Actor Control and phase-specific Actor Base/Drain effect maxima; a failed second reservation restores the complete prior state. `settle_actor_step` similarly commits both valid actual values or restores state and both one-shot reservation authorities.

`CyclePass` accumulates effect ledger deltas on success and rollback, deriving Actor Control as checked `consumed - charged effects`. `StepRollback` carries any valid effect observation independently of the storage transaction. A rejected admitted branch retains its Control maximum and the valid effect charge, or its effect maximum when actual evidence is untrustworthy. Capacity refusal before branch admission charges inspection only. The worker restores the semantic/ledger prefix, books incurred work once, releases reservations, and halts optional service without refunding performed work or touching earlier commits.

Deadline processing, Service encounters, actor probes, ordinary Cycles, and pure terminal cleanup use two-dimensional `WeightMeter` fit checks. Direct ingress is charged at its originating producer. Close admission uses the generated worst-case User cleanup bound and performs no shared-container scan.

Scheduler accounting stays local unless it controls consensus progress. `scanned` and `executed` enforce independent per-pass ceilings; bounded drain statistics expose operation results to callers, tests, and benchmarks without storage writes. Operators derive completed admission from `CycleStarted`/`CycleSummary` and persistent blockage from canonical Service/Deadline state plus starvation detection/recovery events. Loaded-actor and page-touch diagnostics remain stress instrumentation rather than permanent consensus counters.

### Deadline Layer

The canonical Deadline carrier owns known-future process eligibility and temporal Trigger detection without conflating either with Service execution. A serving process owns at most one `DeadlineHandle`; `AtTime` or `Cadenced` may independently own one `TriggerDeadlineHandle`. Every handle binds Actor id, generation, clock/key, page, and slot, so one clock-domain transition cannot invalidate unrelated authority.

`DeadlineHeaders` owns each nonempty Block/Tick key. `DeadlinePages` stores linked C32 pages with exact live counts and vacancy links. `DeadlineHandles` and `TriggerDeadlineHandles` provide reverse ownership. `DeadlineIndexPages`, per-clock lengths, and exact key positions form bounded min-heaps, avoiding actor scans and sparse-key walks.

Insertion preflights the complete destination before mutation. `first_vacant_page` selects one admitted nonfull page; full-page insertion removes that page from the vacancy chain, removal from a formerly full page restores it, and empty-page removal updates neighbors, key ownership, heap position, and reverse handles transactionally. EXP-0163's accepted C32 vacancy design is the only supported geometry.

A retry with later eligibility moves atomically from Service to a Block Deadline. Due process extraction moves the same generation back to Live Service with next-block eligibility before another Action attempt. A Parked dependency review uses the same carrier but returns through Pending interpretation. AtTime/Cadenced deadlines invoke only their typed Trigger worker and never execute a Step directly.

Due processing admits complete source extraction, destination publication, heap repair, and any immediate lifecycle consequence before mutation. Independent RefTime or ProofSize refusal preserves the exact source handle and frontier. `canonical_effectful_later_retry_moves_through_deadline_and_reenters_once` verifies that a singleton Block return one unit short in either dimension consumes only the classifier Weight and leaves the complete source/Service root unchanged. A corrupt page, link, handle, generation, process residence, or heap position fails closed and cannot skip to a later key.

`execute_mandatory_prepass` is the sole production caller of `service_due_deadline_frontiers`. `deadline_service_weight_upper` composes both selectors and component-wise branch maxima; the complete envelope is reserved before either clock runs and actual work settles before the dependency scan or Service. Drain does not repeat the quantum. `mandatory_prepass_services_both_deadline_clocks_before_external_dispatch` and `mandatory_prepass_deadline_reservation_refuses_each_dimension_before_mutation` prove phase placement, B+1 return, source/custody preservation, dimension-independent refusal and finalization headroom. Generated orchestration and exact-Wasm binding remain qualified until refreshed for this path.

The runtime `mandatory_deadline_reservation_composes_both_clock_branch_maxima` regression checks each selector and conservative branch maximum in both dimensions. The shared review ceiling also contains pending-event alternatives; it does not claim that those events run inside Deadline service. `return_due_block_deadline_to_service*` belongs to this Prepass bound, not to `scheduler_complete_outer_weight_upper`; the runtime tests their budgets separately. The Service outer selector still conservatively includes `scheduler_due_deadline_to_service*` profiles, but no current Service path invokes due-frontier traversal: only mandatory Prepass does. These generated alternatives are not an independent live Service charge; removing them from the conservative selector waits for complete live Service containment and coordinated Weight rebinding.

Cancellation, replacement, pause, deactivation, and close remove only their exact generation-bound process and Trigger handles. Surviving destinations publish before hold reconciliation; any late failure rolls back source removal. TryRuntime reconciles every page, vacancy link, header count, reverse handle, key position, heap order, process residence, and semantic pointer.

The package still compiles narrow paged Ready/Waiting helpers for benchmark or historical compatibility fixtures, but fresh-genesis publication and production block service create no legacy ticket, control-cell, Waiting pointer, or sparse wakeup cursor. Those carriers are not fallback authority and are rejected by canonical loading, simulation, and TryRuntime.

Package benchmarks cover Service-to-Deadline placement, due return, retained-bucket vacancy variants, singleton-key deletion at maximum index depth, dependency review, and temporal Trigger owners. Each generated branch admits both Weight dimensions and all required reads/writes before mutation.

### Starvation Safeguard

The scheduler first admits `scheduler_on_idle_base`. If that fixed two-dimensional envelope cannot fit, `on_idle` returns zero without storage or telemetry work.

With the breaker inactive, an inspected canonical Service head that refuses before any admitted attempt because of resource or invariant failure marks the pass starved. Paid empty/closed/already-attempted discovery, an admitted attempt and completed scan bounds remain healthy exits. `DiscoveryUnavailable` supplies no frontier evidence: without earlier progress or an independent accounting fault, `CyclePass::starvation_observed` stays false and telemetry freezes. The saturating state emits `IdleStarvationDetected` once at threshold and `IdleStarvationRecovered` once on an evidenced healthy exit; Healthy blocks perform no telemetry write.

Weight or ProofSize refusal preserves the exact Service head for a later conforming pass. Fee-collection refusal rolls back the complete attempt and retains the same authority. A structural invariant stall persists until corrected by fresh-genesis source repair or a deployed-lineage migration; starvation telemetry grants no bypass, reordering, or emergency execution authority.

An accounting halt inherited from Prepass is blocked service, not recovery. Drain does not retry the rejected head; repeated failures reach the alert threshold, and restored valid execution permits recovery. The global breaker still freezes observability. `mandatory_prepass_retains_rollback_weight_and_drain_does_not_retry` proves that path alongside exact hook/domain totals and finalization.

Code anchors: `execute_cycle_to_cutoff_inner`, `service_canonical_round_head_inner`, and `update_idle_starvation_state`. Package tests cover independent discovery/loading refusal with trie-recorded read boundaries, exact selector admission, same-block suppression, defensive attempt markers, bounded scans, unknown-frontier telemetry freeze, invariant stalls and recovery.

### Temporal Triggers

- Temporal readiness is deterministic only; Actors exposes no probability field, entropy provider, secure/insecure branch, hash fallback, probability event, or probability error.
- `AtTime` and Cadenced use `temporal_anchor_tick = ceil(timestamp_millis / CadenceTickMillis)` as their exact origin and derive no actor-specific phase. `MaxTemporalDelayTicks` bounds this timestamp domain independently from block-domain `MaxExecutionDelayBlocks`; neither horizon is converted or reused across clocks.
- Genesis records an uninitialized temporal anchor and one tick-zero bootstrap Deadline because no consensus timestamp exists during construction. First ordinary temporal service initializes the anchor and re-places the full-delay deadline without latching readiness, charging a Trigger fee, or entering Service. `temporal_bootstrap_rearm_respects_independent_resource_dimensions` covers AtTime and Cadenced canonical publication: one-unit-short RefTime and ProofSize after selection preserve the exact root, while full admission changes only temporal anchor/deadline authority. The reference-runtime Fee Sink test proves the real genesis actor anchors at its first consensus timestamp and cannot execute early.
- Active installation and replacement anchor directly because consensus time is available. Eligibility projects authored AtTime/Cadenced geometry and one-shot consumption, while canonical Trigger runtime state and `TriggerWakeupPointer` own the exact deadline.
- AtTime admission prevalidates nonzero bounded `after_ticks`, zero cooldown, no window, and checked `anchor_tick + after_ticks` arithmetic. A due occurrence marks `consumed = true` and never rearms or catches up.
- Cadenced admission applies the same policy to `every_ticks`. Runtime rearm selects the first aligned tick strictly after the observed tick, so missed periods coalesce without catch-up bursts.
- A due AtTime occurrence clears its Trigger pointer and permanently marks the one-shot opportunity consumed. A useful Cadenced occurrence clears its Trigger pointer and leaves temporal detection disabled while readiness remains latched. Already-latched due work performs no Actor-specific processing.
- Opening atomically rearms Cadenced to the first aligned tick strictly after current consensus time before Pipeline charging, registering the canonical `TriggerDeadlineHandles` member alongside the semantic `trigger_wakeup_pointer` so the shared deadline frontier observes it. Failure rolls back readiness consumption, the member, and fee movement. Only canonical Service residence executes the Actor; any future probabilistic execution requires separate admission policy and secure runtime entropy.
- `deadline_destination_page_bound` caps each bucket at `ceil(MaxActiveActors / 32)` retained C32 pages. `DeadlineHeader::first_vacant_page` and page-local vacancy links make planning read one header and at most one nonfull page without mutation. Insertion unlinks a newly full page; full-page removal links the newly nonfull page; empty-page removal releases both memberships. Insert, remove, and planning reject inconsistent header counts or missing vacancy authority, and insertion/move validate C32 length before slot access. Page/member saturation refuses without enlarging the carrier.
- `deadline_destination_search(p)` remains the standalone comparative benchmark for EXP-0163, not a separately consumed production `WeightInfo` charge. Live planning belongs to the complete caller's retry, return, temporal or other Deadline owner. Comparing that unused standalone equation to the block budget does not prove complete-owner containment.
- `deadline_vacancy_membership_survives_all_page_removal_orders_and_refills` checks complete physical/vacancy reachability and exact generation-bound handles through all six three-page removal orders. The `deadline_vacancy_moves_*`, `deadline_vacancy_neighbor_refusal_*`, and `deadline_vacancy_rejects_*` regressions cover surviving placement, transaction rollback, truncated pages and lost heads; `deadline_destination_search_is_bounded_by_reachable_active_population` covers maximum population and page/member refusal. EXP-0163 owns the single destination-planning comparison; the changed validation and maintenance paths still require complete generated Weight and artifact reconciliation before release acceptance.
- `temporal_actors_share_one_deadline_heap_key_until_last_close` runs in the default test suite and checks exact surviving handles and shared-key release in both close orders. The `mixed_deadline_close_*` fixtures construct an actual Running Actor among idle Cadenced peers, derive page IDs from canonical handles, and verify retained-page holes, interior-page unlink, full heap-tail reclamation, and unchanged sovereign custody. Their historical `close_actor_mixed_waiting_*` diagnostic IDs remain stable; native try-state and benchmark-Wasm smoke validate the corrected setup, not production Weight.

### Trigger Sources

Every Trigger family reaches activation only for an unset readiness latch. Manual rejects redundancy before evaluation or charging; AddressEvent matching and temporal extraction observe the latch before Actor-specific fee work. No Trigger family owns subscription or range topology, so latching and Opening mutate no detector index. Canonical Pending Service publication separates read-only destination planning from mutation.

Successful temporal occurrence carries one Actor-generation authority into the activation commit. Ordinary planning selects Disabled, Service, Deadline, or Parked residence before mutation. Source-specific code owns only detection and source-state progression; no parallel signal key or source tag enters consensus state.

The scheduler carries one loaded Active state through generation/process/Service verification, same-block and lifecycle gates, classification, attempt admission, fee admission, and loaded-head settlement. Control preflight, ingress preflight/consequence, eligibility, simulation, sweep, and activation likewise classify with the Continuation returned by their complete canonical probe instead of rereading it through a view-only helper. A transaction boundary or adapter/task effect that may mutate actor state terminates that borrow and requires an explicit canonical reload before later placement.

Active System installation and replacement invoke the narrow `SystemActorContractValidator` host port before committing the Contract. Generic Actors neither derives a graph nor stores ranks or edges. The unit implementation accepts all contracts; the reference runtime binds its bounded topology policy independently from the portable package.

AddressEvent filtering supports source `Any`, `OwnerOnly`, or `Whitelist` and asset `Any` or `Whitelist`. Every producer event evaluates its configured source atoms and folds all matches into one readiness decision. Funding authorization runs once against the independently supplied source/provenance; custody effects are not coalesced with readiness.

The matched `address_event_trigger_occurrence()` owner contains Actor-specific detection, one useful User fee collection and canonical readiness placement, not the producer's source publication. A fully admitted maximum filter/funding/watch User header exceeds its retained short Any/Any benchmark in RefTime and ProofSize (EXP-0164). This selected counterexample is not a universal coefficient or a second producer charge.

Active control has one semantic owner in `ActorSemanticStates`, one generation-bound `ActorProcesses` record, and exactly one primary process residence. Service owns Pending/Live execution readiness, Deadline owns known-future service, Parked owns a certified negative current-state plan, and Disabled owns no residence. Independent AtTime/Cadenced Trigger deadlines do not execute Steps.

Moving transitions carry validated semantic, Contract, admission, Run, and source-residence authority into destination planning. Commit detaches the source only after the complete destination is admissible; later failure rolls back both. Canonical loaders return bounded projections rather than a second mutable store.

Package tests exercise useful/redundant occurrence boundaries, generation binding, committed-prefix durability, corrupt ownership, lifecycle changes, Service/Deadline/Parked geometry, and transactional rollback. Benchmarks separately measure Trigger detection, destination publication, execution control, Task effects, and cleanup. Host adapters, asset/account types, genesis topology, resource limits, and generated production weights remain embedding responsibilities.

Opening consumes one signalled latch atomically.

The typed `AddressEventIngress::preflight`/`notify` boundary owns signal filtering and funding-policy authorization. Producers perform literal read-only preflight and invoke exactly one consequence under their declared post-movement or transactional-precommit atomicity protocol. They propagate rejection and never mutate control or funding storage directly; the host integration owns protocol inventory and rollback evidence. The private transition has one full-ingress contract; dead `apply_trigger` and `apply_funding` switches are removed, so a caller cannot partially apply one admitted event.

`IngressFailure { error, retry }` classifies recoverable Service/Deadline capacity or placement unavailability as Temporary. Index exhaustion, topology corruption, invalid provenance, and invariant failure are Permanent. Actor tasks preserve the classification through `TaskFailure`; non-Actor producers map it to their outer dispatch error.

The package never scans host events, fingerprints value transfers, or defers ingress correctness to `on_idle`. Trigger filtering consumes only the independently supplied source; funding authorization consumes source and typed provenance without inferring either from the other. `OwnerOnly` and signed allowlists require Signed provenance plus a matching source, `AnyVerifiedIngress` requires at least one verified field, and all-None context remains funding-ineligible.

Host-decided `RuntimePolicy` receives both optional fields unchanged. Accepted funding changes ordinary sovereign custody after bounded policy authorization; Actors stores no tracked-asset set, accumulator, or funding-history basis.

TryRuntime checks current run payload and cursor coherence independently from Contract funding authorization. No Actor-only funding accumulator or opening balance snapshot exists.

### Manual

`manual_trigger` admits one idle Manual occurrence after origin, lifecycle, window, and Trigger-fee checks, latches readiness, and publishes B+1 Service. Step-0 observation predicates are evaluated only at the later Attempt; a Manual call never parks on observation state.

Every other Trigger fails with `ManualSourceDisabled`; paused calls fail with `ActorPaused`, and System Immutable calls fail with `ImmutableActor`. Opening consumes the latch; resource refusal preserves it.

---

## Storage Topology

Primary storage follows explicit owners. Section 13's stable behavioral stores constrain compatibility, while bounded scheduler and ingress machinery remains replaceable implementation state. No synchronized readiness mirror remains.

- `NextActorId`: monotonic actor ID allocator
- `ActorSemanticStates`: canonical dormant-or-active semantic and Contract-generation records. Lifecycle writers publish, checked-increment, preserve, replace, and remove them transactionally. Active records expose the nonzero generation-bound `ActorRef` required by every process carrier
- `ActorIdentities`: dormant physical identities retaining owner, class/custody locator, mutability, cycle nonce and last control-mutation block; each must equal its canonical dormant semantic record
- `ActorProcesses`: generation-bound status and primary-residence record. Creation publishes it; Contract replacement releases the old residence and rotates process and semantic generation atomically. The publication boundary rejects stale authority, multiple primary residences, and failed destination planning
- `ServiceHeader` / `ServiceNodes`: authoritative persistent-ring header and actor-keyed nodes. Publication inserts the process into an empty or populated ring atomically. Append, advance, and remove verify process residence, generation, local links, singleton/interior/cursor structure, count, and B+1 eligibility
- `DeadlineHeaders` / `DeadlinePages` / `DeadlineHandles`: key-addressed bucket headers, retained linked C32 pages, and exact Actor-generation reverse handles published by canonical creation, lifecycle republishing and Contract replacement; transaction-local insert/remove/move preflights destination capacity, validates all three ownership surfaces, and restores source authority on failure. The mixed due dispatcher returns retry deadlines as `Live` continuations and positive Park reviews as `Pending` activation checks instead of applying one caller-supplied role to both branches.
- `BalanceDependencySources` / `DependencySourceBalances`: collision-free forward/reverse source ownership for bounded authored asset watches; completion allocates each reachable source atomically with registration, and adapter mutation ingress advances the checked revision without replacing mandatory timed-review authority. Maximum-watch due rearm has complete retained-source/populated-destination and full-index source-replacement owners. The shared selector consumes subthreshold Pending into a complete source snapshot with unchanged anchors or removes the exact deadline and wakes B+1 Service at threshold.
- `Parked Balance Review Weight Owners`: `dependency_review_weight_upper` selects the component-wise maximum of the due parked-balance review, its deep-index variant, and the Pending parked-balance event owner. Observation state is never a Park dependency.
- `DependencyScanSourceListState` / `DependencyScanSourceNodes`: authoritative bounded circular selector for event-complete dependency scans. Idempotent insertion, arbitrary completion, and removal preserve one fair cursor. Revision coalescing retains one source membership; mandatory prepass advances one admitted member or exact completion/handoff atomically. Malformed topology and capacity refusal preserve prior authority.
- `ActorContractHeads`: scalar Trigger, schedule/window/completion authority, optional bounded Parked Balance plan, commitments, Step count, inline Step 0, and its resource envelope
- `ActorUnsignaledControlCells` / `ActorReadyFrameChunks` / `ActorWaitingFrameChunks` / `ActorControlLocators`: retained pre-fork compatibility declarations with no fresh-genesis publication, execution, or TryRuntime authority

Runtime fixture clock aging mutates only `ActorSemanticStates`; multi-Step and cycle-nonce drivers derive Running authority from that canonical owner. Shared quiescence progression recognizes future `ServiceNodes.eligible_from` authority before advancing through B+1 eligibility, while exact-boundary fixtures progress one block at a time against that canonical eligibility.

Paid two-Actor Service, and circular Actor-graph recursion fixtures assert exact B+1 residence with no legacy ticket. One-block progression prevents quiescence draining from hiding same-block recursion.

The temporary market-retry fixture proves short retry remains in canonical Service, long backoff moves to an exact Block Deadline, Deadline return is Control-only, and the returned B+1 Service turn owns the next Action attempt without legacy queue or wakeup authority. Temporary Oracle-capacity refusal preserves one exact canonical Service retry owner while rolling back cross-system economics and retaining no legacy queue or wakeup authority.

The frozen-current-balance liquidity fixture records the committed prefix and failed tail on consecutive Service turns. It then proves exact cooldown residence in a Block Deadline, Control-only due return, and B+1 resolution-skip service without custody movement or legacy authority. The ED-ineligible split-transfer fixture proves atomic preflight refusal, one short retry retained in canonical Service, B+1 reattempt after recipient repair, and no partial custody movement or legacy authority.

The productive-close fixture advances to exact B+1 Service eligibility before side-effect-free simulation and canonical execution, then proves the committed transfer and complete Service/Deadline/active-state cleanup without legacy authority. The adapter-rollback fixture likewise simulates only at B+1 and proves canonical actor state, balances, and events remain unchanged.

Genesis admission/reserve checks and starvation budgets enumerate active semantic owners and derive resource envelopes from the canonical Contract rather than retained control cells. Malformed-scheduler fee-ingress coverage snapshots the encoded canonical active state rather than requiring a legacy primary cell.

Try-state derives identity, active cardinality, and wakeup ownership expectations from `ActorSemanticStates`. It validates wakeup pointers against generation-bound `DeadlineHandles` / `TriggerDeadlineHandles` plus `DeadlinePages`, and rejects any active semantic owner that retains legacy authority. Legacy locator/primary/Ready/Waiting corruption fixtures no longer masquerade as supported try-state evidence; canonical semantic, Contract, process, and Deadline owners supply those falsifiers.
- `ActorContractTailChunks`: gap-free authority-bound Steps 1..N plus aligned envelopes in chunks of at most four; generated descriptor maximum is 4,070 bytes per chunk
- `ActorRunState`: bounded mutable cursor, outcome, retry, and authority state for one open multi-block Cycle. TryRuntime binds the record to the admitted Contract generation and rejects incoherent cursor or phase geometry. The public integrated V1 regression follows one User Actor from a committed prefix through typed temporary failure, a non-adjacent generation-bound Block Deadline, B+1 recovery with fresh `Percent` resolution, a final useful effect, and Idle completion while asserting exclusive Service-or-Deadline residence and persistent legacy-authority absence. Its per-turn ledger names logical reads and writes, membership transitions, reserved and settled control/effect Weight, charged fees, and useful effects; the interior resident Step proves zero membership insertion, removal, and successor publication.
- `ActorIdentityCount`: O(1) total of active primary identities plus dormant registry identities, bounded by `MaxActorIdentities`
- `ActiveActorCount`: transactionally maintained O(1) active/paused cardinality used by activation and operational-cap checks; try-runtime reconciles it against active `ActorSemanticStates`, generation-bound `ActorProcesses`, concrete Service/Deadline/Park/Pending residence, certified C6 geometry, compact activation authority, funding, and optional run state
The declared `ActorReady*`, `ActorWaiting*`, `WakeupCursor*`, `NextWakeupClock`, `ActorControlLocators`, and `PrepassExecutionCutoff` keys preserve the fresh pre-fork metadata baseline only. Production code does not publish, consume, clean, or validate them, and no benchmark derives current Weight from them. `canonical_lifecycle_uses_no_legacy_scheduler_authority` exercises creation, pause/resume, readiness, retry, Contract replacement, and close while proving these keys remain absent. The immutable 25-row control oracle remains historical shape evidence only.

Benchmark account ownership is explicit. FRAME's default signer exemption remains only on actual signed-call fixtures, including ownership controls and Contract replacement. Automatic Trigger and Task work, Root calls and ingress diagnostics use ordinary accounts. `measured_account` rejects the exempt signer and collector aliasing; `assert_measured_actor_accounts` checks distinct owners and sovereign payers with one shared Fee Sink. The native `measured_account_roles_reject_signer_exemption_and_aliases` regression exercises rejection as well as valid geometry. These are fixture constraints, not restrictions on protocol accounts.

Account-key and cohort-population checks do not certify every resource dimension. Activation/deactivation fixtures still use System Actors and do not independently establish User hold work. Resource closure requires complete reachable branch owners, correctly bound component-wise selectors and runtime budget regressions; generated vectors and exact-Wasm replay bind artifact identity separately. Native accounting equality alone does not establish that ownership.

Benchmark surfaces distinguish outer Service transitions from reusable inner Task and Step atoms. `scheduler_complete_outer_weight_upper` and `deadline_return_weight_upper` select component-wise maxima for their admission bounds. The hot Service path instead composes inspection, host Step control/placement, a generic suffix and any lifecycle allowance. Native discovery admission and charging are explicit; existing complete outer profiles still do not establish generated containment for staged settlement bookkeeping, disjoint inner/suffix ownership, retained failures or terminal composition. Each production selector needs a source-traced binding.

Temporal owners measure `process_due_temporal_deadline` against bounded deep-index and page-neighbor geometry. Useful AtTime and Cadenced work, the three legal busy Cadenced residences, dependency review alternatives, and source-preserving refusal each retain distinct complete owners. Ordinary admission constructs every benchmark state; profiles do not fabricate raw process residence or Deadline authority.

Service placement uses `service_round_admit_eligible`; deepest-index Service-to-Deadline movement uses `service_member_to_deadline_new_key`. Live `insert_service_member` is reached through Trigger publication and Step/Service transitions (`lib.rs`, `scheduler.rs`) and must be covered inside each calling owner. `publish_service_member` is reached only by benchmark fixtures and tests; its `service_member_publish_empty/populated` Weight methods and the standalone `service_member_insert_populated` benchmark are not separately charged on the live path. Adding these standalone values to complete owners would double count insertion, while excluding them does not yet prove physical containment. The generic suffix is a conservative allowance, not proof of another placement mutation. Its apparent overlap cannot justify removal while residual bookkeeping and Service retirement lack complete source-traced containment.

| Path | Placement already inside the inner owner | Remaining qualification |
| --- | --- | --- |
| Retained Step | `advance_service_head` | Complete entry, rearm, collection and settlement coverage |
| Long retry | `transfer_service_member_to_deadline` includes ring removal | Complete deadline and outer bookkeeping composition |
| Close | Consumed-state finalization and Service retirement | `close_actor` measures a Deadline-resident retry, not Service-resident destruction |

Zero-Step admission and User Pipeline pricing select `scheduler_inner_zero_step_complete`, whose source fixture is System/Manual/Persistent. User collection and other Trigger-family diagnostics have no corresponding production `WeightInfo` methods. Hosts must establish full containment for their binding rather than infer it from that fixture, the generic suffix or a pure-cleanup allowance. Production has no paged Ready/Waiting append, consume, tombstone-drain, overdue-wakeup worker, or cursor-registration Weight method.

Pallet and runtime bindings are generated from the same benchmark inventory. Native benchmark tests run try-state over their constructed guards, while runtime budget tests prove every selected branch fits the configured two-dimensional control envelope. Generated vectors, production Wasm, and exact replay bind the final tree separately from these architectural owners.

Admission and guaranteed-capacity calculations use component-wise conservative maxima across complete outer profiles while inner owners remain separate. The project benchmark gate requires every production-selected owner and excludes the immutable historical control oracle. StopCycle remains control-only and emits no Action-fee receipt. Deactivation atomically detaches Service or Deadline, removes any Run without publishing an intermediate successor, records dormant control time, and completes Trigger, Contract, count, and hold cleanup. Opening profiles load canonical semantic, Contract, admission, and Step authority without legacy scheduler fixtures.

`prepare_reachable_opening`'s `_max` inner profiles cover System Actors with Manual Triggers, Persistent completion and bounded current-Step Balance predicates. They retain authored tail count but do not evaluate future Steps, maximize every Contract field or certify User/other-Trigger/effect-invoked containment.

| Opening profile | Current Step | Resulting authority |
| --- | --- | --- |
| `failed_max` | True predicates; missing-receipt Unstake rejects before invocation | Idle in Service; no Run |
| `retry_max` | True predicates; unfunded fixed Transfer | Suspended Run at a Block Deadline |
| `complete_max` | True predicates; StopCycle | Idle in Service; no Run |
| `progress_max` | False predicates skip AddLiquidity before amount resolution | Running in Service; cursor advances |

`assert_reachable_opening` checks retained class, Contract, generation, process, residence and consideration/attempt markers. The native `reachable_opening_tail_profile_boundaries` test rejects every target Action receipt, independent of fee or Weight values. These singleton Opening corners establish neither multi-member ring traversal nor lifecycle destruction; they qualify fixture scope, not complete production containment.

A separate extra User Opening profile retains the maximum signed funding allowlist while replacing control-only `StopCycle` with a positive one-Step `Transfer(Percent(50%))`. Real sovereign asset custody, real Manual Trigger and Pipeline/Action reserves precede measurement; measured inner control invokes the Task effect and settles Idle completion in Service. Direct postconditions bind recipient/source conservation, distinct Pipeline and Action receipts/collector deltas, nonzero actual effect Weight and retained Service authority; native corruption tests reject missing recipient or collector credit. The profile measures control plus effect, but those dimensions remain independent in admission/fee accounting.

A separate User/Manual one-Step positive Transfer diagnostic authors nonce-one terminal completion and measures the inner Step on a singleton Service node after ordinary admission. Its measured path both pays the distinct Action/Pipeline fees and performs the real Transfer before closing process identity, Contract and hold, unlinking Service, and releasing the owner hold; leftover sovereign custody is not unwound. Native corruption rejects a phantom Service count or unreleased hold, and isolated benchmark Wasm reaches the cell.

A second profile creates two real System/Manual zero-Step Service peers before the same measured User Transfer close. It checks the target's two distinct ring neighbors, exact surviving peer process semantics and relinked nodes, the cursor's successor advance and count decrement; native corruption rejects a missing peer or stale cursor. Both profiles are zero-predicate Mutable/Manual terminal witnesses, not complete Service-resident cleanup containment under deeper peer geometry, indexed detectors, maximum legal headers or other Tasks.

A parallel System/Manual one-Step positive Transfer diagnostic uses ordinary Root creation, the maximum signed funding allowlist and real non-native sovereign custody. Measured Opening transfers positive value and advances retained Idle Service without a User state hold or Pipeline/Action fee collection. The host still reports actual effect Weight and emits one zero-fee `ActionFeeCharged` invocation receipt for the Task; no Pipeline receipt or Fee Sink credit occurs. Native corruption rejects missing recipient custody or a fabricated Fee Sink credit, and isolated benchmark Wasm reaches the profile. This is one System class witness, not a System Task/Trigger union or a maximum legal Contract header.

A companion System/Manual Burn diagnostic uses ordinary Root creation, maximum signed funding width and non-native `Burn(Percent(50%))` with no predicate. Measured Opening invokes a positive Task effect, exactly debits the resolved sovereign custody, emits `BurnExecuted` and a zero-fee Action invocation receipt, and retains Idle Service without User state holds, Pipeline receipts or Fee Sink collection. Native corruption rejects a restored Task debit or fabricated Fee Sink credit; isolated benchmark Wasm reaches this one System Burn cell. It does not establish maximum legal Contract width, other Burn headers or the System Task/Trigger union.

Companion extras retain full current-Step BalanceBelow, ObservationAbove, or mixed BalanceBelow/ObservationAbove/BlockNumberAbove predicate capacity while invoking that positive Transfer under the same maximum funding policy. Observation feeds are distinct, maximum-encoded and canonically available with positive values; they are prepared before measurement. Both Observation-bearing variants top up the admitted sovereign after real Manual Trigger collection to cover certified Pipeline and current-Action capacities. Canonical admission may reorder predicates, so the mixed fixture checks the source multiset rather than authored position; it preserves each represented source and requires a true block-number predicate.

Native corruption checks zero, Balance, Observation and mixed widths against actual effect, Action/Pipeline fees and source/recipient accounting; isolated benchmark Wasm reaches all four. A separate positive User/Manual Burn extra retains the maximum signed funding allowlist with a non-native `Burn(Percent(50%))`, no predicate and post-Trigger certified fee capacity. It checks the exact resolved source debit and `BurnExecuted` event alongside independent Pipeline/Action receipts, collector credits and retained Idle Service. Native corruption rejects a restored debit or altered collector credit, and isolated benchmark Wasm reaches the extra.

A positive User/Manual SplitTransfer extra uses the same maximum signed funding policy with the host's maximum legal leg count, distinct non-native recipients, nonzero per-leg shares and certified post-Trigger fee capacity. The measured Opening includes per-leg preflight/transfers and Action/Pipeline collection. Direct postconditions check every recipient, the source's exact distributed debit, declared retained remainder, `SplitTransferExecuted` and separate fee receipts/collector credits under retained Idle Service. Native corruption rejects a missing recipient credit or collector credit; isolated benchmark Wasm reaches the fanout.

A separate User/Manual late-leg SplitTransfer failure extra uses ordinary creation and signed Contract replacement to admit a maximum funding allowlist and two legally failing legs. The benchmark host funds an existing first recipient and leaves a second below its asset's deposit minimum. Measured Opening preflights both, retains one temporary B+1 Service retry and settles one Pipeline plus the attempted Action fee; no leg receives Task custody or `SplitTransferExecuted`. When the spend asset is fee-native, source balance drops only by those independently checked fees. Native corruption detects phantom first-leg or collector credit; after ordinary recipient repair, a native B+1 retry completes both legs and pays only its new Action fee. This two-leg failure diagnostic does not claim maximum-leg failure containment or a production Weight bound.

A separate two-Step User/Manual Transfer diagnostic measures only the positive Opening Step under maximum signed funding width. It preserves a distinct authored StopCycle in the body tail and proves Running cursor 1, one committed effect, exact paid Pipeline/Action receipts, B+1 eligibility and retained Service. Its native continuation consumes the actual Step 1 in the next Service round: StopCycle ends the Run, emits `CycleStopped`, leaves custody and collector balances unchanged and charges no second Pipeline or Action fee. StopCycle may report host effect Weight without an Action receipt; zero effect Weight is not assumed. The second Step is outside the measured Opening.

A second two-Step User/Manual diagnostic commits a positive Transfer to Running, then measures the B+1 carried Burn on a distinct funded non-native asset. The setup proves Step-0 effect and fees before measurement; the measured Step-1 atom debits Burn custody, charges its own Action effect and completes the Run without collecting Pipeline again. Postconditions bind the exact Burn event/debit, independent Action receipt/collector credit, retained Idle Service and the sole existing Pipeline receipt. Native corruption rejects restored Burn custody or changed collector credit.

These scoped diagnostics cover three Tasks and a three-family Transfer predicate mixture plus positive-to-control and positive-to-positive two-Step continuations. They do not cover the full Task/class/Trigger union, a maximum *legal* header, other predicate sources, deeper heterogeneous multi-Step progress or production selector containment.

`assert_retained_service_turn` also checks Running, successful Suspended-tail and Manual zero-Step postconditions. `retained_inner_profiles_require_real_service_advancement` supplies native three-member witnesses: maximum tail-fragment width with zero/maximum current predicates, plus System/User zero Steps. It checks the captured distinct successor and unchanged peer node/process/semantic state, then corrupts each cursor/consideration/attempt marker to require the specific assertion's rejection. Round identity is a setup invariant, not evidence of measured work. Benchmark bodies and singleton measurement geometry remain unchanged; the native witnesses are not generated multi-member containment.

`prepare_zero_step_header` supplies Mutable User/System Manual, AddressEvent, AtTime, and Cadenced completion/close extras with family-relative maximum legal zero-Step headers and distinct Service neighbors; each case has three members. `assert_maximum_zero_step_header` excludes Parked Balance; Manual/AtTime/Cadenced lack AddressEvent's filter payload, and both temporal families forbid a window and nonzero cooldown. Maximum funding and nonce targets remain legal. Signed custody ingress latches AddressEvent only; Manual requires an authorized call with class-correct `Pays` and occurrence Weight. The measured inner collects any User Pipeline payment, completes and advances or closes; source occurrence/loading stay outside it. System setup uses ordinary Root creation with positive native custody, not a fee prerequisite.

The additional maximum-header Immutable User Manual extra admits a zero-Step Contract with an authored nonce-one terminal, then publishes a paid owner Manual occurrence. Ordinary signed owner-close attempts are rejected before and after the latch without deleting process or Service authority. Measured Opening alone closes the Actor, releases the exact User hold/slot, advances its distinct Service peers and leaves sovereign custody unchanged. Native tests check paid-latch preservation across rejected close and authored completion; the mutable counterpart was re-smoked. This is one mutability/Trigger terminal witness, not a complete Immutable class/Trigger/Step containment result.

AtTime fixtures distinguish actual due-source consumption from Manual readiness preserved through signed Contract replacement and generation rotation. `assert_at_time_opening_input` checks the source after loading, immediately before measurement: consumed sources are already absent; preserved latches still own an exact future singleton Tick member. Only the latter Opening removes that member/key/page/index position and releases the User detector hold. Native boundary and result-corruption tests require specific rejection of misplaced source work, retained handles/keys and stale hold records or held currency.

The maximum-header Immutable User AtTime due-close variant creates one authored future Tick source. An owner close attempt before due cannot remove that source; real due processing consumes it and charges one useful readiness occurrence outside measurement. A second owner-close refusal preserves the paid latch and Service residence. Measured nonce-one Opening alone closes the Actor, returns the exact User hold/slot and preserves sovereign custody; native tests check both refusal boundaries. This scoped temporal/mutability witness does not establish deeper Deadline-page/index or all Immutable Trigger geometries.

Cadenced due readiness is consumed before the measured boundary; Opening re-arms one successor Tick member, or atomically reclaims it on nonce close. Postconditions bind its next due key, handle, page, index position and User detector hold, plus peer/custody/lifecycle invariants. The fixture compares Tick index length against the observed post-source baseline, since the host genesis may contain unrelated Tick keys; native corruption checks reject an early rearm, missing successor, stale hold and surviving close handle. These singleton-source witnesses are not complete Deadline-page/index or mutability containment.

The maximum-header Immutable User Cadenced due-close variant verifies that owner-close refusal before due retains its future Tick source, and a second refusal after real due materialization preserves the paid latch and Service work. Measured nonce-one Opening performs terminal cadence cleanup, releasing hold/slot and leaving neither process nor cadence successor while custody remains untouched. A native test checks the two refusal boundaries and absence of a rearmed deadline. This is one temporal mutability cross-product, not a complete Deadline-page/index or Immutable Trigger union.

Postconditions bind class-correct payer/collector deltas, native/non-native custody, exact peers, cursor/count, holds and absent terminal authority. Actor counters and System sovereign occupancy/vacancy are checked without relying on try-runtime; native corruption tests require those specific assertions. User close frees its owner slot and hold; System retains its hold exemption. Native tests reject all target Action receipts and System Pipeline/nonzero Trigger fees. These diagnostics are excluded from production generation and do not rebind admission or Pipeline pricing.

`process_due_temporal_deadline` owns source validation/removal, semantic update, current service-authority loading and occurrence processing in one transaction. The Tick dispatcher admits its branch before invoking that seam; classification remains separate. Both family profiles measure this transaction, not a preloaded helper. Their fixture preserves host-genesis sources and saturates the admitted Tick index through ordinary creation and exact vacant-sovereign reuse. Checks cover source/key cleanup, heap reciprocity, exact B+1 Pending Service, unchanged nonce, no Run, family-specific consumption and Trigger-only debit.

`maximum_temporal_control_steps` preserves the configured Step count and protected Step-0 Task/predicate geometry through ordinary admission; unrelated tails may shrink only through `retain_admitted_contract_geometry`. Funding uses the largest encoded allowlist, with a future nonce terminal and no Parked Balance plan. The automatic worker's hold payer is not a whitelisted signer: owner-account reconciliation belongs to the measured work, unlike signed-call overhead.

The production `at_time_trigger_occurrence` owner removes a sole source-page member with distinct physical and vacancy neighbors. Production `cadenced_trigger_occurrence` additionally fills a 31-member destination vacancy head, repairs its successor, then removes the transient rearm while retaining that key. Both admit the maximum watch count and maximum-width asset identifiers alongside maximum Step-0 Task/predicates and funding policy. They preserve the complete Contract head, every peer generation-bound handle/slot, charge one useful occurrence, and publish Pending without opening a Run or Park episode. `production_temporal_occurrences_cover_maximum_headers_and_neighbors` executes both generated owners with try-state.

`cadenced_parked_balance_occurrence` completes a maximum-step User Contract through real `StopCycle`, enters maximum-watch Parked Balance residence, processes one negative Block review, then measures the next useful Cadenced Tick occurrence. Cadence and Park review retain independent generation-bound deadline handles until the cadence wake atomically removes both the review carrier and dependency authority, publishes Pending Service, charges once, and leaves no Run or deferred cadence handle. Opening later re-arms cadence from current state. `cadenced_occurrence_wakes_parked_balance_and_removes_review_deadline` is the ordinary non-benchmark regression with try-state. The witness exposed and corrected common Park wake cleanup that previously removed review metadata without removing its physical deadline member.

Parked Balance and a retained Run are mutually exclusive: Park publication occurs only after `AttemptDisposition::Completed`, whose successor has cleared Run state. AtTime consumes its sole Tick source before that completing cycle and therefore cannot own a later temporal occurrence while Parked. Cadenced alone can retain independent future Tick authority across Park residence; fixtures must not synthesize the excluded products.

`validate_parked_balance_activation` admits only nonempty, Persistent Contracts without nonce-terminal completion when a Parked Balance plan is present. `temporal_balance_headers_reject_terminal_and_empty_contracts` falsifies all three exclusions for AtTime and Cadenced, with storage-root-preserving admission rejection. The balance-header fixtures therefore disable `auto_close_at_cycle_nonce`; combining both optional fields would manufacture an inadmissible measurement state.

`cadenced_underfunded_rearm_populated` covers the Idle non-useful branch with the maximum admitted Contract and Parked Balance header over populated source/destination vacancy geometry and a deep Tick index. It burns User custody to exactly `MinUserBalance` before service. The due source advances to one retained future detector, fills the destination vacancy, and preserves identity, process, Service, hold, Contract, Run absence, payer custody and collector balance without latch, fee or event. Wasm smoke proves reachability; the extra remains uncalibrated. `underfunded_cadenced_occurrence_advances_without_fee_readiness_or_apoptosis` additionally enables collector rejection and proves capacity refusal never invokes it. AtTime cannot share this branch: its consumed one-shot opportunity selects prepaid custody-neutral apoptosis when Trigger admission is insufficient.

`cadenced_terminal_cleanup_populated` starts from the same maximum legal User Contract/header and populated deep-index geometry, then makes canonical identity terminal before due service. The worker removes source and transient rearm authority, detaches the exact process publication, closes identity/Contract/hold state, preserves sovereign custody and collector balance, and leaves every peer handle intact. Wasm smoke proves reachability. This exposed retained `ActorProcesses` after temporal close; terminal classification and underfunded AtTime apoptosis now use `remove_actor_publication_and_finalize`. Focused AtTime and try-state assertions reject any surviving Service or deadline authority.

`busy_cadenced_occurrence_rearms_without_fees_or_future_cycle` covers ordinary User/System Running, resident Suspended, and deadline-owned Suspended states, with funded and depleted custody. It preserves complete Run bytes, identity, Service order/eligibility and retry authority while advancing to one future aligned Tick deadline. An enabled collector-failure injection is never reached: busy coalescing charges no Trigger/Pipeline fee, executes no Task and creates no deferred Cycle. Specification §2.3 owns this behavior and supersedes older deferred-latch prose.

`cadenced_running_rearm`, `cadenced_suspended_service_rearm` and `cadenced_suspended_deadline_rearm` are separate generated production owners over a deep Tick index. Each combines the maximum admitted Contract/Parked Balance header with a real maximum module-error Run. The Service-suspended and deadline-suspended profiles use ordinary `RetryLater`; Running uses `ContinueNextStep`, retains a live suffix, and subtracts only the impossible suspension payload from the type-derived Run maximum. The Block deadline remains future under Block-first service. Unsupported hosts reject the real below-minimum-recipient asset fixture rather than fabricate a module error.

The Tick classifier reads generation-bound semantic authority and returns `TemporalTriggerBusy` only for Running or Suspended Cadenced work. Dispatch admits the component-wise maximum of the three busy owners before mutation; useful Idle work retains its separate Trigger-fee owner.

`busy_temporal_preserved_state` snapshots complete Run bytes, identity, process, Service node/header, block-retry handle, hold, native payer/collector balances and events. Rearm must preserve that snapshot and change only the cadence source. `production_busy_temporal_rearms_preserve_phase_maximum_runs` executes all three owners with try-state; benchmark-Wasm smoke confirms each owner and the semantic classifier. Park residence plus Run, Parked Balance plus nonce/close completion, AtTime Park plus a future occurrence, and Running Block-deadline residence are excluded by completion, admission, one-shot and retry semantics respectively. Deadline-neighbor coverage remains separate.

`temporal_deadline_dispatch_preserves_source_on_resource_refusal` checks one-unit-short RefTime and ProofSize at both useful selector and branch admission. `busy_cadenced_rearm_refuses_each_resource_dimension_before_mutation` repeats those independent corners for all three promoted busy residences and requires exact storage-root preservation. `temporal_deadline_transaction_restores_source_after_loading_refusal` checks rollback after source removal when Contract loading fails, then successful retry. These tests prove declared admission and transaction boundaries, not sufficiency of retained coefficients.

`temporal_deadline_cadenced_deep_index` is a full-dispatch diagnostic outside production generation. Its counterexample falsifies any loaded-owner-only envelope and is not additive to the selected complete occurrence owner. Singleton and populated smoke execution establish reachability, not production coefficients.

`charge_trigger_occurrence` separates insufficient capacity (`InsufficientFee`) from a collector rejection after admission (`TriggerFeeCollectionFailed`); the retained prechecked helper uses the same collection-failure classification. Automatic skip/fallback branches recognize only genuine underfunding. Manual returns the typed error; certified AddressEvent ingress propagates it to the producer's atomicity owner, as `address_event_collection_failure_rolls_back_certified_movement_and_retries_once` proves with real native movement.

`temporal_collection_failure_preserves_exact_source_and_retries_once` covers both AtTime and Cadenced: collector rejection restores source extraction, any cadence rearm, holds and readiness, and recovery consumes the same source once. `cadenced_occurrence_wakes_parked_balance_and_removes_review_deadline` adds the maximal distinct composition: a User cadence reaches Park, rearms a negative review, then late collector rejection restores the complete Tick source, Block review, dependency plan/episode, custody and balances before one successful retry atomically wakes Pending Service. Underfunding keeps its distinct family-specific consumption rules.

Fresh-genesis closure deletes the pre-cutover worker rather than constructing mixed authority. No Trigger family owns deferred materialization; no zero-Weight compatibility slot or overdue-wakeup profile remains.

The retired `drain_overdue_wakeups_cursor*`, branch/unit Weight composition, and three generated runtime method bodies are physically deleted; their methods remain absent from `WeightInfo`, fallback bindings, parity coverage, and active runtime tests. Both disabled replay blocks are deleted, including the obsolete full-executive schedule-ledger test; its dead human invocation contract is also removed. The family-0 fault benchmark, storage, clear call, event/error/type/API field, and record/clear Weight methods are removed; no materialization-fault projection remains.

Runtime integration and active production replay no longer call the drain or reference its Weight/fault owners. The obsolete tick-drain campaign and W5 ledger helper are deleted; mixed-clock and temporal occurrence coverage remain on canonical Service/Deadline.

Temporal readiness, retry recovery, mixed-close setup, and post-consumption checks use canonical Service/Deadline frontiers. Canonical Deadline publication and recovery retain independent owners; pre-cutover waiting declarations have no worker or benchmark authority.

Final generated Weight, vectors, metadata, ABI, production Wasm, and exact replay remain coupled to the same source-converged tree.

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

The implementation separates identity-only dormancy from one generation-bound Active epoch:

```text
Dormant ⇄ Active { Disabled | Service(Pending or Live) | Deadline | Parked }
Idle -- useful readiness/Opening --> Running ⇄ Suspended -- completion/abort --> Idle or Closed
```

Lifecycle calls preserve the split-store boundary:

- `activate_actor` accepts one typed `ActorContract`, validates trigger/schedule, Steps, funding policy, auto-close target, resource bounds, class restrictions, active capacity, and idle envelope, then publishes matching semantic, Contract, process, residence, detector, and User-hold authority transactionally.
- `deactivate_actor` detaches Service/Deadline/Parked and Trigger authority, removes process, Contract, detector, hold, and optional Run state, then publishes the preserved identity as Dormant without moving sovereign custody.
- `update_contract` rotates the checked generation after removing the old residence and detector authority; the replacement publishes one complete new Active epoch or rolls back.
- `finalize_actor_loaded_inner` and `close_inactive_actor` synchronously reclaim every bounded owner, release the User slot/hold, preserve balances, and emit one `ActorClosed`; no Retired process or cleanup debt is written.

Creation expresses dormancy by Contract absence. Lowest-free and exact-slot User creation accept `Option<ActorContract>`; System creation allocates a matching sovereign locator and may install Active or Dormant state. Public Dormant creation requires Mutable, while host genesis may declare a sealed Immutable Dormant System identity.

Mutable Actors may pause, resume, replace Contract, deactivate, cancel, or close through their authorized control surface. Immutable User Actors reject owner destruction/mutation but still execute authored triggers and mandatory terminal semantics. Immutable System Actors reject Manual admission and external lifecycle mutation. User Contracts cannot contain `Mint`.

A Cycle derives `run.cycle_nonce = identity.cycle_nonce + 1`; termination commits that nonce once. Running/Suspended state retains one coherent Run, while Idle retains none. Nonce exhaustion, window expiry, retry/global failure bounds, productive completion, auto-close, Pipeline apoptosis, and sweep use their typed terminal owner without granting arbitrary close authority.

Manual and AddressEvent retain block cooldown/window gates. AtTime/Cadenced use independent Tick Trigger deadlines and admit neither block cooldown nor windows. A busy Cadenced Actor keeps its current Service or retry Deadline while the Trigger worker installs one future aligned Tick; it creates no deferred Cycle or fee. New Service publication in block `B` remains ineligible until `B + 1`.

Package and runtime regressions cover activation/deactivation, pause/resume, Contract replacement, cooldown/window ordering, busy cadence, exact-slot custody reuse, Park/Pending cleanup, synchronous close, and actor-to-actor B+1 ingress.

## Actors Read-Model Contract

This subsystem follows the project-wide [`read-model.contract.en.md`](../../../../docs/read-model.contract.en.md) split.

### Canonical on-chain Actor projections

The current pallet provides chain-native bounded reads for known Actor and scheduler truth through:

- `actor_semantic_states(actor_id)`, `actor_hot(actor_id)`, `actor_contract(actor_id)`, and `active_actor_state(actor_id)` for the current lifecycle, Contract, generation, admission, and optional Run projection
- `actor_processes(actor_id)`, `service_header()`, `service_nodes(actor_id)`, and exact Deadline handles/pages for current process residence and bounded physical authority
- `actor_state_hold(actor_id)` for the retained User hold authority; System Actors remain hold-exempt
- `owner_slot_bitmap(owner)` plus deterministic `sovereign_account_id(owner, owner_slot)` and `sovereign_index(sovereign)` for bounded per-owner recovery
- Deterministic `sovereign_account_id_system(sovereign_id)` for System Actor addressing against the known runtime catalog
- `ActorEligibilityApi` and `ActorCostApi` for bounded semantic eligibility and separated current cost projections
- `ActiveActorLimit`, `GlobalCircuitBreaker`, `IdleStarvationState`, bounded current faults, operational events, and live execution side effects

`SovereignAccountDeriver` maps User custody from `SCALE(ActorsPalletId, b"user", owner, owner_slot)` and System custody from `SCALE(ActorsPalletId, b"system", sovereign_id)`. The host owns the concrete infallible `AccountId` mapping; explicit tags separate the two deterministic account domains before hashing, and the DEOS runtime preserves its established 32-byte identities.

These are authoritative bounded surfaces for known-Actor inspection, per-owner recovery, scheduler state, and current operator observability. Funding policy is part of the Contract; Actors exposes no funding accumulator or historical funding projection.

Full Active classification validates semantic generation, process status/residence, Service or Deadline authority, Contract/admission identity, optional Run, current Step, and independent Trigger deadline without scanning the Actor population. TryRuntime adds Contract-tail, detector, hold and reverse-ownership checks. Its carrier audits may scan retained topology; they are read-only and compiled only with `try-runtime`, not part of scheduler admission or execution.

| Audit owner | Carrier invariants |
| --- | --- |
| `do_try_state_service_ring` | Exact Active/process/node generations and kinds; count/cursor agreement; one complete cyclic ring with reciprocal links and no disconnected members |
| `do_try_state_deadline_carrier` | Exact map-key/semantic owners for both handle families; distinct occupied slots; C32 page/header/count and vacancy-set agreement; packed clock heaps with exact reverse positions and no orphan pages |

The `try_state_carrier_*` regressions corrupt otherwise valid admitted state and prove rejection without root mutation. They cover ownerless records, stale references, split rings, slot aliasing, detached owned pages, vacancy cycles and index drift; specific errors distinguish detached-page and heap-packing checks from earlier failures. Lawful witnesses retain empty round markers, noncontiguous page ids, process Deadline handles without hot pointers, and independent temporal handles beside Disabled, Service, retry or Parked residence. These audits do not establish production Weight containment.

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
| `6` | `manual_trigger` | latch readiness and publish Service/Parked destination |
| `7` | removed | retired separate funding mutation; `update_contract` owns authored replacement |
| `8` | `close_actor` | prechecked pure destruction in place |
| `9` | `update_contract` | mutable actors; atomically replace the complete authored Actor Contract |
| `10` | `set_global_circuit_breaker` | breaker control |
| `11` | `permissionless_sweep` | liveness touchpoint, no normal cycle |
| `12` | removed | retired separate steps/completion mutation; `update_contract` owns authored replacement |
| `13` | `set_active_actor_limit` | governance operational cap tuning |
| `14` | `permissionless_sweep_many` | bounded batch touchpoint, no process publication |
| `15..=16` | reserved | retired field-specific auto-close mutations; `update_contract` owns authored replacement |
| `17` | removed | retired close-plan mutation |
| `18..=20` | reserved | retired transitional dormant creation calls; canonical User/System creation expresses dormancy as absent Contract |
| `21` | `activate_actor` | typed Active Actor Contract with schedule, `ContractSteps`, funding policy, and admission validation |
| `22` | `deactivate_actor` | remove contract/scheduler state while preserving identity and balances |

Calls `4`, `5`, `6`, `8`, `9`, `21`, and `22` use the class-specific control authority: signed owner for User actors, signed owner or governance for System actors. Active-only calls reject dormant identities; `close_actor` handles either lifecycle.

---

## Validation Coverage

Package validation lives in the `src/tests.rs` fixture/module root, domain suites under `src/tests/`, `src/benchmarking.rs`, the independent `embedding-runtime`, and compile-time exhaustive semantic contracts. Tests pin SCALE indices, storage names and types, Actor-state decomposition, scheduler/Trigger/lifecycle invariants, Task atomicity, retry transitions, custody conservation, and try-state reconciliation.

Replayable state-machine traces cover suspension, continuation, cancellation, exclusive process residence, owner slots, and current balances. The seeded model drives create/activate/deactivate/signal/trigger/pause/resume/Contract-update/Service/Deadline/execute/close/slot-round-trip/suspend/continue/cancel sequences against conservation, cross-store invariants, and try-state after every operation. Temporary DEX failure and recovery exercise randomized fault suspension and repair. Each control operation admits only its typed lifecycle event family, including ordered cancellation and summary events when replacing, deactivating, or closing a suspended cycle.

Mandatory falsifiers cover Service/Deadline saturation, protected User fee-native floor, collector failure after admission, invalid `Fresh`, and nonce exhaustion for both classes, all ending in try-state.

FRAME benchmarks isolate bounded package branches; every production host must prove fixture reachability and envelope coverage before generating and binding runtime-specific weights. A Fixed-Transfer Suspended-head assurance matrix uses authored maximum-length Contracts, real sovereign custody, Opening, due return, and a second retry across host-feasible current-predicate counts. It checks current-state reevaluation, custody, and canonical reconciliation; it does not establish a Weight envelope for other head Tasks or all host configurations.

External-consumer profiles prove that the crate composes without DEOS types. Concrete runtime adapters, generated artifacts, stress SLOs, and operational gates belong to the integration architecture.

## Integration Handoff

A production host must bind generated `WeightInfo`, concrete adapters and origins, defensible Service/Deadline/Actor bounds, fee conversion and collection, ingress producers, genesis Actors, and independent runtime evidence. The package embedding guide owns that checklist; [`docs/actors.integration.en.md`](../../../../docs/actors.integration.en.md) records the DEOS realization.

Implementation mirror for [Specification](./specification.en.md).
