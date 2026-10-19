# Actors Experiments

The Actors track root: charter, canonical best verified results, current records, release hazards and retained principles. The tables below own the best-result summary; records/raw artifacts own detailed evidence, and `BACKLOG.md` owns open work.

| Field | Value |
| --- | --- |
| Track ID | actors |
| Status | Active |
| Scope owner | DEOS Actors physical execution, storage, scheduling and control topology |
| Depends on tracks | None |

## Scope and Boundary

- `Owns`: Actor Contract and Step representation, hot/cold state, run state, service ring and temporal topology, detector geometry, block service, resource allocation and executor lowering.
- `Excludes`: Normative Actor semantics, adapter-internal execution, Router-internal route selection, open work, raw datasets and per-experiment causal interpretation.

## Governing Invariants

- Preserve deterministic semantics, strict service order, one causal hop per block, atomic Step effects, User/System neutrality, bounded state, component-wise Weight, No Ceiling Tax and production Weight soundness.
- Observations never activate Actors; they are read only by fresh-only Step predicates at the check or Attempt.
- Governing contracts: `template/pallets/actors/docs/specification.en.md`, `docs/deos/actors-resource-policy.specification.en.md`, `docs/deos/actors-performance-assurance.specification.en.md`.

## Current Baseline

[EXP-001](./EXP-001.md) owns the physical baseline and decision evidence: one stable Actor-generation process, an actor-keyed Live/Pending ring, retained C32 deadline pages with paged heaps, revisioned parked-balance wake, immutable block rounds, transactional residence and synchronous reclamation. The [assurance ledger](assurance.md#inherited-exp-001-assurance-claims) owns inherited O16–O38 dispositions and generated-owner inventory. Decision scope remains physical architecture only; complete production bounds and throughput remain open release work.

## Best Verified Results

These tables are the canonical source of truth for achieved best results in this track. The best-result columns explicitly report `SuccessfulEconomicSteps / DistinctActorsWithSuccessfulEffects` per block, not a redefinition of normative `S / A`. Performance-assurance §1.1 retains `S = CommittedSteps` and `A = DistinctActorsCommitted`; the counts coincide in these all-successful T1/R2 profiles. Future reports also separate `ActorsConsidered` and typed failures, skips and suspensions. Each result qualifies only for its declared workload, validation tier and snapshot; it is not an absolute execution ceiling, sustained capacity, current-worktree attestation or release acceptance. Unverified counterparts stay explicitly empty, not inferred from Weight or weaker evidence.

The current practical coordinate is 128 prefunded one-Step Mutable Actors, Manual readiness at block 2 and the first eligible complete block 3; System and User are separate populations. Reference Actors/hooks, production resource policy and ordinary Q1/B+1 remain. `C` and `E` are settled Actor Control/effect Weight, each written as `[RefTime ps, ProofSize bytes]`. CPU, backend I/O and whole-block latency are unavailable in these records; proof data and further context remain with the evidence owner.

| Profile | Class | Best verified exact-Wasm successful Steps / effect Actors | Control C | Effect E | Evidence / applicability |
| --- | --- | --- | --- | --- | --- |
| Native Transfer, no predicates, shared live BOB recipient | System | 76 / 76 | [195666461000,2424197] | [111228508000,470896] | T1; historically verified first block, not sustained/current-tree capacity |
| Native Transfer, no predicates, shared live BOB recipient | User | 63 / 63 | [182663406000,2425065] | [92202579000,390348] | T1; historically verified first block, ordinary User fees retained |
| Foreign → Native Router purchase + four actor-local true balance predicates | System | 50 / 50 | [123107448050,2436037] | [171189200000,962650] | R2; exact production-Wasm first block, 78 Actors remain pending |
| Foreign → Native Router purchase + four actor-local true balance predicates | User | 44 / 44 | [122633473124,2434005] | [150646496000,847132] | R2; exact production-Wasm first block, 84 Actors remain pending |

R2 promotes the formerly native-only Router results after separate 128-Actor native author/import and exact production-Wasm proof/root validation. The Task is bounded exact-output `SwapOut`, Native output one ED, Foreign input cap ten ED, 5% slippage and actual `DirectXyk`; its AND clause reads four distinct sovereign balance keys. All 50/44 served Actors have successful swaps, independent custody/fee/hold/FIFO/Q1 checks and zero skipped/failed Steps. R0/R1 remain supporting conformance/diagnostic evidence, not additional best rows. No result establishes sustained capacity or a universal admission ceiling.

| R2 class | Execution proof bytes | Compact execution bytes | Verification proof bytes | Measured Pipeline fee, Native units | Measured Action fee, Native units |
| --- | --- | --- | --- | --- | --- |
| System | 92085 | 66227 | 1627211 | 0 | 0 |
| User | 91154 | 65729 | 1626280 | 2600050772 | 1205171968 |

Verification proof includes runtime code; these are fixture proof metrics, not network PoV or a sum of per-key proofs. Manual Trigger fees belong to setup block 2 and are excluded from measured Pipeline/Action totals, not waived. Each R2 route spends `1005025126` Foreign units, including Router fee `5025125`, for `1000000000` Native units. Actor fees above are separate from that Foreign trading fee. Historical Control ProofSize remaining is `63172/65204` bytes; those exact-Wasm runs did not observe the rejecting guard. The separately labeled native follow-up below now explains the denominator and local guard, without changing the historical validation tier. No CPU/backend/whole-block-time metric is inferred.

- `T1`: `template/target/actors-transfer-baseline/control-owner-first/summary.json`, SHA-256 `f6caa99512379acdc5d05d00c82f77a705abd13bce6e12651163e5d2deec937a`; exact production Wasm `204a76db7a1022b6b36b460fb0c6366d77c2a70b46c0adcfb6d2b37bb20e28eb`; Weight `21f23afdebd532a71b4ca6e11098087b7658778ea57870969a24071f8de704f6`. First-block success counts are observed, not a proven admission ceiling; the actual next rejecting guard was unobserved. These bytes are historical, not the later `8cc3b938…` production artifact.
- `R2`: `template/target/actors-router-predicate-baseline/first-summary.json`, SHA-256 `925feefdf5c25e78a5248d6dbbb080fd1b27c93895021cdb8e977d46c6410f07`; source stream `56ee2110dca36ad41abe9d094bd158c7cad6f931748d076cdc00768f9359d403`, accepted Wasm `8cc3b938fcbe720ed328a9546d9ba2c107026cbe71a00b9b86e68d70d15298fb`, Weight `21f23afd…`, metadata `0c56d808…`. Exact-selection logs `wasm-system-first-lib.log` / `wasm-user-first-lib.log` have SHA-256 `af6be188…` / `06698277…`; full identities, quotes, routes and terminal ledgers remain in the summary/raw evidence. Each profile executed once; no favourable extension, repricing or production-source change. T1 and R2 are distinct verified snapshots, not an implicitly matched current-tree A/B.
- `R0`: `template/target/actors-router-predicate-baseline/short-validation.json`, SHA-256 `b5115399588cc95e1ef47b13e48e4d474bc8aa00075edb84460d02539972159d`; source stream `4f8be19555dacb8c140107c869f340dffe812567c3674b8b4e4a5dead709d904`, accepted Wasm `8cc3b938fcbe720ed328a9546d9ba2c107026cbe71a00b9b86e68d70d15298fb`. Both two-Actor class controls replay with proof/root agreement and successful purchases; they establish fixture/replay conformance, not a best 128-Actor record.
- `R1`: `template/target/actors-router-predicate-baseline/native-profiles-first.log`, SHA-256 `cafe19dbc1aa601e90481d51ce6d9297e407db68d358ac43693e5055ae34a0e2`; parsed projection `native-first-observation.json`; workload freeze `preparation.json`, SHA-256 `35a23332343c3f7376ebe4f59c1231a5321433896c62fd94081e621e6e30e75b`. Native diagnostics used the unchanged Weight identity above; no production Wasm was executed. Do not repin these observations to later source or promote them on compilation alone.
- `Readiness limitation`: The separate 1,024-Actor cadence campaigns observed one Transfer Step/Actor per measured block, in an unwarmed readiness-source-limited window. Those are not saturated execution records and cannot compete with the prepared first-block coordinate. Their evidence remains in `template/target/actors-transfer-baseline/{system-recurring,user-recurring}/summary.json`.
- `Promotion rule`: Update the best table only after a qualifying run for the same coordinate; keep different budgets, phases, hosts, Weight identities or validation tiers separate. Do not cherry-pick a favourable block from a longer window or average System/User and light/heavy work. The original stale-authority correctness/ownership gate remains open; none of these records certifies the eventual corrected kernel.

## Optimization Diagnostics

These native/benchmark-Wasm counterfactuals are not capacity records, promoted production-Wasm results or selected optimizations.

| Coordinate | Measured result | Evidence / status | Limit |
| --- | --- | --- | --- |
| Native H1 A4/B4 Pipeline collection; seven User workflows plus eight refusal/recovery controls | 23 logical Head Gets omitted; 4585 value / 1288 key bytes; roots, fees and both Weight axes unchanged | EXP-003 O4/O5; `template/target/actors-pipeline-fee-read/comparison.json` `6430dd80…`; finite native qualification | B4 unretained after O6 rejection; no CPU/I/O/PoV/throughput inference |
| Benchmark-Wasm zero-Step User Service, five matched scalar pairs | Primary lower in 1/5; median paired B4-minus-A4 +3352.5 ns versus A4 range 5098 ns; protected Running threshold crossed | EXP-003 O6; `template/target/actors-pipeline-fee-read/timing/summary.json` `368ae029…`; A4/B4 `e21a9baf…` / `290e6be5…` | Rejected under frozen rule; RefTime cause Inconclusive; A4 restored, no repricing |

## Native Resource-Frontier Follow-up

P28-25's bounded native diagnostic reuses R2's deterministic fixture and unchanged production owners. Actual frozen Control is `[303230282388,2499209]`, not the isolated third: the configured fixed envelope `[1090309152836,2988132]` is subtracted before allocation. This exactly explains the `996044` proof-byte denominator difference, not reclaimable capacity. System/User heads65/59 are eligible and fail `service_canonical_round_head_inner`'s effectful Control-proof guard: required114141, Prepass available88236/90268; RefTime, combined meter and next effect fit. Drain refuses the same heads; both progress in block4.

Evidence: `template/target/actors-router-predicate-baseline/frontier-summary.json` (`f24c299b…`), final paired plain/instrumented source manifests `8a719af7…` / `2d8468bb…`, unchanged Weight `21f23afd…`; instrumented/plain native root/resource/economic streams agree and diagnostic kernel edits are removed. An initial build-seal gap was explicitly requalified once without changing workload or guard results; final source/binary seals support native diagnosis, not a new production-Wasm guard proof. Blocks3/4 yield50+52 System and44+46 User successes, leaving26/38; no full-horizon average or new best row is promoted. The integration owner retains full ledger/phase attribution. The fixed envelope exceeds the normative quarter and the provider retains maxima without settled-prefix credits: P0/P28-19 owns this reference-policy nonconformance, not an inferred physical overrun or free reserve. P28-26/27 own remaining horizon and matched causal work.

## Supporting Assurance

The single [resource-assurance conclusions document](assurance.md) owns inherited O16–O38 claim dispositions and the generated-owner inventory, and compiles O41, I/K, fixed-prefix, Authorship and native-validation conclusions with explicit evidence limits. No historical snapshot or companion manifest is retained. It supports existing work and EXP-001 without becoming a fourth Experiment or changing the baseline; canonical open work and production qualification remain separate.

## Current Records

| ID | Status | Decision | Consumers |
| --- | --- | --- | --- |
| [EXP-001](./EXP-001.md) | Accepted | Current-state architecture, C32 vacancy, pull-model amendment and O15 companion placement; selectors installed, complete qualification open. O39/O41/[O42](./EXP-001.md#protocol-ticket-ordered-index-comparison) remain Proposed/unadmitted; O40's uniform-floor repetition remains Rejected. Assurance claims and diagnostic details have separate owners, not new physical decisions | Baseline; `Actors/P28-12`; `Actors/P28-15`; `Actors/P28-16` |
| [EXP-002](./EXP-002.md) | Accepted | Logical Put elision with matched H1 truth; O3 paired elapsed-body comparison closed Inconclusive, no CPU/speedup claim | `Actors/P28-12`; `Actors/P28-15`; `Actors/P28-18` |
| [EXP-003](./EXP-003.md) | Rejected | O1–O3 closed Inconclusive; Pipeline O4/O5 native truth passes but O6 fails complete-Service/protected criteria; A4 restored, no further sampling | `Actors/P28-12`; `Actors/P28-14`; `Actors/P28-18` |

[EXP-002](./EXP-002.md) selects equality-gated semantic submission against frozen B28-core: the fresh read/expected-value validation remains, while redundant Puts disappear with identical finite H1 truth. Its current-production-Wasm checks are scoped regressions; CPU, proof, broad-suite and release acceptance remain separate.

Weight-coverage findings (retained owners underbounding selected geometries) are not experiments; their witnesses live in the owning `BACKLOG.md` items until coordinated generation dominates them.

## Release Hazards

Routes remaining release-facing review; it does not reopen accepted physical choices.

| Hazard | Current evidence | Smallest release falsifier | Owner |
| --- | --- | --- | --- |
| Legacy execution authority | One sparse `ActorRunState`; no Opening snapshot types; canonical tests reject legacy Ready/Waiting authority | A supported path reads snapshot, Ready/Waiting, successor-ticket or deferred-Cycle state as authority | `Actors/P28-23` |
| Round reentry and duplicate residence | Canonical regressions cover the persistent ring | A trace recaptures a turn, loses the frontier, or leaves zero/multiple residences after success, refusal or rollback | `Actors/P28-23` |
| Lost wakeup and stale acknowledgment | Historical W6 evidence used observation-only Park and no longer applies; parked-balance revision regressions pass natively | A callback, coalesced revision, timed review or saturation trace acknowledges beyond its covered revision or loses a newer obligation | `Actors/P28-23` |
| Deadline phase and recovery | Historical W7 met no-growing debt with censored recovery; the restored mandatory stage is not bound by any Weight/Wasm | Native phase tests substitute for production binding, or censored recovery is promoted to completion | `Actors/Mandatory Deadline Service` |
| Close and replacement isolation | Synchronous close preserves peers, custody and holds; try-runtime inverse ownership and corruption checks pass | Close or replacement leaks authority or custody, or validation accepts an ownerless carrier | `Actors/P28-23` |
| Complete resource ownership | Success/rollback accounting and terminal admission reconcile natively; selected witnesses in the Service Weight backlog items show underbounded close, Trigger and SplitTransfer owners | One reachable branch lacks a complete owner, or incurred work vanishes on rollback | `Actors/Service Weight Ownership` |
| Evidence identity | Control identity fingerprints consumed User header profiles; no current tree number has inherited authority | A release claim uses stale source, Weight, metadata, vector, ABI, Wasm or workload evidence | `Actors/P28-23` |

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
