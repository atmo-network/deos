# DEOS Backlog

> Open framework work only; durable protocol lives in `AGENTS.md`, and completed delivery history lives in `CHANGELOG.md`.
>
> Release boundary: `DEOS 0.7.2 — Work-Proportional AAA` shipped through the validated `0.7.2` checkpoint history. Open AAA work now begins with the isolated `0.7.3` Continuation contract below.

## DEOS 0.7.3 — Progress-Preserving AAA Continuation

> `0.7.3` begins only after `0.7.2` ships its work-proportional storage, scheduler, wakeup, direct-ingress, signal-latch, and pure-close foundations. It adds bounded progress preservation without reopening removed `0.7.1` containers, replaying committed prefixes, or turning AAA into a general-purpose VM. `0.7.2` release gates do not depend on this section.

### Slice 1 — Continuation contract and sparse state

- [ ] `0.7.3 / S1 / Specification precursor`: Refine `docs/aaa.specification.en.md` first for the selected Continuation failure, cursor, snapshot, mutability, accounting, retry, cancellation, and boundedness semantics; delete superseded assumptions rather than implement against backlog prose alone.
- [ ] `0.7.3 / S1 / Mechanism`: Add `StepErrorPolicy::RetryLater` for Mutable User and Mutable System actors and use **Continuation** consistently; do not add actor-wide `ExecutionMode::Resume`, overload lifecycle `resume_aaa`, create whole-plan rollback, or create a general checkpointing VM.
- [ ] `0.7.3 / S1 / Failure semantics`: Preserve `ContinueNextStep` as task rollback + final failed outcome + next step in the same attempt; preserve `AbortCycle` as task rollback + logical-run termination + continuation discard so the next external signal starts at step `0`; define `RetryLater` as task rollback + suspension at the unresolved cursor + durable retry without replaying committed prefix.
- [ ] `0.7.3 / S1 / Retryability taxonomy`: Define one closed, bounded classification at the task/adapter result boundary for explicitly temporary versus permanent incapacity. `FundingUnavailable` and named recoverable runtime conditions may suspend under `RetryLater`; unsupported capability, invalid configuration, impossible asset/pool identity, arithmetic/validation failure, and other permanent errors must finalize the failed step, terminate the logical run, and discard Continuation rather than retry forever. Pin every adapter/task error to one class in specification and tests.
- [ ] `0.7.3 / S1 / Prefix closure`: Use one scalar cursor only while every index below it has one final outcome and no index above it executes while the cursor step remains unresolved. Advance on success, `ConditionsNotMet`, terminal-zero/final `ResolutionSkipped`, and failure under `ContinueNextStep`; suspend on explicitly retryable adapter/task failure, `FundingUnavailable`, or another specified temporary incapacity under `RetryLater`. Reject non-progressing skip followed by later execution rather than add a completion bitmap.
- [ ] `0.7.3 / S1 / Funding disposition`: Preserve shipped `0.7.2` non-terminal `FundingUnavailable` for `ContinueNextStep` and `AbortCycle` plans by making the step final and advancing; only `RetryLater` converts that resolution into suspension at the current cursor. Keep this one explicit disposition matrix rather than hide a second funding-only control mode.
- [ ] `0.7.3 / S1 / Lazy storage`: Admit a new logical run and build its trigger snapshot in memory without writing continuation state; execute from step `0`, write nothing if the run completes in one attempt, and create sparse `ContinuationState` only at suspension with cursor, continuation-local attempt, validated suffix bounds, trimmed frozen trigger snapshot, and cumulative outcome totals.
- [ ] `0.7.3 / S1 / Snapshot minimization`: Store frozen ordinary-asset/staking-share entries only for `PercentageOfTrigger` references in the unresolved suffix; store no snapshot when the suffix has none, omit redundant actor/run identity, and use existing `cycle_nonce` as the logical-run identifier unless executable evidence proves it insufficient.
- [ ] `0.7.3 / S1 / Mutability boundary`: Reject plans containing `RetryLater` for Immutable actors; defer immutable continuation until independent embedding evidence proves a safe permanent-failure and cancellation policy.
- [ ] `0.7.3 / S1 / Dependencies`: Require the completed `0.7.2` ActorHot/ActorProgram/ActorFunding split and preserve its sparse extension boundary; Continuation absence must add no read, write, or decoded proof to dormant and ordinary one-attempt paths.
- [ ] `0.7.3 / S1 / Exit`: The public error-policy contract, cursor invariant, sparse value shape, snapshot bound, Mutable-only admission rule, SCALE consequences, and storage-version consequence are explicit before execution changes begin.

### Slice 2 — Suspension, retry, and logical-run accounting

- [ ] `0.7.3 / S2 / Execution`: Suspend only on a Slice 1 temporary incapacity at the unresolved cursor after task-scoped rollback, persist the minimal Slice 1 state, and resume from that cursor without replaying any committed prefix or executing a later index while it remains unresolved; a permanent failure under `RetryLater` must take the specified terminal logical-run path without writing retry state.
- [ ] `0.7.3 / S2 / Nonce and attempt`: Increment `cycle_nonce` exactly once when the logical run opens, reuse it for all continuation attempts, increment continuation `attempt` per admitted retry, update the admitted-attempt cooldown clock each time, and treat pre-admission weight deferral as neither an attempt nor a failure.
- [ ] `0.7.3 / S2 / Completion accounting`: Evaluate auto-close and lease progress only after successful full logical-run completion; continuation attempts do not advance lease progress; promote pending funding only on full success; reset `consecutive_failures` only then; increment failures for an attempt suspended by actual retryable adapter failure or funding unavailability; define inclusive threshold-terminal cancellation without compensation; preserve `0.7.2` behavior for plans without `RetryLater`.
- [ ] `0.7.3 / S2 / Internal readiness`: Treat continuation presence as internal pending work that needs no external signal; gate retry by lifecycle pause, global breaker, schedule window, canonical effective eligibility/retry cooldown, residual admission, and terminal conditions, then use the `0.7.2` paged wakeup substrate for delayed retry.
- [ ] `0.7.3 / S2 / Retry eligibility`: Define continuation eligibility as `max(last_admitted_attempt_block + retry_cooldown, schedule_window.start)` with non-applicable terms omitted and saturating bounded arithmetic; use attempt start as the canonical clock unless measurements prove finish-block ownership necessary. Do not apply the external Timer cadence term to an already-open logical run, and do not count a pre-admission weight deferral as an attempt.
- [ ] `0.7.3 / S2 / Concurrent signals`: While suspended, external signals only set the `0.7.2` `pending_signal` latch for the next logical run and funding ingress accumulates independently; after continuation success clear continuation, promote funding, emit one logical-run summary, and schedule a new run when the latch remains set.
- [ ] `0.7.3 / S2 / Dependencies`: Depend on Slice 1 plus the `0.7.2` effective-eligibility, queue, wakeup, signal-latch, funding, and pure-close foundations; do not add a second scheduler or signal inbox.
- [ ] `0.7.3 / S2 / Exit`: Suspended work retries without a second external trigger, one nonce spans all attempts, completed prefixes execute exactly once, pending external work remains latched for a later run, and ordinary plans retain their `0.7.2` accounting.

### Slice 3 — Residual weight, fee, and funding admission

- [ ] `0.7.3 / S3 / Residual F1 admission`: Calculate attempt weight and User fee upper bounds only over `cursor..plan.len()`; do not re-admit or recharge committed prefix steps, allow the unresolved step's evaluation/execution fee on each retry, use live spendable capacity/minimum-balance checks, perform at most one fee collection per attempted step, and add no stranded run-scoped fee reservation.
- [ ] `0.7.3 / S3 / Cached bounds`: Persist or derive only suffix indexes/bounds that avoid a demonstrably costly scan; reject a maximum-plan reservation, duplicated prepared-task buffer, or full frozen balance snapshot as substitutes for precise residual admission.
- [ ] `0.7.3 / S3 / Funding`: Keep armed/pending funding in the `0.7.2` ActorFunding owner; suspension must not promote pending funding, retries must observe live spendability while preserving frozen trigger-percentage inputs, and full completion must promote each pending amount exactly once.
- [ ] `0.7.3 / S3 / Last-funding interaction`: Specify before executor implementation whether a suspended `PercentageOfLastFunding` step resolves only from the armed batch frozen at logical-run opening or may consume funding arriving during suspension. Prefer frozen armed-batch identity plus pending-for-next-run semantics unless conservation and replay tests justify a bounded explicit top-up rule; pin pruning, cancellation, plan mutation, terminal close, and subsequent promotion behavior in the same contract.
- [ ] `0.7.3 / S3 / Dependencies`: Depend on Slice 2 logical-run semantics and the regenerated `0.7.2` one-transfer task weights; recalculate admission from production Wasm artifacts rather than hand-authored estimates.
- [ ] `0.7.3 / S3 / Evidence`: Cover same-cursor repeated fee charging, no committed-prefix fee recurrence, suffix-weight shrinkage, minimum-balance changes between attempts, funding arriving during suspension, no reservation leakage, and zero Continuation overhead for successful one-attempt plans.
- [ ] `0.7.3 / S3 / Exit`: Every admitted retry proves and charges only its unresolved suffix, funding remains conserved across attempts, and successful one-attempt execution writes no Continuation state.

### Slice 4 — Cancellation, invalidation, and events

- [ ] `0.7.3 / S4 / Cancellation`: Add a Mutable-only `cancel_continuation(aaa_id)` control call and one canonical cancellation routine. Cancel without success, compensation, funding promotion, or rollback of committed effects on explicit cancellation, plan replacement, pure close, terminal transition, or another mutation that invalidates frozen program/snapshot assumptions. Choose and lock one public repeat-call contract before metadata freeze: idempotent success with no second event, or one explicit `ContinuationNotFound` error; internal invalidation must remain idempotent regardless.
- [ ] `0.7.3 / S4 / Invalidation matrix`: Pin `0.7.3` to cancellation on execution-plan, funding-policy, active schedule, or schedule-window mutation; cancellation also applies on dormancy, close, terminal transition, explicit cancel, and window expiry. Pause/resume and breaker toggles preserve but defer Continuation because they do not reinterpret the suffix. Do not permit changed program assumptions beneath a retained cursor/snapshot; consider narrower schedule-preserving mutation only as a later append-only relaxation after evidence.
- [ ] `0.7.3 / S4 / Events`: Prefer `CycleSuspended { aaa_id, cycle_nonce, attempt, cursor, reason, cumulative_outcomes }`, `CycleContinued { aaa_id, cycle_nonce, attempt, cursor }`, and `CycleCancelled { aaa_id, cycle_nonce, reason }`; reuse `CycleStarted` only for logical-run opening, existing step events across attempts under the same nonce, and `CycleSummary` exactly once at completion.
- [ ] `0.7.3 / S4 / Pure close integration`: Extend the `0.7.2` canonical terminal cleanup routine to delete sparse Continuation state without executing tasks, charging fees, promoting funding, scanning shared scheduler containers, or making close fallible after preconditions.
- [ ] `0.7.3 / S4 / Dependencies`: Depend on Slices 1–3 so cancellation and telemetry describe final state and fee semantics rather than transitional behavior.
- [ ] `0.7.3 / S4 / Exit`: Every invalidating transition has one deterministic outcome, committed effects and sovereign balances remain intact, cancellation cannot imply success, and indexers can reconstruct the logical run without duplicate start/summary events.

### Slice 5 — Semantic, model, and production-weight evidence

- [ ] `0.7.3 / S5 / Semantic evidence`: Test committed-prefix exactly-once behavior, same-cursor retry, one-attempt zero Continuation writes, stable and trimmed frozen snapshots, live spend/minimum checks, same nonce/multiple attempts, attempt failure accounting, pure weight deferral, every task/adapter retryability class, permanent-error termination without retry state, pending funding promotion only at full completion, signals/funding during suspension, pause/breaker/window retry, plan/schedule invalidation, explicit cancellation, close/terminal cancellation, and unchanged plans without `RetryLater`.
- [ ] `0.7.3 / S5 / Pipeline scenarios`: Add named regressions for `SwapExactIn -> AddLiquidity -> Transfer` with the middle step temporarily failing, Transfer prefix followed by a temporary DEX failure, and Burn prefix followed by a required failure; prove task-level rollback, plan-level non-atomicity, committed-prefix non-replay, same-step retry, and no compensation on cancellation.
- [ ] `0.7.3 / S5 / Lifecycle scenarios`: Cover Manual and `OnAddressEvent` suspension without another external trigger, pending signal during suspension, funding arrival during suspension, pause/resume, global breaker, explicit cancel, plan update, dormancy, schedule/window mutation and expiry, inclusive failure threshold, auto-close only after successful completion, and stale queue/wakeup entries after every cancellation path.
- [ ] `0.7.3 / S5 / Admission scenarios`: Cover User fees across multiple attempts, frozen `PercentageOfTrigger`, the selected `PercentageOfLastFunding` contract, live balance reduction, funding unavailable at the cursor, first-attempt success with no Continuation write, minimal/maximal suffix, maximum bounded snapshot, and separately measured User/System retries.
- [ ] `0.7.3 / S5 / Model evidence`: Extend the deterministic seeded state machine with `suspend`, `continue`, and `cancel continuation`; enforce cursor bounds, prefix closure, sparse-store referential integrity, queue-ticket/wakeup-pointer uniqueness, funding conservation, and cancellation rules after every transition, with shrinking and replayable traces.
- [ ] `0.7.3 / S5 / Benchmarks`: Benchmark suspension, retry, completion, pending-signal ingress, absent-Continuation negative readiness, pure close with Continuation present, and suffix-only weight/fee admission from optimized production Wasm; generate independent classes where storage paths differ materially.
- [ ] `0.7.3 / S5 / Decision rule`: Require absent Continuation to preserve the `0.7.2` dormant/negative/one-attempt envelope and require retry cost to scale with the unresolved suffix; reject cached state or scheduler changes that fail to improve the measured path they claim to bound.
- [ ] `0.7.3 / S5 / Dependencies`: Depend on Slices 1–4 and complete before the independent embedding freezes portable interfaces.
- [ ] `0.7.3 / S5 / Exit`: Pallet/runtime tests, model invariants, metadata assertions, try-runtime checks, generated pallet weights, and bound production runtime weights agree on one measured Continuation implementation.

### Slice 6 — Independent runtime Continuation embedding

- [ ] `0.7.3 / S6 / Embedding`: Extend the `0.7.2` non-TMCTOL runtime with Mutable User and Mutable System Continuation, deterministic unsupported-adapter policy paths without retry state, an explicitly temporary failing-adapter retry fixture, cancellation, pure close, direct ingress during suspension, residual suffix admission, deterministic retry after cooldown, disabled-DEX profile, and zero genesis System AAA assumptions.
- [ ] `0.7.3 / S6 / Capability matrix`: Prove `RetryLater` suspends and resumes only against the temporary runtime-local adapter fixture; prove unsupported adapters create no retry state and follow `ContinueNextStep`, `AbortCycle`, or permanent-failure-under-`RetryLater` control flow as specified; preserve User `SwapExactOut` when the optional DEX fixture exists, preserve System-only `Mint`, and reject Immutable Continuation without importing DEOS/TMCTOL policy.
- [ ] `0.7.3 / S6 / Evidence`: Run the second runtime's pallet integration, executive ingress-during-suspension, queue/wakeup retry, cancellation/close, residual admission, production-weight, metadata, try-runtime, and fresh-genesis/storage-version tests in CI/local completion gates.
- [ ] `0.7.3 / S6 / Boundary feedback`: Treat any required DEOS helper trait, TMCTOL topology copy, mandatory System actor, unbounded adapter scan, or breaking portable correction as release-blocking architecture evidence; revise the pre-`1.0` contract and repeat affected earlier slices.
- [ ] `0.7.3 / S6 / Dependencies`: Depend on Slices 1–5 and regenerated portable interfaces.
- [ ] `0.7.3 / S6 / Exit`: The independent runtime exercises the full Continuation lifecycle without DEOS topology or a breaking correction; only this evidence may unblock later AAA `1.0` consideration.

### Slice 7 — Specification, metadata, and release convergence

- [ ] `0.7.3 / S7 / Sequence`: Reconcile the Slice 1 normative specification against verified final semantics; after Slices 1–6 ship and pass, update `docs/aaa.architecture.en.md` from implementation truth, then converge the embedding guide, pallet/runtime READMEs, web-client authoring/status surfaces, canonical/indexed read models, bilingual wiki, operational surfaces, and release evidence.
- [ ] `0.7.3 / S7 / Indexer correlation`: Define one logical-run start and terminal outcome, attempt correlation by `(aaa_id, cycle_nonce, attempt)`, step-event correlation under the stable nonce, cancellation reasons, and materialized attempt history without adding on-chain history or a second global run identifier.
- [ ] `0.7.3 / S7 / Metadata and compatibility`: Document and lock the new `StepErrorPolicy` discriminant, Continuation storage prefix/key/value/modifier/hashers, events, errors, call index and arguments, Mutable-only restriction, changed logical-run summary timing, and fresh-genesis storage-version consequence; add no compatibility alias or pretend live migration ceremony before launch.
- [ ] `0.7.3 / S7 / Proposal source`: Treat the missing repository file `aaa.resume.proposal.en.md` as unavailable input and use the accepted semantics captured here plus shipped `0.7.2` behavior as the implementation source until that proposal is restored or intentionally retired.
- [ ] `0.7.3 / S7 / Release evidence`: Synchronize `CHANGELOG.md`, package/runtime release markers, benchmark command provenance, production-Wasm measurements, metadata-derived assertions, wiki state, and release scripts only after every implementation and embedding gate passes.
- [ ] `0.7.3 / S7 / Dependencies`: Depend on all prior slices; documentation may lead contract definition but architecture and release claims must remain subordinate to shipped code and measured artifacts.
- [ ] `0.7.3 / S7 / Exit`: Close only when committed prefixes never replay, successful one-attempt plans write no Continuation state, suspended work retries without a second external trigger, concurrent signals schedule a later run, residual admission charges only the suffix, cancellation preserves committed effects/balances without promotion, production measurements remain work-proportional, and the independent runtime requires no breaking correction.

### DEOS 0.7.3 release gate matrix

- [ ] `0.7.3 / Semantic gate / Prefix and cursor`: Prove every index below the persisted cursor has exactly one final outcome, no later index executes while the cursor step remains unresolved, committed prefix effects never replay, and `ContinueNextStep`/`AbortCycle` retain their shipped `0.7.2` behavior.
- [ ] `0.7.3 / Semantic gate / Retryability`: Prove every task/adapter error maps to one explicit temporary or permanent class, only temporary incapacity creates retry state, permanent failure under `RetryLater` terminates without requeue or wakeup while other policies retain their control flow, and no unclassified error defaults to retry.
- [ ] `0.7.3 / Semantic gate / Logical run`: Prove one `cycle_nonce` spans all admitted attempts, pre-admission weight deferral changes no attempt/failure state, full completion emits one summary and performs success accounting once, and suspension requires no second external trigger.
- [ ] `0.7.3 / Semantic gate / Snapshot and funding`: Prove frozen trigger-percentage inputs remain stable and contain only unresolved-suffix references, live spendability/minimum-balance checks remain current, funding arriving during suspension remains conserved, and pending funding promotes exactly once only after full completion.
- [ ] `0.7.3 / Semantic gate / Scheduler and signals`: Prove Continuation retry reuses canonical effective eligibility plus the paged queue/wakeup substrate, retains at most one queue ticket and one live wakeup per actor, preserves signals received during suspension for the next logical run, and creates no second scheduler or inbox path.
- [ ] `0.7.3 / Semantic gate / Cancellation and close`: Prove explicit cancellation, plan replacement, invalidating schedule/window change, failure threshold, terminal transition, and pure close select their specified deterministic outcome without compensation, committed-effect rollback, funding promotion, hidden tasks, shared-container scans, or post-precondition failure.
- [ ] `0.7.3 / Semantic gate / Mutability and tasks`: Prove `RetryLater` remains Mutable-only at every plan-admission path, Immutable actors expose no cancellation ambiguity, `Mint` remains System-only, and `SwapExactOut` remains available to User and System AAA with unchanged task atomicity.
- [ ] `0.7.3 / Model gate / State machine`: Pass deterministic seeded randomized/model-based sequences over open, execute, suspend, retry, external signal, funding ingress, pause/resume, breaker recovery, program/schedule mutation, cancel, terminal transition, close, and reopen, with shrinking/replayable traces and per-transition cursor, scheduler, funding, and cross-store invariants.
- [ ] `0.7.3 / Performance gate / Work proportionality`: From optimized production Wasm, prove absent Continuation adds no material read/write/ProofSize regression to dormant, negative-readiness, or ordinary one-attempt paths; retry admission and fees scale with the unresolved suffix; and suspension, retry, completion, cancellation, ingress-during-suspension, and close carry generated weights for their actual storage paths.
- [ ] `0.7.3 / Performance gate / Decision rule`: Set no arbitrary percentage target before measurement; reject full-plan reservations, maximum-container proofs, duplicated frozen state, or cached suffix structures that fail to reduce the measured path they claim to bound.
- [ ] `0.7.3 / Evidence gate / Pallet and runtime`: Pass focused and full AAA pallet tests, DEOS runtime integration tests, deterministic model tests, try-runtime integrity checks, metadata-derived discriminant/storage assertions, production-weight verification, and the complete non-TMCTOL Continuation embedding suite.
- [ ] `0.7.3 / Evidence gate / Independent embedding`: Require the second runtime to exercise explicitly temporary adapter retry, unsupported-adapter behavior under every error policy without retry state, suspension, direct ingress and signals during suspension, residual admission, cancellation, pure close, `Mint`/`SwapExactOut` policy, fresh-genesis schema, and zero System-AAA baseline without importing DEOS/TMCTOL topology or helper-only traits.
- [ ] `0.7.3 / Evidence gate / Production`: Build the optimized runtime Wasm, regenerate and bind real production weights, validate RefTime and ProofSize admission, run Cargo formatting/check/tests and workspace/all-target Clippy with warnings denied, and run `./.agents/skills/alignment/scripts/completion-gate.sh` without skips that hide touched AAA/runtime/wiki surfaces.
- [ ] `0.7.3 / Evidence gate / Convergence`: Synchronize specification, architecture, embedding guide, pallet/runtime entrypoints, web-client authoring/status/query surfaces, canonical/indexed read models, bilingual wiki, metadata locks, benchmark evidence, changelog, package/runtime release markers, and release scripts; a stale contract, failed gate, missing independent embedding, unexplained external gate, or breaking portable correction keeps `0.7.3` open and AAA below `1.0`.

## Post-0.7.3 AAA Possibilities

- [~] `Probabilistic Trigger Extension`: Consider probability only as a future append-only progressive trigger extension after a concrete deterministic and financially secure entropy capability exists, has an owned runtime ingress/security model, and carries production ProofSize/weight evidence; `0.7.2` contract contraction does not permanently reject the capability.
- [~] `Immutable Continuation`: Consider `RetryLater` for Immutable actors only after the `0.7.3` independent embedding defines bounded cancellation, permanent adapter failure, terminal handling, and governance-nonintervention semantics.
- [~] `AAA 1.0 Declaration Gate`: Consider the append-only `1.0` line only after completed `0.7.2` work-proportional foundations and the `0.7.3` Continuation independent non-TMCTOL embedding find no remaining breaking correction; any discovered boundary defect must revise the pre-`1.0` candidate and repeat the gate.
- [ ] `AAA Control-Plane Tooling`: After the `0.7.3` consensus contract stabilizes, define canonical plan representation and add plan diff/version history, dry-run amount resolution, fee/weight forecasting, partial-execution simulation, donation-sensitivity classification, governance payload composition, and indexed cycle/funding history without expanding consensus state.

## Product / Client Work

### Wallet and portfolio boundary

- [~] `Wallet Portfolio Boundary`: Any expansion to a full portfolio UX remains blocked until a materialized/indexed asset-discovery surface exists beyond live chain storage

### Web-client product stabilization

- [ ] `Reserved Edge-Lane Growth`: Only if product pressure creates another reserved left/right lane, define the concrete lane role and extend `RESERVED_LANE_SPECS` without reintroducing user-reorderable edge-lane state.
- [ ] `Governance State Separation`: Only if proposal composition or archive work grows enough to create a named ownership conflict, split the state boundary at that concrete seam.
- [ ] `Materialized Provider Boundary`: Only when a second indexed/archive provider family exists, decide whether `adapters/materialized-history/` should become a first-class `materialized/` or `providers/` slice.

## Runtime Framework Evolution

> These slices keep DEOS current with useful Polkadot SDK runtime patterns while preserving the framework boundary: adopt configuration discipline, reusable primitives, and economic mechanisms; do not import unrelated product layers such as Revive contracts by default.
> Source context for agents beyond their training cutoff: Polkadot SDK `stable2606` release notes — <https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2606>.

- [ ] `Runtime Cadence Profile`: Define a cadence profile contract that derives time-sensitive runtime constants from a configurable block-duration target instead of hardcoding one block speed. Exit criteria: audit voting periods, AAA cooldowns/retry windows, staking epochs, cleanup windows, and docs for assumptions that would break when moving between conventional ~6s blocks and faster sub-second / ~500ms profiles; add a validation guard for new block-count assumptions where practical.
- [ ] `V3 Scheduling / Block-Bundling Readiness`: Document and encode a non-enabled readiness profile for future V3 scheduling / block-bundling adoption. Exit criteria: list runtime/operator prerequisites, benchmark and block-weight margin checks, `on_idle` / hook pressure review, message-queue/XCM budget considerations, and a clear condition for moving from legacy scheduling to V3-ready or V3-enabled.
- [ ] `Staking Reward Source Abstraction`: Evolve staking reward ingress so distribution logic is separated from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow. Exit criteria: specify and prototype a minimal runtime/pallet interface for `ExternallyFundedPot`-style reward sources, epoch snapshot timing, pot denominator fixing, and compatibility with current auto-compound claim flows.
- [ ] `Budget Recipient Primitives`: Introduce typed budget-recipient primitives or runtime helpers for framework-owned economic destinations such as staking reward pots, governance treasuries, liquidity reserves, and System AAA actors. Exit criteria: replace any new raw-account economic routing in touched surfaces with typed recipient derivation and decide whether a future mutable registry pallet is justified or overkill.
- [ ] `Unclaimed Reward Policy`: Make staking/native reward leftovers explicit runtime policy instead of implicit residue. Exit criteria: define rollover / return-to-Fee-Sink / burn / treasury-routing options, choose the current reference policy, and cover expiry or settlement behavior with tests.

## Collator Economics & Fee Routing

> Phase 1 uses trusted, permissioned collators, collects 100% of transaction, AAA, governance-opening, and XCM-execution fees in the Fee Sink, and distributes available native balance 50/50 into staking ingress and liquidity provisioning.
> A future permissionless phase may introduce equal security/staking/liquidity thirds only after bounded security-reward settlement ships; indivisible remainder stays in Fee Sink for a later cycle.

- [ ] `Permissionless Collator Reward Contract`: Before assigning the future equal-third security branch, define bounded active-set eligibility, contribution attribution, settlement cadence, custody, payout recipients, unclaimed leftovers, failure behavior, and read-model surfaces; do not assume that a `CollatorRewardPot` account or pallet is the final topology.
- [~] `Phase 2 Reward Routing Preparation`: Keep Phase 2 as a runtime-upgrade boundary, not a launch-time parameter
  - [~] `Claimable LP Nomination Flow`: Activate explicit LP-nomination reward-weight provider only when permissionless collators ship
  - [ ] `LP Nomination Activation`: Expose LP-point nomination to specific collators only when permissionless collator selection is enabled

## Conditional / Externally Gated Work

### Governance execution expansion policy

> Only actionable when a concrete domain-owned control surface, payload family, or failure-state slice is selected beyond the current baseline.

- [ ] `L2 Parameter Expansion`: Only after a genuinely delegated/domain-owned parameter surface exists, add the next `L2ParameterChange` path beyond the Axial Router pair
- [ ] `Execution Observability Expansion`: Only when a new payload family or failure-state slice ships, broaden per-kind observability beyond the current bounded detail/events
- [ ] `Browser Composition Expansion`: Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next composition surface
- [ ] `Governance Archive Integration`: Only when a materialized/indexed governance backend is selected, connect the reserved archive boundary to live archive search and ballot timelines

### Block reward source policy

> Only actionable when the launch economy selects a concrete block subsidy / issuance source instead of assuming one exists.

- [ ] `Block Subsidy Activation`: Only after the reference economy defines a concrete block-reward source and amount policy, decide whether issuance enters the Fee Sink or the future security-reward budget; do not revive immediate author payout by default.

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Native Staking Acquisition Routes`: Only if pool-ratio divergence makes deterministic acquisition insufficient, add router quote comparison, slippage bounds, and fallback behavior

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Relay-Beacon Replacement Contract`: Only if a new parachain-consumable per-block protocol beacon exists, define the replacement contract against that actual surface
- [ ] `Relay-Beacon Proof Ingestion`: Only if that future per-block beacon exists, design a weight-accounted `ConsensusHook` snapshot finalized against the real upstream surface
- [ ] `AAA Relay-Beacon Integration`: Only if that future per-block beacon exists, wire AAA to it and measure proof-size and weight impact
- [ ] `Permissionless Collator Activation`: Only after a production-ready per-block relay/protocol beacon exists, design and prototype activation instead of reviving a local threshold line
