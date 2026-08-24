# DEOS Framework Delivery History

> Canonical delivery history for the standalone DEOS repository line, restarted at `0.0.0`.
>
> Each release keeps at most 8 outcome records of at most 512 characters.

## 0.7.24: Topology Scaling and Block-Paced Execution

- `Actors / Block-Paced Execution`: Added the mandatory payload-free Actor Prepass, fixed prepass cutoff, explicit block phases, one committed Step per Actor per block, and `N -> N+1-or-later` causal eligibility while preserving strict class-neutral FIFO and progress-preserving Continuation.
- `Runtime / Economic Zipper`: Partitioned schedulable Weight component-wise into a one-third Actor Control ceiling and equal Actor-effect/user base turns over Shared Economic capacity, with work-conserving idle borrowing, maximum reservation, valid-actual reclaim, independent RefTime/ProofSize fragmentation, three Mandatory bases including Actor Prepass, fail-closed reconciliation, and reserved-envelope telemetry that is not mislabeled as actual usage.
- `Actors / Active Frontier`: Bounded production identity and Active Actor capacity at 10,000 without dormant scans; separated hot scheduling/run authority from cold Contract bodies and Step chunks; retained the 32-Step ceiling after measured C24/C16 sweeps; and added exact active-lifetime User RunState holds.
- `Actors / Trigger Scaling`: Retained useful-transition-only charging and latching, authoritative re-arm, P128 Crossing membership and tail/preflight, N64 non-tail work, independent P64 broad fanout, bounded family rotation, and one active plus one deferred Pipeline without stale or duplicate semantic authority.
- `Actors / Evidence and Assurance`: Added fee-free prepaid apoptosis for underfunded one-shot AtTime and unfundable retries, with per-attempt maximum liability and actual-cost settlement; regenerated Weight, vectors, metadata, descriptors, ABI and bounds; completed mixed 10,000-Actor traversal at block 679 with zero failures; and passed package/runtime, heavy scaling, Clippy, no-std, TryRuntime, embedding, identity, provenance and threat gates.
- `Asset Conversion / Trust Closure`: Separated semantic and physical asset identity; rejected ledger aliases at the pool boundary; made Router the only public XYK and atomic permissionless pool-lifecycle surface; removed post-dispatch LP repair; integrated full-balance reserves; separated fee domains; and proved pool/LP/Oracle rollback and bidirectional integrity.
- `Architecture / Experiments`: Closed Step-centric memory, Contract-ceiling, allocation, and active-lifetime hold experiments; retained direct FIFO/A0 where gains were sub-material; excluded external successor staging by interleaved-order proof; embedded all CSV/TSV evidence in owning records; and made retained delimited evidence fail validation.
- `Documentation / Wiki`: Synchronized Actors, Oracle, Router, embedding, resource-policy, and integration truth; projected Economic Zipper, block pacing, Hot/Cold Contract, active frontier, and state holds into strict bilingual OKF pages; and passed structural, graph, localization, and independent semantic-parity review.

## 0.7.23: Reactive Topology Closure and Cohort Throughput

- `Actors / Declarative Trigger Topology`: Moved Crossing phase/revision and cadence into canonical `ActorHot` Trigger state, reduced locators to generation-checked physical authority, and routed create/activate/replace/deactivate/close/genesis through one transactional transition compiler with exact no-op, full family matrix, rollback, fresh-genesis storage version 9, and refundable family-based User bonds while System Actors remain exempt.
- `Runtime / Lifecycle and Ingress Weight`: Generated worst-case lifecycle owners for Crossing install/remove/replace/cleanup, retained maximum-call dispatchability and signed transaction-extension evidence, and made host-composed Oracle publication own its complete synchronous broad/Crossing consequence exactly once across empty, combined, queued, refresh, Router, capacity-rejection, and rollback paths while deferred materialization stays independent.
- `Actors / Branch-Exact Crossing`: Replaced composite worker charging with read-only work plans, staged probes, transaction-time revalidation, exact counters, and fixed-depth radix search charged only without a retained threshold at `127,253,000 / 81,886`; no-match, rearm, skip, coalesced, placed, terminal, corruption, and meter-deferral paths now admit and consume only their reachable multidimensional owner.
- `Actors / Cohort Throughput`: Added bounded generation-checked cohort and aggregate FIFO authority for up to four homogeneous candidates, tail/non-tail stable compaction, physical-tail refill, grouped split-destination writes, exact locator repair, one queue/cursor commit, queue-boundary and resumption evidence, and scalar fallback; generated emptied/trimmed non-tail owners are `601,343,000 / 81,886` and `603,089,000 / 81,886`.
- `Actors / Materialization and Capacity`: Unified wakeups, Crossing, and broad fanout under deterministic rotated grants, protected minima, bounded second-pass lending, explicit repairable worker faults, and a positive FIFO execution floor; bound each feed to 9,000 User/10,000 total memberships, four homogeneous candidates/block, 2,250/2,500-block uncontended horizons, 10,000 placements, and 64 queued transitions without SLA claims.
- `Actors / Assurance and Security`: Added a pure Trigger/lifecycle model, randomized state-machine and exact-event equivalence, comprehensive corruption and fault-injection matrices, checked capacity/bond arithmetic, stale-authority and exact-root rollback, paused/breaker oscillation, queue pressure, mixed-family fairness, and 10,000-Actor no-match/herd convergence; reorganized tests by domain while preserving minimized proptest regressions.
- `Runtime / API and Client`: Bumped Actor eligibility API to version 4 and exposed semantic Trigger state, placement, bounded faults, capacity, and runtime-owned bond policy without physical topology; regenerated metadata, PAPI descriptors, ABI/bounds, fee/bond and ingress/observation evidence, and client authoring for hysteresis, no-retrofire, typed capacity failure, coalescing, broad-fanout cost, and queue backpressure.
- `Packaging / Documentation / Release`: Updated independent embedding, no-default native/Wasm and TryRuntime contracts; pinned Rust 1.94.1 and removed linker workarounds/`--allow-undefined`; synchronized Actors/Oracle specs, package and DEOS integration architecture, bilingual strict-OKF Wiki, generated Weight/capacity/delta evidence, release audits, and the explicit no-`0.7.22`-migration fresh-genesis boundary.

## 0.7.22: Reactive Topology and Relevant-Work Scaling

- `Validation / Remote Gate`: Reduced remote CI to one stale-cancelled pull-request `validation-gate` running the deterministic heavy project profile; removed duplicate `main` and version-tag jobs while retaining pinned toolchains, lockfiles, local full release acceptance, and the project-versus-Skill dependency boundary.
- `Release / Fresh Genesis`: Made the `0.7.22` fresh-genesis-only boundary explicit for Actors, Governance, the composed runtime, and local release operation, with release-line auditing that rejects missing boundary statements and no claim of `0.7.21` upgrade support.
- `Governance / Dispatchability`: Split typed payload validation into a deposit-backed compact hash/domain/kind admission witness consumed on successful submission and a separately measured enactment read, added per-kind byte ceilings and stale-witness rejection, generated production weights, and proved the maximum signed Root action through the real transaction-extension and `CheckWeight` path without changing block limits or dispatch class.
- `Governance / Atomicity`: Proved pre-fee rejection for witness/status, author/domain-capacity, and maturity pressure; exact rollback after post-fee authorship failure; state-preserving witness refresh failure; and signed enactment of only the exact preimage bytes selected by committed proposal hash even when a competing valid payload exists.
- `Runtime / Dispatchability Matrix`: Added a 60-family matrix for every public DEOS custom-pallet call at maximum bounded input against Normal `max_extrinsic` and `max_total`; it exposed and closed witness preparation's 4.20 MB proof defect by validating hash-bound 262-byte call data against compact preimage status, with regenerated preparation Weight of `22,419,000 / 3,556`.
- `Release / Canonical Ref Audit`: Extended the local release-line audit to reject a plain version ref beside any canonical `vX.Y.Z` tag and to bind the prepared tag, local `main`, and an explicitly supplied locally validated commit/tree without hidden CI or GitHub state.
- `Actors / ObservationCrossing`: Added exact Rising/Falling hysteresis semantics, no-retrofire Active initialization, lossless revision-ordered Oracle transition queues, sparse exact-threshold radix pages with generation-checked ownership and cursor-safe compaction, fair bounded continuation, exact terminal source cleanup, fresh-genesis reconciliation, atomic publication rollback, maximum-herd evidence, and production-generated multidimensional worker weights.
- `Actors / Activation Topology`: Unified all five detectors through one activation sink and FIFO; added a host-owned ranked DEOS System activation DAG with pre-commit validation and descriptive projection; preserved paid, same-block-bounded User cycles with economic apoptosis; carried loaded canonical state through scheduler, control, ingress, simulation, and read classification; rebalanced worker reserve to admit one complete Crossing unit; and regenerated release evidence.

## 0.7.21: Runtime Truth Closure

- `Actors / Canonical State`: Added one five-partition loader for absence, dormancy, active state, Continuation ownership, and corruption; routed execution, simulation, reads, controls, scheduler placement, wakeups, fanout, and try-state through it; removed partial probes and closed malformed-state admission with typed failure and production-measured Weight.
- `Actors / Scheduling and Ingress`: Preserved strict FIFO through mixed block/tick wakeups, live-head stalls, retries, and terminal index exhaustion; exactly invalidated a cancelled Continuation's wakeup before signal re-prime; separated ten-year clock horizons; made fee collection ledger-only; and certified each movement protocol with explicit preflight, consequence, rollback, provenance, and Weight ownership.
- `Governance / Bounded Truth`: Replaced unbounded epoch catch-up with one chronological persisted four-phase service path and measured family caps; made reward, tally, deadline, projection, and epoch arithmetic checked; and reconciled concurrent transferable vote power through one aggregate source-custody position with exact rollback and maximum-horizon release.
- `Runtime / Arithmetic and Atomicity`: Widened authoritative Router, TMC, Staking, Oracle, Actors, and Governance calculations; retained saturation only for explicit floors, conservative Weight caps, and telemetry; and added exact-root rollback evidence for task adapters, routing, pool indexing, mint distribution, staking, Governance terminal actions, and XCM deposits.
- `Runtime / Identity and Authority`: Reserved host-owned XCM locations, enforced the single-asset holding bound, kept teleport and arbitrary execution closed, replaced truncating generic custody derivation with host mappings, proved custody non-aliasing, and closed every privileged origin against ordinary signers while retaining only typed Governance and feed-local Oracle exceptions.
- `Security / Release Assurance`: Added panic-surface and full-iteration audits, exact bootstrap identity checks, production weights for all seven custom pallets, and an independent release-assurance Skill for dated dependency reachability, multidimensional Weight deltas, threat-boundary review, artifact identity, and attestation preparation.
- `Client / Documentation`: Regenerated metadata, descriptors, Actors ABI/bounds, ingress/observation evidence, and fee vectors; made eligibility display reject unknown runtime variants; and synchronized package, embedding, integration, framework architecture, and bilingual Wiki truth with the shipped runtime boundaries.
- `Validation / Release Evidence`: Added heavy pull-request/`main` and full version-tag CI topology; made full regeneration compare exact worktree content; made clean client validation prepare Svelte state and generate PAPI descriptors once without recursive install hooks; retained production-Wasm benchmark provenance; and prepared checksummed, attested release artifacts.

## 0.7.20: Execution Hardening

- `Actors / Weight Truth`: Corrected lifecycle under-measurement by benchmarking worst-case scalar trigger filters, recycled subscription slots, and middle dirty-list unlinking; priced the full System-swap Oracle/reserve guard and competing route candidates. Final affected lifecycle charges rise 1.28x–3.00x in RefTime and up to 3.8x in ProofSize over 0.7.19, while both System swap envelopes include the omitted reference-path read.
- `Actors / Execution and Fee Sink`: Contracted each Actor to one trigger and one FIFO pipeline; added bounded block/timestamp-tick wakeup heaps under one coordinator; and made cadence use 500 ms consensus ticks with ceil activation, floor readiness, no catch-up, and genesis anchoring only after the first consensus timestamp. Fee collection stays ledger-only; Fee Sink processes 10% above the per-leg ED threshold every 120 ticks before the current 50/50 allocation.
- `Runtime / Governance Hardening`: Made stake/unstake and ballot-terminal resolution atomic; normalized protection power proportionally with `U256` and `7x` headroom. Added aggregate `$VETO`/receipt custody that reuses locked power across concurrent proposals, admits only newly free units, and releases after the maximum horizon. The measured 256-voter terminal path now charges `1,124,323,000 / 656,094` and `269 / 271` DB; release charges `68,166,000 / 6,208` and `5 / 5`.
- `Actors / Benchmark Harness`: Repaired host-generic mixed-FIFO and circular-chain benchmark tests, asserted exact ticket release for every executed cohort actor, and moved contract rewrites past the per-actor control-mutation clock so benchmark compilation and execution can decide production evidence.
- `Packaging / Host and Evidence Contracts`: Marked all six packaged `weights.rs` implementations as unmeasured estimates and required host-generated production weights in embedding guides; documented System-owner authority and bounded reference-unavailable swap retry. Replaced subjective Wiki signals with explicit evidence and made project-scoped teardown portable to macOS without widening process ownership.
- `Router / Boundary and Pool Identity`: Replaced native-width saturating price products with `U256`; equal reserves near `u128::MAX` published `1` instead of `PRECISION`, poisoning references. Prevented pool creation from surviving failed post-dispatch LP/Oracle indexing: `PoolIndexExtension` now preflights the LP token, reverse-index admission, and both feed identities/capacity before dispatch. The regression previously returned `InvalidTransaction` while retaining the unindexed pool.
- `TMC / Boundary Arithmetic`: Widened spot products and made the mint discriminant fail closed instead of saturating: high supply could understate a representable ceiling and overflow could turn payment into zero output. Rejected Local/Foreign variants aliasing one ledger ID, required nonzero precision, removed try-state's false burn failure, and regenerated TMC, Router, and Actors host weights.
- `Asset Registry / XCM Identity`: Enforced the foreign mask and rejected retained reverse identities after ledger deletion, preventing misclassification and stale bijection loss. Made existing-asset linkage transactional. Bound `$NTVE` XCM identity to `Here`, routed relay `Parent` reserves through registered foreign `pallet-assets` ledgers instead of native balances, and disabled teleport dispatch consistently with the executor. Added ledger/rollback regressions and regenerated registry weights.

## 0.7.19: Canonical Ownership and Surface Pruning

- `Actors / Contract Root`: Made `ActorContract` the sole authored, stored, simulated, and client contract root; represented dormancy by contract absence; removed schedule/input/state wrappers and field-specific auto-close mutation; and preserved exact no-op, replacement, Continuation, retry, fee, custody, and bounded scheduler semantics.
- `Actors / Runtime Surface`: Replaced phase/next-block reconstruction with canonical eligibility classifications, exposed partition-preserving `ActiveActorState`, made metadata paths follow natural Rust modules without `replace_segment`, and narrowed simulation rollback failure to the actual transaction-depth boundary.
- `Oracle / Governance / Derived State`: Deleted Oracle `FeedCount` and Governance `ActiveProposalCounts`, derived cardinality from bounded canonical ID registries, regenerated production weights, reconciled retained reverse indexes, and consolidated proposal preimage availability into one status carrying optional byte length.
- `Staking / Capability Algebra`: Replaced Cartesian native-security mode/readiness projection with a mode-shaped view, removed synthetic `Inactive` and redundant capability booleans, and retained exact settlement, liability, custody, epoch-plan, and Trusted Set claimability behavior.
- `Router / Client`: Removed raw Oracle/XYK wrappers and blanket dispatch-error classification while preserving quote/prepared/committed/exact-output and cause/retry distinctions; aligned the client to canonical Contract, eligibility, staking, and preimage projections with one bounded status-label improvement.
- `Validation / Automation`: Separated project, package, client, CI, release, and Skill ownership; removed Skill coupling, source-hash manifests, historical replay, corpus wrappers, redundant setup, and obsolete bridges; restored the contiguous atomic network ladder with a checksum-pinned Polkadot/Omni binary bootstrap; and passed full project validation plus independent Alignment, Domain DAG, and Wiki Sync checks.
- `Documentation / Wiki`: Compressed the documentation hub, consolidated randomness policy into Core Architecture, retired the Account Abstraction Actors expansion, removed agent/Skill content from the user Wiki, and reduced duplicated scripts, fixtures, pages, graph nodes, and provenance dependencies.
- `Runtime / Genesis Ownership`: Made named runtime presets the sole owners of complete genesis state, removed post-generation JavaScript economic mutation and override variables, routed outer ChainSpec metadata through `chain-spec-builder`, rejected mismatched para identity, and withheld a Live profile until the runtime owns a production preset.

## 0.7.18: Semantic Compression and Contract Truth

- `Staking / Atomic Expiry`: Contracted native security reward expiry into one measured bounded transition that returns unclaimed and uncredited custody to Fee Sink, reconciles liability, clears all bounded claim markers, and removes the snapshot and pot without an intermediate Expired state or cleanup call.
- `Staking / Retention`: Made session progression settle the oldest overdue epoch, enforced the horizon/current/one-plan bound, canceled unactivated plans after failure or `TrustedSet` contraction, and finalized Open obligations without liability loss; evidence covers four cleanup-free horizons plus partial claims, excess custody, pending unlock, and compound-eligible transition state.
- `Staking / Availability and Event Truth`: Unified calls, readiness, adapters, scripts, and client preparation behind one mode-derived operation classifier; `TrustedSet` preserves claims/exits while rejecting new obligations, and each batch reward event reports sequential post-claim liability rather than the final batch value.
- `Actors / Semantic Contraction and Equivalence`: Canonicalized Actor Contract, Step, Precondition, Predicate, failure/retry, and Fee Sink prose around retained SCALE owners; split four type owners behind one metadata-stable facade; froze public SCALE inventories and compiler-exhaustive eligibility/simulation error seams; added pinned cross-version equivalence plus exhaustive production/simulation parity for every Step transition row; and synchronized the bilingual wiki with fail-closed drift audits.
- `Staking / Surface and Module Contraction`: Unified security and staking views/calls, removed synthetic exposure and the convenience-only account collator-LP aggregate, and grouped pool, custody, security/reward, bounded-view, and invariant mechanics behind one metadata-stable FRAME/storage/public-model facade with fail-closed ownership audits.
- `Governance / Semantic Truth`: Unified admission and fixed/read-only policy, removed constructor-free variants, and contracted lifecycle truth into explicit proposal identity, one shared approval, one finalized record, and receipt/reason-only execution detail; bounded recent history now projects the canonical record without parallel execution queries.
- `Router / Oracle / Surface Truth`: Preserved distinct Router quote, prepared-estimate, committed-outcome, exact-output, route, Weight, error, and local cause/retry facts without Actors coupling; removed the constructor-free `NoMultiHopRoute` duplicate; and kept Oracle current-scalar ownership, typed feed/aggregation/lifecycle reachability, equal-output refresh without revision/hook change, and transactional changed-publication rollback.
- `Validation / Release and Wiki`: Replaced inferred error counts with recursive typed witnesses; contracted validation to direct fast/heavy/full profiles and one pull-request gate; reduced release output to Wasm, metadata, descriptors, five generated semantic/runtime evidence assets, and `SHA256SUMS`; migrated 94 bilingual concepts to strict OKF v0.2 with provenance, 47 IDs, 214 edges, crash-coherent sync, native Russian, and reviewed graph labels.

## 0.7.17: Protocol Coherence and Native Security

- `Staking / Native Security`: Completed one SessionIndex identity across planning, funding, claims, and client views; added immutable pots, compound settlement, production weights, single-owner adapters, and ballots frozen against yield, custody, and later participation changes.
- `Actors / Contract Identity`: Replaced Actor Program identity with canonical Actor Contract storage, input, read, metadata, ABI, artifact, digest, client, documentation, and generated-evidence surfaces without compatibility aliases.
- `Actors / Preconditions`: Implemented canonical bounded DNF with explicit Opening/Current timing, frozen Continuation results, current-state prior-step visibility, duplicate-clause rejection, exact canonical no-op updates, and full-visit evaluation.
- `Validation / Coherence`: Added completion-gated audits rejecting retired semantic owners, unreserved strategic capacity, inferred rewards, secondary security flags, raw-error retry policy, and placeholder public variants.
- `Artifacts`: Regenerated production Wasm, metadata, descriptors, Actor ABI/runtime evidence, and client projections; package vectors and wiki gates pass, and a second full generation reaches zero tracked drift.
- `Documentation / Ownership`: Reconciled five package specifications and implementation maps with one cross-system closure map binding each public family to its constructor, invariant, executable evidence, and single owner.
- `Validation`: Passed canonical full validation, reproducible production Wasm, Actors assurance, benchmark compilation, finalized two-collator progress/failover/restart, signed transfers, and the composed Router/Oracle/Burn Actor path.

## 0.7.16: Stable2606 Assurance and Pre-Genesis Closure

- `Platform / Identity`: Pinned `polkadot-stable2606-1`, Rust `1.93.1`, Node `22.22.0`, npm `11.7.0`, and workspace `0.7.16`.
- `Governance`: Added primary-eligible signed ingress for protocol `L1RootAction` through the existing proposal lifecycle, with eligibility checked before fees or mutation and no new dispatchable, storage domain, or Root shortcut.
- `Router / Actors`: Preserved typed route and adapter failures through Actor execution, separated failure class from retry policy, failed unknown causes closed, and expanded rollback, recovery, LP-index, and host-pool invariants.
- `Assurance`: Added finalized network, session-transition, composed Router/Oracle/Burn Actor, authorization planning, and deterministic build evidence.
- `Contraction`: Removed receipt-era staking state/calls, duplicate owners, aliases, retired identities, dead scripts, and release-only control surfaces. Active handwritten non-test surface contracts, with two calls and one storage item removed and no public-path growth.
- `Artifacts / Validation`: Regenerated eight weight bridges, Wasm, metadata, descriptors, and projections. A clean commit passes exact `full`, including deterministic build and zero-drift regeneration; benchmark compilation remains isolated and cannot mutate production Wasm.
- `History / Shape`: Compacted every release to no more than 8 concise records and moved implementation diaries, open gates, and subsystem truth back to their canonical owners.
- `Deferred`: Kept cadence profiles, V3 scheduling readiness, staking reward-source abstraction, budget-recipient primitives, unclaimed-reward policy, and permissionless collator rewards outside the frozen 0.7.16 scope.

## 0.7.15: DEOS Router Canonical Identity

- `Router`: Replaced the active Axial Router crate, pallet, runtime, account, client, tooling, documentation, and wiki identities with DEOS Router without compatibility aliases.
- `Evidence`: Regenerated route-class weights, metadata, descriptors, Actors evidence, conformance vectors, and production Wasm against the canonical identity.
- `Validation`: Added a permanent retired-identity audit and passed package, runtime, client, benchmark, network, Actors, and completion gates.

## 0.7.14: Router Route Truth Closure

- `Contract`: Accepted one bounded route-truth specification for exact-input/output intents, deterministic maximum-output selection, typed failures, adapter ownership, and honest market limits.
- `Routing`: Introduced bounded three-asset paths and one prepared-route owner for direct XYK, TMC mint, and Native-anchored candidates with deterministic ties and fresh execution preparation.
- `Safety`: Kept fees, market legs, directional Oracle publication, actual-bound checks, Actor ingress, events, and rollback inside one transaction.
- `State`: Added canonical LP reverse indexing, endpoint and collision checks, `try_state`, runtime host-pool agreement, and corruption matrices.
- `Evidence`: Added an independent embedding runtime, five measured route classes, adversarial vectors, metadata-bound client projection, and cross-domain outcome equality checks.

## 0.7.13: DEOS Actors Canonical Identity

- `Actors`: Replaced the active AAA package, pallet, ABI, runtime, client, tooling, documentation, and wiki identities with DEOS Actors; moved the pallet seed to `deactors` without compatibility shadows.
- `Validation`: Regenerated evidence and passed package, embedding, runtime, client, benchmark, occupancy, production-Wasm, wiki, and completion gates.

## 0.7.12: AAA Semantic Contraction and Contract Realization

- `Contract`: Realized one actor classifier, terminal precedence, lifecycle, simulation, eligibility, event, error, and state contract without pre-launch compatibility shadows.
- `Safety`: Unified User viability, economic apoptosis, fee rollback, Immutable boundaries, and persistent live-head classification without bypassing FIFO order.
- `Evidence`: Regenerated metadata, descriptors, manifests, vectors, production weights, Wasm, client projections, event traces, and drift checks.
- `Validation`: Passed package, external-runtime, runtime, client, benchmark, production, and 10,000-actor occupancy gates while contracting authored surface.

## 0.7.11: AAA Kernel Cooling and Transactional Closure

- `Transactions`: Unified control, execution, fees, scheduling, reactive delivery, and cleanup under transactional ownership while preserving committed prefixes.
- `Scheduling`: Consolidated retry, eligibility, FIFO service, deferral, starvation telemetry, and terminal handling under bounded deterministic authorities.
- `Resources`: Bound service and cache revalidation by runtime capacity with resumable progress and fail-closed admission during cache changes.
- `Economics`: Clarified funding snapshots, protected balances, fee envelopes, task limits, certified ingress, typed observations, and custody domains.
- `Delivery`: Contracted the ABI, moved readiness arithmetic behind runtime truth, regenerated artifacts, and expanded adversarial package, runtime, client, and stress evidence.

## 0.7.10: AAA Runtime Closure and Fail-Closed Release Truth

- `Kernel`: Delivered bounded User/System actors with typed lifecycle, linear plans, deterministic triggers, task-local atomicity, Mutable-only Continuation, owner slots, custody locators, and strict FIFO.
- `Integrity`: Unified queue, wakeup, retry, cleanup, and observation-fanout topology checks so corruption, capacity, arithmetic, fee, and adapter failures preserve exact state.
- `Economics`: Completed funding snapshots, fee reserve, protected minima, exact-once Fee Sink ingress, System fee exemption, sovereign custody, and factual outcomes.
- `Integration`: Added Oracle reactions, certified address ingress, XCM and transaction-extension producers, staking/TMCTOL adapters, runtime APIs, and honest read-model boundaries.
- `Evidence`: Generated production Actors/Oracle weights, Wasm, V16 metadata, ABI, manifests, fee vectors, descriptors, observation evidence, and integration artifacts.
- `Control Plane`: Added metadata-bound authoring, diff, governance composition, matching-Wasm analysis, forecasts, simulation, observation inspection, and fail-closed release audits.

## 0.7.9: Reactive Truth Closure

- `Inspection`: Added finalized feed/actor delivery inspection with exact dirty topology, queue/wakeup admission, revision state, and identity-gated timing estimates.
- `Analysis`: Rebuilt feedback analysis around explicit observation causality, resource coupling, provenance, and fail-closed uncertainty.
- `Evidence`: Added a deterministic 20-scenario reactive corpus covering races, fairness, pressure, rollback, topology churn, retries, bounds, and corruption.
- `Artifacts`: Regenerated Actors and Oracle weights, preserved the Oracle proof bridge, and rebuilt Wasm, metadata, descriptors, and runtime evidence.
- `Validation`: Passed workspace, try-state, client, Svelte, Domain DAG, wiki, corpus, script, architecture, backlog, completion, and release-line gates.

## 0.7.8: Reactive Delivery and Feedback Analysis

- `Actors`: Replaced empty-slot scans with exact occupied subscriber pages and a reciprocal active-dirty list carrying cursor, revision, and dirty-age ownership.
- `Oracle`: Made changed publication, dirty ingress, and events atomic; topology or capacity rejection now rolls back publication and composed Router swaps.
- `Inspection`: Added finalized feed fanout, cleanup, actor queue, ticket, and wakeup inspection without prefix scans or execution-time prediction.
- `Control Plane`: Added bounded feedback findings and a canonical configuration IR shared by JSON, TOML, and structured Markdown.
- `Ownership`: Separated package contracts from DEOS integration and aligned public subsystem, package, Cargo, documentation, and wiki naming.

## 0.7.7: Typed Observation and Reactive AAA

- `Oracle`: Added a standalone typed-observation pallet with bounded feeds/producers, LastValue and EMA aggregation, lifecycle, transactional hooks, try-state, weights, and an embedding runtime.
- `Router`: Moved directional EMA state from Router storage into Oracle while preserving pre-execution sampling and whole-swap rollback.
- `Actors`: Added typed observation conditions and sources, bounded subscriptions, O(1) dirty ingress, and deferred fanout through the existing scheduler.
- `Lifecycle`: Restricted trigger percentages to covered address sources and allowed one-shot closure only after committed productive completion.
- `Correctness`: Added tri-state discovery, admission-before-mutation, independent scan offers, exact transfer preflight, and persistent native-flow anchors.
- `Control Plane`: Added manifest-driven reactive authoring, bounded Oracle inspection, matching-Wasm outcomes, and classified scenarios.
- `Packages / Validation`: Localized package docs, refreshed dependencies and identities, regenerated artifacts, and passed runtime, client, embedding, benchmark, wiki, and release gates.

## 0.7.6: AAA Intent, Failure, and Service Semantics

- `Market Guard`: Required System swaps to use a fresh nonzero EMA or direct-pool reserve reference and fail Temporary before mutation when neither exists.
- `Scheduler`: Replaced whitelist extraction with type-derived System/User FIFO lanes sharing one ticket namespace, cutoff, wakeup substrate, and bounded service phases.
- `Authority`: Added typed execution context and removed the System-id whitelist, raw market cap, and generic post-resolution clamp.
- `Transfers / Liquidity`: Made split deposits atomic and passed explicit LP/output minima into downstream liquidity operations with typed retry behavior.
- `Retry / Intent`: Added bounded retry exhaustion and canonical `SwapIn`/`SwapOut` with explicit live-quote or absolute input limits.
- `Evidence`: Synchronized metadata, client analysis, matching-Wasm, weights, Wasm, package/runtime tests, and the 10,000-actor release gate.

## 0.7.5: AAA Market Safety

- `Scheduler`: Added bounded System/User service over one queue with deterministic capped retry backoff and one canonical ticket/wakeup path.
- `System Policy`: Bounded System market inputs and reference deviation without adding an authority class or generic policy object.
- `Failures`: Added exhaustive Router and liquidity failure classes with fail-closed Permanent defaults for unknown downstream errors.
- `Router`: Added caller-aware exact-output quotes and execution over bounded direct and Native-anchored XYK candidates.
- `Liquidity`: Added explicit LP and withdrawal minima across runtime, metadata, authoring, analysis, and UI surfaces.
- `Swap`: Added a nonzero exact-output spend cap bounded by live preservable input.
- `Contraction / Evidence`: Removed speculative execution classes and lanes, regenerated artifacts, and passed package, runtime, client, scheduler, and occupancy gates.

## 0.7.4: Verifiable Step Composition

- `Triggers`: Separated bounded sources from Immediate/Cadenced admission and unified Manual and AddressEvent timing through one scheduler and latch.
- `Ingress`: Made multi-source address ingress transactional and exactly-once for funding and scheduler effects.
- `Conditions`: Added flat `Always`, `All`, and `Any` sets with full observation, whole-group errors, measured atomic-count weights, and no nesting.
- `Control`: Added `StopCycle` as an explicit successful terminal task without branching, callbacks, or a second scheduler.
- `Semantics`: Generated one Rust-owned task/amount manifest and removed handwritten TypeScript semantic reconstruction.
- `Control Plane`: Added typed trigger authoring, structural diff, scenario classification, metadata-bound composition, and stable failure-outcome presentation.
- `Evidence`: Added an independent aggregate-plan runtime, refreshed metadata/weights/Wasm, and passed package, runtime, client, stress, and occupancy gates.

## 0.7.3: Progress-Preserving AAA Continuation

- `Continuation`: Added Mutable-only Temporary retry with one unresolved-step cursor, frozen suffix inputs, cumulative outcomes, and one logical-run nonce.
- `Scheduling`: Reused canonical FIFO/wakeup state for retries, preserved one ticket or wakeup, and priced only unresolved suffix work.
- `Lifecycle`: Added deterministic cancellation and suspension/continuation events with actor-local invalidation and no compensation or prefix rollback.
- `Portability`: Moved the embedding runtime under the package and proved User/System continuation, errors, ingress, disabled adapters, metadata, try-state, and no-std.
- `Control Plane`: Added canonical plan artifacts, structural diff, static forecasting, adapter-local simulation, and governance-call composition.
- `Simulation`: Added rollback-only package/runtime simulation, versioned API bytes, finalized transport, and matching-Wasm provenance checks.
- `Evidence`: Generated production weights/Wasm, exercised live fresh and continued simulation, synchronized docs/client/wiki, and added governance/indexer delivery organs.

## 0.7.2: Work-Proportional AAA

- `State / Scheduling`: Split hot, program, and funding stores; added paged FIFO/wakeups, sparse cursors, identity-only dormancy, and independent RefTime, ProofSize, scan, execution, and promotion budgets.
- `Ingress / Lifecycle`: Replaced event scans and compatibility inboxes with transactional producer ingress; made terminal close prevalidated, actor-local, and state-clean while preserving sovereign balances.
- `Economics`: Limited User fees to attempted steps, priced actual funding promotion, split task weight classes, added LP reverse indexing, and made starvation telemetry sparse.
- `Portability / Evidence`: Added a zero-topology no-std runtime and synchronized specifications, embedding, metadata, weights, Wasm, wiki, and release gates around a fresh baseline.

## 0.7.1: AAA Semantic Hardening

- `Kernel`: Corrected stale-funding behavior, breaker precedence, rolled-back close deferral, FIFO semantics, task surface, and pre-launch dispatch indices.
- `Runtime Capacity`: Split dispatch/idle capacity, removed the unused Operational reserve, enforced two-dimensional plan admission, and proved carry-over, convergence, close liveness, and native flow under stress.
- `Contract / Validation`: Synchronized specs, ABI/storage inventories, architecture, tables, metadata, and release gates around the hardened semantic core.

## 0.7.0: AAA Contract Convergence

- `Compatibility`: Established the consolidated pre-launch actor contract and reset runtime versions to the fresh-genesis baseline.
- `Lifecycle`: Completed creation, mutability, pause, triggers, cycles, closure, immutable anchors, breaker behavior, amount resolution, adapters, and terminal ordering.
- `Scheduling`: Added durable queues, exact wakeups, bounded retry/cleanup, active cardinality, and two-dimensional hook admission with starvation evidence.
- `Funding / Authority`: Replaced dedicated funding calls with transfer batches and added typed provenance plus owner, allowlist, runtime-policy, and verified-ingress controls.
- `Fees / Integration`: Unified User fees, preserve-spend accounting, task rollback, DEX/staking/liquidity adapters, and transactional producer ingress.
- `Protocol Fees`: Separated Router trading fees from general Fee Sink collection and kept trusted-collator allocation distinct from future permissionless rewards.
- `Evidence`: Regenerated production weights and reconciled specs, runtime, embedding, wiki, metadata, scripts, and stress gates.

## 0.6.11: Wiki Confidence and Builder Economy

- `Builder Economy`: Added a bilingual `$BLDR` owner page for useful-work invoices, TMC/treasury/liquidity wiring, Native protection, activation boundaries, and demand honesty.
- `Consolidation`: Merged weak graph and troubleshooting leaflets into stronger Generated Wiki and validation owners, reducing the wiki to 48 bilingual page IDs.
- `Confidence / Locale`: Restricted confidence to conservative bands, enforced source freshness and metadata agreement, corrected stale claims, and improved Russian terminology.

## 0.6.10: Router Honesty and Actor Vocabulary

- `Entrypoint`: Rebuilt the root README around framework identity, economic circuits, local setup, navigation, validation, and market-claim honesty.
- `Manifesto / Context`: Restored the research manifesto, added failure honesty, flattened durable project protocol, and retired the release-specific SDK insights guide.
- `Scripts`: Reclassified composite web-client state seeding as a named admin utility rather than a numbered atomic script.
- `Router`: Aligned docs and tests with maximum-recipient-output selection, prior-EMA deviation checks, informational quote fields, and no MEV-immunity claim.
- `Falsification`: Added a competing-candidate route test and expanded economic-claim and stale-vocabulary audits.
- `Actors`: Promoted Liquidity Actor terminology across runtime, primitives, tests, docs, and wiki while retaining only explicit compatibility constants.

## 0.6.9: Stable2606 Runtime Line

- `Platform`: Upgraded the template to Polkadot SDK `2606.0.0`, current companion crates, Wasm builder, API line, Rust toolchain, and lockfile.
- `Runtime`: Added the required relay-parent-offset API, retained disabled V3 scheduling, updated LP fee bindings, and advanced runtime compatibility identity.
- `Operations`: Moved local binaries, docs, context, and wiki projections to stable2606 and retired the obsolete stable2603 cargo-update blocker.

## 0.6.8: Validation and Knowledge Projection Hardening

- `Simulator`: Defined the simulator as the TMCTOL mathematical hypothesis lab, not a shadow runtime or parity owner.
- `Wiki / Router`: Updated bilingual route and economic-claim pages for maximum output, delivered-output truth, EMA limits, and honest market guarantees.
- `Dependencies / Tests`: Refreshed compatible client dependencies and corrected a stale Oracle-deviation runtime-test expectation.
- `Validation`: Added dynamic workspace release-marker checks and a neutral completion gate that runs runtime tests for runtime-source changes.
- `Release`: Synchronized package markers and lockfile identity with the release line.

## 0.6.7: Backlog Gating Audit Hardening

- `Backlog`: Reclassified remaining work as conditional, external, watch-only, or product-pressure-triggered instead of implying active local work.
- `Validation`: Added a completion-gate audit that rejects ungated implementation-looking items and preserves explicit dependency-watch language.
- `Release`: Synchronized workspace package and lockfile markers.

## 0.6.6: Economic Claim Coverage Expansion

- `Validation`: Expanded machine-readable economic-claim coverage to staking reward rollover and actor active-cap cleanup, reaching ten audited claims.
- `Release`: Synchronized workspace package and lockfile markers.

## 0.6.5: Liquidity Actor Naming Hygiene

- `Naming`: Renamed Router/runtime `ZapManagerAccount` surfaces to `LiquidityActorAccount` and aligned TMC resolution, helpers, tests, docs, and context.
- `Router`: Added fixed reserve-depth regressions for the 20% EMA price-deviation breakpoint.
- `Validation`: Added a machine-readable economic-claim inventory with anchors, falsification tests, proof classes, and tautology-risk checks.
- `Release`: Synchronized workspace package and lockfile markers.

## 0.6.4: Axial Router Contract Honesty and Recipient-Aware TMC Mint

- `TMC / Router`: Routed minted user allocation to the requested recipient and based quotes, selection, slippage, outcomes, and events on recipient output rather than total emission.
- `API`: Renamed mint-preview helpers to distinguish total mint from recipient receipts.
- `Selection`: Removed the vestigial efficiency score and made route choice explicitly maximize recipient output; fee and impact fields remain informational.
- `Failures / Tests`: Removed dispatch-error collapse, exposed price deviation directly, and replaced a tautological sandwich test with honest round-trip characterization.
- `Docs`: Corrected Router/TMC signatures, route protections, Oracle roles, quote semantics, recipient flow, and market-mitigation claims.
- `Release`: Tracked the remaining actor-account rename separately and synchronized workspace package markers.

## 0.6.3: Framework Instance Contract

- `Framework Boundary`: Defined DEOS mechanisms, invariants, execution safety, read models, and validation as framework truth while leaving brand, dApps, founder policy, bucket policy, labor culture, and demand strategy to downstream instances.

## 0.6.2: AAA External Runtime Embedding Contract

- `Embedding`: Added a first-class guide separating portable actor-kernel obligations from runtime adapters, UI/read models, and DEOS/TMCTOL System Actor topology.
- `Atomicity`: Published a task-scoped rollback matrix covering adapters, close tails, failure policies, events, and multi-step host mutation.
- `Validation / Wiki`: Protected embedding-guide reachability and synchronized bilingual actor/forking pages.
- `Consolidation`: Added wiki leaflet and confidence guards, then merged weak partner, fork, threat, positioning, and route pages into stronger owners.
- `Context`: Clarified DEOS as reusable substrate for its first downstream ecosystem.
- `Backlog`: Split shipped transaction/Actor fee routing from the future block-reward source gate.

## 0.6.1: Self-Contained Validation and AAA Hardening

- `Tooling`: Vendored Domain DAG validation, added portability/release-line/skill audits, removed operator-local paths, refreshed dependencies, and made clone-local validation self-contained.
- `Actors`: Wrapped task execution in storage transactions, covered late adapter failures, documented embedding obligations, and replaced manager-centric comments with role language.

## 0.6.0: AAA Reusable Standard Foundation

- `Actors`: Added portable `Stake`, `Unstake`, and `DonateLiquidity` tasks behind runtime adapters.
- `Specification`: Defined liquidity-donation ownership across amount resolution, events, pair ratios, receipt suppression, reserve donation, and native policy.
- `Tests`: Covered adapter success, injected failures, second-asset rollback, failure policies, and cycle summaries.
- `Release`: Synchronized the reusable actor package marker.

## 0.5.4: Proactive Improvement Backlog Anchor

- `Backlog`: Retired the improvement lane, moved dependency watches to gated work, removed command inventories, and hardened open-work shape checks.
- `Validation`: Expanded fast audits for scripts, templates, numerics, simulator determinism, suppressions, backlog, Domain DAG, wiki trust, and optional dependency posture.
- `Scripts`: Added prerequisites, zero-warning Clippy, strict bootstrap parsing, current artifact names, and safer local defaults.
- `Simulator`: Replaced ambient randomness and wall-clock assertions with seeded deterministic conservation and formula coverage.
- `Client`: Centralized complete numeric parsing for amounts, ids, epochs, slippage, assets, internal ids, and signer presets.
- `Frontend / Provenance`: Tightened small-screen UI, domain ownership, signer naming, and browser-time versus chain-time labeling.
- `Dependencies / Context`: Refreshed compatible packages, classified remaining advisories, and promoted the new validation and ownership rules into durable docs.

## 0.5.3: Stable2603-3 Operator Hotfix

- `Operations`: Moved local SDK binaries and benchmark guidance to `polkadot-stable2603-3` / node `v1.22.3`.
- `Docs`: Recorded the binary patch-line change separately from the unchanged 2603 crate baseline.

## 0.5.2: Partner Pitch Entry Surface

- `Onboarding`: Routed the root README and Start Here flow through a dedicated Partner Pitch before deep architecture.
- `Wiki`: Added a safe-change boundary separating downstream customization from protocol-review changes.

## 0.5.1: Onboarding And Changelog Hotfix

- `Onboarding`: Added bilingual Start Here routes for understanding, running, and safely forking DEOS.
- `Navigation`: Prioritized a small onboarding spine above the full wiki graph.
- `Forking`: Added a first-change map and minimum validation by change type.
- `Backlog`: Converted onboarding follow-ups into closable partner-feedback and clean-room setup work.
- `History`: Reorganized changelog entries around domain outcomes instead of operation chronology.

## 0.5.0: Baseline

- `Wiki`: Delivered a self-contained bilingual newcomer product with entry routes, domain maps, flows, diagrams, glossary, status, and provenance.
- `Reference Client`: Aligned wallet, swap, staking, governance, automation, logs, charts, settings, accounts, and wiki with bounded runtime/read-model truth.
- `UI Kit`: Centralized reusable controls, cards, notices, badges, overlays, asset selection, hydration-safe ids, and styling contracts.
- `Domain DAG`: Added ownership headers, acyclic import boundaries, pressure checks, and forbidden-edge validation.
- `Adapters / Read Model`: Split transport responsibilities behind one facade and separated canonical chain truth from indexed, archived, cached, or session-derived data.
- `Validation`: Added formatting, Svelte, production-build, Domain DAG, and trusted-wiki entrypoints.
- `Docs / Backlog`: Rewrote current architecture and converted broad maintenance intentions into concrete or gated work.

## 0.4.0: Crystallize AAA And TMCTOL Kernel

- `Actors`: Hardened cycles, close tails, triggers, timer rearming, funding, scheduling, close plans, and terminal behavior without expanding the task language.
- `Immutability`: Protected System Actor protocol anchors from mutation, pause, closure, reopen, or discretionary override.
- `TMCTOL`: Defined conditional guarantees, floor inputs, bucket/LP state, burn liveness, Zap postconditions, conservation, and conformance status.
- `Read Model`: Added a bounded storage-free TMCTOL runtime API over existing canonical state.
- `Router / Runtime`: Bounded governance-settable fees and removed analytics/dashboard state from consensus.
- `Staking`: Added Phase 1 Fee Sink reserve strengthening without donor LP minting.
- `Operations / Backlog`: Aligned stable2603-2 tooling and moved completed hardening out of open work.

## 0.3.2: Harden Template Release Surface And Fix Phase 1 Fee Sink Bridge

- `Weights`: Replaced usable production placeholders with concrete SDK weights and made XCM benchmark generation work through current SDK paths.
- `Readiness`: Added static checks for fallback XCM weights, unclassified placeholders, stale staking aliases, and asset-conversion naming drift.
- `Fees`: Unified transaction and User Actor collection into the 20% author / 80% Fee Sink contour with Fee Sink fallback.
- `Staking`: Separated staking yield, liquidity donation, and future claimable-reward accounts.
- `Actors`: Materialized Fee Sink as a System Actor with the Phase 1 native staking/native LP split plan.
- `Simulator`: Added Phase 1 and Phase 2 reward-routing conservation coverage.
- `Docs`: Distinguished trusted-collator pool rewards from future permissionless-collator economics.

## 0.3.1: Stable2603-1 Patch Release

- `Staking`: Merged LP transfer isolation, empty-pool preconditions, bounded Actor donation behavior, and governance-custody ordering into the canonical specification.
- `Docs`: Removed the temporary proposal after its accepted content reached the specification.

## 0.3.0: Staking Rework

- `Native Staking`: Centered launch staking on liquid `stNTVE`, the canonical `NTVE/stNTVE` pool, locked LP nomination, and bounded epoch rewards.
- `Security / Governance`: Added LP custody, unlock/redelegate lifecycle, deterministic ranking, and vote power from locked native, receipt, and LP positions.
- `Rewards`: Separated native claim/compound settlement from generic same-asset rewards.
- `Actors / Liquidity`: Kept portable staking tasks and separated reserve donation from LP-minting compounding.
- `Read Models / Client`: Added bounded native-staking views and a signer-gated browser workflow for staking, LP nomination, governance custody, and rewards.
- `Operations`: Added guarded local bootstrap, plan-only calls, readiness probes, and canonical pool/Actor sequencing.
- `Governance / Docs`: Closed the bounded v1 governance baseline, kept archive search materialized, and synchronized specs, architecture, wiki, and backlog.

## 0.2.0: Runtime Hardening

- `Staking`: Made idle maintenance budget-aware and ranking deterministic through bounded caches, exact fallback, repair, and stale-exposure retirement.
- `Rewards`: Added indexed, resumable, budget-aware ingress with bounded scans and reverse indexes.
- `TMC`: Preflighted asset existence and made mint/distribution transactional with pallet and runtime conservation tests.
- `XCM / Governance`: Added origin/barrier/queue coverage and documented tactical treasury spend as one bounded topology.
- `Router`: Prevalidated gross affordability and routed fees transactionally before XYK, mint, or multi-hop execution.
- `Evidence`: Expanded Router economics tests and regenerated Router, staking, TMC, and Actor weight surfaces.
- `Docs`: Synchronized fail-closed XCM, tactical treasury, gross-debit, and fee-ordering architecture.

## 0.1.2: Complete TMCTOL-to-DEOS Naming Drift Migration

- `Identity`: Established DEOS as the framework identity while retaining TMCTOL for the economic standard.
- `Runtime`: Renamed runtime identity, artifacts, metadata, and descriptors to DEOS.
- `Scripts`: Updated build, chain-spec, network, release, validation, and Actor tooling to DEOS identifiers.
- `Client`: Updated connection types, snapshots, signers, defaults, and local-storage keys.
- `Boundary`: Retained TMCTOL names only where they identify the economic standard under test.

## 0.1.1: Unified Governance Resolution Policy, Stateless Adapter, And Proposal Descriptor Deduplication

- `Governance`: Unified execution and query resolution through `CoreResolutionOutcome` and one storage-free tally builder.
- `Tests`: Covered zero turnout, thresholds, ties, approval failures, Binary and Invoice winners, and equal-weight last-wins behavior.
- `Bugfix`: Removed the false Invoice tie produced when no Nay vote existed.
- `Client`: Decoupled governance state and proposal hydration from the concrete chain connection and shared common-field loading.

## 0.1.0: Baseline Reset, O(1) Asset Registry, And Governance Deduplication

- `Asset Registry`: Added an O(1) bidirectional `AssetId <-> Location` index.
- `Governance`: Reset pre-fork storage lineage and deduplicated terminal resolution paths.
- `Client`: Split governance definitions, constants, rendering, and provider write surfaces into clearer domain owners.

## 0.0.0: Delivered Baseline

- `Platform`: Shipped a Polkadot SDK 2603 / Omni Node-ready runtime with CI, local tooling, benchmarks, and deployment assumptions.
- `Identity`: Established DEOS as the reusable framework and TMCTOL as its first economic standard.
- `Economic Kernel`: Delivered Actors, Router, TMC, TOL buckets, Asset Registry, and token-driven runtime coordination.
- `Governance`: Delivered bounded dual-track domains, proposal metadata, queries, upgrade authorization, and tactical treasury governance.
- `Staking`: Delivered multi-asset receipt staking, native support, sparse rewards, auto-compound settlement, and governance-conditioned export.
- `Client / Docs`: Delivered the reference browser, typed docs, generated wiki, context, open-work, and delivery-history surfaces.
- `Validation`: Established simulator, Rust, benchmark, client, local-network, and operator evidence paths.
