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

## 4. Domain Ownership

The client is organized by explicit owners rather than a generic `shared/` bucket.

Primary slices:

- `market/` — swap direction, quotes, execution, price/session history.
- `portfolio/` — balances, bounded asset projection, transfers, deposits.
- `staking/` — staking-facing types/contracts.
- `governance/` — proposal store, labels, payload helpers, review helpers, projections.
- `automation/` — automation-facing contracts/types.
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

That script runs formatting, Svelte checks, and the production build. For source-boundary and wiki trust checks, the repo fast audit stack already includes the Domain DAG and trusted wiki markdown gates:

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

`validate:wiki` resolves the validator through `WIKI_TRUST_VALIDATOR` or the repo-local wiki-sync skill path. It preserves the default repo wiki directory when forwarding extra validator args. Run `npm run validate:wiki -- --help` for launcher options.

To run every configured client-adjacent gate:

```sh
npm run validate:all
```

## 11. Product Boundary Reminder

The web client is a reference client for a forkable framework, not the final downstream ecosystem product.

Polish should make framework behavior understandable and forkable. It should not smuggle downstream business-product logic into the core repo.
