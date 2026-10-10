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

Governance failure presentation is owned by `governance/types.ts` and `governance/labels.ts`; the PAPI adapter transports the typed reason without reclassifying generic failures. Prepared `InsufficientTreasuryCapacity` renders as insufficient treasury capacity with no payout in finalized receipt rows, while `DispatchFailed` remains generic and unknown reasons remain unavailable. `test-governance-execution.mjs` checks this source projection; it does not decode chain bytes, verify RPC or prove rendered layout. The new failure enum and Invoice schema require coordinated metadata/descriptors/Wasm rebinding before live use; retained committed metadata is not evidence for changed discriminants or fields.

Invoice authoring encodes bounded CID, explicit treasury, beneficiary, asset and base amount through `governance/treasury-payload.ts`. The store obtains `MaxCidBytes` from the provider; missing/unsupported metadata yields unavailable capability and no payload encoding, never a hardcoded live fallback. Local checks cover canonical lowercase base32/version and length only; runtime witness/admission owns complete CID parsing and current treasury authority. Approval does not reserve a payout.

`adapters/governance/invoice-receipt.ts` maps only the current typed receipt shape, decoding the PAPI byte vector as strict UTF-8. Missing fields, invalid bytes and the old schema remain unavailable without aliases. Finalized rows show CID, approved source, target, actual payout and capacity limitation. Native and browser tests share a source-codec vector, not a metadata identity; live RPC and rendered validation remain separate.

The prepared automation control reader resolves each known System Actor through bounded `ActorSemanticStates` and `ActorProcesses` queries at one finalized hash. Active records must share the process generation; Dormant and absent semantic owners retain no active process. Orphan, missing, stale-generation and unsupported authorities refuse. Current residence is Service Live/Pending, Deadline Block/Tick, Parked or Disabled; the widget no longer displays a legacy FIFO ticket or derives placement from empty hot fields. A User's retained AwaitingFunding status stays separate from its residence, and System financial waiting refuses. Missing canonical metadata is Unavailable, not NotRegistered; errors preserve unavailable catalog rows and hide fabricated zero balances/Cycles. Prepared behavior-monitor and admission/freeze source projections are described below; coordinated descriptor/metadata refresh, real RPC and rendered conformance remain open under Gate B.

Authoring permits `0..=MaxContractSteps`; deleting the final Step exposes an explicit control-only state rather than fabricating an Action. Current cursor and unsuccessful-attempt count at that cursor are canonical-chain truth; no cycle-global attempt ordinal exists. Historical attempt timelines remain materialized. The automation authoring, analysis, and local simulation contracts derive `MaxContractSteps` and `MaxRetryAttempts` from the generated Actors ABI manifest; RetryLater remains Mutable-only and does not fabricate adapter retryability.

Actors exposes no permanent cache epoch or generic revalidation progress surface. Weight-, adapter-, or envelope-affecting upgrades use their concrete migration contract and finite semantic Weight-class proof rather than client-visible per-actor cache repair state.

The widget also consumes the read-only `ActorEligibilityApi::actor_eligibility` projection (`automation/eligibility.ts` plus the `actor-eligibility` blockchain adapter) for each actor: `NotRegistered`, `Dormant`, or the canonical Active classification with terminal reason and exact execution-phase payload at the same finalized block. Generated metadata binds the complete terminal-reason set, and unknown eligibility, reason, phase, or typed-failure variants fail closed instead of entering display state. The browser never reimplements cadence phase, cooldown, schedule window, retry backoff, breaker, or latch arithmetic; when the runtime API is unavailable the row shows `Unavailable` instead of guessing.

Prepared v8 maps authenticated current residence into Service Live/Pending, Deadline Block/Tick, Parked reason/review block or Disabled and refuses the retired placement encoding. Separate `financialWait` preserves the original unpaid boundary/liability, current fee-native service spendability above the protected minimum, exact non-negative shortfall, entry milliseconds and original grace Tick independently of residence. Amounts remain exact `bigint` values; the adapter rejects zero/overflow debt and inconsistent shortfalls and retains the original Suspended retry count. A zero shortfall does not certify current execution admission, collector success or future solvency; null means no financial episode. Selected v8 native metadata qualifies the SCALE/source fixtures, while committed metadata/descriptors and real RPC/rendered conformance still require the coordinated binding cohort. Service position and available Weight decide actual admission.

Prepared Park projection accepts only `ParkedBalanceBelowThreshold`; retired observation-era reasons refuse in both the canonical storage reader and eligibility adapter. Native metadata fixtures require the sole variant at SCALE index 0, so an older four-reason schema cannot qualify this source. Production bindings remain gated.

`automation/behavior-projection.ts` strictly projects the prepared v8 behavior episode: lifecycle start generation, CurrentOutcome/Accumulated mode, canonical referenced caps, required-only monitor values and actual clean-Cycle nonce/flags. No policy, stateless policy and allocated monitor remain distinct; accumulated values are explicitly capped, while CurrentOutcome shows no invented history. Missing, malformed, foreign-shaped or above-cap data refuses. The expanded widget presents these fields separately from residence and financial waiting and shows unavailable monitor data when an authored policy lacks a usable projection. Source and isolated native SCALE fixtures qualify the schema; production bindings, live RPC and rendered conformance remain gated.

`automation/resource.ts` and the `actor-resource` blockchain adapter preserve configured resource budgets, transient current-block state, and latest finalized telemetry from `ActorResourceApi` without reconstructing Resource Algebra in the browser. The budget/state/telemetry reads share one caller-selected finalized block; absent current state and absent telemetry remain distinct null projections. Current state carries an optional frozen budget, and the finalized snapshot retains its own complete budget rather than borrowing configured limits. The widget labels the snapshot's fixed envelope explicitly; `test-actors-resource-projection.mjs` covers distinct budgets and unfrozen absence. Current state is authoritative for its tagged block, while finalized telemetry remains bounded non-authoritative operational evidence rather than archive history.

Prepared `ActorResourceApi` v2 also supplies separate creation-admission and machine-freeze booleans at that hash. Structural capability checks and strict boolean projection preserve resource data while returning explicit unavailable flag status for missing metadata, malformed results or RPC refusal. The widget gives both flags and current block-local halt separate rows; an absent current state is no current record, not an inferred finalization event. Current flags never borrow the latest finalized halt or individual Actor pause. Native v2 SCALE fixtures qualify the source contract; production bindings, live RPC and rendered conformance remain gated.

`automation/cost.ts` owns the independent browser contract for `ActorCostApi` and `ActionFeeCharged`: Creation-backed lifetime cleanup, family-specific Trigger, Pipeline Machine, maximum next Action, actual Action receipt, and state hold retain named fields and identities without an activation total or remaining machine budget. `adapters/blockchain/actor-cost.ts` pins the typed API call to the caller's finalized block; unknown variants, malformed identities, inconsistent Pipeline/hold totals, retired cleanup-fee fields (including zero), and transport failure remain explicit unavailability. Pipeline total equals its machine fee; final Actor destruction is not a repeated surcharge.

`automation/cost-vectors.ts` validates runtime-generated `actors-cost-vectors.json` against exact metadata and Actors Weight hashes. The retained fixture matrix covers Manual `0/1/4/8/12` geometry, all four one-Step Trigger families, dormant User absence, explicit System exemption, Pipeline/hold totals, current Action locality, and cold-tail state-hold growth. No visible cost panel is shipped yet.

The automation domain validates metadata-bound Actor Contract artifacts. It discovers `ActorContract`, `ActorType`, and `Mutability` from exact runtime metadata, requires SCALE decode/re-encode equality, derives deterministic `contractId`, produces lossless JSON-safe projections, and classifies cross-genesis or cross-metadata diffs as incompatible until explicit rebinding.

Its static forecast mirrors pallet amount-resolution policy over an explicitly supplied state pin: fee reserve, minimum balance, User minimum balance, current balance, and staking shares remain distinct inputs. Package-generated fee-envelope vectors constrain User/System fee policy, suffix arithmetic, reservation release, rollback pricing, and fee-native protected-minimum semantics before browser forecast use. Cost output keeps RefTime, ProofSize, zero-valued retired evaluation-fee slots, Action-effect upper fee, and lifecycle overhead separate. The generated suffix vectors no longer price Actor control per Step; typed Trigger and Pipeline Machine quote projection and comprehensive runtime-generated cost vectors now come from `ActorCostApi`. `StaticAllStepsReached` does not simulate adapter quotes, mutations, failures, or early aborts.

`automation/cycle-deferred.ts` projects runtime-owned candidate identity without deriving it: fresh and Continuation nonce/attempt/cursor fields retain exact metadata meaning, and malformed values remain visibly unknown. The fee-budget projection uses the same checked protected-floor predicate as scheduler admission and rollback simulation.

`automation/semantic-manifest.ts` owns canonical task, predicate, and amount names so generated-manifest validation does not depend back on its analysis consumer. `automation/analysis.ts` consumes canonical artifact inspection, that Rust-generated versioned semantic manifest, and the existing forecast aggregator. Analyzer v12 preserves exact canonical `close_when` and `on_unfunded` under metadata-derived limits. It emits identity-bound structural rows, economic/failure/data dependencies, factual findings and per-cursor model envelopes. `costCoverage` explicitly excludes unqualified outcome-policy and financial-wait work; these envelopes cannot certify a complete behavior-service maximum.

`automation/configuration-ir.ts` owns format-neutral configuration IR version `2`, deterministic normalization, diagnostics, structural diff, and JSON/TOML/structured-Markdown adapters. It removes presentation step keys, preserves ordered typed authoring structure, and delegates all validation and lowering to `authoring.ts`. Cross-format fixtures must produce equal runtime projection, SCALE decode, and `contractId`; comments and Markdown prose remain non-executable.

`automation/feedback-analysis.ts` composes analyzed programs into a bounded deterministic graph of actors, exact typed observations, asset classes, exact actor-account signals, and declared parameter actuators. Every edge carries one causal/resource/coordination/declared-external family, one evidence provenance, and all supplying plan/observation/declaration identities.

`automation/risk-warnings.ts` projects shipped runtime facts into typed composition warnings: Immutable custody without a reachable terminal condition, residual custody through locator reuse, deep actor-graph amplification, canonical same-block trigger/revision coalescing, strict-FIFO head-of-line, and the distinction between simulator `Completed` and all-tasks-success. Each warning carries a severity and an evidence string; Cycle abort is not an Actor lifetime terminal, and Immutable warnings describe absent authored closure without promising permanent custody. The module adds no protocol policy and consumes only the static analysis plus artifact.

Reactive SCCs exclude resource edges and require an observation plus a genuine reactive-causal edge. Observation-to-Actor edges come only from authored predicate reads evaluated at execution time; publications never activate Actors. Runtime producer identity derives endogenous observation provenance; explicit external identity derives exogenous provenance; missing evidence stays unknown. Deactivated observations remain inspectable but cannot contribute actor/parameter effect edges. Producer, lifecycle, and evidence disagreement fails closed.

Known sovereign balances use `AccountAsset`; missing accounts stay actor-scoped `Unknown`; parameter effects may use `AssetClass`; adapters may provide exact runtime/artifact-derived `Pool`, `Reserve`, or `Tmc` identities. Equal symbols across accounts and equal pairs across distinct pools do not merge. Exact shared resources emit typed coupling/contention findings only, never reactive cycles.

Actor signals bind sender-plan and recipient-state identities. Declared exact market resources and mixed runtime/state contexts fail closed.

Stability, probability, causal strength, contention harm, and economic impact remain `Unknown`.

Unscored reactive findings separate structural paths from evidence-bound timing and policy comparisons. Static analysis derives cooldown from schedule bytes and derives absent hysteresis/persistence from the current stateless linear Predicate language. Chatter rows identify the plan. Evidence-bound rows require identified runtime and weight evidence; mismatch suppresses them without hiding graph truth.

Trigger projection preserves the scalar metadata union directly: `Manual`, `AddressEvent`, `AtTime`, or `Cadenced`. AtTime retains its one-shot tick delay, while Cadenced retains its periodic tick interval. Observation comparisons belong to Step preconditions, so a price-reactive strategy composes a Cadenced or AddressEvent trigger with a fresh-only observation predicate.

Authoring treats `SourceFilter::Any` as an explicit fee-griefing exposure rather than a convenience default: any certified sender matching the asset filter may latch readiness, and the resulting User attempt consumes the actor's Weight-derived fee budget. The client does not infer trusted senders or runtime reimbursement; authors narrow exposure with `OwnerOnly` or a bounded whitelist.

Optional minimum-balance evidence carries its own identity and finalized block hash. Fixed Transfer warnings require exact asset, zero recipient-balance, and below-minimum amount evidence; absent evidence produces no state claim.

Task adapters, assets, typed recipient surfaces, effects, availability, successful control, weight owner, bounded algorithm, amount roles, predicate read surfaces, and exact Predicate SCALE indices come from manifest format `2`; unknown versions, variants, or index drift fail closed. Each row preserves exact aggregate semantics and separate success/failure controls, including `StopCycle` cycle completion.

`automation/authoring.ts` owns the typed linear draft and immutable add/replace/remove/reorder operations. It validates runtime bounds, trigger-source canonicality, class rules, and optional nonzero-u64 auto-close targets; lowers every trigger/predicate/task/amount/policy directly to metadata-shaped `ActorContract`; and delegates exact SCALE and `contractId` production to `contract-artifact.ts`. Transfer authoring supports one bounded distribution capture followed by same-asset reuse. The editor distinguishes captured volume from live percentages; validation rejects duplicate capture, early/cross-asset reuse and shares above 100%. Drafts explicitly carry absent/canonical `closeWhen` and `onUnfunded`; legacy completion fields refuse. Artifact production authenticates the metadata vocabulary and reads its clause/atom limits, so caller limits cannot override host bounds. The outcome editor owns bounded AND/OR editing and User-only AwaitFunding. Native codec conformance is preparation evidence, not runtime admission or production binding. Authoring-only row keys disappear during lowering; no graph, recipe identity, generic call, or successor field enters the artifact.

Observation predicates author the complete typed feed identity, raw `u128` threshold, and nonzero `max_age_blocks`. Static analysis labels their read as fresh-only and records unavailable, uninitialized, and stale states as ordinary false.

`observation/` owns the inspection contract. The blockchain adapter reads bounded DEOS Oracle `Oracle.FeedIds`, then fetches only the selected exact `Oracle.Feeds` and `Oracle.Observations` keys at one finalized hash. It projects scale-preserving formatting, producer, provenance, aggregation, lifecycle, update block, revision, authored maximum age, and the four current statuses under direct canonical-chain provenance. It never reconstructs history.

The Observe view states the equal-value refresh/revision distinction and that Actors read observations only through execution-time predicates. It labels DEOS Router pre-execution reserve feeds as local execution references rather than fair-price, manipulation-resistance, MEV, or ordering proofs. Observation history remains a materialized-provider concern and appears as unavailable rather than browser-derived archive truth.

Atomic publication rejection leaves no canonical observation record. Its DEOS Router `InvalidOracleData` mapping belongs to the existing transaction/log feedback surface; the Observe view must not synthesize a persistent failure state from a rolled-back attempt.

The step editor presents output-authored `SwapOut` first and requires explicit `InputLimit::LiveQuote` or `InputLimit::Absolute` intent. Live mode discloses future-price exposure; absolute mode requires a positive canonical base-unit ceiling. Liquidity minima remain fixed positive runtime `u128` fields, and invalid bounds fail before artifact construction.

The step editor presents `AbortCycle` as “Abort on task failure” without changing encoded identity. A compact disclosure separates false-precondition advance, resolution skip, funding unavailability, Temporary task failure, and Permanent task failure; the `StopCycle` warning names false-precondition skip separately from predicate-evaluation and User fee-collection failure.

`AutomationWidget` keeps actor monitoring, observation inspection, and composition in separate views. `AutomationTriggerEditor` exposes exactly one Manual, filtered AddressEvent, one-shot AtTime, or periodic Cadenced trigger. Strategies needing independent sources use separate Actors.

Composition renders stable numbered rows with an absent unconditional guard or one bounded `Precondition` DNF, explicit `Opening`/`Current` predicate timing, and task-parameter, amount, and error-policy controls for every current primitive. Removing the final predicate makes `precondition` absent; `StopCycle` renders terminal completion without adding an edge. The widget creates artifacts but has no submission action.

Automation amount, asset, Predicate, Task, and Step editors live in `src/lib/automation/` as domain-owned presentation components. `src/lib/widgets/AutomationWidget.svelte` remains the layout entrypoint and composes those controls; the widget directory does not own automation internals.

The blockchain adapter supplies V16 metadata and runtime identity at one finalized block without fetching runtime code or invoking the simulation API. After validation, the widget displays exact `contractId`, metadata/genesis/runtime pins, finalized context, and SCALE size. Forecast, adapter-local, and matching-Wasm lanes remain separately labeled `Not run` until their own required model or provider executes.

The adapter-local simulation kernel evaluates every bounded-DNF predicate without short-circuiting, propagates the first observed predicate error after full visitation, and skips only the current task when aggregate truth fails. It clones state per task, commits successful effects, discards failed effects, preserves prior prefixes, and models abort, continue, Mutable-only Temporary retry, and successful `StopCycle` suffix termination. Every result says `AdapterLocalProjection`; only matching-runtime Wasm may claim runtime-level simulation.

`automation/behavior-policy.ts` validates the ten-metric canonical bounded `close_when` grammar, including declaration/lexicographic order, unique metrics/clauses and threshold absorption. The metadata-independent local simulation takes explicit policy limits and carries only referenced capped metrics plus current-Cycle clean flags between calls. It evaluates policy after the permitted Task and qualified normal completion, preserves committed prefixes, distinguishes Action streaks from clean completion, and gives retry exhaustion priority. Missing/mismatched history and legacy completion inputs refuse; service-fee insolvency and AwaitFunding remain unsupported model capabilities rather than synthetic Task failures. These model projections do not authenticate runtime policy admission or resource prices; production metadata/descriptors, ABI and resource bindings still require the coordinated Gate B refresh.

Matching-Wasm response validation accepts the runtime's bounded `Completed`, `Aborted`, `Suspended`, and `Closed` states. A `Closed(ProductiveCycleCompleted)` response must retain the requested artifact/state pin and round-trip through canonical runtime SCALE before the client accepts it.

Actors call composition discovers the pallet and outer `RuntimeCall` from the artifact metadata, then exposes exact SCALE bytes, hash, `contractId`, runtime identity, and required origin. User calls remain direct owner-signed actions. Root-required System calls report `UnsupportedActorRootCall`: current strategic `L1RootAction` decodes only the dedicated runtime-upgrade payload, so call-byte composition does not imply governance admission.

The matching-Wasm trust gate hashes supplied runtime code and binds it to artifact metadata, runtime versions, finalized block/state identity, `ActorSimulationApi` v2 identity, explicit synthetic Actor Control/Shared Economic budget, actor id, mode, and `contractId`. Its metadata-discovered codec requires the exact six-input signature, canonical SCALE round trips, typed success including `Continued` and `Closed(CloseReason)`, at most one ordered Step record including `Stopped`, cursor-local unsuccessful-attempt projection, and equality between provider summary and runtime bytes.

The DEOS simulation adapter selects the current finalized hash or accepts an explicitly identified finalized fixture block, reads its header state root, V16 metadata, runtime version, genesis identity, and `:code`, and calls the typed simulation API at that same hash without submission. The caller supplies explicit synthetic Actor Control and Shared Economic Weight components; the request/response pin preserves them and never presents them as production capacity. Local Omni Node evidence covers exact-plan rejection, successful fresh execution, and a stored Continuation attempt. The remote node remains a trusted provider: pin equality prevents drift but does not independently verify Wasm execution or state correctness.

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
