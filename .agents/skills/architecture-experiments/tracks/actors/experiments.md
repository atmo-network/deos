# Actors Experiments

The Actors track index: charter, current records, release hazards and retained principles. Records own their evidence; `BACKLOG.md` owns open work.

| Field | Value |
| --- | --- |
| Track ID | actors |
| Status | Active |
| Scope owner | DEOS Actors physical execution, storage, scheduling and control topology |
| Depends on tracks | None |

## Scope and Boundary

- `Owns`: Actor Contract and Step representation, hot/cold state, run state, service ring and temporal topology, detector geometry, block service, resource allocation and executor lowering.
- `Excludes`: Normative Actor semantics, adapter-internal execution, Router-internal route selection, open work and experiment results.

## Governing Invariants

- Preserve deterministic semantics, strict service order, one causal hop per block, atomic Step effects, User/System neutrality, bounded state, component-wise Weight, No Ceiling Tax and production Weight soundness.
- Observations never activate Actors; they are read only by fresh-only Step predicates at the check or Attempt.
- Governing contracts: `template/pallets/actors/docs/specification.en.md`, `docs/actors-resource-policy.specification.en.md`, `docs/actors-performance-assurance.specification.en.md`.

## Current Baseline

[EXP-001](./EXP-001.md) is the physical baseline and the only live record: one stable Actor-generation process, an actor-keyed Live/Pending ring, retained C32 deadline pages with paged heaps, revisioned parked-balance wake, immutable block rounds, transactional residence and synchronous bounded close. Decision scope: physical architecture only; production Weight and throughput remain open release work.

## Current Records

| ID | Status | Decision | Consumers |
| --- | --- | --- | --- |
| [EXP-001](./EXP-001.md) | Accepted | Current-state physical architecture, including C32 deadline vacancy authority; pull-model amendment | Baseline; `Actors/Service Weight Ownership`; `Actors/Service Admission And Deadline Geometry`; `Actors/Pull-Model Residue`; `Release/0.7.27 Assurance` |

Weight-coverage findings (retained owners underbounding selected geometries) are not experiments; their witnesses live in the owning `BACKLOG.md` items until coordinated generation dominates them.

## Release Hazards

Routes remaining release-facing review; it does not reopen accepted physical choices.

| Hazard | Current evidence | Smallest release falsifier | Owner |
| --- | --- | --- | --- |
| Legacy execution authority | One sparse `ActorRunState`; no Opening snapshot types; canonical tests reject legacy Ready/Waiting authority | A supported path reads snapshot, Ready/Waiting, successor-ticket or deferred-Cycle state as authority | `Release/0.7.27 Assurance` |
| Round reentry and duplicate residence | Canonical regressions cover the persistent ring | A trace recaptures a turn, loses the frontier, or leaves zero/multiple residences after success, refusal or rollback | `Release/0.7.27 Assurance` |
| Lost wakeup and stale acknowledgment | Historical W6 evidence used observation-only Park and no longer applies; parked-balance revision regressions pass natively | A callback, coalesced revision, timed review or saturation trace acknowledges beyond its covered revision or loses a newer obligation | `Release/0.7.27 Assurance` |
| Deadline phase and recovery | Historical W7 met no-growing debt with censored recovery; the restored mandatory stage is not bound by any Weight/Wasm | Native phase tests substitute for production binding, or censored recovery is promoted to completion | `Actors/Mandatory Deadline Service` |
| Close and replacement isolation | Synchronous close preserves peers, custody and holds; try-runtime inverse ownership and corruption checks pass | Close or replacement leaks authority or custody, or validation accepts an ownerless carrier | `Release/0.7.27 Assurance` |
| Complete resource ownership | Success/rollback accounting and terminal admission reconcile natively; selected witnesses in the Service Weight backlog items show underbounded close, Trigger and SplitTransfer owners | One reachable branch lacks a complete owner, or incurred work vanishes on rollback | `Actors/Service Weight Ownership` |
| Evidence identity | Control identity fingerprints consumed User header profiles; no current tree number has inherited authority | A release claim uses stale source, Weight, metadata, vector, ABI, Wasm or workload evidence | `Release/0.7.27 Assurance` |

## Retained Principles

General findings from earlier research. No earlier number binds the current tree.

- `Population independence`: Adding 9,885 non-due identities changed neither service order nor charged Control for a 100-Actor due frontier; a scalability claim may not assume service scans the identity population.
- `Reclamation consumes service`: Legal prefix reclamation consumed bounded service capacity under the old FIFO; keep current synchronous close distinct from that topology.
- `Control flow needs an owner`: Consensus selection needs an explicit measured Weight owner even when it performs no Task effect; no storage access is not zero RefTime.
- `Deep temporal coverage`: Deep-index coverage and full-capacity rearm were never established for every reachable temporal path; any surviving retry or review cursor needs executable coverage.
- `No single-knob ceilings`: Raising block ProofSize alone also raises fixed reservations and cannot close an Actor Control shortfall; do not tune a numeric ceiling without target-relay and complete-owner evidence.
- `Narrow rejections`: A rejected multi-way deadline cursor was a result under its premises, not a ban on a new carrier.
- `Pull over push`: Push-reactive observation Triggers cost disproportionate fanout, Crossing and Prepass budget and were removed in 0.7.27; price reactions compose `Cadenced` or `AddressEvent` with an observation predicate. Reintroduction requires measured evidence that pull composition misses a concrete latency or cost contract.

## Research Portfolio

Future triggers, not work items. Open a record only with a measured owner and a smallest falsifier.

- Truthful ProofSize: actual PoV versus `MaxEncodedLen` charges (consumed by `Actors/0.7.28 PoV Accounting`).
- Proof anatomy, storage-key locality and canonical cell compression once a production proof attribution isolates a binding owner.
- Shared immutable Contract bodies when repeated identical bodies measurably bind proof or state.
- Actor overhead against an equivalent external operation, and specialized versus generic executor lowering.
- Cadence cohorts for bounded herds and work-conserving base-turn allocation.

## Cross-Track Dependencies

- Actors may consume effect-Weight and failure evidence from the [Adapters track](../adapters/experiments.md) and route envelopes from the [Router track](../router/experiments.md).
- Router does not depend on Actor queue geometry; split cyclic questions at their evidence boundary.

## Entry and Exit Conditions

- `Entry`: The decision changes Actor-owned storage, scheduling, control allocation, lifecycle geometry or execution lowering without changing semantics.
- `Exit`: Adapter-internal or route-internal choices transfer to their owning track; the track becomes Dormant when no decision-relevant Actor question remains.

## History

Numbering restarted at `EXP-001` in 0.7.27, when the track kept only the decisions that define the current architecture; workloads moved to the performance-assurance specification §6 and Weight-coverage witnesses to `BACKLOG.md`. Research up to 0.7.26 remains in release tag `v0.7.26` under four-digit IDs; it has no authority over the current tree and is not cited.
