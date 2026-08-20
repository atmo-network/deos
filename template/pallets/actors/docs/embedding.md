# DEOS Actors Runtime Embedding Guide

> Package-owned contract for reusing `pallet-deos-actors` outside the current DEOS/TMCTOL reference runtime.

**Status**

- **Component**: `pallet-deos-actors` (Rust crate `pallet_deos_actors`)
- **Release line**: `0.7.20`
- **Audience**: external runtime implementers embedding Actors without inheriting DEOS/TMCTOL topology
- **Companions**: [`README.md`](../README.md), [DEOS Actors Specification](./specification.en.md), [DEOS Actors Architecture](./architecture.en.md)
- **Source navigation**: `src/types.rs` is the stable public facade; the architecture map routes Contract, Lifecycle, Scheduler, and Observation type owners; `src/contract.rs` owns semantic classification rather than public type definitions
- **Non-goals**: DEOS governance policy, TMCTOL bucket topology, System Actor catalog standardization, UI product flows

`pallet-deos-actors` is a bounded deterministic actor kernel. A downstream runtime can embed it without inheriting DEOS governance policy, TMCTOL bucket topology, native staking design, or the current System Actor catalog.

Use this guide with the normative [DEOS Actors Specification](./specification.en.md) and shipped [DEOS Actors Architecture](./architecture.en.md). The specification defines portable semantics; this package document defines the host contract; the architecture document maps the reusable package implementation and source ownership.

## Executable Portability Evidence

The repository's [`embedding-runtime`](https://github.com/atmo-network/deos/tree/main/template/pallets/actors/embedding-runtime) package is a minimal external-consumer runtime fixture owned by this pallet boundary. It uses local account and asset types, local weight bindings, zero genesis System Actors, smaller queue/wakeup pages, native balance ingress through its own transaction extension, and no DEOS/TMCTOL primitives or topology.

The default profile proves deterministic unsupported optional adapters and all three error policies, including that Permanent adapter failures never create Continuation. The opt-in `dex-fixture` profile adds User `SwapOut` with an absolute cap and an explicitly Temporary `SwapIn` fixture for Mutable User/System Continuation. This package is executable portability evidence, not a second product or a normative actor topology.

| Portable capability | Independent evidence |
| --- | --- |
| Actor topology | Zero genesis System actors; local `u64` accounts and `u32` assets |
| Error policy | Abort, continue, and Temporary-only retry paths |
| Continuation | Mutable User/System suspend, cooldown, suffix resume, cancel, and pure close |
| Concurrent ingress | Executive transfer during suspension latches once without duplicate ticket or wakeup |
| Permissions | Immutable rejects `RetryLater { max_attempts }`; User rejects `Mint`; User accepts `SwapOut` |
| Optional capability | Default DEX/staking/donation fail Permanent; DEX fixture supplies only local pairs |
| State and weights | Continuation metadata, fresh storage version, try-state, and nonzero production-derived classes |

Both profiles use the pallet's canonical FIFO and wakeup stores. The fixture defines no DEOS primitive, TMCTOL helper, System Actor catalog, custom scheduler, or product topology.

## 1. Minimal Host Runtime Obligations

An embedding runtime must provide only the bounded host surface that Actors cannot own itself:

- `AssetOps`: Transfer, burn, mint, balance, minimum-balance, and exact `preflight_transfer` consequences over local `AccountId`, `AssetId`, and `Balance` types. Preflight covers both source withdrawal and recipient depositability, preserves explicit retry classification, and must agree with the following transfer under unchanged state. Generic Actors does not promise that a provider-backed or reserved-only zero-free native recipient can accept sub-minimum ingress.
- `ObservationProvider`: Current typed scalar state classified as Unavailable, Uninitialized, Fresh, or Stale over the embedding's `ObservationFeedId`; the same type identifies the scalar `ObservationChange { feed }` trigger. The one-feed subscription index, transactional latest-revision dirty ingress, and independently metered deferred fanout ship with the package. Fanout sets the existing pending latch and invokes the existing scheduler only; the boundary has no concrete Oracle, producer, history, or off-chain dependency.
- `ObservationChangeIngress`: Typed certified-publisher boundary accepting one externally owned monotonic feed revision. The host publisher calls this trait rather than the pallet's inherent implementation directly.
- `DexOps`: Caller-aware exact-in quotes and capacity-bounded exact-out swaps with deterministic fees, slippage, rounding, and failure behavior. Swap methods receive only `ExecutionContext { actor, actor_type }`; the package derives immutable `ActorType` from stored actor state so an embedding never reconstructs authority from account catalogs or sovereign-address heuristics.
- `LiquidityOps`: Addition, removal, and pair-scoped donation; donation permits reserve strengthening without LP receipt minting when supported by runtime policy.
- `StakingOps`: Generic staking operations plus adapter-visible share balance and optional transferable share-asset mapping for Unstake amount resolution.
- `FundingAuthority`: Default-deny authorization for explicit actor/source pairs when an actor selects `RuntimePolicy`; pallet-owned policies do not delegate.
- `Time` and `CadenceTickMillis`: Deterministic consensus milliseconds and the host's nonzero tick quantum. The pallet floors readiness, ceils activation anchors, and never derives cadence from local block count.
- `WeightInfo`: The single runtime-derived numeric authority covers worst-case Task classes, calls, Predicates, fees, ingress, scheduler work, orchestration, Continuation, finalization, and cleanup in both Weight dimensions. Transfer, Mint, and split fanout include possible synchronous address-event ingress; Burn remains independently priced without transfer-ingress proof; adapter-free `StopCycle` prices its explicit stop event.
- `Weight generation`: The `SubstrateWeight` and `()` implementations shipped in `src/weights.rs` are hand-written estimates, not benchmark output, and both underprice execution. The DEOS reference runtime measures `create_user_actor` at roughly thirteen times the packaged RefTime with seven times its ProofSize. Run `frame-benchmarking` against your own runtime and bind the generated file.
- `contract`: Exhaustive read-only classification of each Task, Predicate, amount resolution, and error policy. Its task weight owner delegates to `WeightInfo`; it does not replace runtime measurement, capability configuration, or canonical task parameters.
- `WeightToFee`: Deterministic conversion from task weight upper bound to fee-native execution charge.
- `FeeCollector` + `FeeSink`: One atomic runtime boundary that transfers every User fee in full into the mandatory deposit-capable collection destination.
- `AddressEventIngress`: Typed certified-ingress boundary (`AddressEventIngress::preflight`/`notify`) over the package `AddressEvent` value. Preflight is read-only (lifecycle, funding, trigger, and required placement); notify executes exactly once after movement and rejects through `IngressFailure { error, retry }` with the same closed Temporary/Permanent classification as `TaskFailure`.
- Governance/system origins, a two-dimensional hook weight meter, runtime-reserved `ActorOnIdleReserve`, owner-slot/queue/wakeup/active/total-identity/sweep bounds, `MaxOpeningSnapshotEntries`, `MaxIdleStarvationBlocks`, and fee constants.
- Canonical FIFO configuration: non-zero page size, bounded `MaxQueueLength`, `MaxQueueEntriesScannedPerBlock`, and `MaxExecutionsPerBlock`. One `NextQueueTicket`, one cutoff, one actor-local ticket, one scheduler, one wakeup substrate, and one Continuation owner govern every actor. Actor class, actor id, execution share, and priority policy never change service order.
- Under `runtime-benchmarks`, `setup_predicate_assets` must provide enough valid distinct assets to measure the maximum bounded-DNF predicate count honestly; repeated keys do not establish worst-case ProofSize.

The host runtime owns those bindings. Actors core owns scheduling, admission, task orchestration, lifecycle, bounded state, fee reservation, amount resolution, task-scoped transactions, and observability events. Dormant identities retain address/ownership lineage under the total-identity bound but own no contract or scheduler state; runtime-specific custody-only addresses remain outside generic actor storage.

Deterministic User and System custody derivation survives host account-provider removal. Reattachment recovers the same address but does not recreate balances removed by external reaping, dust handling, or other host policy.

## 2. Optional Fail-Closed Adapters

A runtime may bind unit implementations for capabilities it does not expose:

- `AssetOps = ()` rejects Transfer, Burn, and Mint mutation and reports no deposit viability; it never fabricates successful ledger effects.
- `ObservationProvider = ()` returns Unavailable for every feed and makes every observation Predicate false without creating a Task failure.
- No DEX means swap tasks fail deterministically through `StepErrorPolicy`.
- No liquidity support means `AddLiquidity`, `RemoveLiquidity`, and `DonateLiquidity` fail through `LiquidityOps`.
- No staking means `Stake` and `Unstake` fail through the staking adapter.
- No runtime-authorized funding pairs means `FundingAuthority = ()`, which safely denies every `RuntimePolicy` provenance.

Unsupported adapters are valid only when user-facing plan builders and runtime docs make the unsupported task surface clear. They must fail deterministically without panic, mutation, loops, or off-chain nondeterminism. Unsupported capability is permanent and must never become retryable Continuation state.

## 3. Ownership Boundary

- `Actors owns`: Actor ids, sovereign account derivation, owner slots, queue/wakeup scheduling, lifecycle transitions, fee reservation, amount resolution, task admission, task-scoped atomicity, step error policy, and bounded events.
- `Runtime owns`: Asset ledgers, caller-aware DEX pricing, liquidity pool and donation policy, staking topology/share mapping, atomic fee-collection policy and sink depositability, ingress producers, governance origins, genesis System Actor definitions, and task weight calibration.
- `UI owns`: Plan authoring affordances, dry-run/simulation UX, unsupported-task hiding, user recovery flows, per-cycle timeline rendering, and warnings around `ContinueNextStep` after mutating tasks.
- `Docs own`: The separation between portable task-language patterns and a concrete runtime's System Actor topology.

Business policy belongs in runtime adapters or genesis actor configuration, not in `pallet_deos_actors` core.

## 4. Actor Permission Model

User actors are signed-owner controlled, fee-bearing, slot-bounded, and cannot mint. System actors are governance-created, slotless, fee-exempt, and may be Mutable or Immutable. No runtime extrinsic may mutate, pause, trigger, or close a System Immutable actor, but that control guard must not block mandatory internal terminal transitions such as consecutive-failure closure. A later attachment to its vacant custody locator creates a fresh identity rather than reopening the former actor.

`ensure_control_origin` accepts either the signed `owner` or `SystemOrigin` for a System actor, so the `owner` account passed at creation holds the same pause, resume, trigger, contract-update, and close authority as governance over that actor. A host that supplies a signable `owner` therefore delegates governance-equivalent control to that account. Pass an account with no known private key when System control must stay governance-only; the reference runtime uses the `ActorsPalletId` account for exactly this reason.

A downstream runtime decides whether to ship any genesis System actors. Genesis System Actor topology is runtime-owned configuration, not a pallet requirement.

## 5. Portable Patterns vs Reference Topology

Reusable Actor Contract patterns include:

- Fee collection and redistribution.
- Scheduled burn or treasury transfer.
- Balance-ingress triggered routing.
- Liquidity provisioning or donation through runtime adapters.
- Ordinary final Actor Contracts whose Steps move actor-owned balances before a later pure close.

The DEOS/TMCTOL catalog of burn actors, fee sink, liquidity actors, buckets, treasuries, BLDR lanes, and native staking LP provisioning is one reference topology. External runtimes should copy only the task-language patterns that match their own economic standard.

## 6. Boundary Contracts

- `Fee admission`: Actors reserves fee-native spend capacity and invokes atomic `FeeCollector` at most once per attempted User step after read-only evaluation/preparation. Non-executable outcomes charge evaluation-only; executable outcomes charge evaluation plus upper-bound execution fee together. Collection transfers the full amount into `FeeSink`, while downstream allocation remains outside the generic pallet.
- `Fee conversion`: Actors asks `WeightToFee` for deterministic upper-bound pricing; runtime task bounds must include adapter and routing work in both Weight dimensions.
- `Task amount safety`: Transfer, SplitTransfer, Burn, exact-input swap, liquidity, staking, and donation spend through `PreserveSpend`, leaving the adapter-reported asset minimum intact.
- `DEX amount safety`: Exact-in quotes must match caller fees and executable route selection; exact-out receives a policy-derived maximum input that preserves minimum balance and future fee reserve. A host runtime that resolves `RemoveLiquidity` from an LP token must own an O(1) reverse pair index or an equivalently bounded generated lookup outside generic Actors and update it on every supported pool-creation path.
- `Staking amount safety`: Unstake current/trigger/all modes resolve through adapter shares; last-funding mode requires an adapter-mapped transferable share asset.
- `Ingress triggers`: Every supported producer must route through the typed `AddressEventIngress::preflight`/`notify` boundary (or the resolved host adapter over it) with one read-only preflight, one value movement, and exactly one post-movement notification in the same transaction. A rejected notification restores movement and every Actors effect; recoverable queue/wakeup capacity is Temporary and monotonic exhaustion/corruption/invariant failure is Permanent. Runtime event scanning, balance-diff discovery, and deferred compatibility ingress storage are unsupported.
- `Hook admission`: `on_idle` must reserve its generated fixed base before storage access. `ActorOnIdleReserve` must equal genuinely reserved gross headroom and Actors meters against the component-wise minimum of that reserve and actual remaining Weight. Opening plans and bounded `RetryLater` suffixes plus measured pure cleanup must fit after `scheduler_admission_overhead` in both Weight dimensions. Temporal and explicit repair units consume only available headroom and may defer actor execution, so exact actor-local markers and cross-block convergence form part of the embedding contract.
- `Starvation observability`: `IdleStarvationState` is `Healthy`, `Starving { consecutive_blocks }`, or `Alerted { consecutive_blocks }`. Each starved actor-service pass increments the stored count, threshold detection and alerted recovery emit once, and breaker periods freeze the state because no actor-service pass occurs.
- `Continuation`: Mutation adapters return explicit `TaskFailure { error, retry }`; unknown failures and unsupported unit adapters are Permanent. Only Mutable actors admit `RetryLater { max_attempts: 2..=MaxRetryAttempts }`; the protocol-fixed host constant is 10. Runtimes bind generated suspension, retry, completion, cancellation, snapshot-surface, and suffix-admission classes without adding another scheduler or off-chain keeper dependency.
- `Task weight class`: The runtime must classify every task with a deterministic upper bound. Admission may be conservative; it must not underprice execution.
- `Core task admission`: Extend the portable `Task` enum only for a reusable economic primitive that existing composition plus an adapter cannot express without violating atomicity or custody. The change must ship bounded typed parameters, amount/funding/donation semantics, adapter ownership, events/errors/rollback behavior, generated two-dimensional weights, production-budget evidence, semantic tests, and an explicit SCALE/schema-version decision together; runtime topology and product policy stay in adapters and actor graphs.
- `Compatibility/versioning`: `0.7.x` is a fresh-genesis pre-launch line. The first launched downstream compatibility epoch, not an unrelated package-major declaration, begins append-only public enum discriminants and dispatch indices for that chain. Later encoded argument or storage-layout changes require the owning compatibility/migration contract, while additive host adapters and weight recalibration follow package/runtime version policy. A Cargo version never substitutes for pallet `StorageVersion`, runtime `spec_version`, or a bounded live-chain migration.
- `Read model`: Known actor state, owner-slot recovery, scheduler state, current `IdleStarvationState`, and bounded events are canonical on-chain surfaces. Historical starvation duration, fleet dashboards, long timelines, rankings, and analytics are indexed/materialized views reconstructed from detection/recovery events.

## 7. Task-Scoped Atomicity Contract

Actors guarantees Task-scoped atomicity, not whole-contract atomicity. A failed executable Task rolls back all Task-local storage effects and its success event. Earlier successful Steps in the same Actor Contract remain committed. After rollback, `StepErrorPolicy` decides whether the cycle aborts or continues.

| Surface | Actors guarantee | Adapter/runtime obligation |
| --- | --- | --- |
| `Transfer` | Transfer task runs in the task transaction; failed transfer emits failure/summary only | Asset adapter must not preserve partial debit/credit on failure |
| `Swap` | Swap task rolls back if adapter returns error after intermediate mutation | DEX adapter must keep quote, debit, credit, fee, and pool mutation atomic or rely on the Actors transaction |
| `AddLiquidity` | LP success event persists only when the whole task succeeds | DEX adapter must not leave one reserve, LP mint, or debit committed after a late error |
| `Stake` | Stake success event persists only when adapter succeeds | Staking adapter must not leave partial receipt mint, pool share update, or source debit after failure |
| `Unstake` | Unstake success event persists only when adapter succeeds | Staking adapter must not burn shares without returning underlying value on failure |
| `DonateLiquidity` | Donation success event records returned amounts only on success | Donation adapter owns pair balancing and must roll back partial donation/burn/reserve mutation on failure |
| Pure close | No task executes; sovereign balances remain untouched | Runtime must bind generated cleanup weight and preserve actor-owned accounts |
| `ContinueNextStep` | Failed task rolls back, emits `StepFailed`, then later steps may execute | Plan authors should add balance guards after mutating tasks |
| `AbortCycle` | Failed task rolls back, emits `StepFailed`, aborts cycle, and may increment failure count | Adapter rollback must complete before abort handling |
| Earlier successful step | Remains committed after a later task fails | Whole-plan compensation is outside Actors core |
| Task-local rollback | Reverts task storage effects and success event | Multi-step adapter mutations must be transaction-safe |
| Event visibility | Success event is not emitted or is rolled back with failed task; failure/summary events remain observable | Adapters should not emit misleading durable success events outside the transaction boundary |

Pure close has no task-scoped atomicity case: it prechecks fallible invariants, then deletes only actor-owned state and indexes without fees or balance movement.

## 8. External Runtime Test Checklist

A runtime embedding Actors should add local tests for any adapter that mutates more than one storage item:

- Late failure after a partial transfer, burn, pool update, receipt mint, share burn, or donation mutation rolls back task-local state.
- `ContinueNextStep` after a failed mutating task preserves earlier successful steps and executes later eligible steps.
- `AbortCycle` after a failed mutating task rolls back the failed task and aborts without whole-plan rollback.
- Explicit, automatic, dormant, and sweep close preserve sovereign balances, execute no task, and emit `ActorClosed` exactly once.
- Unsupported no-op adapters fail deterministically without panics or hidden state mutation.
- Adapter-level success events do not survive a failed task unless they are explicitly outside the Actors transaction boundary and documented as such.
- Fee collection failure rolls back the payer debit and leaves Fee Sink unchanged.
- Funding-accumulator overflow fails before or transactionally rolls back signed, internal-protocol, and XCM value movement; expired actors receive balance without readiness or accumulator mutation.
- Exact-out never debits above the Actors-provided capacity, and Unstake dynamic modes resolve against shares rather than the base asset.
- Healthy empty `on_idle` leaves no starvation-state key or recovery event; first starvation, one-time alert, prolonged alert, breaker clearing, and one-time recovery match the transition contract.
- Mutable User and System plans suspend only on explicitly Temporary failures or `FundingUnavailable` under `RetryLater { max_attempts }`; retry starts at the same cursor without prefix replay and uses one nonce across attempts.
- Permanent and unsupported-adapter failures create no Continuation; Immutable admission rejects bounded `RetryLater`; User `SwapOut` requires explicit `InputLimit::LiveQuote` or `InputLimit::Absolute(nonzero)` intent, always enforced with current preservable capacity; liquidity addition/removal require fixed non-zero output minima at every adapter boundary; System-only `Mint` remains unchanged.
- Direct ingress during suspension preserves one queue ticket/wakeup and latches the next signal. The current retry retains its frozen funding snapshot; accepted later ingress checked-adds into `funding_accumulated` for the next fresh run regardless of current-run completion, failure, or cancellation.
- Saturated mixed-class load preserves one strict global ticket order, drains only tombstones before the oldest live head, blocks followers behind an unadmitted head, preserves exact physical occupancy through lazy tombstones, and never executes one actor twice in a block.
- Explicit cancellation, plan/policy/schedule replacement, deactivation, terminal transition, and pure close delete Continuation without compensation, prefix rollback, funding restoration, or sovereign-balance movement.

## 9. Non-Goals

Embedding Actors does not require and must not imply:

- Arbitrary user code execution.
- Unbounded task graphs or dynamic smart-contract-like behavior.
- DEOS/TMCTOL System Actor topology.
- DEOS governance or native staking policy.
- Indexer-backed UX as a consensus dependency.
- Off-chain keepers as a correctness requirement.
