# pallet-deos-actors

`pallet-deos-actors` (Rust crate `pallet_deos_actors`) is the reusable FRAME package for the DEOS Actors bounded economic actor runtime. DEOS provides its production-oriented reference composition.

## SDK baseline

This pallet uses the workspace's locked Polkadot SDK 2609 dependency line. The [reference integration owner](../../../docs/deos/actors.integration.en.md#sdk-dependency-line) records exact SDK pins and package/runtime/caller validation scope; node qualification is a separate reference-composition concern, not a reusable pallet contract.

## Scope

The current kernel/runtime slice provides:

- Genesis-selected, fail-closed User creation admission with a bounded chain getter; existing User service and System creation remain independent. The privileged `set_user_creation_enabled` call changes admission independently of freeze and existing service. Public-call categorical diagnostics are described in the architecture map; governance enablement and production Weight/metadata qualification remain open
- User and System Actor creation with deterministic sovereign accounts; the existing breaker refuses every public creation form before Contract, slot or locator validation
- Breaker refusal for explicit controls and sweep; frozen Prepass retains frontiers and same-block thaw cannot resume Drain. Shared certified ingress temporarily refuses positive Active consequences; registered balance-dependency publication also refuses. Both scalar setters remain available. Complete producer rollback, Full Executive and physical qualification remain open in the architecture map
- Bounded Actor Contracts whose Steps own an optional canonical `Precondition` DNF of current-state Predicates and one typed Task (`Transfer`, `Swap`, `AddLiquidity`, `Stake`, `Unstake`, `DonateLiquidity`, or adapter-free `StopCycle`, etc.); absence is the sole unconditional form
- One scheduler over an actor-keyed persistent Service ring with one authoritative head/cursor, generation-bound process residence, B+1 eligibility, cyclic cross-class encounter order, and canonical C32 Block/Tick Deadline carriers
- Exactly one `Manual`, `AddressEvent`, `AtTime`, or `Cadenced` Trigger per Actor, publishing the existing readiness latch only while Idle; observation comparisons are pull-time Step predicates, never Trigger sources
- Bounded `on_idle` execution with sparse Healthy/Starving/Alerted state and one-time detection/recovery events
- Fee admission, lifecycle controls, Mutable System-only individual pause/resume, and pure prechecked terminal cleanup
- Sparse progress-preserving `ActorRunState` for Mutable suspension, with an open nonce separate from finalized identity, one scalar cursor, exact eligibility, current-state reevaluation, exact outcomes, Temporary-only retry, deterministic cancellation, and no prefix replay
- A bounded `simulate_current_contract` rollback core and versioned `ActorSimulationApi` declaration that require exact stored-contract identity, follow fresh/current-run readiness, return ordered outcomes, and roll the entire attempt back
- A read-only `actor_eligibility` projection behind the versioned `ActorEligibilityApi` declaration that reports current readiness, the scheduler-owned phase, and the next eligible block by reusing the same cadence/cooldown/window/backoff/breaker/latch owners as admission
- Runtime-configured adapters for assets, swaps, liquidity, staking, typed failure retryability, fee collection, direct ingress, and weights; swap adapters receive only the actor account and authoritative immutable `ActorType` through a minimal execution context
- Exhaustive package-owned instruction contracts for every Task, Predicate, amount resolution, and error policy, with weight ownership delegated to the single `WeightInfo` interface
- Genesis provisioning of System actors through runtime configuration

The [Service-turn contract](./docs/specification.en.md#32-block-rounds-and-service-order) now has a native-tested canonical consumer for plain Close-mode collector refusal after full admission and safe Actor rollback. Only round/cursor bookkeeping commits. Its additional physical completion port defaults to unavailable; without a qualified host bound, collector refusal still retains the head and halts. [Package architecture](./docs/architecture.en.md#persistent-service-execution) owns the exact delta and test domain; native declaration tokens do not enable the DEOS reference or establish production costs.

## Key rule

DEOS Actors is a **bounded deterministic actor runtime**, not a general-purpose smart-contract VM.
Actors execute declarative plans against runtime adapters under explicit Service, Deadline, fee, Weight, and lifecycle limits. Event-driven triggers such as matched asset ingress are one important part of that model, but they live alongside deterministic scheduling and bounded execution rather than replacing them.

`Percent` reads the applicable live balance or share surface at every execution attempt. `Fixed` preserves its authored debit and fails rather than silently shrinking when current capacity is insufficient.

Active Actor Contracts author optional `close_when` and `on_unfunded`. The legacy completion enum is removed. Present outcome policy requires available host Control, inspection and Close declarations plus ordinary admission; missing policy resources return `BehaviorPolicyUnavailable`, not arithmetic overflow. Native complete-declaration models retain admission, without qualifying production costs. AwaitFunding remains unavailable. Absent `close_when` and OnUnfunded Close retain independent lifecycle terminals and current funding dispositions. The [embedding guide](docs/embedding.md) owns host obligations; Gate B in the root backlog owns complete production/client/artifact closure.

`StopCycle` provides one fieldless successful terminal control. It emits `CycleStopped`, completes through normal summary and auto-close handling, and cannot select a cursor or mutate Actor lifecycle.

The library exposes standalone bounded `CloseWhen` expressions, strict canonical validation/evaluation and a sparse policy-requirement compiler under [specification §7.7](./docs/specification.en.md#77-outcome-policy-language) and [§7.8](./docs/specification.en.md#78-derived-policy-requirements-and-binding). Compilation selects caps/tracking requirements and binds supplied semantic/admission IDs; verified contexts reduce sparse episode state into a candidate, preserving caps, resets and rollback. Re-certification rebinds compatible state rather than resetting it. The cold expression/plan/episode anchor and required-only mutable monitor participate in Contract geometry installation, reconstruction, re-certification, replacement and destruction; the internal Service transaction now reduces actual Step/Cycle facts and commits monitor/Close atomically. Prepared native fixtures cover permitted Action before policy Close, clean flags, zero-Step completion and late-refusal rollback. Policy-bearing effectful Steps now have distinct fail-closed maximum/actual control ports; a host implementing only ordinary Step pricing cannot price them. Zero-Step policies have equivalent separate control ports shared by Pipeline pricing and Service settlement, with native User fee/Close rollback witnesses. Public policy admission, lifecycle/call coverage and physical Weight qualification remain open.

## Reconfiguration rule

Within the existing task and adapter language, a large class of protocol changes should be expressed by reconfiguring actors, triggers, and graphs of asset flows rather than by rewriting the runtime.
Runtime upgrades are reserved for extending primitives, adapter surfaces, or safety invariants.

## Scheduler rule

Readiness and execution must stay deterministic and bounded:

- Future eligibility uses generation-bound Deadline residence rather than ad hoc scans
- Hot-path execution happens only under configured per-block limits
- Timer readiness uses exact deterministic cadence with no actor-specific phase, probability, or entropy contract
- `on_idle` does useful work only with remaining block budget
- Suspended retries reuse Service/Deadline authority and admit only the unresolved suffix; they create no second scheduler, inbox, or off-chain correctness dependency

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
- Bind governance/system origins, owner-slot, Service, Deadline, identity, and sweep bounds, fee constants, Task Weight classes, and native asset identity.
- Bind declaration-only `PipelineControlWeight` (`()` is unavailable); the complete-Contract economic interface has no live fee or authority consumer yet. The embedding guide owns its qualification and staged routing.
- Decide which tasks are allowed for User vs System actors and keep any chain-specific policy in adapters or genesis actor configuration, not in pallet core.
- Provide deterministic genesis System Actor definitions only for actor roles the runtime actually wants to ship.
- Treat example Actor Contracts as reusable Task-language patterns; treat the DEOS/TMCTOL System Actor catalog as one runtime's topology, not as the pallet's required deployment shape.
- Classify adapter mutation failures explicitly as Permanent or Temporary; unknown and unsupported failures stay Permanent.
- Bind fixed `MaxRetryAttempts` plus generated suspension, retry, completion, cancellation, and suffix-admission Weight when Mutable plans expose `RetryLater { max_attempts: 2..=MaxRetryAttempts }`.
- Validate adapter failure atomicity and Mutable User/System run suspension with runtime-local tests when adapters perform multi-step mutations.

## Focused integration validation

From the DEOS checkout, run `./scripts/ci-local.sh --only tests --package pallet-deos-actors --test-target current_state_semantic_oracle --default-features` to select the independent scheduler-model target even when the separate library harness fails. Use `--all-features` for that feature context. This target checks its own logical oracle and fixed trace, not public policy admission, full package completion or physical Weight bounds. External package consumers can use Cargo's ordinary `--test current_state_semantic_oracle` selection.

The test/benchmark-only tariff candidate conformance runs through `./scripts/ci-local.sh --only tests --package pallet-deos-actors-embedding-fixture --all-features --test-filter pipeline_tariff_candidate`. It checks diagnostic declaration/identity composition, not live admission, backing or calibrated prices; the embedding guide owns those boundaries.

The consumed-refusal family runs through `./scripts/ci-local.sh --only tests --package pallet-deos-actors --all-features --test-filter service_refusal`. It uses native declaration tokens to check canonical rollback, cursor completion, paid rights and frozen resource accounting; it is not a physical benchmark or production enablement.

## Fee-envelope evidence example

The package-owned example generates deterministic fee-envelope vectors bound to an embedding runtime's metadata and production Actors weights. It requires explicit artifact paths and does not require a DEOS repository layout:

```sh
cargo run -p pallet-deos-actors --example fee_envelope_vectors -- \
  --metadata /path/to/metadata.scale --weights /path/to/actors_weights.rs \
  --check /path/to/fee-envelope-vectors.json
```

Replace `--check PATH` with `PATH` to write the artifact, or omit it to print JSON. `--help` describes the interface without loading artifacts. Missing inputs fail closed; supplied files are hashed exactly, not validated as runtime metadata or benchmark output by this utility.

## Non-goals of the current slice

The current kernel does not yet include:

- Arbitrary user code execution
- Hidden off-chain nondeterminism as a correctness dependency
- Unbounded task graphs or unmetered loops
- Direct pallet-specific business logic embedded into Actors core

See the package-owned [DEOS Actors Architecture](./docs/architecture.en.md), [DEOS Actors Specification](./docs/specification.en.md), and [embedding guide](./docs/embedding.md) for reusable implementation, semantics, and host obligations. Concrete reference composition belongs to [`docs/deos/actors.integration.en.md`](../../../docs/deos/actors.integration.en.md).
