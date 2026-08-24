# pallet-deos-actors

`pallet-deos-actors` (Rust crate `pallet_deos_actors`) is the reusable FRAME package for the DEOS Actors bounded economic actor runtime. DEOS provides its production-oriented reference composition.

## SDK baseline

This pallet is maintained against the current DEOS `Polkadot SDK 2606 / node 1.24.0` line.
The 2606 upgrade did not require pallet-local semantic changes here; the relevant fallout landed in runtime/parachain-system/asset-conversion integration surfaces rather than in `pallet_deos_actors` core logic.

## Scope

The current kernel/runtime slice provides:

- User and System Actor creation with deterministic sovereign accounts
- Bounded Actor Contracts whose Steps own an optional canonical `Precondition` DNF with explicit Opening/Current timed Predicates and one typed Task (`Transfer`, `Swap`, `AddLiquidity`, `Stake`, `Unstake`, `DonateLiquidity`, or adapter-free `StopCycle`, etc.); absence is the sole unconditional form
- One scheduler over a canonical paged FIFO with monotonic `NextQueueTicket`, common block cutoff, exact physical occupancy, one actor-local live ticket, strict global ticket order across actor types, and shared time-ordered wakeup storage
- Exactly one `Manual`, `AddressEvent`, `ObservationChange`, or timestamp-tick `Cadenced` trigger per Actor; one-feed subscriptions and latest revisions stay bounded in reusable paged state while independently metered deferred fanout coalesces into the existing readiness latch and scheduler
- Bounded `on_idle` execution with sparse Healthy/Starving/Alerted state and one-time detection/recovery events
- Fee admission, lifecycle controls, pause/resume, and pure prechecked terminal cleanup
- Sparse progress-preserving `ActorRunState` for Mutable suspension, with an open nonce separate from finalized identity, one scalar cursor, exact eligibility, immutable Opening/funding snapshots, exact outcomes, Temporary-only retry, deterministic cancellation, and no prefix replay
- A bounded `simulate_current_contract` rollback core and versioned `ActorSimulationApi` declaration that require exact stored-contract identity, follow fresh/current-run readiness, return ordered outcomes, and roll the entire attempt back
- A read-only `actor_eligibility` projection behind the versioned `ActorEligibilityApi` declaration that reports current readiness, the scheduler-owned phase, and the next eligible block by reusing the same cadence/cooldown/window/backoff/breaker/latch owners as admission
- Runtime-configured adapters for assets, swaps, liquidity, staking, typed failure retryability, fee collection, direct ingress, and weights; swap adapters receive only the actor account and authoritative immutable `ActorType` through a minimal execution context
- Exhaustive package-owned instruction contracts for every Task, Predicate, amount resolution, and error policy, with weight ownership delegated to the single `WeightInfo` interface
- Genesis provisioning of System actors through runtime configuration

## Key rule

DEOS Actors is a **bounded deterministic actor runtime**, not a general-purpose smart-contract VM.
Actors execute declarative plans against runtime adapters under explicit queue, scheduler, fee, weight, and lifecycle limits. Event-driven triggers such as matched asset ingress are one important part of that model, but they live alongside deterministic scheduling and bounded execution rather than replacing them.

`PercentageAtOpening` reads a typed balance/share snapshot captured when a fresh cycle opens. Its values remain independent of trigger kind, signal payload, and AddressEvent amount.

Active Actor Contracts choose `Persistent` or `CloseAfterProductiveCycle`. Productive closure requires successful logical-cycle completion with at least one committed effectful task; false Precondition results, skipped Steps, rollback, suspension, abort, retry exhaustion, and bare `StopCycle` do not qualify.

`StopCycle` provides one fieldless successful terminal control. It emits `CycleStopped`, completes through normal summary, funding, and auto-close handling, and cannot select a cursor or mutate actor lifecycle.

## Reconfiguration rule

Within the existing task and adapter language, a large class of protocol changes should be expressed by reconfiguring actors, triggers, and graphs of asset flows rather than by rewriting the runtime.
Runtime upgrades are reserved for extending primitives, adapter surfaces, or safety invariants.

## Scheduler rule

Readiness and execution must stay deterministic and bounded:

- Future eligibility goes through the wakeup layer rather than ad hoc scans
- Hot-path execution happens only under configured per-block limits
- Timer readiness uses exact deterministic cadence with no actor-specific phase, probability, or entropy contract
- `on_idle` does useful work only with remaining block budget
- Suspended-run retries reuse the same FIFO/wakeup substrate and admit only the unresolved suffix; they create no second scheduler, inbox, or off-chain correctness dependency

## Runtime-as-Config rule

The pallet must stay generic.
Concrete chain policy belongs in runtime configuration, including:

- `AssetOps`, swap-only `DexOps`, `LiquidityOps`, and `StakingOps`
- Fee conversion, fee collection, and task weight classes
- Ingress hooks and genesis System Actor topology
- Governance/system origins and operational bounds

## External runtime embedding checklist

A runtime can reuse `pallet-deos-actors` without adopting the full DEOS/TMCTOL topology by providing the bounded configuration surface only. The package-owned host-runtime contract lives in [`docs/embedding.md`](./docs/embedding.md). Executable portability evidence lives in the separate [`embedding-runtime`](https://github.com/atmo-network/deos/tree/main/template/pallets/actors/embedding-runtime) Cargo package under this pallet ownership boundary; that fixture is not a second product or a normative topology.

Minimal checklist:

- Implement asset, optional domain, fee-collection, direct-ingress, benchmarking, and task-weight adapters for local runtime types.
- Bind governance/system origins, owner-slot limits, queue/wakeup bounds, fee constants, task weight classes, and native asset identity.
- Decide which tasks are allowed for User vs System actors and keep any chain-specific policy in adapters or genesis actor configuration, not in pallet core.
- Provide deterministic genesis System Actor definitions only for actor roles the runtime actually wants to ship.
- Treat example Actor Contracts as reusable Task-language patterns; treat the DEOS/TMCTOL System Actor catalog as one runtime's topology, not as the pallet's required deployment shape.
- Classify adapter mutation failures explicitly as Permanent or Temporary; unknown and unsupported failures stay Permanent.
- Bind `MaxOpeningSnapshotEntries`, fixed `MaxRetryAttempts`, and generated suspension, retry, completion, cancellation, and suffix-admission weights when Mutable plans expose `RetryLater { max_attempts: 2..=MaxRetryAttempts }`.
- Validate adapter failure atomicity and Mutable User/System run suspension with runtime-local tests when adapters perform multi-step mutations.

## Non-goals of the current slice

The current kernel does not yet include:

- Arbitrary user code execution
- Hidden off-chain nondeterminism as a correctness dependency
- Unbounded task graphs or unmetered loops
- Direct pallet-specific business logic embedded into Actors core

See the package-owned [DEOS Actors Architecture](./docs/architecture.en.md), [DEOS Actors Specification](./docs/specification.en.md), and [embedding guide](./docs/embedding.md) for reusable implementation, semantics, and host obligations. Concrete reference composition belongs to [`docs/actors.integration.en.md`](../../../docs/actors.integration.en.md).
