# DEOS Web Client Architecture Notes

This note fixes the stable architecture vocabulary for the repository-local web client. It complements [`../README.md`](../README.md): the README is the workspace entrypoint, while this document is the durable implementation contract for the shipped client architecture.

The goal is to describe the implementation truth, subsystem boundaries, and contracts used by the reference product surface, not to track release planning or intermediate refactors.

## 1. Product Role

The web client is the browser-facing DEOS reference client for the current TMCTOL standard.

It is not the source of protocol truth. Runtime contracts and `/docs` remain authoritative. The client should expose bounded chain truth honestly, label materialized or session-derived data clearly, and keep user actions understandable before signing.

The current product shape includes:

- wallet/session/account selection;
- bounded balances and tracked-asset transfers;
- DEOS Router quotes and signed swap execution;
- native staking views and signed staking/governance-custody actions;
- governance viewing, voting, advisory submission, tactical treasury invoice submission, preimage review, and runtime-upgrade relay guidance;
- automation, status, statistics, chart, and execution-feedback surfaces;
- generated wiki reading from repo-local trusted markdown.

## 2. Vocabulary Boundary

### 2.1 Widgets

A `widget` is a user-facing functional surface. It exists because it answers a product/domain question for the user.

Examples:

- `SwapWidget` — trade preview and execution.
- `WalletWidget` — balances, receive address, and bounded sends for the selected account.
- `StakingWidget` — native staking facts and signed staking/governance-custody actions, including capability-gated liquid reward claims and atomic compound with explicit finalized epoch, operator, and minimum LP output.
- `GovernanceWidget` — proposals, votes, submissions, preimage review, and bounded outcomes.
- `LogWidget` — canonical transaction, receipt, finalization, and network-feed feedback.
- `WikiWidget` — generated wiki navigation and trusted markdown reading.
- `AccountWidget`, `SettingsWidget`, `StatusWidget`, and `AccountChip` — shell-adjacent widgets hosted in reserved lanes.

Widgets may consume stores, contracts, UI Kit, and adapter facades. They must not import concrete adapter internals.

### 2.2 Layout

`layout` is spatial infrastructure. It arranges widgets but does not define product semantics.

Layout owns:

- workspace frame;
- center tile tree;
- panes, tabs, split handles, drop overlays;
- reserved header/footer/sidebar lanes;
- default topology and mobile linearization.

Files such as `WorkspaceFrame`, `TileContainer`, `PaneHost`, `SplitHandle`, `AppHeader`, `AppFooter`, and `SidebarPanel` belong under `src/lib/layout/`, not under widgets.

### 2.3 Reserved Edge Lanes

Header, footer, and sidebar are reserved edge lanes outside the user-reorderable center pane tree.

Their widget sets are developer-configured through layout specs. Mobile may intentionally map a different widget set into reserved lanes than desktop. Do not reintroduce user-reorderable edge-lane state without a concrete product reason.

## 3. Read-Model Honesty

The client follows the project-wide read-model split:

- `canonical-chain` — bounded on-chain state/projections intended for live client use;
- `materialized` — indexed/archive/search/analytics views outside consensus truth.

The browser realization axis is separate:

- `direct`;
- `session-cache`;
- `session-derived`;
- `provider`.

A session-built chart or retained UI panel must not masquerade as archive truth. A future archive/search/dashboard surface must declare its materialized provider boundary explicitly. `ReadModelValue.fetchedAt` is only a browser observation timestamp for cache/session freshness; canonical chain time or finality must come from bounded chain facts such as `asOfBlock` / `asOfHash` when those facts matter.

The automation widget reads known System actors from `Actors.ActorIdentities`, `Actors.ActorHot`, compact `Actors.ActorContractHead`, sparse `Actors.ActorRunHead`, and `Actors.ActorFunding` at one finalized block. It does not reconstruct physical Contract tails or Run payloads. Authoring permits `0..=MaxContractSteps`; deleting the final Step exposes an explicit Opening-only state rather than fabricating an Action. Current cursor and unsuccessful-attempt count at that cursor are canonical-chain truth; no cycle-global attempt ordinal exists. Historical attempt timelines remain materialized. The automation authoring, analysis, and local simulation contracts derive `MaxContractSteps` and `MaxRetryAttempts` from the generated Actors ABI manifest; RetryLater remains Mutable-only and does not fabricate adapter retryability.

Actors exposes no permanent cache epoch or generic revalidation progress surface. Weight-, adapter-, or envelope-affecting upgrades use their concrete migration contract and finite semantic Weight-class proof rather than client-visible per-actor cache repair state.

The widget also consumes the read-only `ActorEligibilityApi::actor_eligibility` projection (`automation/eligibility.ts` plus the `actor-eligibility` blockchain adapter) for each actor: `NotRegistered`, `Dormant`, or the canonical Active classification with terminal reason and exact execution-phase payload at the same finalized block. Generated metadata binds the complete terminal-reason set, and unknown eligibility, reason, phase, or typed-failure variants fail closed instead of entering display state. The browser never reimplements cadence phase, cooldown, schedule window, retry backoff, breaker, or latch arithmetic; when the runtime API is unavailable the row shows `Unavailable` instead of guessing. The projection never promises service, because queue position and available Weight decide actual admission.

`automation/materialization.ts` and the `actor-materialization` blockchain adapter preserve the named `CrossingCapacity` dimensions and all three current `MaterializationFaults` families from `ActorEligibilityApi` v6. Both calls use one caller-selected finalized block; absent faults remain explicit nulls, exact fault coordinates remain typed, and unknown classes or wakeup keys fail closed. This is bounded current canonical-chain truth, not fault history or inferred scheduler health.

`automation/resource.ts` and the `actor-resource` blockchain adapter preserve configured resource budgets, transient current-block state, and latest finalized telemetry from `ActorResourceApi` without reconstructing Resource Algebra in the browser. The three reads share one caller-selected finalized block; absent current state and absent telemetry remain distinct null projections. Current state is authoritative for its tagged block, while finalized telemetry remains bounded non-authoritative operational evidence rather than archive history.

`automation/cost.ts` owns the independent browser contract for `ActorCostApi` and `ActionFeeCharged`: Creation, family-specific Trigger, Pipeline Machine/cleanup, maximum next Action, actual Action receipt, and state hold retain named fields and identities without an activation total or remaining machine budget. `adapters/blockchain/actor-cost.ts` pins the typed API call to the caller's finalized block; unknown variants, malformed identities, inconsistent Pipeline/hold totals, and transport failure remain explicit unavailability.

`automation/cost-vectors.ts` validates runtime-generated `actors-cost-vectors.json` against exact metadata and Actors Weight hashes. The retained fixture matrix covers Manual `0/1/4/8/32` geometry, all six one-Step Trigger families, dormant User absence, explicit System exemption, Pipeline/hold totals, current Action locality, and cold-tail state-hold growth. No visible cost panel is shipped yet.

The automation domain validates metadata-bound Actor Contract artifacts. It discovers `ActorContract`, `ActorType`, and `Mutability` from exact runtime metadata, requires SCALE decode/re-encode equality, derives deterministic `contractId`, produces lossless JSON-safe projections, and classifies cross-genesis or cross-metadata diffs as incompatible until explicit rebinding.

Its static forecast mirrors pallet amount-resolution policy over an explicitly supplied state pin: fee reserve, minimum balance, User minimum balance, opening snapshot, last funding, and staking shares remain distinct inputs. Package-generated fee-envelope vectors constrain User/System fee policy, suffix arithmetic, reservation release, rollback pricing, and fee-native protected-minimum semantics before browser forecast use. Cost output keeps RefTime, ProofSize, zero-valued retired evaluation-fee slots, Action-effect upper fee, and lifecycle overhead separate. The generated suffix vectors no longer price Actor control per Step; typed Trigger and Pipeline Machine/cleanup quote projection and comprehensive runtime-generated cost vectors now come from `ActorCostApi`. `StaticAllStepsReached` does not simulate adapter quotes, mutations, failures, or early aborts.

`automation/cycle-deferred.ts` projects runtime-owned candidate identity without deriving it: fresh and Continuation nonce/attempt/cursor fields retain exact metadata meaning, and malformed values remain visibly unknown. The fee-budget projection uses the same checked protected-floor predicate as scheduler admission and rollback simulation.

`automation/semantic-manifest.ts` owns canonical task, predicate, and amount names so generated-manifest validation does not depend back on its analysis consumer. `automation/analysis.ts` consumes canonical artifact inspection, that Rust-generated versioned semantic manifest, and the existing forecast aggregator. It emits identity-bound `StaticStructuralProjection` trigger admission, ordered rows, economic/failure/data-dependency surfaces, factual unscored findings, and one bounded suffix envelope for every cursor.

`automation/configuration-ir.ts` owns format-neutral configuration IR version `1`, deterministic normalization, diagnostics, structural diff, and JSON/TOML/structured-Markdown adapters. It removes presentation step keys, preserves ordered typed authoring structure, and delegates all validation and lowering to `authoring.ts`. Cross-format fixtures must produce equal runtime projection, SCALE decode, and `contractId`; comments and Markdown prose remain non-executable.

`automation/feedback-analysis.ts` composes analyzed programs into a bounded deterministic graph of actors, exact typed observations, asset classes, exact actor-account signals, and declared parameter actuators. Every edge carries one causal/resource/coordination/declared-external family, one evidence provenance, and all supplying plan/observation/declaration identities.

`automation/risk-warnings.ts` projects shipped runtime facts into typed composition warnings: Immutable custody without a reachable terminal condition, residual custody through locator reuse, deep actor-graph amplification, canonical same-block trigger/revision coalescing, strict-FIFO head-of-line, and the distinction between simulator `Completed` and all-tasks-success. Each warning carries a severity and an evidence string; the module adds no protocol policy and consumes only `ActorContractStaticAnalysis` plus the Actor Contract artifact.

Reactive SCCs exclude resource edges and require an observation plus a genuine reactive-causal edge. Runtime producer identity derives endogenous observation provenance; explicit external identity derives exogenous provenance; missing evidence stays unknown. Deactivated observations remain inspectable but cannot contribute actor/parameter effect edges. Producer, lifecycle, and evidence disagreement fails closed.

Known sovereign balances use `AccountAsset`; missing accounts stay actor-scoped `Unknown`; parameter effects may use `AssetClass`; adapters may provide exact runtime/artifact-derived `Pool`, `Reserve`, or `Tmc` identities. Equal symbols across accounts and equal pairs across distinct pools do not merge. Exact shared resources emit typed coupling/contention findings only, never reactive cycles.

Actor signals bind sender-plan and recipient-state identities. Declared exact market resources and mixed runtime/state contexts fail closed.

Stability, probability, causal strength, contention harm, and economic impact remain `Unknown`.

Unscored reactive findings separate structural paths from evidence-bound timing and policy comparisons. Static analysis derives cooldown from schedule bytes and derives absent hysteresis/persistence from the current stateless linear Predicate language. Chatter rows identify the plan. Other timing/policy rows require generated runtime, metadata, weights, fanout limits, and field provenance; mismatch suppresses them without hiding graph truth.

Trigger projection preserves the scalar metadata union directly: `Manual`, `AddressEvent`, `ObservationChange`, `ObservationCrossing`, `AtTime`, or `Cadenced`. Observation families retain exact metadata-decoded feed and hysteresis projections without inventing a callback or result. AtTime retains its one-shot tick delay, while Cadenced retains its periodic tick interval.

Authoring treats `SourceFilter::Any` as an explicit fee-griefing exposure rather than a convenience default: any certified sender matching the asset filter may latch readiness, and the resulting User attempt consumes the actor's Weight-derived fee budget. The client does not infer trusted senders or runtime reimbursement; authors narrow exposure with `OwnerOnly` or a bounded whitelist.

Optional minimum-balance evidence carries its own identity and finalized block hash. Fixed SplitTransfer warnings require exact asset, zero recipient-balance, and below-minimum leg evidence; absent evidence produces no state claim.

Task adapters, assets, typed recipient surfaces, effects, availability, successful control, weight owner, bounded algorithm, amount roles, predicate read surfaces, and exact Predicate SCALE indices come from manifest format `2`; unknown versions, variants, or index drift fail closed. Each row preserves exact aggregate semantics and separate success/failure controls, including `StopCycle` cycle completion.

`automation/authoring.ts` owns the typed linear draft and immutable add/replace/remove/reorder operations. It validates runtime bounds, trigger-source canonicality, class rules, and optional nonzero-u64 auto-close targets; lowers every trigger/predicate/task/amount/policy directly to metadata-shaped `ActorContract`; and delegates exact SCALE and `contractId` production to `contract-artifact.ts`. Authoring-only row keys disappear during lowering; no graph, recipe identity, generic call, or successor field enters the artifact.

Observation predicates author the complete typed feed identity, raw `u128` threshold, and nonzero `max_age_blocks`. Static analysis labels their read as fresh-only and records unavailable, uninitialized, and stale states as ordinary false.

`observation/` owns the inspection contract. The blockchain adapter reads bounded DEOS Oracle `Oracle.FeedIds`, then fetches only the selected exact `Oracle.Feeds` and `Oracle.Observations` keys at one finalized hash. It projects scale-preserving formatting, producer, provenance, aggregation, lifecycle, update block, revision, authored maximum age, and the four current statuses under direct canonical-chain provenance. It never reconstructs history.

`projectObservationDeliveryInspection` owns the fail-closed reactive delivery projection over one finalized input. `projectObservationFanoutServiceTopology` is its single numerical owner: it counts occupied-page attempts plus cursorless restart/cleanup transitions, then derives the exclusive-budget lower bound and stable-topology fair-service ceiling from cursor distance, active-feed count, and the identified RefTime/ProofSize service budget. Impossible revision, active-list, cursor, page-count, or mixed-snapshot relationships throw instead of producing partial timing claims.

The blockchain adapter pins every Oracle and Actors query to one finalized hash, follows the exact active dirty-feed links and occupied subscriber-page links under stored count bounds, and never enumerates subscribers through a storage prefix. `runtime-evidence.generated.ts` owns the browser's expected runtime/metadata/code/descriptor/weight identity and fanout envelope; its code identity is the exact production compact compressed Wasm.

`scripts/generate-observation-runtime-evidence.mjs --check` fails when runtime source, production Wasm, metadata, descriptors, or generated Actors weights drift. Canonical `full` regenerates this tracked evidence and fails on resulting drift before publication evidence can be accepted.

The adapter facade reads runtime versions, V16 metadata, runtime code, and fanout constants at the same finalized hash as observation state, then compares them with generated evidence. Transport failure or any identity, version, constant, descriptor-bound metadata, code-bound weight, or budget mismatch produces `EvidenceMismatch`: factual Oracle/Actors topology remains visible while every numerical service estimate becomes unavailable.

The Observe surface renders feed fanout/cleanup topology, exact age, conditional estimates or exact mismatch reasons, and expected/observed evidence separately from selected-actor queue admission and execution state. Each estimate context identifies its revisions, page state, active count, selected/cursor positions, and budget; queue blocking, any context change, or a newer selected-feed revision invalidates the ceiling rather than extending its claim.

The Observe view states latest-state coalescing and the equal-value refresh/revision distinction. It labels DEOS Router pre-execution reserve feeds as local execution references rather than fair-price, manipulation-resistance, MEV, or ordering proofs. Observation history remains a materialized-provider concern and appears as unavailable rather than browser-derived archive truth.

Atomic publication rejection leaves no canonical observation or dirty-delivery record. Its direct Actors error or DEOS Router `InvalidOracleData` mapping belongs to the existing transaction/log feedback surface; the Observe view must not synthesize a persistent failure state from a rolled-back attempt.

The step editor presents output-authored `SwapOut` first and requires explicit `InputLimit::LiveQuote` or `InputLimit::Absolute` intent. Live mode discloses future-price exposure; absolute mode requires a positive canonical base-unit ceiling. Liquidity minima remain fixed positive runtime `u128` fields, and invalid bounds fail before artifact construction.

The step editor presents `AbortCycle` as “Abort on task failure” without changing encoded identity. A compact disclosure separates false-precondition advance, resolution skip, funding unavailability, Temporary task failure, and Permanent task failure; the `StopCycle` warning names false-precondition skip separately from predicate-evaluation and User fee-collection failure.

`AutomationWidget` keeps actor monitoring, observation inspection, and composition in separate views. `AutomationTriggerEditor` exposes exactly one Manual, filtered AddressEvent, typed ObservationChange/Crossing, one-shot AtTime, or periodic Cadenced trigger. Strategies needing independent sources use separate Actors.

Composition renders stable numbered rows with an absent unconditional guard or one bounded `Precondition` DNF, explicit `Opening`/`Current` predicate timing, and task-parameter, amount, and error-policy controls for every current primitive. Removing the final predicate makes `precondition` absent; `StopCycle` renders terminal completion without adding an edge. The widget creates artifacts but has no submission action.

Automation amount, asset, Predicate, Task, and Step editors live in `src/lib/automation/` as domain-owned presentation components. `src/lib/widgets/AutomationWidget.svelte` remains the layout entrypoint and composes those controls; the widget directory does not own automation internals.

The blockchain adapter supplies V16 metadata and runtime identity at one finalized block without fetching runtime code or invoking the simulation API. After validation, the widget displays exact `contractId`, metadata/genesis/runtime pins, finalized context, and SCALE size. Forecast, adapter-local, and matching-Wasm lanes remain separately labeled `Not run` until their own required model or provider executes.

The adapter-local simulation kernel evaluates every bounded-DNF predicate without short-circuiting, propagates the first observed predicate error after full visitation, and skips only the current task when aggregate truth fails. It clones state per task, commits successful effects, discards failed effects, preserves prior prefixes, and models abort, continue, Mutable-only Temporary retry, and successful `StopCycle` suffix termination. Every result says `AdapterLocalProjection`; only matching-runtime Wasm may claim runtime-level simulation.

Matching-Wasm response validation accepts the runtime's bounded `Completed`, `Aborted`, `Suspended`, and `Closed` states. A `Closed(ProductiveCycleCompleted)` response must retain the requested artifact/state pin and round-trip through canonical runtime SCALE before the client accepts it.

Actors call composition discovers the pallet and outer `RuntimeCall` from the artifact metadata, then exposes exact SCALE bytes, hash, `contractId`, runtime identity, and required origin. User calls remain direct owner-signed actions. Root-required System calls report `UnsupportedActorRootCall`: current strategic `L1RootAction` decodes only the dedicated runtime-upgrade payload, so call-byte composition does not imply governance admission.

The matching-Wasm trust gate hashes supplied runtime code and binds it to artifact metadata, runtime versions, finalized block/state identity, runtime API identity, actor id, mode, and `contractId`. Its metadata-discovered codec requires exact `ActorSimulationApi` version/signature, canonical SCALE round trips, typed success including `Closed(CloseReason)`, bounded ordered step evidence including `Stopped`, cursor-local unsuccessful-attempt projection, and equality between provider summary and runtime bytes.

The DEOS simulation adapter selects the current finalized hash or accepts an explicitly identified finalized fixture block, reads its header state root, V16 metadata, runtime version, genesis identity, and `:code`, and calls the typed simulation API at that same hash without submission. Local Omni Node evidence covers exact-plan rejection, successful fresh execution, and a stored Continuation attempt. The remote node remains a trusted provider: pin equality prevents drift but does not independently verify Wasm execution or state correctness.

## 4. Domain Ownership

The client is organized by explicit owners rather than a generic `shared/` bucket.

Primary slices:

- `market/` — swap direction, quotes, execution, price/session history.
- `portfolio/` — balances, bounded asset projection, transfers, deposits.
- `staking/` — staking-facing types/contracts.
- `governance/` — proposal store, labels, payload helpers, review helpers, projections.
- `automation/` — automation authoring policy, canonical Actor Contract artifacts/diffs, and bounded actor/Continuation projection contracts.
- `observation/` — typed feed identity, exact scalar formatting, current Fresh/Stale/Unavailable/Uninitialized classification, and inspection UI.
- `log/` — transaction progress, receipts, account log, network feed.
- `wallet/` — wallet session, signer discovery, address validation, local-dev signer routing.
- `system/` — chain snapshot, endpoint/session wiring, adapter runtime context, persistence.
- `wiki/` — trusted wiki loader/renderer helpers.

Broad foundation contracts may remain at root only when they are intentionally cross-cutting, such as `read-model.ts` and `economics.ts`. Shared low-level numeric literal parsing lives under `format/` so domain slices can validate complete literals without depending on UI Kit presentation helpers.

## 5. Adapter Boundary

`src/lib/adapters/contract.ts` is the live UI adapter contract. It exposes named lifecycle/read/write/feed capabilities while preserving an aggregate adapter facade for the application shell.

Concrete transport code stays behind adapter directories:

- `adapters/blockchain/` — PAPI-backed reference-chain implementation;
- `adapters/governance/` — typed governance providers;
- `adapters/materialized-history/` — explicit future-provider boundary for indexed/archive governance history.

Router quote adapters invoke bounded FRAME view functions at the selected snapshot's explicit `at` block hash; the transport hash supplies quote state identity without embedding an unverifiable hash inside the SCALE payload. They consume canonical `family` rather than the retired mechanism projection. Router `SwapExecuted` formatting reads total input, recipient output, and family from the nested canonical outcome instead of reconstructing legacy flat event fields.

Concrete adapters receive endpoint, selected address, and dApp name from `system/adapter-context.ts`. They should not import wallet stores or endpoint state directly.

The staking snapshot reads one fail-closed `native_security_view()` and the single boundary diagnostic at the same finalized block as pool/custody views. The canonical view owns mode, live readiness, current and Planned epoch identity, and retained-settlement presence; the diagnostic separately projects the latest `NotReady`, `SnapshotOpened`, or `SnapshotOpenFailed` planning outcome. The client derives action availability from mode instead of consuming a duplicated capability view.

Write preparation checks runtime capability before nomination, redelegation, liquid claim, or compound. Certified funding remains runtime/Actor-owned with no user action. Claim/compound require an explicit retained epoch, compound additionally requires operator and minimum LP output, and custody exits stay available in `TrustedSet`.

Signed governance authoring derives typed payload bytes and hash together, checks runtime admission policy and exact preimage status, and blocks submission until those bytes are noted. The PAPI write adapter repeats that fail-closed check before signing. Hash-first construction stays outside the browser signed path and belongs only to administrative bootstrap/recovery.

## 6. UI Kit

`src/lib/ui/` is the local UI Kit and owns reusable presentation primitives.

It centralizes:

- safe button defaults (`Button`, `IconButton`, `SelectableTile`);
- surfaces (`Card`, `SectionCard`, `StatCard`, `DetailRow`, `Notice`, `Badge`);
- form controls (`TextField`, `NumberInput`, `TextArea`, `SelectField`);
- shells (`PopoverPanel`, `SidePanelDialog`);
- provenance display (`ReadModelBadge`);
- chart/presentation helpers (`Sparkline`, `format.ts`, `class.ts`).

Rules:

- UI Kit must not import product/domain slices.
- Repeated raw controls should graduate into UI Kit.
- Buttons default to non-submit behavior unless a real form boundary opts into submit.
- UI Kit class merging accepts Svelte-style string/array/object class values through one helper.
- Form primitives own label/control wiring and hydration-safe generated ids.
- Numeric domain inputs validate complete literals before conversion; token amount fields use the shared strict parser/formatter in `format.ts` rather than JavaScript prefix/coercion parsing.

## 7. Domain DAG Gate

`web-client/domain-dag.json` is the architecture gate for the client.

It checks:

- local import cycles;
- required ownership headers;
- generic shared-bucket drift;
- entrypoint reach-through;
- domain-to-widget imports;
- UI-kit-to-domain imports;
- adapter-to-widget imports;
- widget-to-concrete-adapter imports;
- calibrated widget size/callback surface pressure.

Surface-pressure warnings are triage signals. They should lead to real ownership improvements only when the warning identifies a stable hotspot. Do not create folder theater just to silence a metric.

## 8. Responsive Composition

Widgets should adapt to arbitrary pane sizes without losing their main action.

Current rules:

- Prefer internal grids, summary cards, and local panels over long flat stacks.
- Collapse secondary diagnostics before primary actions.
- Use width-first breakpoints for pane-size adaptation to avoid height feedback loops.
- Let full-height widgets rely on `PaneHost` for the outer scroll/height box instead of inventing nested scroll hosts.
- The footer status surface should remain a compact full-width lane with horizontal overflow under pressure rather than growing into a tall grid.

## 9. Generated Wiki Boundary

`WikiWidget` renders generated repo-local wiki markdown from `/wiki`.

This content is an isolated strict OKF v0.2 bundle and is treated as trusted reviewed repository content, not user input. Its reserved root `index.md` declares the bundle but remains outside localized navigation and graph nodes. The independent Wiki Sync Skill owns validation before generated output is retained; the client does not invoke or depend on Skill internals.

The widget consumes generated metadata:

- `_meta/navigation.json` for section/page navigation;
- `_meta/aliases.json` for alias-aware lookup;
- `_meta/graph.json` for related-page navigation;
- `_meta/state.json` for explicit status and provenance;
- `_meta/locales.json` for locale/page discovery.

The wiki reader keeps established facts and Wiki-to-Wiki navigation primary. Source-document provenance remains non-rendered metadata for validation, not a source-path panel or article navigation into repository documentation. Concrete non-document resources may remain linked.

## 10. Validation

For client changes, run the smallest meaningful checks first. For the full client-local gate:

```sh
cd web-client
npm run validate
```

That script runs formatting, deterministic client regressions, Svelte checks, and the production build. Independent Domain DAG and Wiki Sync Skills may inspect this workspace, but neither Skill's validator is a client or project validation dependency.

## 11. Product Boundary Reminder

The web client is a reference client for a forkable framework, not the final downstream ecosystem product.

Polish should make framework behavior understandable and forkable. It should not smuggle downstream business-product logic into the core repo.
