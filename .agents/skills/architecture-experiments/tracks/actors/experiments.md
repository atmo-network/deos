# Actors Experiments

The Actors track index: charter, current records, release hazards, retained principles and the archive of every earlier record. Records own their evidence; `BACKLOG.md` owns open work.

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

[EXP-0161](./EXP-0161.md) is the physical baseline: one stable Actor-generation process, an actor-keyed Live/Pending ring, retained C32 deadline pages with paged heaps, revisioned parked-balance wake, immutable block rounds, transactional residence and synchronous bounded close. Decision scope: physical architecture only; production Weight and throughput remain open release work.

## Current Records

| ID | Status | Kind | Decision | Consumers |
| --- | --- | --- | --- | --- |
| [EXP-0161](./EXP-0161.md) | Accepted | Consolidated | Current-state physical architecture; pull-model amendment | Baseline; `Actors/Service Weight Ownership`; `Actors/Pull-Model Residue`; `Release/0.7.27 Assurance` |
| [EXP-0162](./EXP-0162.md) | Accepted | Leaf | Frozen W1–W12 workload, censoring and materiality contract | `Actors/Final Weight And Measurements`; `Actors/Mandatory Deadline Service` |
| [EXP-0163](./EXP-0163.md) | Measured | Consolidated | Vacancy authority accepted; retained close and occurrence-only Deadline owners underbound | `Actors/Service Admission And Deadline Geometry`; `Actors/Service Failure And Terminal`; `Actors/Mandatory Deadline Service` |
| [EXP-0164](./EXP-0164.md) | Measured | Consolidated | Retained Manual and AddressEvent owners underbound maximum headers | `Actors/Service Detector And Placement` |
| [EXP-0165](./EXP-0165.md) | Rejected | Consolidated | Retained four-leg SplitTransfer owner underbounds User, inline-expiry and bridge branches | `Actors/Service Step And Fees` |

## Release Hazards

Routes remaining release-facing review; it does not reopen accepted physical choices.

| Hazard | Current evidence | Smallest release falsifier | Owner |
| --- | --- | --- | --- |
| Legacy execution authority | One sparse `ActorRunState`; no Opening snapshot types; canonical tests reject legacy Ready/Waiting authority | A supported path reads snapshot, Ready/Waiting, successor-ticket or deferred-Cycle state as authority | `Release/0.7.27 Assurance` |
| Round reentry and duplicate residence | Canonical regressions cover the persistent ring | A trace recaptures a turn, loses the frontier, or leaves zero/multiple residences after success, refusal or rollback | `Release/0.7.27 Assurance` |
| Lost wakeup and stale acknowledgment | Historical W6 evidence used observation-only Park and no longer applies; parked-balance revision regressions pass natively | A callback, coalesced revision, timed review or saturation trace acknowledges beyond its covered revision or loses a newer obligation | `Release/0.7.27 Assurance` |
| Deadline phase and recovery | Historical W7 met no-growing debt with censored recovery; the restored mandatory stage is not bound by any Weight/Wasm | Native phase tests substitute for production binding, or censored recovery is promoted to completion | `Actors/Mandatory Deadline Service` |
| Close and replacement isolation | Synchronous close preserves peers, custody and holds; try-runtime inverse ownership and corruption checks pass | Close or replacement leaks authority or custody, or validation accepts an ownerless carrier | `Release/0.7.27 Assurance` |
| Complete resource ownership | Success/rollback accounting and terminal admission reconcile natively; EXP-0163–EXP-0165 record underbounded owners | One reachable branch lacks a complete owner, or incurred work vanishes on rollback | `Actors/Service Weight Ownership` |
| Evidence identity | Control identity fingerprints consumed User header profiles; no current tree number has inherited authority | A release claim uses stale source, Weight, metadata, vector, ABI, Wasm or workload evidence | `Release/0.7.27 Assurance` |

## Retained Principles

General findings from archived records. No archived number binds the current tree.

- `Population independence` (EXP-0075): Adding 9,885 non-due identities changed neither service order nor charged Control for a 100-Actor due frontier; a scalability claim may not assume service scans the identity population.
- `Reclamation consumes service` (EXP-0076): Legal prefix reclamation consumed bounded service capacity under the old FIFO; keep current synchronous close distinct from that topology.
- `Control flow needs an owner` (EXP-0113, EXP-0114): Consensus selection needs an explicit measured Weight owner even when it performs no Task effect; no storage access is not zero RefTime.
- `Deep temporal coverage` (EXP-0117): Deep-index coverage and full-capacity rearm were never established for every reachable temporal path; any surviving retry or review cursor needs executable coverage.
- `No single-knob ceilings` (EXP-0155): Raising block ProofSize alone also raises fixed reservations and cannot close an Actor Control shortfall; do not tune a numeric ceiling without target-relay and complete-owner evidence.
- `Narrow rejections` (EXP-0118): A rejected multi-way deadline cursor is a historical result under its premises, not a ban on a new carrier.
- `Pull over push` (0.7.27 cut, commit `5d33ddf`): Push-reactive observation Triggers cost disproportionate fanout, Crossing and Prepass budget; price reactions compose `Cadenced` or `AddressEvent` with an observation predicate. Reintroduction requires measured evidence that pull composition misses a concrete latency or cost contract.

## Research Portfolio

Future triggers, not work items. Open a record only with a measured owner and a smallest falsifier.

- Truthful ProofSize: actual PoV versus `MaxEncodedLen` charges (consumed by `Actors/0.7.28 PoV Accounting`).
- Proof anatomy, storage-key locality and canonical cell compression once a production proof attribution isolates a binding owner.
- Shared immutable Contract bodies when repeated identical bodies measurably bind proof or state.
- Actor overhead against an equivalent external operation, and specialized versus generic executor lowering (archived proposals EXP-0014, EXP-0015).
- Cadence cohorts for bounded herds and work-conserving base-turn allocation (archived proposals EXP-0011, EXP-0007).

## Cross-Track Dependencies

- Actors may consume effect-Weight and failure evidence from the [Adapters track](../adapters/experiments.md) and route envelopes from the [Router track](../router/experiments.md).
- Router does not depend on Actor queue geometry; split cyclic questions at their evidence boundary.

## Entry and Exit Conditions

- `Entry`: The decision changes Actor-owned storage, scheduling, control allocation, lifecycle geometry or execution lowering without changing semantics.
- `Exit`: Adapter-internal or route-internal choices transfer to their owning track; the track becomes Dormant when no decision-relevant Actor question remains.

## Archive

Every earlier record, retained for identity and navigation. Full records are preserved at commit `961748c34eddcc2370d5e6f55bd9a8cc03d02e10` under `.agents/skills/architecture-experiments/tracks/actors/` (read with `git show 961748c34edd:<path>`); that commit must stay reachable through a tag before the release branch is squashed. `Retired mechanism` marks records whose measured mechanism was deleted by the pull-model cut.

| ID | Release | Status | Title | Current owner |
| --- | --- | --- | --- | --- |
| EXP-0001 | 0.7.23 | Rejected | Current Actor Throughput and Service Stability Baseline | — |
| EXP-0002 | 0.7.24 | Invalidated | Actor Contract Production Ceiling | — |
| EXP-0003 | 0.7.24 | Rejected | Actor Contract Body Geometry | — |
| EXP-0004 | 0.7.25 | Invalidated | Actor Step Representation | — |
| EXP-0005 | 0.7.24 | Accepted | Current Execution Authority Geometry | — |
| EXP-0006 | 0.7.24 | Accepted | Actor Control Allocation Sweep | — |
| EXP-0007 | 0.7.24 | Proposed | Is symmetric base-turn allocation work-conserving? | — |
| EXP-0008 | 0.7.24 | Accepted | Crossing Membership Page Geometry | Retired mechanism |
| EXP-0009 | 0.7.24 | Accepted | Crossing Production Cohort | Retired mechanism |
| EXP-0010 | 0.7.24 | Accepted | Compact Observation Activation Authority | Retired mechanism |
| EXP-0011 | 0.7.24 | Proposed | Which cadence/wakeup cohort serves bounded herds? | — |
| EXP-0012 | 0.7.24 | Proposed | Which canonical FIFO-internal geometry reduces queue work? | — |
| EXP-0013 | 0.7.24 | Accepted | One-Step Actor Resource Decomposition | — |
| EXP-0014 | 0.7.24 | Proposed | What is Actor overhead over an equivalent external operation? | — |
| EXP-0015 | 0.7.24 | Proposed | Does specialized lowering beat the generic executor? | — |
| EXP-0016 | 0.7.24 | Accepted | Inline Step 0 with Lazy Tail Geometry | — |
| EXP-0017 | 0.7.24 | Proposed | Service Quantum Reopen Gate | — |
| EXP-0018 | 0.7.24 | Rejected | Fresh Step-0 Causal Timing | — |
| EXP-0019 | 0.7.24 | Invalidated | Minimal User Apoptosis | — |
| EXP-0020 | 0.7.24 | Accepted | Trigger Latch Disable and Re-arm Topology | — |
| EXP-0021 | 0.7.24 | Superseded | Contract Step Ceiling Sweep | — |
| EXP-0022 | 0.7.24 | Invalidated | Equal-Thirds Resource Allocation | — |
| EXP-0023 | 0.7.24 | Accepted | Step-Centric Memory Topology | — |
| EXP-0024 | 0.7.24 | Accepted | Active-Lifetime Run-State Hold | — |
| EXP-0025 | 0.7.25 | Accepted | Canonical Actor Control Physical Geometry | — |
| EXP-0026 | 0.7.25 | Rejected | Twelve-Step Reference-Profile Physical Consequences | — |
| EXP-0027 | 0.7.25 | Rejected | Immutable Contract-Body State-Hold Cache | — |
| EXP-0028 | 0.7.25 | Accepted | Actor Control Weight Ownership and Phase Accounting | — |
| EXP-0029 | 0.7.25 | Accepted | Reachable Actor State Geometry | — |
| EXP-0030 | 0.7.25 | Accepted | Action Invocation Receipt Ownership | — |
| EXP-0031 | 0.7.25 | Accepted | Pipeline Opening Collection Ownership | — |
| EXP-0032 | 0.7.25 | Accepted | Current Predicate Component Bound | — |
| EXP-0033 | 0.7.25 | Accepted | Opening Predicate Capture Bound | — |
| EXP-0034 | 0.7.25 | Accepted | Opening Amount Capture Bound | — |
| EXP-0035 | 0.7.25 | Accepted | Zero-Result Predicate Traversal Ownership | — |
| EXP-0036 | 0.7.25 | Accepted | Zero-Result Amount Traversal Ownership | — |
| EXP-0037 | 0.7.25 | Accepted | User and System Zero-Step Base Ownership | — |
| EXP-0038 | 0.7.25 | Accepted | Temporal Zero-Step Opening Ownership | — |
| EXP-0039 | 0.7.25 | Accepted | Crossing Opening Rearm Ownership | Retired mechanism |
| EXP-0040 | 0.7.25 | Accepted | Observation Subscription Cleanup Ownership | Retired mechanism |
| EXP-0041 | 0.7.25 | Accepted | Pipeline Admission Apoptosis Ownership | — |
| EXP-0042 | 0.7.25 | Accepted | Public Close Cleanup Ownership | — |
| EXP-0043 | 0.7.25 | Accepted | Window Expiry Close Ownership | — |
| EXP-0044 | 0.7.25 | Accepted | User State-Hold Reconciliation Bound | — |
| EXP-0045 | 0.7.25 | Measured | Authored Amount and LastFunding Algebra | — |
| EXP-0046 | 0.7.25 | Measured | Predicate and DNF Reachable Geometry | — |
| EXP-0047 | 0.7.25 | Accepted | Funding Snapshot Consumption Bound | — |
| EXP-0048 | 0.7.25 | Accepted | Run and Contract Head-Tail Reachability | — |
| EXP-0049 | 0.7.25 | Accepted | Host Active Identity and Sovereign Population | — |
| EXP-0050 | 0.7.25 | Accepted | Legal Waiting Primary Population | — |
| EXP-0051 | 0.7.25 | Accepted | Legal Waiting Reference Population | — |
| EXP-0052 | 0.7.25 | Accepted | Joint Suspended and User Close Geometry | — |
| EXP-0053 | 0.7.25 | Measured | V10 Admission Refusal Lower Bound | — |
| EXP-0054 | 0.7.25 | Measured | Native Admission Refusal and Precedence | — |
| EXP-0055 | 0.7.25 | Accepted | Waiting Publication Production Bound | — |
| EXP-0056 | 0.7.25 | Accepted | Waiting Removal and Heap Repair Production Bound | — |
| EXP-0057 | 0.7.25 | Accepted | Useful Trigger Detection Weight Coverage | — |
| EXP-0058 | 0.7.25 | Accepted | Scheduler Source and Destination Weight Coverage | — |
| EXP-0059 | 0.7.25 | Accepted | Running and Suspended Control Weight Coverage | — |
| EXP-0060 | 0.7.25 | Accepted | Control and Effect Settlement Containment | — |
| EXP-0061 | 0.7.25 | Accepted | Joint Encoded Actor Control Cell Reachability | — |
| EXP-0062 | 0.7.25 | Accepted | Nonzero Action Fee Collection Ownership | — |
| EXP-0063 | 0.7.25 | Accepted | Waiting / Deadline Heap Production Envelope | — |
| EXP-0064 | 0.7.25 | Accepted | Actor Admission Envelope | — |
| EXP-0065 | 0.7.25 | Invalidated | Production Binding Closure | — |
| EXP-0066 | 0.7.25 | Accepted | Integrated Actor Throughput | — |
| EXP-0067 | 0.7.25 | Accepted | User Opening Settlement Binding | — |
| EXP-0068 | 0.7.25 | Accepted | User Opening Completion Inner Owner | — |
| EXP-0069 | 0.7.25 | Accepted | User Completion Header Envelope | — |
| EXP-0070 | 0.7.25 | Accepted | System Schedule Throughput and Censored Service | — |
| EXP-0071 | 0.7.25 | Accepted | Contract Geometry Service | — |
| EXP-0072 | 0.7.25 | Accepted | Heterogeneous Task Service | — |
| EXP-0073 | 0.7.25 | Accepted | Lifecycle Outcome and Prefix Durability | — |
| EXP-0074 | 0.7.25 | Accepted | Arrival Eligibility and Generation | — |
| EXP-0075 | 0.7.25 | Accepted | Due-Frontier Population Isolation | — |
| EXP-0076 | 0.7.25 | Accepted | Ready Tombstone Prefix Pressure | — |
| EXP-0077 | 0.7.25 | Accepted | User Resource Independence | — |
| EXP-0078 | 0.7.25 | Accepted | Temporal Materialization Phase Ownership | — |
| EXP-0079 | 0.7.25 | Accepted | Temporal Generated Charge and Admission Ownership | — |
| EXP-0080 | 0.7.25 | Accepted | Matched-Phase StorageProof Node Coverage | — |
| EXP-0081 | 0.7.25 | Accepted | Manual Service Stop Reconstruction | — |
| EXP-0082 | 0.7.25 | Accepted | Successful System Transfer Charge Ownership | — |
| EXP-0083 | 0.7.25 | Accepted | No-Due and Genesis Control Baseline | — |
| EXP-0084 | 0.7.25 | Accepted | System Schedule Charged-Owner Accounting | — |
| EXP-0085 | 0.7.25 | Accepted | Signed-Demand Accounting Evidence Linkage | — |
| EXP-0086 | 0.7.25 | Accepted | Lifecycle Charged-Owner Accounting | — |
| EXP-0087 | 0.7.25 | Accepted | User and Reactive Target Population Coverage | — |
| EXP-0088 | 0.7.25 | Accepted | Funded User Fee-Path Resource Coverage | — |
| EXP-0089 | 0.7.25 | Accepted | Reactive Executed-Path Resource Coverage | — |
| EXP-0090 | 0.7.25 | Accepted | Heavy Economic Task Resource Coverage | — |
| EXP-0091 | 0.7.25 | Accepted | Sustained Service Evidence Sufficiency | — |
| EXP-0092 | 0.7.25 | Accepted | Remaining Service-Stop Evidence | — |
| EXP-0093 | 0.7.25 | Accepted | Long Retry Resource Placement | — |
| EXP-0094 | 0.7.25 | Accepted | Combined User Completion Header and Tail Envelope | — |
| EXP-0095 | 0.7.25 | Accepted | Production Binding Refresh | — |
| EXP-0096 | 0.7.25 | Accepted | Cadenced Latch Authority Under Ready Backpressure | — |
| EXP-0097 | 0.7.25 | Accepted | Funded User Action Fee-Path Coverage | — |
| EXP-0098 | 0.7.25 | Accepted | Crossing Executed-Path Resource Coverage | Retired mechanism |
| EXP-0099 | 0.7.25 | Accepted | Continuation Authority Ownership Audit | — |
| EXP-0100 | 0.7.25 | Accepted | Existing-Path Backpressure Safety Audit | — |
| EXP-0101 | 0.7.25 | Accepted | Reactive Target Residual Convergence | — |
| EXP-0102 | 0.7.25 | Accepted | Production Constraint Disposition | — |
| EXP-0103 | 0.7.26 | Accepted | Crossing Prepass Materialization Rate Ownership | Retired mechanism |
| EXP-0104 | 0.7.26 | Accepted | Crossing Materialization Constraint Reconciliation | Retired mechanism |
| EXP-0105 | 0.7.26 | Accepted | Crossing Materialization Candidate Admission | Retired mechanism |
| EXP-0106 | 0.7.26 | Measured | Crossing Placed Pair-Owner Fallback Candidate | Retired mechanism |
| EXP-0107 | 0.7.26 | Accepted | Crossing Pair-Fallback Admission-Ladder Correctness | Retired mechanism |
| EXP-0108 | 0.7.26 | Accepted | Crossing Pair-Fallback Resource Containment | Retired mechanism |
| EXP-0109 | 0.7.26 | Accepted | Crossing Pair-Fallback Bounded Claim Closure | Retired mechanism |
| EXP-0110 | 0.7.26 | Accepted | Crossing Pair-Fallback Controlled Candidate Comparison | Retired mechanism |
| EXP-0111 | 0.7.26 | Accepted | Retained Pair-Fallback Implementation Convergence | — |
| EXP-0112 | 0.7.26 | Accepted | Retained Cohort Binding Decision | — |
| EXP-0113 | 0.7.26 | Accepted | Crossing Admission-Selection Weight Ownership | Retired mechanism |
| EXP-0114 | 0.7.26 | Accepted | Crossing Admission-Selection Owner Binding Correction | Retired mechanism |
| EXP-0115 | 0.7.26 | Accepted | Final Production Cohort End-to-End Comparison | — |
| EXP-0116 | 0.7.26 | Accepted | 0.7.26 Campaign Efficiency Frontier and Disposition | — |
| EXP-0117 | 0.7.26 | Rejected | Qualified Tick Drain Leg-Split Proof Share | — |
| EXP-0118 | 0.7.26 | Rejected | Deadline-Order Multi-Way Cursor Deep-Drain Proof Share | — |
| EXP-0119 | 0.7.26 | Accepted | Final Converged-Tree End-to-End Comparison | — |
| EXP-0120 | 0.7.27 | Accepted | Current-State Co-Access and Initial Stable Geometry | [EXP-0161](./EXP-0161.md) |
| EXP-0121 | 0.7.27 | Accepted | Autonomous Discovery and Activation-Check Boundary | [EXP-0161](./EXP-0161.md) |
| EXP-0122 | 0.7.27 | Accepted | Single-Residence Return Policy | [EXP-0161](./EXP-0161.md) |
| EXP-0123 | 0.7.27 | Accepted | Complete Park Certificate and Wake Plan | [EXP-0161](./EXP-0161.md) |
| EXP-0124 | 0.7.27 | Accepted | Negative Work, Fairness, and Griefing Budget | [EXP-0161](./EXP-0161.md) |
| EXP-0125 | 0.7.27 | Accepted | One Concrete Live/Sleep/Park/Pending Carrier | [EXP-0161](./EXP-0161.md) |
| EXP-0126 | 0.7.27 | Accepted | Immutable Block Round and Persistent Ring Frontier | [EXP-0161](./EXP-0161.md) |
| EXP-0127 | 0.7.27 | Accepted | Monotone Dependency Acknowledgment and Lost-Wakeup Closure | [EXP-0161](./EXP-0161.md) |
| EXP-0128 | 0.7.27 | Accepted | Atomic Residence Transfer and Saturation Recovery | [EXP-0161](./EXP-0161.md) |
| EXP-0129 | 0.7.27 | Accepted | Revocation Status and Authorized Revival | [EXP-0161](./EXP-0161.md) |
| EXP-0130 | 0.7.27 | Accepted | Generation-Safe Bounded Cleanup | [EXP-0161](./EXP-0161.md) |
| EXP-0131 | 0.7.27 | Accepted | Retained-State Collateral and Reclamation Debt | [EXP-0161](./EXP-0161.md) |
| EXP-0132 | 0.7.27 | Accepted | Legacy Scheduler Elimination and Isolation Map | [EXP-0161](./EXP-0161.md) |
| EXP-0133 | 0.7.27 | Accepted | Complete Generated Resource-Domain Ownership Matrix | [EXP-0161](./EXP-0161.md) |
| EXP-0134 | 0.7.27 | Accepted | Composed Supported-Domain Soundness Matrix | [EXP-0161](./EXP-0161.md) |
| EXP-0135 | 0.7.27 | Accepted | Current-State Actors Physical Architecture Closure | [EXP-0161](./EXP-0161.md) |
| EXP-0136 | 0.7.27 | Accepted | Current-State Actors Workload and Materiality Freeze | [EXP-0162](./EXP-0162.md) |
| EXP-0137 | 0.7.27 | Accepted | Current-State Actors Physical-Choice Sufficiency Ledger | [EXP-0161](./EXP-0161.md) |
| EXP-0138 | 0.7.27 | Accepted | Deadline Destination Vacancy Authority | [EXP-0163](./EXP-0163.md) |
| EXP-0139 | 0.7.27 | Measured | Parked Two-Clock Close Weight | [EXP-0163](./EXP-0163.md) |
| EXP-0140 | 0.7.27 | Measured | Service-Resident Deep-Tick Close ProofSize | [EXP-0163](./EXP-0163.md) |
| EXP-0141 | 0.7.27 | Measured | Complete Temporal Deadline Owner | [EXP-0163](./EXP-0163.md) |
| EXP-0142 | 0.7.27 | Measured | Suspended Deadline-Resident Two-Clock Close | [EXP-0163](./EXP-0163.md) |
| EXP-0143 | 0.7.27 | Measured | Observation Fanout Maximum-Header Page | Retired mechanism |
| EXP-0144 | 0.7.27 | Measured | Fanout Nominal Share Versus Shared Materialization Budget | Retired mechanism |
| EXP-0145 | 0.7.27 | Measured | Actor Control Materialization Gate For Full-Header Fanout | Retired mechanism |
| EXP-0146 | 0.7.27 | Interpreted | Observation Fanout Terminal Selector Reachability | Retired mechanism |
| EXP-0147 | 0.7.27 | Measured | ObservationChange Full-Header Trigger Fee Owner | Retired mechanism |
| EXP-0148 | 0.7.27 | Measured | AddressEvent Full-Header Trigger Owner | [EXP-0164](./EXP-0164.md) |
| EXP-0149 | 0.7.27 | Measured | Manual Full-Header Trigger Admission | [EXP-0164](./EXP-0164.md) |
| EXP-0150 | 0.7.27 | Rejected | Four-Recipient SplitTransfer User-Ingress Weight Falsifier | [EXP-0165](./EXP-0165.md) |
| EXP-0151 | 0.7.27 | Rejected | Inline Expiry Close in Four-Leg SplitTransfer Effect | [EXP-0165](./EXP-0165.md) |
| EXP-0152 | 0.7.27 | Inconclusive | Active Native-Conversion Recipient in Two-Leg SplitTransfer | [EXP-0165](./EXP-0165.md) |
| EXP-0153 | 0.7.27 | Inconclusive | Observation Fanout Quantum Under One-third Actor Control | Retired mechanism |
| EXP-0154 | 0.7.27 | Inconclusive | Bounded Observation Fanout in the Mandatory Runtime Prepass | Retired mechanism |
| EXP-0155 | 0.7.27 | Rejected | Proportional Fixed Queue Reserve in a Larger Proof Block | — |
| EXP-0156 | 0.7.27 | Inconclusive | Crossing Placed-Pair Late-Fee Rollback Owner | Retired mechanism |
| EXP-0157 | 0.7.27 | Rejected | Crossing Skip-Pair Default Reservation Screen | Retired mechanism |
| EXP-0158 | 0.7.27 | Rejected | Scalar Crossing Skip Weight Owner | Retired mechanism |
| EXP-0159 | 0.7.27 | Rejected | Full-Page Crossing Skip-Pair Probe | Retired mechanism |
| EXP-0160 | 0.7.27 | Rejected | Maximum-Header Crossing Pair Fee Rollback | Retired mechanism |
