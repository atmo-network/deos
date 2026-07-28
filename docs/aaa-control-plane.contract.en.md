# AAA Control-Plane Contract

> Off-chain contract for representing, reviewing, forecasting, composing, and indexing bounded AAA programs without expanding consensus state.

## Scope

The control plane operates above `pallet-deos-aaa`. It does not change task semantics, scheduler behavior, storage, dispatch indices, or runtime admission. Its first obligation is to preserve an exact relationship between a human-readable plan and the runtime bytes that governance or an owner may submit.

This contract owns executable plan-artifact identity, structural diff rules, forecast provenance, simulation boundaries, governance composition inputs, and materialized history classification. The AAA specification remains authoritative for semantics; runtime metadata and SCALE remain authoritative for concrete encoding.

## Canonical Executable Plan

A canonical executable plan is a chain-bound artifact with these required fields:

| Field | Encoding | Meaning |
| --- | --- | --- |
| `format` | UTF-8 literal | `deos.aaa.plan` |
| `formatVersion` | unsigned integer | Control-plane envelope version; initially `1` |
| `genesisHash` | `0x`-prefixed 32-byte hex | Target chain identity |
| `specVersion` | unsigned integer | Runtime semantic compatibility marker |
| `transactionVersion` | unsigned integer | Signed-call compatibility marker |
| `metadataHash` | `0x`-prefixed 32-byte hex | `blake2_256` of the exact runtime metadata bytes used for encoding |
| `aaaType` | `User` or `System` | Runtime `AaaType` admission context |
| `mutability` | `Mutable` or `Immutable` | Runtime admission and `RetryLater` context |
| `programScale` | `0x`-prefixed SCALE bytes | Concrete runtime `ProgramInput` value |
| `planId` | `0x`-prefixed 32-byte hex | Deterministic artifact identity |

`programScale` is the canonical program representation. JSON objects, form state, labels, comments, token symbols, decimal display amounts, and generated previews are projections only. They must never substitute for exact runtime types or enter `planId` implicitly.

`planId` is `blake2_256("deos:aaa-plan:v1" || LE32(specVersion) || LE32(transactionVersion) || genesisHash || metadataHash || SCALE(aaaType) || SCALE(mutability) || programScale)`. The enum bytes come from the identified metadata; integer components use fixed little-endian encoding, never host-language stringification.

A canonical artifact is executable only when live genesis hash, runtime versions, and metadata hash match the envelope. A mismatch requires explicit re-decode, revalidation, re-encoding, and a new `planId`; tooling must not silently rebind stale bytes.

## Configuration IR and Tokenomic Genome

`deos.aaa.configuration-ir` version `1` is the format-neutral typed authoring structure. It preserves actor type, mutability, completion policy, trigger/admission policy, exact observation feeds, cooldown and schedule window, funding policy, ordered linear steps, exact `ConditionSet`, task parameters, amount resolutions, and failure policy. Presentation-only step keys never enter the IR.

JSON, TOML, and structured Markdown adapters preserve the same normalized IR. The TOML and Markdown forms store each top-level typed field as exact JSON data, so adapter grammar cannot reinterpret runtime concepts. Comments, surrounding Markdown prose, object-key order, whitespace, and file extension do not affect normalization, canonical lowering, SCALE bytes, or `planId`.

Normalization rejects unknown format versions, missing required fields, and unknown top-level fields. Diagnostics reuse canonical authoring validation; structural diff reports deterministic JSON-pointer paths. Acceptance requires `parse -> IR -> canonical authoring lower -> metadata decode/re-encode` equality. The IR is an off-chain convenience surface, not another runtime language, recipe engine, or consensus rule.

## Human Projection

A human projection must decode `programScale` through the exact metadata identified by `metadataHash`. It must show every `ProgramInput`, trigger, schedule window, funding policy, step, exact `ConditionSet` mode, atomic condition, task, amount-resolution variant, asset id, account id, ratio, and error policy including the exact nonzero `RetryLater.maxAttempts` payload without lossy defaults. `Always`, `All`, and `Any` remain distinct typed objects rather than an ambiguous raw array.

Balances and identifiers use full base-unit decimal strings in transport JSON. Accounts and opaque bytes use canonical hex. Ratios expose their integer `Perbill` parts as well as optional display percentages. Token symbols, labels, localized prose, and decimal formatting remain annotations resolved from an explicitly identified registry snapshot.

Projection acceptance requires `decode(programScale) -> projection -> encode == programScale`. Unknown variants, missing fields, overflow, noncanonical numbers, or metadata lookup failure reject the artifact instead of producing a partial editable plan.

Trigger projection preserves source and admission as independent typed dimensions. `Immediate` and `Cadenced::{Always, WhenSignalled}` remain explicit; Manual, AddressEvent, and `OnObservationChange { feed }` atoms stay in one bounded OR-only source set rather than becoming conditions or graph edges. Observation source projection preserves the complete typed feed identity and introduces no threshold, callback, amount, or revision promise. Typed authoring rejects `PercentageOfTrigger` under Manual, observation, cadence-only, or mixed readiness before encoding; AddressEvent-only plans additionally require every source asset filter to cover each authored snapshot surface, with `Any` required when an Unstake receipt asset cannot be derived locally. Authoring normalizes whitelist members and source atoms by canonical SCALE bytes, rejects semantic duplicates and empty/oversized source sets, and never lets Manual bypass a Cadenced gate.

## Observation Inspection

The control plane reads bounded `Oracle.FeedIds` and only the selected exact feed/configuration and current-observation keys at one finalized hash. It displays the complete directional identity, decimal scale and exact formatting, producer, provenance, aggregation, lifecycle, update block, change-only revision, authored maximum age, current age, and Fresh/Stale/Unavailable/Uninitialized status as direct canonical-chain storage truth.

Latest-state fanout remains reconsideration-only. Equal-value publication may refresh `updated_at` without incrementing revision; dirty revisions may coalesce before subscriber execution, and the inspector promises neither intermediate-revision delivery nor per-revision execution. Historical samples, timelines, and alerts require an explicitly materialized provider.

The reactive delivery projection MUST fail closed unless DEOS Oracle revision, AAA dirty state, active-list topology, subscriber-page topology, and finalized block come from one snapshot. It reports Clean, PendingFanout, FanoutInProgress, or AwaitingCleanup; exact `dirty_since` age; latest/fanout revisions; active-list head/tail/count/cursor and zero-based selected/cursor positions; next subscriber page; occupied and current-revision page counts; and remaining fanout service units. One service unit represents one occupied-page attempt or a pending restart/cleanup transition when no page cursor exists; completing a final page may clear or restart the feed within that page unit.

The control plane derives an exclusive-budget lower bound and a conservative fair-service ceiling from one service-topology function. The ceiling counts round-robin distance from the current fair cursor and one selected-feed turn per active feed until selected fanout completion.

Each available estimate carries a context identity over revisions, page cursor/counts, active count, selected/cursor positions, and budget evidence. Any changed input produces a different context rather than silently preserving the old ceiling.

Both estimates require stable active topology, no newer selected-feed revision, available identified RefTime/ProofSize service budget, and eventual queue capacity. They end at fanout or cleanup and never predict queue admission, condition evaluation, actor execution, or one exact future block.

One deterministic client projection MUST derive expected runtime versions, compact runtime-code identity, exact V16 metadata identity, generated descriptor identity, AAA weight identity, fanout Weight values, and effective service bounds from their canonical files. Changed-scope validation MUST reject a stale projection. Live numerical estimates additionally require finalized runtime/code/metadata evidence matching that projection; mismatch withholds estimates without hiding factual chain state.

Optional selected-actor inspection reads only exact `ActorHot(aaa_id)` at that finalized hash. It reports actor existence, pending signal, type-derived System/User lane, queue ticket or wakeup block/page/slot, and one factual status: ActorMissing, NoPendingSignal, PendingQueueAdmission, Queued, or WakeupScheduled. Queue and wakeup pointers are mutually exclusive. PendingQueueAdmission means a signal lacks both paths at the snapshot; it does not predict when capacity returns.

`AxialRouterPreExecutionReserves` identifies local pool reserves observed before Router execution. The inspection surface MUST state that this is not an external fair-price, manipulation-resistance, MEV-protection, or ordering proof. Authored slippage, System reference-deviation guards, and execution-time conditions remain separate contracts.

## Linear Authoring

The canonical authoring model is a typed ordered `Step[]`, not a graph or a generic DSL. Each row exposes one `Always`, `All conditions`, or `Any condition` mode, one task with typed parameters and amount resolutions when present, and one error policy. `RetryLater` exposes a required `maxAttempts` integer from `1` through `u32::MAX`; the initial unsuccessful attempt counts as `1`, and `1` means immediate close without suspension. `Always` hides atom editors; `All` and `Any` require at least one atom. Fieldless `StopCycle` exposes no synthetic parameter. Off-chain step keys may stabilize editing, but lowering removes them and preserves only exact runtime order.

Authoring follows `select trigger sources and admission → add/reorder step → select condition mode and atoms → select task → configure typed parameters → select error policy → validate → analyze → forecast/simulate → encode`. Mode changes preserve atoms only through an explicit lossless operation. Validation rejects empty groups and enforces actor and primitive bounds, bounded canonical trigger sources, Mutable-only retry with a nonzero `u32` attempt limit, System-only minting, bounded/unique split and allowlist values, canonical integer/ratio/address shapes, and active-plan cardinality before canonical encoding.

`authoring.ts` lowers each typed field directly to the metadata-discovered `ProgramInput` shape, then delegates SCALE bytes and `planId` to the canonical artifact codec. Reordering changes array order only; no authoring operation creates a successor index, branch, callback, nested program, runtime call, recipe identity, or runtime dependency on presentation state. `StopCycle` reveals its fixed successful terminal transition directly and never accepts a target cursor.

A high-level recipe may exist only when deterministic lowering reveals the complete editable ordered steps before artifact creation. Recipe labels never enter runtime bytes, governance semantics, or `planId` except through the exact lowered `ProgramInput` they produce.

The canonical reactive-authoring fixture uses `OnObservationChange → All(ObservationBelow, BalanceAbove) → SwapIn → RetryLater`. The same explicit plan supports `Persistent` and `CloseAfterProductiveRun`; every feed, threshold, asset, amount, slippage bound, freshness window, and retry limit remains authored policy rather than a runtime bucket default.

The reference client presents a numbered linear rail with explicit `Then step N` progression and no successor control. It binds exact artifacts only against metadata and runtime versions read at one finalized block. Forecast, adapter-local simulation, and matching-Wasm simulation occupy distinct evidence lanes; an unrun lane remains visibly unrun and cannot inherit another lane's provenance.

## Structural Diff and Version History

A structural diff compares decoded typed trees only when both artifacts share `genesisHash` and `metadataHash`. It reports additions, removals, moves, and field changes by stable structural path. Array position is semantic for execution-plan steps and conditions; tooling must not sort them for presentation.

Artifacts with different metadata hashes are `IncompatibleUntilRebound`. A migration-aware tool may decode each side with its own metadata and present a named comparison, but it must not claim byte-level or dispatch compatibility.

Version history is materialized truth. An indexer may correlate artifacts with actor calls and lifecycle events, but consensus stores no plan archive. Every history item records source transaction/block identity, observed finality, `planId`, target actor or creation intent, and whether artifact bytes were available or reconstructed.

## Forecast and Dry-Run Provenance

Every forecast records the canonical `planId`, block hash or state snapshot, metadata hash, and runtime API or local model version used. Results are advisory and become stale when any dependency changes.

Weight forecasts must preserve RefTime and ProofSize separately. Fee forecasts identify evaluation, execution upper bound, fee conversion, and lifecycle overhead rather than returning one unexplained number. Amount resolution identifies live balances, trigger snapshots, minimum-balance constraints, fee reservation, and adapter quotes used by each step.

Local simulation cannot claim runtime truth unless it executes the matching runtime Wasm against the identified state snapshot. Heuristic or adapter-local projections must carry a narrower provenance label and may not authorize submission automatically.

A matching-runtime request binds `planId`, genesis, block hash and number, state root/source, runtime-code hash, metadata hash, runtime versions, and runtime API identity. The provider must echo the complete pin and return canonical SCALE result bytes. Client-side hash and echo validation prevents accidental identity drift; it does not prove that an untrusted provider executed the runtime, so the provider or verified executor remains an explicit evidence boundary.

## Static Program Analysis

`ProgramStaticAnalysis` is a deterministic `StaticStructuralProjection` over one validated canonical artifact. It binds `planId`, genesis, metadata hash, runtime-model identity, weight-model identity, optional adapter-capability identity, and analyzer version. It decodes through the canonical plan codec and delegates cost aggregation to the existing forecast contract; it owns neither another SCALE implementation nor another weight calculator. The versioned semantic manifest is generated from the exhaustive package contract, covers every task and amount variant in SCALE order, and fails release validation when its committed client artifact becomes stale. The same quick/full release gate decodes canonical fixtures through TypeScript analysis and compares the complete classified task and amount contract, so package and control-plane evidence cannot pass independently while drifting.

Trigger analysis reports `Immediate`, `CadencedAlways`, or `CadencedWhenSignalled`, exact cadence where present, source count, Manual/AddressEvent/ObservationChange source kinds, and exact observation feed projections. `ExternallySignalledAdmission` and `PeriodicAdmission` remain factual structural findings; they never claim scheduler position, signal presence, queue admission, or runtime execution. Analyzer version `9` retains `TriggerAmountCompatibilityViolation` and projects exact `Persistent | CloseAfterProductiveRun` policy from metadata. Typed authoring rejects uncovered trigger-amount filters and unknown completion variants before encoding.

Every ordered step reports exact aggregate mode, atomic condition count, observation surfaces, condition RefTime/ProofSize and evaluation-fee upper bounds, possible execute/skip admission, task, lossless typed parameters, amount semantics, error-policy variant and retry-attempt limit when present, one successful control, separate possible failure controls, adapter requirements, economic effects, recipient and signal surfaces, observation windows, Temporary-failure reachability, and Continuation eligibility. Successful control is `advance` or `complete-cycle`; failure controls remain limited to `advance`, `terminate`, and Mutable-only `stutter-current`. Analysis never merges success and failure into one ambiguous control set or emits an arbitrary successor. `SplitTransferDepositPreflight` states that every non-zero recipient allocation must be depositable at execution time and one rejection fails the whole task Temporary and atomically. Optional `AaaMinimumBalanceEvidence` binds an explicit identity and finalized block hash to per-asset minimum balances and observed recipient balances. Only a fixed leg with observed zero recipient balance and `0 < floor(total × share) < minimum` emits `SplitTransferLegBelowKnownMinimum`; missing asset, recipient, or balance evidence stays silent rather than predicting viability. The analysis identity echoes the evidence identity and block hash. System swap rows additionally emit `SystemReferenceDeviationGuard`: the reference is a fresh EMA or direct reserve fallback, acts only as a local execution guard, may reflect manipulated pool state, and proves neither fair price nor transaction-order protection. User swap rows omit this System-only finding.

`Any` analysis states that several predicates may be true while the task executes at most once. It remains one linear step rather than multiple edges. Forward dependencies connect earlier task writes to atomic reads without claiming a branch, and costs use total configured atom count regardless of ordering or truth position.

`StopCycle` analysis reports no adapter, amount, asset, recipient, economic effect, Temporary failure, or Continuation eligibility. Completion-policy authoring distinguishes a persistent strategy from one-shot productive closure and states that only a committed effectful task qualifies; false/latest-state conditions, skips, failed rollback, suspension, abort, retry exhaustion, and bare `StopCycle` do not consume the one-shot lifecycle. Its successful control is cycle completion at the current index; condition failure remains ordinary advance, while pre-execution condition or fee failure remains governed by the configured error policy. `StopCycleFailureMayFallThrough` identifies `ContinueNextStep`, the exact row, and whether its suffix contains economic effects. Authoring shows the same warning. Every suffix envelope remains a structural maximum unless separately proven path-sensitive evidence applies.

Authoring presentation distinguishes a false condition, resolution skip, funding unavailability, Temporary task failure, and Permanent task failure. The stored `AbortCycle` variant is shown as abort-on-task-failure behavior: ordinary condition and resolution skips advance, funding unavailability advances for Abort/Continue but suspends under Mutable Retry, and only task or pre-execution errors follow the failure-policy termination path.

Forward data dependencies report an earlier asset write read by a later condition, task, or amount surface. They describe behavioral predication and live/frozen data flow, not workflow branches. Without supplied runtime observations, analysis MUST NOT claim current balances, adapter health, active Continuation state, execution outcome, or scheduler position.

For every cursor from zero through plan length, the analyzer emits one suffix envelope with remaining-step count, maximum RefTime and ProofSize, evaluation and execution fee upper bounds, lifecycle and funding-promotion overhead, adapters, asset surfaces, committed effect classes, and retryable indexes. Suffix zero is the full-program envelope; remaining-step count decreases exactly once per cursor. Every result stays bound to the artifact and model identities used to derive it.

Findings remain factual and unscored: trigger admission shape, committed effects before a retryable step, retry-live observations, current-balance mixing after an earlier write, unsupported/unknown adapters, unknown Temporary-failure classification, potential actor-signal edges, budget-relative ProofSize dominance, and the conditional administrative actions that invalidate an active Continuation. Capability claims remain unknown unless an identified profile proves them.

## Closed-Loop Feedback Projection

`AaaFeedbackModel` consumes identified `ProgramStaticAnalysis` results rather than decoding plans or recreating task semantics. It builds a deterministic bounded graph over actors, typed observation feeds, shared assets, exact actor-account signal recipients, and declared future parameter actuators. Observation-to-actor edges come only from exact trigger or condition feed identity; actor effects come from existing analyzed steps plus explicit typed effect matchers.

Each observation is classified as `Endogenous`, `Exogenous`, or `Unknown`. Exogenous observations cannot declare actor-effect matchers, duplicate feed projections fail closed, and actuator controllers and targets must resolve exactly. Shared-asset edges show structural read/write coupling; they do not claim that one balance movement caused a later action. Parameter actuators remain explicit declared nodes rather than inferred runtime capabilities.

Every graph edge belongs to exactly one `ReactiveCausal`, `ResourceCoupling`, `Coordination`, or `DeclaredExternalCausality` family and carries `RuntimeDerived`, `ArtifactDerived`, `Declared`, or `Unknown` provenance plus every supplying identity. Observation trigger/read and declared actor effects form reactive causality; exact actor signals form coordination; asset reads/writes remain resource coupling.

Artifact-derived edges identify the canonical plan; observation edges also identify known observation evidence; declared actor effects identify both declaration and plan. Runtime producer `AxialRouterPreExecutionReserves` derives endogenous provenance; explicitly external producers derive exogenous provenance; absent producer evidence stays `Unknown`. Mixed actor runtime/metadata/weight/analyzer contexts and provenance/evidence contradictions fail closed before graph construction.

Observation lifecycle comes from the same evidence reference. Active and paused observations retain structural causal candidates; deactivated observations remain visible but cannot contribute actor/parameter effects to a reactive SCC. Router producer/lifecycle requires runtime-derived evidence, external provenance requires a declaration identity, and unknown producer/lifecycle carries no identity.

Strongly connected components use causal, coordination, and declared-external edges only. A reactive component must include an observation and a genuine reactive-causal edge. Each cycle reports its canonical node path and per-edge kind, family, and provenance under fixed node/edge ceilings. Shared-resource SCCs never produce `ReactiveSelfCycle` or `ReactiveCrossActorCycle`.

Resource identity uses the narrowest proven scope. A runtime-derived sovereign account produces `AccountAsset`; a missing account produces actor-scoped `Unknown`; a declared parameter effect may name `AssetClass`. Adapters may add exact runtime/artifact-derived `Pool`, `Reserve`, or `Tmc` touches. Declared exact market resources fail closed.

Equal asset symbols under distinct accounts and equal asset pairs under distinct pool identities never merge. Shared-resource findings require multiple actors on one exact identity; pool, reserve, TMC, and asset coupling remain distinct. `PotentialResourceContention` additionally requires multiple writers. No resource edge participates in reactive SCC analysis.

Sovereign accounts require runtime-derived evidence from one finalized state identity. Exact actor-signal edges carry the recipient account's state identity beside the sender plan identity. Declared accounts, duplicate accounts, and mixed finalized-state identities fail closed.

Reactive findings remain factual and unscored. Structural evidence may report endogenous feedback, self/cross-actor cycles, and shared-observation actuator contention. Timing and policy findings require one explicit evidence snapshot identifying runtime, weights, cadence, estimated delivery, hysteresis/persistence, declared gain, and reactive-ingress priority; absent or unknown evidence produces no claim.

The static analyzer derives actor cooldown directly from canonical `ProgramInput.Active.schedule.cooldown_blocks`; dormant programs expose no cooldown. The current linear Condition language has no stateful hysteresis or temporal persistence primitive, so threshold feedback derives their absence from the plan instead of accepting policy declarations. Chatter findings carry the plan identity.

Delivery and cadence require runtime-derived evidence, gain remains declared or unknown, and reactive-ingress priority remains runtime-derived or unknown. Wrong provenance, identity substitution, and known/unknown disagreement fail closed.

Timing/policy findings additionally require verified runtime code, V16 metadata, production AAA weight identity, and exact generated fanout limits for service units, active dirty feeds, and subscriber pages. Runtime or constant mismatch preserves the structural graph and supplied factual snapshot under `EvidenceMismatch` but emits no evidence-bound timing/policy finding. A caller cannot promote drift to `Verified` by supplying the expected label alone.

The evidence-bound set covers freshness below the estimated delivery envelope, threshold chatter possibility, missing hysteresis or persistence, declared high gain, cooldown/feed-rate mismatch, and a System actor explicitly declared on ordinary reactive ingress. Every such row carries the snapshot identity. The model never infers gain, exact delivery time, scheduler service, instability, exploitability, or harm.

The falsification corpus covers price → swap → price, fee funding → downstream market action → price, actor funding → downstream actor activation, explicit parameter-policy effects, and unmatched exogenous observations. The projection is off-chain control-plane evidence and creates no consensus rule, scheduler priority, or execution authority.

## Matching-Runtime Simulation Provider

The first runtime provider simulates one attempt of an existing active actor whose stored `ProgramInput::Active`, `AaaType`, and `Mutability` exactly match the validated artifact. It supports an idle actor's next fresh cycle and a suspended actor's next Continuation attempt. It does not simulate creation, dormant activation, a proposed replacement program, scheduler throughput, queue position, or future block timing.

The request carries `aaa_id`, exact decoded program, actor type, mutability, and mode `FreshCurrentPlan | CurrentContinuation`. `FreshCurrentPlan` requires idle run state and starts at cursor `0` with the next cycle nonce. `CurrentContinuation` requires suspended run state and reuses the stored nonce, unresolved cursor, trigger snapshot, and cumulative outcomes while incrementing the attempt exactly once.

The runtime API runs only after normal liveness, lifecycle, window, nonce, fee-budget, and Continuation invariants pass. A mismatch or unavailable prerequisite returns a bounded typed error and performs no task. The API remains bounded by the actor's admitted plan, configured maximum steps, existing task weights, and the same adapter calls as production execution; it must not inspect an unbounded event or storage history.

The minimum result carries status `Completed | Aborted | Suspended`, cycle nonce, attempt, start cursor, optional unresolved cursor, finalized-through index, cumulative outcome totals, ordered bounded step outcomes, and canonical SCALE result bytes. A suspended result keeps its cursor unresolved; completed or aborted results expose no live Continuation cursor.

The entire API call executes inside an outer rollback transaction. Successful task effects remain visible to later simulated suffix tasks, failed task-local effects roll back under the existing pallet transaction boundary, and all simulated writes, events, fees, scheduler changes, funding promotion, closure, and adapter effects roll back before the API returns. Explicit rollback remains mandatory even when the host normally discards runtime-API overlays.

A provider calls this API against the exact finalized block named by the request, obtains runtime code and metadata from that same state, and returns their hashes with the block header state root. Remote RPC execution remains trusted-provider evidence unless a verified local executor or state proof independently establishes the same code and state.

## Partial Execution and Donation Sensitivity

Simulation follows task-scoped atomicity and non-atomic plans. It must preserve successful prefixes, roll back failed task-local effects, apply `AbortCycle`, `ContinueNextStep`, or Mutable-only `RetryLater { maxAttempts }`, and expose the unresolved cursor plus cursor-local unsuccessful-attempt count without inventing compensation. Local exhaustion reports `Closed / RetryAttemptsExhausted`; finalized runtime simulation carries the same typed close reason while remaining the stronger execution projection.

Donation sensitivity classifies which resolved amounts can change when third parties transfer assets into actor-controlled or adapter-observed accounts before execution. The result identifies the affected step and amount surface; it does not predict external behavior or treat a donation as an attack by default.

## Governance Composition

Governance composition consumes a validated canonical artifact and a separately selected target/action. It must show the exact runtime call, origin/domain requirement, encoded call bytes, preimage or payload hash when applicable, and the artifact `planId`.

The plan artifact does not contain a signature, signer, nonce, tip, proposal advocacy, or governance decision. Signing and submission remain explicit approval boundaries. A composed payload becomes stale under the same runtime identity rules as its source plan.

## Read-Model Boundary

Canonical-chain truth includes current bounded actor program/state, dispatch outcomes, events, and runtime metadata at an identified block. Plan files, diffs, forecasts, simulations, annotations, and long version/cycle/funding histories are local or materialized truth.

Provider failure must degrade to the narrower live-chain surface with an explicit unavailable or stale state. The client must not synthesize archive continuity from session cache or present reconstructed plan bytes as directly stored chain artifacts.

## Scenario Corpus

The corpus tests language coverage rather than market viability. `Expressible` means one or more canonical bounded plans preserve the stated mechanism. `Partial` means the task grammar covers the execution core but a named policy observation, configured reference actor, or external truth remains absent. `Unavailable` means canonical atoms and parameters cannot represent the load-bearing decision without a new primitive or external decomposition.

| # | Scenario | Canonical plan sketch | Class | Boundary and cost |
| --- | --- | --- | --- | --- |
| 1 | DEOS Burn Actor | Address event → per configured foreign asset: `All(BalanceAbove(dust))` + `SwapIn` → `Burn(AllBalance native)` | Expressible | Plan length bounds the configured asset set. Every reached atom and swap adds evaluation, task Weight, and fee exposure; failed swaps follow their explicit policy. |
| 2 | DEOS Fee Sink phase-one allocation | Address event → native `SplitTransfer` to staking ingress and liquidity ingress | Expressible | One atomic bounded fan-out owns the 50/50 runtime policy. Indivisible remainder stays under task arithmetic; recipients never become dynamic successors. |
| 3 | DEOS `$BLDR` splitter | Address event → `All(BalanceAbove(dust))` → bounded `$BLDR` `SplitTransfer` | Expressible | A false threshold skips one task. Multiple recipients increase the generated split-leg class, not control-flow power. |
| 4 | DEOS Liquidity Actor provisioning | Address event or timer → optional swap → `AddLiquidity` or `DonateLiquidity` | Partial | Typed tasks and retry semantics exist, but the reference actor remains dormant until reserve, pair, slippage, ratio, and funding policy become concrete. Adapter quotes remain runtime-local. |
| 5 | Periodic DCA | `Cadenced::Always` → `SwapIn(Fixed or PercentageOfCurrent)` | Expressible | One-step DCA needs no Continuation. Each tick pays one task envelope and any User evaluation/execution fees; market outcome remains adapter-dependent. |
| 6 | Threshold payroll or treasury fan-out | `All(BalanceAbove(asset, threshold), BlockNumberAbove(start))` → `SplitTransfer` | Expressible | Every atom runs even after false truth. Fan-out stays atomic and bounded; recurring cadence belongs to the trigger rather than a loop. |
| 7 | Resilient swap-to-liquidity pipeline | `SwapIn → AddLiquidity → Transfer`, with bounded `RetryLater { maxAttempts }` only where Temporary failure is meaningful | Expressible | Successful prefixes remain committed. Suspension stores one scalar cursor and required frozen suffix snapshots; retry re-evaluates live conditions without replaying the prefix. |
| 8 | Stake then maturity-gated unstake | Stake plan plus a later timer/block-gated Unstake plan | Partial | Basic stake and unstake tasks exist, but reward, maturity, health, or exchange-rate predicates have no atomic condition. Separate actors add custody, scheduling, latency, and operational cost. |
| 9 | Oracle-price take-profit or stop-loss | Fresh typed price observation → swap or successful stop | Expressible | Observation conditions compare one explicitly authored feed identity and raw scalar threshold. Unavailable, uninitialized, or stale truth skips the task; local-pool provenance does not imply external fair price. |
| 10 | Dynamic target-ratio portfolio rebalance | Observe portfolio ratio → choose assets and calculated amounts → trade toward target | Unavailable | The language has no ratio predicate, arithmetic expression, dynamic asset selection, or iterative convergence. A bounded runtime adapter could own one typed future mechanism only after concrete demand and weights. |
| 11 | Descending buy buckets | Independent one-shot actors: price change → `All(ObservationBelow(level), BalanceAbove(spend))` → `SwapIn` → bounded retry | Expressible | DEOS Router produces local pre-execution samples, DEOS Oracle owns current truth, and AAA owns reaction. Non-fresh truth skips; retry exhaustion closes only that bucket. |
| 12 | Ascending sell buckets | Independent one-shot actors: reverse-price change → `All(ObservationAbove(level), BalanceAbove(sell))` → `SwapIn` → bounded retry | Expressible | Every bucket authors its feed, threshold, amount, slippage, and retry limit. Local-pool provenance proves neither fair price nor ordering protection. |
| 13 | Treasury reserve-ratio reaction | Manual `SplitTransfer` execution core; treasury-ratio predicate absent | Partial | Current Oracle meaning covers directional local-pool price, not treasury balance ratio. No producer or truth owner exists; automatic execution must remain disabled rather than mislabeling a price feed. |
| 14 | Liquidity-depth reaction | Manual `AddLiquidity` execution core; absolute-depth predicate absent | Partial | Current price feeds expose a ratio, not absolute reserve depth. A future typed producer and meaning must own depth before the core can become reactive. |
| 15 | Non-price block-height release | Cadence → `BlockNumberAbove` → one-shot `Transfer` | Expressible | FRAME System/runtime block number owns canonical current truth. Before the threshold the condition skips; transfer failure follows the authored policy. |

Cost presentation uses configured atomic count, task-specific generated RefTime/ProofSize, execution-fee upper bounds, lifecycle overhead, and optional Continuation overhead. `Any` with several true atoms still prices every atom and executes one task. Static suffix envelopes remain structural maxima even after a reachable `StopCycle`; path-sensitive savings require separately proven runtime evidence.

The corpus authorizes no new primitive in `0.7.7`. Partial and unavailable rows remain factual mechanism gaps. A partial fixture may lower only its bounded execution core; tooling must keep it manual and label the missing decision surface. Product tooling may propose decomposition into actors only when it exposes additional custody, fee, latency, scheduler, governance, and operational consequences.

## Validation Contract

Control-plane implementations must cover:

- Deterministic `planId` fixtures and rejection of malformed hex, overflow, stale metadata, and wrong-chain artifacts.
- Exact SCALE decode/project/re-encode round trips for every current plan variant.
- Ordered structural diffs, including incompatible-metadata classification.
- Separate RefTime, ProofSize, fee, state-snapshot, and quote provenance.
- Task rollback, committed-prefix, Continuation cursor, and donation-sensitivity scenarios.
- Runtime-provider rejection for artifact/program mismatch, wrong mode/run state, unavailable fee budget, stale code/state identity, and any write escaping the outer rollback.
- Governance payload byte visibility without implicit signing or submission.
- Finality, reorg, duplicate, replay, and missing-artifact behavior for indexed histories.

No control-plane test substitutes for pallet tests, runtime integration, production weights, or live operator authorization.
