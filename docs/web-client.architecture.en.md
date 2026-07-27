# DEOS Web Client Architecture Notes

This note fixes the stable architecture vocabulary for the repository-local web client. It complements [`../web-client/README.md`](../web-client/README.md): the README is the workspace entrypoint, while this document is the durable implementation contract for the shipped client architecture.

The goal is to describe the implementation truth, subsystem boundaries, and contracts used by the reference product surface, not to track release planning or intermediate refactors.

## 1. Product Role

The web client is the browser-facing DEOS reference client for the current TMCTOL standard.

It is not the source of protocol truth. Runtime contracts and `/docs` remain authoritative. The client should expose bounded chain truth honestly, label materialized or session-derived data clearly, and keep user actions understandable before signing.

The current product shape includes:

- wallet/session/account selection;
- bounded balances and tracked-asset transfers;
- Axial Router quotes and signed swap execution;
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
- `StakingWidget` — native staking facts and signed staking/governance-custody actions.
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

The automation widget reads known System actors from `AAA.ActorHot`, `AAA.ActorProgram`, and sparse `AAA.ContinuationState`. Current cursor, logical-run attempt, and unsuccessful-attempt count at that cursor are canonical-chain truth. Historical attempt timelines remain materialized. The automation authoring contract exposes nonzero `RetryLater { maxAttempts }` only for Mutable plans and does not fabricate adapter retryability.

The automation domain validates metadata-bound plan artifacts. It discovers `ProgramInput`, `AaaType`, and `Mutability` from exact runtime metadata, requires SCALE decode/re-encode equality, derives deterministic `planId`, produces lossless JSON-safe projections, and classifies cross-genesis or cross-metadata diffs as incompatible until explicit rebinding.

Its static forecast mirrors pallet amount-resolution policy over an explicitly supplied state pin: fee reserve, minimum balance, trigger snapshot, last funding, and staking shares remain distinct inputs. Cost output keeps RefTime, ProofSize, evaluation fee, execution upper fee, and lifecycle overhead separate. `StaticAllStepsReached` does not simulate adapter quotes, mutations, failures, or early aborts.

`automation/semantic-manifest.ts` owns canonical task, condition, and amount names so generated-manifest validation does not depend back on its analysis consumer. `automation/analysis.ts` consumes canonical artifact inspection, that Rust-generated versioned semantic manifest, and the existing forecast aggregator. It emits identity-bound `StaticStructuralProjection` trigger admission, ordered rows, economic/failure/data-dependency surfaces, factual unscored findings, and one bounded suffix envelope for every cursor.

Trigger projection keeps admission independent from source identity. Manual and AddressEvent remain source kinds; every `OnObservationChange` entry additionally preserves its exact metadata-decoded feed projection without inventing a threshold, condition, callback, or execution result.

Optional minimum-balance evidence carries its own identity and finalized block hash. Fixed SplitTransfer warnings require exact asset, zero recipient-balance, and below-minimum leg evidence; absent evidence produces no state claim.

Task adapters, assets, typed recipient surfaces, effects, availability, successful control, weight owner, bounded algorithm, amount roles, condition read surfaces, and exact Condition SCALE indices come from manifest format `2`; unknown versions, variants, or index drift fail closed. Each row preserves exact aggregate semantics and separate success/failure controls, including `StopCycle` cycle completion.

`automation/authoring.ts` owns the typed linear draft and immutable add/replace/remove/reorder operations. It validates runtime bounds, trigger-source canonicality, and class rules; lowers every trigger/condition/task/amount/policy directly to metadata-shaped `ProgramInput`; and delegates exact SCALE and `planId` production to `plan-artifact.ts`. Authoring-only row keys disappear during lowering; no graph, recipe identity, generic call, or successor field enters the artifact.

Observation predicates author the complete typed feed identity, raw `u128` threshold, and nonzero `max_age_blocks`. Static analysis labels their read as fresh-only and records unavailable, uninitialized, and stale states as ordinary false.

`observation/` owns the inspection contract. The blockchain adapter reads bounded `Oracle.FeedIds`, then fetches only the selected exact `Oracle.Feeds` and `Oracle.Observations` keys at one finalized hash. It projects scale-preserving formatting, producer, provenance, aggregation, lifecycle, update block, revision, authored maximum age, and the four current statuses under direct canonical-chain provenance. It never reconstructs history.

The Observe view states latest-state coalescing and the equal-value refresh/revision distinction. It labels Axial Router pre-execution reserve feeds as local execution references rather than fair-price, manipulation-resistance, MEV, or ordering proofs. Observation history remains a materialized-provider concern and appears as unavailable rather than browser-derived archive truth.

The step editor presents output-authored `SwapOut` first and requires explicit `InputLimit::LiveQuote` or `InputLimit::Absolute` intent. Live mode discloses future-price exposure; absolute mode requires a positive canonical base-unit ceiling. Liquidity minima remain fixed positive runtime `u128` fields, and invalid bounds fail before artifact construction.

The step editor presents `AbortCycle` as “Abort on task failure” without changing encoded identity. A compact disclosure separates condition-false advance, resolution skip, funding unavailability, Temporary task failure, and Permanent task failure; the `StopCycle` warning names false-condition skip separately from atomic-condition and User fee-collection failure.

`AutomationWidget` keeps actor monitoring, observation inspection, and composition in separate views. `AutomationTriggerEditor` exposes Immediate/Cadenced admission, Always/WhenSignalled mode, and up to four Manual or filtered AddressEvent sources.

Composition renders stable numbered rows with explicit `Always`, `All conditions`, or `Any condition` mode plus task-parameter, amount, and error-policy controls for every current primitive. Removing the final grouped atom returns the row to `Always`; `StopCycle` renders terminal completion without adding an edge. The widget creates artifacts but has no submission action.

Automation amount, asset, condition, task, and step editors live in `src/lib/automation/` as domain-owned presentation components. `src/lib/widgets/AutomationWidget.svelte` remains the layout entrypoint and composes those controls; the widget directory does not own automation internals.

The blockchain adapter supplies V16 metadata and runtime identity at one finalized block without fetching runtime code or invoking the simulation API. After validation, the widget displays exact `planId`, metadata/genesis/runtime pins, finalized context, and SCALE size. Forecast, adapter-local, and matching-Wasm lanes remain separately labeled `Not run` until their own required model or provider executes.

The adapter-local simulation kernel evaluates every `All`/`Any` atom without short-circuiting, propagates the first observed atomic error after the full group, and skips only the current task when aggregate truth fails. It clones state per task, commits successful effects, discards failed effects, preserves prior prefixes, and models abort, continue, Mutable-only Temporary retry, and successful `StopCycle` suffix termination. Every result says `AdapterLocalProjection`; only matching-runtime Wasm may claim runtime-level simulation.

Matching-Wasm response validation accepts the runtime's bounded `Completed`, `Aborted`, `Suspended`, and `Closed` states. A `Closed(ProductiveRunCompleted)` response must retain the requested artifact/state pin and round-trip through canonical runtime SCALE before the client accepts it.

AAA call composition discovers the pallet and outer `RuntimeCall` from the artifact metadata, then exposes exact SCALE bytes, hash, `planId`, runtime identity, and required origin. User calls remain direct owner-signed actions. Root-required System calls report `UnsupportedAaaRootCall`: current strategic `L1RootAction` decodes only the dedicated runtime-upgrade payload, so call-byte composition does not imply governance admission.

The matching-Wasm trust gate hashes supplied runtime code and binds it to artifact metadata, runtime versions, finalized block/state identity, runtime API identity, actor id, mode, and `planId`. Its metadata-discovered codec requires exact `AaaSimulationApi` version/signature, canonical SCALE round trips, typed success including `Closed(CloseReason)`, bounded ordered step evidence including `Stopped`, cursor-local unsuccessful-attempt projection, and equality between provider summary and runtime bytes.

The DEOS simulation adapter selects the current finalized hash or accepts an explicitly identified finalized fixture block, reads its header state root, V16 metadata, runtime version, genesis identity, and `:code`, and calls the typed simulation API at that same hash without submission. Local Omni Node evidence covers exact-plan rejection, successful fresh execution, and a stored Continuation attempt. The remote node remains a trusted provider: pin equality prevents drift but does not independently verify Wasm execution or state correctness.

## 4. Domain Ownership

The client is organized by explicit owners rather than a generic `shared/` bucket.

Primary slices:

- `market/` — swap direction, quotes, execution, price/session history.
- `portfolio/` — balances, bounded asset projection, transfers, deposits.
- `staking/` — staking-facing types/contracts.
- `governance/` — proposal store, labels, payload helpers, review helpers, projections.
- `automation/` — automation authoring policy, canonical plan artifacts/diffs, and bounded actor/Continuation projection contracts.
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

Concrete adapters receive endpoint, selected address, and dApp name from `system/adapter-context.ts`. They should not import wallet stores or endpoint state directly.

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

This content is treated as trusted reviewed repository content, not user input. The safety boundary lives at repository validation through:

```sh
cd web-client
npm run validate:wiki
```

The widget consumes generated metadata:

- `_meta/navigation.json` for section/page navigation;
- `_meta/aliases.json` for alias-aware lookup;
- `_meta/graph.json` for related-page navigation;
- `_meta/state.json` for status/confidence/provenance;
- `_meta/locales.json` for locale/page discovery.

The wiki reader should keep page content primary and show related context/provenance as supporting information.

## 10. Validation

For client changes, run the smallest meaningful checks first. For the full client-local gate:

```sh
cd web-client
npm run validate
```

That script runs formatting, Svelte checks, and the production build. For source-boundary, wiki trust, and wiki consolidation checks, the repo fast audit stack already includes the Domain DAG plus wiki gates:

```sh
../scripts/validate-local.sh --audit-only
```

From inside the client workspace, the same boundary gate is available directly:

```sh
npm run validate:dag
```

`validate:dag` resolves the validator through `DOMAIN_DAG_VALIDATOR`, `SKILL_DIR`, or the repo-local `.agents/skills/domain-dag` copy. It preserves the default web-client root when forwarding extra validator args, and the Domain DAG config includes `scripts/` so package launchers stay under the same source-boundary/header gate. Run `npm run validate:dag -- --help` for launcher options.

For wiki-rendering/content changes, run:

```sh
npm run validate:wiki
```

`validate:wiki` runs the trusted markdown validator and the consolidation guard. It resolves them through `WIKI_TRUST_VALIDATOR` / `WIKI_CONSOLIDATION_AUDITOR` or the repo-local wiki-sync skill path, preserving the default repo wiki directory when forwarding extra validator args. Run `npm run validate:wiki -- --help` for launcher options.

To run every configured client-adjacent gate:

```sh
npm run validate:all
```

## 11. Product Boundary Reminder

The web client is a reference client for a forkable framework, not the final downstream ecosystem product.

Polish should make framework behavior understandable and forkable. It should not smuggle downstream business-product logic into the core repo.
