# DEOS Backlog

> Open framework work only. Completed delivery history belongs in `CHANGELOG.md`; normative semantics remain owned by specifications, architecture documents, code, generated metadata, and production Weight.
>
> Pre-`1.0` boundary: no DEOS network will launch before `1.0`. The `0.7.x` line remains fresh-genesis and may change storage, metadata, runtime APIs, validation topology, and release mechanics without deployed-lineage migration or live-network ceremony.

---

# DEOS 0.7.24 — Topology Scaling

> **Release framing:** `Resource-Proportional Actors + Economic Zipper + Block-Paced Execution + Active-Frontier Scaling`
>
> **Dependency:** start from the accepted `v0.7.23` Reactive Topology Closure. Preserve canonical Trigger-state ownership, generation-checked physical authority, transactional topology transitions, strict FIFO, branch-exact Weight, repairable worker faults, checked arithmetic, economic ownership for User Actors, and the fresh-genesis boundary.
>
> **Primary objective:** permit locally rich Actor Contracts with a target ceiling of 32 Steps without making small Actors pay for unused complexity, while proving bounded, starvation-free first traversal of the 10,000-Active-Actor production population under generated two-dimensional Weight.
>
> **Canonical delivery order:**  
> `Specification → Implementation → Tests → Implementation Correction → Domain Architecture`
>
> Architecture documents record behavior that has already survived implementation and tests. They are not used to retroactively legitimize code.

---

# Asset Conversion Closure


---

# Release Laws

## Semantic Responsibility Law

```text
Actor boundaries follow semantic responsibility.
Runtime bounds follow physical cost.
```

Step count does not decide how many Actors should exist. One Actor may remain locally complex when doing so preserves a coherent:

```text
responsibility
custody boundary
failure boundary
activation semantics
observability surface
```

## Physical Decomposition Law

```text
Contract headers
Step cells/pages
run state
execution tickets
detector indexes
admission certificates
```

are physical topology belonging to one Actor Contract. They are not separate authored Contracts.

## No Ceiling Tax

```text
cost(one-step Actor, MaxContractSteps = 8)
≈
cost(one-step Actor, MaxContractSteps = 32)
```

Increasing the protocol ceiling must not add the following to a short Actor:

```text
cold body reads
unused predicate work
maximum-pipeline orchestration
maximum-pipeline fees
maximum-pipeline ProofSize
```

## Block-Paced Execution

```text
one Actor Cycle
→ at most one committed Step per block
```

A successor Step receives:

```text
eligible_at >= current_block + 1
```

It may execute later because of FIFO pressure or congestion, but never earlier. Physical ChunkSize remains independent of the fixed Q1 ServiceQuantum, and causal CohortSize derives only from one shared Trigger cause.

## Causal Speed Limit

Under the authoritative fixed timing model, readiness created by an operation in block `N` may cause an Actor Step no earlier than block `N + 1`; this is an eligibility floor rather than an execution SLA, and ordinary FIFO plus available `on_idle` Weight may delay service further.

```text
A effect in N
→ readiness B
→ B Step no earlier than N + 1
```

Therefore:

```text
maximum endogenous propagation
= one causal hop per block
```

## Step Atomicity

The following commit atomically:

```text
preconditions
+ amount resolution
+ one Task effect
+ StepOutcome
+ fee settlement
+ cursor transition
```

The full pipeline is explicitly non-atomic. Effects of previously committed Steps remain durable if a later Step fails, is cancelled, or the Actor is closed.

## Dual-Resource Law

```text
1/3  Actor Control Plane
2/3  Shared Economic Execution
```

The Actor Control Plane pays for:

```text
detection
materialization
hot-state loading
lazy Step loading
preconditions
amount resolution
FIFO management
run-state bookkeeping
fee bookkeeping
retry/completion
fault handling
```

Shared Economic Execution pays for:

```text
ordinary user extrinsics
Actor Transfer
Actor Swap
Actor Liquidity
Actor Stake/Unstake
Actor Mint/Burn
other Task effects
```

## Economic Zipper

Under continuous demand from both sides, Shared Economic Execution is split symmetrically:

```text
1/3 Actor base turn
1/3 user base turn
```

The split is work-conserving:

```text
unused Actor capacity → users
unused user capacity  → Actors
```

Fairness is measured by `Weight`, not by call count.

## Class-Neutral Service

```text
User Actor
System Actor
```

share:

```text
one Actor FIFO
one ActorControlMeter
one SharedEconomicMeter
one Step service protocol
```

Actor class never influences ordering or resource preference. System Actor fee exemption remains an economic policy, not a scheduler priority.

## Active-Frontier Law

```text
IdleTax     ≈ fixed bounded work
EventCost   ≈ affected discovery frontier
ControlCost ≈ pending materialization frontier
EffectCost  ≈ executed Step effects
```

Consensus work must not scale with total dormant identity population.

---

# Quantitative Release Contract

The earlier 100-Steps-per-block planning witness is not a production promise: complete generated control/effect Weight disproves it for the accepted topology. Release acceptance instead requires a starvation-free first traversal of the 10,000-Active-Actor population within the measured 1,300-block horizon, with no claim that the horizon is an execution SLA.

Every production profile reports independently:

```text
S = committed Actor Steps / block
A = distinct Actors progressed / block
```

For the exact one-Step Q1 witness, `S = A`; mixed or specification-admitted multi-Step profiles MUST preserve both axes and their Pareto frontier rather than relabeling either as the other.

The production target is:

```text
MaxContractSteps = 32
```

Production `MaxContractSteps` is fixed at `32`. The `8` and `16` profiles exist only for comparative benchmarks; any proposal to reduce the production ceiling must reopen Phase S and amend the owning specification before implementation or release work continues.

---

# Canonical Work Protocol

## Stage Gates

No later stage begins until the previous stage is closed:

```text
S0  Normative specification accepted
I0  Implementation complete
T0  Tests implemented and executed
F0  Implementation corrected against test evidence
A0  Architecture records final accepted mechanism
```

## Reopening Rule

If a test proves that the specification itself is defective:

```text
test failure
→ specification change record
→ reopen Specification
→ invalidate affected Implementation/Test/Fix tasks
→ repeat the chain
```

A test or architecture document must never be silently changed to match accidental implementation behavior.

## Correction Rule

During `Implementation Correction`:

- implementation is corrected against the accepted specification;
- every consensus-relevant defect gets a minimized regression;
- production Weight is regenerated after final code changes;
- architecture documents remain untouched until the implementation converges.

---

# PHASE I — IMPLEMENTATION

Implementation follows the accepted canonical Actors, resource-policy, and performance-assurance specifications. A verified specification defect reopens specification convergence before affected implementation continues. Architecture-affecting alternatives use the track-local `<track>/EXP-NNNN` inventories routed by `.agents/skills/architecture-experiments/SKILL.md`; the backlog tracks remaining work, while the Skill-owned Experiment Records retain candidates, measurements, rejected alternatives, decisions, and lineage.

Before Phase T begins, the Experimental Closure Gate requires every architecture-affecting alternative to be Accepted, Rejected, Inconclusive, or explicitly deferred here with rationale; every accepted benchmark-sensitive choice must have durable evidence and every rejected candidate must remain discoverable.

---

## I4A — Q1 Freshness and Load-Elastic Service Research


`Deferred architecture`: Q1 hot-path decomposition and general Q2/Q4 service remain dormant in EXP-0017. Reopen only if final block-resource evidence misses the committed-Step target and controlled decomposition proves current-fragment control/persistence overhead remains binding. Dynamic re-cohorting, Task-type cohorts, Weight/fee-aware ranking, and User/System priority scheduling remain outside 0.7.24.

---

## I5 — Mandatory Actor Prepass Inherent

The versioned payload-free Mandatory Actor Prepass is implemented with required Timestamp/parachain context, duplicate and ordering rejection, one-way phase/finalization guards, bounded stale cleanup and materialization, immutable FIFO cutoff, Actor base service, and valid actual Weight.

---

## I6 — Per-Block Resource Meters


---

## I7 — Actor Drain

The resource-bounded Actor Drain is implemented against the fixed prepass cutoff with strict FIFO, remaining Actor Control and Shared Economic capacity, generated actual Weight, and bounded telemetry.

---

## I9 — ObservationCrossing Scaling

The shipped coordinator, rotated grants, sparse radix authority, contiguous cohort snapshot, hot-header classification, grouped queue/rearm writes, non-tail refill, locator repair, terminal handling, scalar fallback, bounded faults, and class-neutral service are the retained baseline.

---

## I15 — Benchmark and Assurance Harness


---

# PHASE T — TESTS

Tests are written only after the corresponding implementation is complete.

---

## T1 — Block Phase and Economic Zipper Tests

---

## T2 — Block-Paced and Causal Execution Tests

The retained suites already prove one committed Step per Actor/block, durable prefixes, exact Continue/Retry/Stop/Abort/cancellation behavior, fixed N+1 eligibility, actor-produced causal chains, useful-transition latching, mixed-length initial cohorts, cohort dissolution, and congestion-delayed FIFO progress.

---

## T4 — Opening, Current, and Contract Geometry Tests

The retained suites already prove immutable Opening balance/observation/predicate/funding snapshots, live Current evaluation, prior-effect and previous-outcome visibility, retry semantics, effect-before-evaluation admission, false-predicate accounting, hot/current-fragment-only execution, C6 commitment/integrity, exact rollback, 32-Step lifecycle dispatchability, ceiling invariance, semantic identity, and runtime admission identity.

---

## T6 — Task Effect Equivalence Tests

The retained parity matrix covers every Task family, canonical host-state/effect-Weight ownership, Actor-only control overhead, typed Temporary/Permanent policy, rollback, and exactly-once induced consequences.

---

## T7 — FIFO Tests

The retained suites already prove monotonic tickets, one live ticket, temporal-to-FIFO materialization, separate deferred-cycle readiness, tombstone cleanup, saturation, strict heavy/corrupt heads, index exhaustion, exact-slot recreation, and class-neutral order.

---

## T8 — Detector Scaling Tests

The retained package/runtime suites already cover aggregate Crossing branches, scalar fallback, exact rollback, occupied broad pages, revision resumption, sparse block/tick wakeups, no catch-up, Manual timing, AddressEvent filtering, useful-transition latching, and producer rollback.

---

## T9 — Economics Tests

The retained suites cover 32-Step state holds, reserve/release rollback, useful Trigger and complete Pipeline charging, actual Action fees, false-predicate non-invocation, no ceiling fee, protected minima, and System fee exemption without priority.

---

## T10 — Runtime API Tests

Existing tests retain bounded runtime projections, fail-closed variants, current materialization faults, and generated metadata freshness.

---

## T11 — Model and Property Tests

The retained pure and state-machine models cover lifecycle, signals, success, Continue, Retry, suspension, retained readiness, updates, close, breaker, exact events/fees/cursors, TryRuntime, and minimized regressions.

---

## T12 — Mandatory Scale Profiles


---

# PHASE F — IMPLEMENTATION CORRECTION

Phase F owns only concrete defects exposed by Phase T. Add one minimized, independently closable correction item per verified failure; return specification defects to Phase S and remove each implementation item immediately after its regression and owning validation pass. Speculative fix inventories, evergreen review rules, and duplicate release checklists do not belong in the backlog.

---

# PHASE A — DOMAIN ARCHITECTURE

Architecture documents are updated only after a frozen green implementation exists.

If architecture writing reveals a new semantic inconsistency, Phase A stops and the affected domain returns to Phase S.

Before Phase A closes, the Architecture Provenance Gate requires every significant physical choice to cite a normative specification, Accepted Experiment Record, production benchmark, or correctness/security invariant without copying the research log into architecture prose.

---

## A1 — Actors Domain Architecture

**Owner:** `template/pallets/actors/docs/architecture.en.md`

The current document already owns the retained Actor state, C6 geometry, identity, run, Q1, snapshots, latch, FIFO, Weight, rollback, storage, source, generated-owner, TryRuntime, and non-goal truth.


---

## A2 — Runtime Resource Architecture

**Owner:** `docs/actors.integration.en.md`

Record:

```text
context inherents
→ Actor Prepass
→ user dispatch
→ Actor Drain
```


---

## A3 — Oracle Domain Architecture

**Owner:** `template/pallets/oracle/docs/architecture.en.md`

The current Oracle document already owns publication atomicity, provenance, backpressure, fault, Weight, and no-cascade truth.


---

## A4 — Router and Economic Effect Architecture

**Owners:** `template/pallets/router/docs/architecture.en.md` and affected pallet architecture docs.

The current package and integration documents already own canonical economic effects, atomicity, sequential market state, induced consequences, and production Weight equivalence.


---

## A5 — Actors Embedding Architecture

**Owner:** `template/pallets/actors/docs/embedding.md`

Record host ports:

```text
TaskEffectWeight
ActorControlWeight
Actor state holds
mandatory prepass integration
shared economic accounting
current context requirements
```


---

# Non-Goals

- No literal alternation `User call → Actor call → User call` based on node-local arrival order.
- No shared consensus queue combining mempool entries and Actor tickets.
- No class priority for System Actors.
- No same-block execution of multiple Steps from one Actor Cycle.
- No splitting one Task across blocks.
- No whole-pipeline atomicity.
- No generic priority queue or bypass around a heavy FIFO head.
- No universal Trigger index.
- No authored page/chunk/batch/slice geometry.
- No second semantic execution engine.
- No arbitrary runtime-dispatch Task.
- No promise of 10,000 maximum-cost 32-Step Cycles in ten minutes.
- No production promise of one million Active Actors.
- No network-wide physiology graph; that belongs to `0.7.25`.
- No live-network migration or upgrade ceremony.

---

# Canonical Delivery Sequence

The release work must proceed in this exact order:

```text
1. Converge the canonical Actors, resource-policy, and performance-assurance specifications.
2. Reconcile the backlog against accepted specification and retained implementation truth.
3. Implement mandatory Actor Prepass, component-wise resource meters, Economic Zipper, and Actor Drain.
4. Scale Crossing and broad fanout, then converge named runtime projections.
5. Run final semantic, resource, model, API, and scale acceptance cohorts.
6. Generate production Weight/Wasm/metadata evidence and correct only verified defects.
7. Freeze the accepted tree and update package, integration, embedding, and Wiki architecture from shipped truth.
8. Run final release validation on that exact documented tree.
```

---

# Release Thesis

`0.7.24` is not merely a performance release. It establishes a new physical model for DEOS Actors:

```text
rich Contracts
+ lazy local state
+ one-Step causal clock
+ symmetric economic contention
+ bounded control physiology
+ active-frontier scaling
```

Only after these laws are implemented, falsified, corrected, and frozen in architecture should `0.7.25 — Network Physiology` analyze cascades, feedback loops, dependency topology, effect topology, circulation, and systemic externalities.
