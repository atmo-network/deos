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

Split dragging previews the ratio directly on the owning split at most once per animation frame and commits one immutable store update plus persistence write on pointer release. It must not rebuild and serialize the full layout tree for every pointer event.

Split resize and full-layout reset synchronously suppress tab FLIP animation so centered tabs track geometry changes without springing. Tab drag/reorder changes retain the elastic movement cue.

Browser profiling identified offscreen Wiki category layout—not split-store mutation—as the active resize bottleneck. Each generated navigation category now uses `content-visibility: auto` with an intrinsic fallback size, so offscreen groups skip layout/paint work while search filtering, pane-owned scrolling, and inactive-tab unmounting remain unchanged.

The tile renderer flattens every consecutive same-axis run in the binary persistence tree into one ordered flex group. Segment growth weights derive from the product of ancestor ratios. Separators remain fixed non-growing 12px items. Vertical segments receive recursive pixel minimums: 96px per leaf, summed through vertical descendants and maxed across horizontal descendants.

A handle keeps fine-pointer hit testing inside its fixed 12px visual/geometric lane so an invisible grip cannot intercept adjacent pane scrollbars. Coarse pointers receive a 44px touch hit target over that lane.

During the gesture, the handle disables native touch panning, freezes the rendered sizes of the whole group for preview, and changes only its two adjacent segments. One final commit converts the resulting segment pixel sizes back into the existing binary-tree ratios.

The first, middle, and final handle therefore share identical adjacent-only semantics. Non-adjacent panes keep their size, minimums remain enforced, and binary storage topology no longer leaks asymmetric resize behavior into the UI.

The root tile viewport owns layout-level overflow and wraps the rendered tree in its recursively computed minimum height. This creates a layout-scroll fallback when the external viewport cannot contain all pane minima and handles without weakening normal pane minimums. Rendered short-viewport validation remains the gate for claiming complete clipping and footer containment across browsers.

### 2.3 Reserved Edge Lanes

Header, footer, and sidebar remain reserved edge lanes outside the user-reorderable center pane tree.

The header exposes one selected-account disclosure button with a semantic user icon, equal four-sided padding, and no balance summary. Workspace reset belongs to the Settings widget inside the sidebar rather than a parallel header action. The chevron reflects actual sidebar motion: left/right opens and closes the wide right lane, while up/down opens and closes the compact bottom shelf.

In mobile composition, header and footer overlay the full-height center scrollport rather than shortening it. The scroll container reaches both viewport edges so its global vertical scrollbar remains flush with the viewport. A stable both-edge scrollbar gutter mirrors the consumed width on the left, and internal horizontal padding preserves a centered content gutter.

The accordion derives top and bottom scroll clearance from lane geometry and safe-area insets. Intermediate content may pass beneath the chrome while the first and last resting positions remain unobscured. Section rows use vertical proximity snapping against the same padding: a nearby row settles below the header, while non-mandatory snapping preserves continuous scrolling and never creates sticky blockers.

The reference sidebar occupies the wide desktop right edge. Below the side-lane breakpoint it becomes a portalled modal bottom shelf in both intermediate tile and mobile accordion modes, so opening it never changes center layout geometry. Settings exposes no left/right placement policy. `WorkspaceFrame` derives both responsive modes from one observed frame-width update without changing the persisted open preference.

`MobileSidebarSheet` delegates modal semantics to Bits UI Dialog. The header account chip remains the external `aria-controls`/`aria-expanded` trigger. The portalled overlay and content trap focus and lock background scroll. Escape and backdrop interactions close through the layout-store mutation, and close autofocus returns to the chip.

The bounded dynamic-viewport sheet uses a flat dimming overlay without backdrop blur or a decorative drag handle. It gives the sidebar panel one internal tall-content scroll owner, relies on the external trigger, backdrop, and Escape instead of duplicating close chrome, honors reduced motion and bottom safe-area padding, and loads the panel only after open state requests it.

Only the wide right-edge placement stays outside the dialog topology. Intermediate tile and mobile accordion widths share the overlay contract while passing their actual mobile mode into the sidebar's configured widget projection.

Layout specs define the default widget placement while the reserved lane geometry remains developer-owned.

`WorkspaceWidgetId` provides one exhaustive identity union for every tile/sidebar-capable widget. `PanelId` and `SidebarWidgetId` name placement roles over that union. Labels, semantic icons, and the cached dynamic-import registry share one complete metadata surface.

Versioned frame state persists the sidebar member order and nullable expanded widget. The tile tree persists the complementary tile placement.

Load reconciliation gives explicit sidebar membership priority over duplicate tile entries, appends missing known widgets to the sidebar, removes stale or duplicate ownership, and collapses emptied tree branches. It always retains at least one tile widget if corrupt state claims every widget for the sidebar. The mobile projection derives only from the reconciled tile tree, so sidebar-owned widgets never appear twice.

Explicit `null` preserves an all-collapsed index while the sidebar remains open. Every closed-to-open transition and restored open session expands the first widget in user order; an empty sidebar renders an honest empty state. Reset restores the canonical eight tile widgets plus Account/Settings sidebar placement.

The shared sidebar renderer presents each persisted member through one accessible single-expanded accordion in both the wide right lane and compact bottom shelf. It mounts only the expanded widget and uses the same explicit left 44px two-bar drag grip as the mobile accordion, while the icon-and-label surface owns disclosure. Widget membership is user-mutable, but the reserved lane's edge and overlay topology remain fixed.

On wide desktop, tile tabs and sidebar two-bar grips share one typed drag payload. Dropping a tab into an accordion position removes it from and collapses its source tile branch. Dropping a sidebar grip onto a tab bar or pane edge removes it from the sidebar and either inserts or splits the target tile.

Native dragover keeps only a local sidebar projection. Cancellation clears that preview without persistence. A successful cross-container drop commits the complementary tree/sidebar state once, and the one-tile minimum rejects attempts to stash the final tile widget.

`Shift+ArrowRight` moves a focused tile tab to the sidebar. `Shift+ArrowLeft` moves a focused sidebar grip to the first tile stack and restores focus at the destination. Cross-container drag remains disabled in the modal compact shelf, where focus trapping and touch ordering retain their existing semantics.

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

The automation widget reads known System actors from `AAA.ActorHot`, `AAA.ActorProgram`, and sparse `AAA.ContinuationState`. Current cursor/attempt status is canonical-chain truth. Historical attempt timelines remain materialized. The automation authoring contract exposes `RetryLater` only for Mutable plans and does not fabricate adapter retryability.

The automation domain validates metadata-bound plan artifacts. It discovers `ProgramInput`, `AaaType`, and `Mutability` from exact runtime metadata, requires SCALE decode/re-encode equality, derives deterministic `planId`, produces lossless JSON-safe projections, and classifies cross-genesis or cross-metadata diffs as incompatible until explicit rebinding.

Its static forecast mirrors pallet amount-resolution policy over an explicitly supplied state pin: fee reserve, minimum balance, trigger snapshot, last funding, and staking shares remain distinct inputs. Cost output keeps RefTime, ProofSize, evaluation fee, execution upper fee, and lifecycle overhead separate. `StaticAllStepsReached` does not simulate adapter quotes, mutations, failures, or early aborts.

The adapter-local simulation kernel clones state per task, commits successful effects, discards failed task effects, preserves prior prefixes, and models abort, continue, and Mutable-only Temporary retry outcomes with one scalar cursor. Donation classification identifies the amount surface and observation window. Every result says `AdapterLocalProjection`; only a matching-runtime-Wasm adapter may claim runtime-level simulation.

AAA call composition discovers the pallet and outer `RuntimeCall` from the artifact metadata, then exposes exact SCALE bytes, hash, `planId`, runtime identity, and required origin. User calls remain direct owner-signed actions. Root-required System calls report `UnsupportedAaaRootCall`: current strategic `L1RootAction` decodes only the dedicated runtime-upgrade payload, so call-byte composition does not imply governance admission.

The matching-Wasm trust gate hashes supplied runtime code and binds it to artifact metadata, runtime versions, finalized block/state identity, runtime API identity, actor id, mode, and `planId`. Its metadata-discovered codec requires exact `AaaSimulationApi` version/signature, canonical SCALE round trips, typed success, bounded ordered step evidence, and equality between provider summary and runtime bytes.

The DEOS simulation adapter selects the current finalized hash or accepts an explicitly identified finalized fixture block, reads its header state root, V16 metadata, runtime version, genesis identity, and `:code`, and calls the typed simulation API at that same hash without submission. Local Omni Node evidence covers exact-plan rejection, successful fresh execution, and a stored Continuation attempt. The remote node remains a trusted provider: pin equality prevents drift but does not independently verify Wasm execution or state correctness.

Canonical-chain widgets resolve the shared system connection surface before presenting factual values. The surface distinguishes provider initialization, connected data, retained session data after connection loss, an explicit non-canonical preview provider, an unconfigured provider, and provider failure.

Blocking states leave factual values unavailable, suppress unsupported chain-only sections, and keep task-specific disabled-action explanations rather than zero-shaped facts. They do not repeat the global connection/readiness banner inside every widget. The footer Status surface owns that shared explanation, while widgets retain local notices for stale or preview provenance and domain-specific failures.

Statistics, Chart, Automation, Governance, Log, Staking, Swap, and Wallet consume this contract.

Automation treats an absent adapter capability, a failed actor query, and a successful empty actor projection as distinct states. Its blockchain adapter propagates actor-query failure rather than converting it into false emptiness.

Governance resolves the contract from its own chain provider and gates active/composition and bounded-finalized projections together. `GovernanceArchiveSection` remains visible as an independently labeled materialized-provider boundary when canonical governance state is unavailable.

Log keeps account-scoped transaction progress and receipts independent from connectivity. Its network mode owns an explicit idle/loading/ready/error feed lifecycle. The blockchain adapter propagates event-query failure, successful empty finalized blocks create a valid empty session projection, and failed refreshes retain earlier events under the shared stale warning.

Staking blocks when no system snapshot exists. After connection loss it preserves the last pool, wallet, account-position, and operator/custody evidence under an explicit stale warning while disabling all staking/custody reads and writes independently of signer readiness.

Swap keeps direction, amount, and slippage as local form state but renders balances and outputs as unavailable before the first snapshot. It marks retained balances stale, cancels quote and network-fee work when the live connection disappears, and disables execution until a live snapshot returns. Market quote methods propagate transport/runtime failure, while a successful `null` quote remains the distinct canonical no-route result.

Wallet separates browser/session custody from chain projection. Account selection, address copy, signer/watch-only status, and unsent drafts remain local and usable without a provider. Asset discovery and balances do not render before a snapshot; retained holdings carry the shared stale warning; balance-derived max-fill and transfer submission require a live snapshot.

Account owns browser/session identity and signer controls rather than balances or positions. Selection, address validation, dev presets, and injected-wallet discovery work offline. Selected-account copy explains that chain-backed widgets update after reconnection, and system/governance refresh executes only when the chain reports a connected state.

Status keeps account and signer indicators independent from chain readiness. It derives network and finalized-block presentation from the shared connection surface, shows no block value before a snapshot exists, and appends an explicit stale or preview qualifier to retained or non-canonical block evidence. This completes the disconnected-state inventory for current reference widgets.

## 4. Domain Ownership

The client is organized by explicit owners rather than a generic `shared/` bucket.

Primary slices:

- `market/` — swap direction, quotes, execution, price/session history.
- `portfolio/` — balances, bounded asset projection, transfers, deposits.
- `staking/` — staking-facing types/contracts.
- `governance/` — proposal store, labels, payload helpers, review helpers, projections.
- `automation/` — automation authoring policy, canonical plan artifacts/diffs, and bounded actor/Continuation projection contracts.
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

- safe button defaults (`Button`, including labeled icon-only sizing, and `SelectableTile`);
- surfaces (`Card`, `SectionCard`, `StatCard`, `DetailRow`, `Notice`, `Badge`);
- controls and progressive disclosure (`Button`, `Icon`, `BackButton`, `SelectableTile`, `DisclosureSection`), with Lucide glyphs normalized through the `sm`/`md`/`lg` Icon scale;
- form controls (`TextField`, `NumberInput`, `TextArea`, `SelectField`);
- supplementary disclosure (`Tooltip`);
- provenance metadata retained in domain read-model contracts without a persistent global readiness control in the footer;
- chart/presentation helpers (`Sparkline`, `format.ts`, `class.ts`).

Rules:

- UI Kit must not import product/domain slices.
- Repeated raw controls should graduate into UI Kit.
- Buttons default to non-submit behavior unless a real form boundary opts into submit.
- UI Kit class merging accepts Svelte-style string, array, and object class values through one conflict-aware helper.
- Default and variant classes precede caller classes so superseded base, responsive, state, and prop-provided Tailwind utilities disappear before DOM emission rather than accumulating historically.
- Form primitives own label/control wiring and hydration-safe generated ids.
- Numeric domain inputs validate complete literals before conversion; token amount fields use the shared strict parser/formatter in `format.ts` rather than JavaScript prefix/coercion parsing.
- Tooltips carry only supplementary non-interactive detail that users can safely miss on touch devices; essential instructions, risk, state, and recovery remain visible or use an explicit disclosure surface.
- The doubled tooltip arrow follows Bits UI's resolved `data-side` after collision-driven placement flips. It shifts the inset white arrow toward the content on every side so the outer arrow remains a continuous border.
- Read-model values retain source-specific provenance in their domain contracts.
- The reference UI centralizes the visible readiness dot in the footer status strip instead of repeating it across widgets and rows; the tooltip carries source and scope detail without expanding primary widget content.

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
- Design widgets intrinsically across both width and height: define intermediate compositions as well as full and minimum modes, remove secondary detail progressively under pressure, and preserve the minimal viable task plus essential failure/safety state instead of merely forcing the full interface into another grid.
- Give every widget a pane-owned intrinsic root unit: tight panes use `0.875rem`, regular panes `0.9375rem`, and open panes `1rem`. Scoped Tailwind v4 `--spacing` and text tokens derive from that unit so ordinary utilities scale consistently.
- The `text-3xs`, `text-2xs`, and `text-compact` utilities preserve the established 9px/10px/11px hierarchy at the regular root without nested `em` compounding.
- Do not scale pane chrome, borders, scrollbars, 44px touch targets, D3 geometry, container thresholds, or the 56px scrollport contract. The shared host applies the approved scale across widget content. Compact type uses non-compounding pane-root tokens, while container phase thresholds remain physical pixels so browser text zoom cannot select a different information architecture at the same pane geometry.
- Preserve workspace and domain contracts rather than legacy widget composition: internal information architecture, navigation stacks, hierarchy, and layout may be rebuilt when that produces a clearer intrinsic task flow without weakening signer safety, errors, provenance, or transaction feedback.
- Mature layout before reskinning: concentrate the current pass on task order, grouping, spacing, control sizing, disclosure, and width/height composition, making only usability-required color or contrast corrections; treat broad palette, effects, and theme restyling as a later pass over the stabilized layout system.
- Treat this repository's client as the forkable UI backbone: it stabilizes workspace topology, intrinsic geometry, widget information architecture, minimum viable phases, accessible states, safety, provenance, and interaction logic. A downstream instance may layer a distinctive palette, gradients, effects, typography, and visual character over that backbone without reproducing or weakening its structural contracts.
- Keep active/available controls solid; use reduced opacity or translucent treatment only when it communicates disabled, inactive, hidden, loading, or transient drag/overlay state rather than as default decoration.
- Give critical signed operations progressive quote transparency. The full Swap phase exposes minimum received, route, effective rate, authoritative price impact, Router fee percentage and amount, separately reported liquidity-provider fee, and estimated network fee in the Native asset.
- The authoritative Router quote and action availability must not wait for supplementary network-fee estimation, which resolves independently behind a bounded timeout. Compact and shallow phases may remove supporting evidence while preserving the primary action and failure state. Fiat equivalents remain absent until an honest price-provider boundary exists.
- Prefer Tailwind atomic and arbitrary-value utilities for readable component presentation and responsive phases; keep local CSS when D3 geometry, trusted Markdown descendant styling, runtime-computed values, or semantic container rules would become less maintainable as dense utility strings. Current residual widget CSS follows that boundary: shared Staking/Statistics auto-fit contracts, Governance/Swap intrinsic phases, Chart geometry, Log height/width phases, and Wiki state/trusted-Markdown descendants retain one semantic owner rather than duplicating dense utilities at every call site.
- Avoid widget-local scrollbars: content should first reflow, condense, disclose, or reprioritize, and any remaining vertical or horizontal overflow belongs to the owning `PaneWidgetHost` scroll container rather than a nested widget viewport.
- Prefer intrinsic CSS layout, container queries, overflow, and wrapping over `ResizeObserver`/viewport state whenever presentation alone drives the phase change; keep JavaScript measurement for behavior that CSS cannot express.
- `layout/mobile-projection.ts` establishes the narrow workspace's pure data contract. It traverses leaves depth-first and tabs in saved order, retains source metadata, overlays valid unique panel IDs from a preferred mobile order, appends missing panels in tree order, and resolves the persisted expanded panel or an explicit all-collapsed state. Only a stale non-null panel ID falls back to the first desktop-active task.
- Mobile reordering returns a separate bounded panel-ID order and never mutates the desktop tree. `WorkspaceFrameState.mobile` persists that order plus the nullable expanded panel. A persisted `null` keeps every task collapsed and scannable; pressing an expanded header returns to that state.
- Legacy normalization upgrades sidebar-only frames with empty/null defaults and filters duplicate or unknown panel IDs. Layout-store expansion and reorder mutations normalize against the current tree before one persistence write. Reset clears mobile preferences with the rest of the workspace.
- Below `MOBILE_LAYOUT_BREAKPOINT`, root `TileContainer` replaces recursive splits with `MobileWorkspaceStack`. Every projected panel receives a native toggle-button header with `aria-expanded` and `aria-controls`; only the resolved expanded widget mounts through `PaneWidgetHost`.
- The mobile body asks `PaneWidgetHost` for an intrinsic height with a 96px floor and a `68dvh`/48rem cap inside one safe-area-aware flex stack. Its scrollport spans the full mobile frame behind the reserved header and footer. Accordion sections remain non-shrinking: compact tasks stop at their useful footprint, while long readers retain pane-owned scrolling. Desktop pane, tab, drop, and resize chrome never enters this phase.
- Wiki and Swap hash activation updates both the desktop leaf's active tab and the persisted mobile expansion preference so either composition reveals the linked task.
- Each mobile header centers its task label between equal side columns: a 44px left grip and a right disclosure chevron. The grip owns pointer/touch drag-and-drop with scroll suppression limited to the handle plus focused `ArrowUp`/`ArrowDown` movement. The label and chevron surface owns disclosure only.
- Pointer/touch reorder keeps a local preview order, marks the dragged row as the pending destination, and uses reduced-motion-aware FLIP displacement without persisting each crossing. Release performs one frame-owned order commit. Cancellation animates back without a write. Window-level lifecycle handling preserves completion when keyed movement changes the grip's DOM position.
- Keyboard movement commits one bounded step immediately. Both paths keep expansion keyed by panel ID, restore grip focus, announce outcomes through a polite atomic live region, and never change desktop tile storage. Remaining conditional render-matrix growth stays tracked in `BACKLOG.md`.
- Sidebar Account and Settings reuse the same one-dimensional ordering contract in both sidebar placements. A left two-bar grip owns pointer/touch and `ArrowUp`/`ArrowDown` reordering, while the icon-and-label plus chevron surface owns disclosure.
- Sidebar reorder keeps a reduced-motion-aware local FLIP preview, commits persisted `widgetOrder` once on release or one bounded step per keyboard command, restores stored order without a write on cancellation, restores label focus, announces the result, and leaves desktop tile and mobile panel topology unchanged.
- Let pane chrome and reserved lanes own strong outer separation; inside widgets, use spacing, type hierarchy, and alternating surface fills before borders or shadows, reserving outlines for controls, focus/selection, overlays, errors, data-visualization tracks, and semantic document structures.
- Use the established Lucide icon language for familiar compact secondary actions such as copy, close, expand, or direction changes, with accessible labels and tooltips. Retain text when the action meaning, risk, or state would otherwise become ambiguous.
- `layout/widget-icons.ts` derives tile and sidebar icon views from one semantic icon map over `WorkspaceWidgetId`. Tab labels, drag projections, mobile accordion headings, and sidebar headings render that icon beside the textual name without changing the accessible label. Desktop tabs keep icon and text in one horizontal flex row.
- Let full-height widgets rely on `PaneWidgetHost` for both-axis overflow, overscroll containment, and conditional inner-border feedback instead of inventing nested scroll hosts; widgets may clip only deliberate geometry such as charts, not ordinary oversized content.
- The footer status surface should remain a compact intrinsic-width no-wrap lane; labeled values progressively collapse to icons with focus/hover detail while operational state remains represented semantically.

### Hash deep links

The browser hash owns shareable widget-local navigation without replacing the tile manager:

- `#wiki/<page-id>` activates the Wiki tab and opens the generated page ID; `#wiki` opens the Wiki index state.
- `#swap/native/foreign` and `#swap/foreign/native` activate Swap and set its input/output direction.
- Widget interactions update the same hash contract, while `WorkspaceFrame` only activates the existing panel placement and does not rebuild or relocate user layout.
- Invalid or unknown hashes fail closed without mutating layout or domain state.

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
- `_meta/locales.json` for locale/page discovery;
- `_meta/search.json` for per-page-bounded bilingual plain text used by body search.

Navigation title, summary, section, and alias matching remains synchronous. On the first non-empty query, the widget dynamically imports the generated search manifest and merges body-only matches with bounded contextual snippets; it does not eagerly import the 90 localized Markdown chunks. The wiki validation command checks that this manifest exactly covers every state page and locale, respects the 12,000-character per-page bound, and matches generated output before the client build can pass.

The Wiki Back action belongs only to the stacked reader phase where the navigation index is absent. The wide two-column phase uses its persistent left navigation rail and must not duplicate Back above the article.

The wiki reader keeps page content primary and shows one consolidated `Related` surface. Explicitly authored final related links take priority, graph-derived incoming and outgoing relations supplement them, and page IDs deduplicate the merged list. Technical relation labels remain supporting tooltip information rather than a second `Read next` section.

Cached or fast page loads suppress intermediate loading chrome for 120ms. Only a slower request earns a quiet article-shaped skeleton, never a dashed placeholder that flashes before trusted content.

`src/lib/wiki/page-icons.ts` owns the presentation-only semantic Lucide mapping for page IDs. Every current page receives a distinct meaning-oriented glyph in navigation and related-page surfaces. Unknown future IDs fail softly to a document-question glyph, and generated wiki metadata remains free of client-specific component references.

## 10. Validation

For client changes, run the smallest meaningful checks first. Pure tile resize math runs through `npm run test:layout` and is included in the default validation path. Browser-level split-handle lifecycle coverage runs through `npm run test:browser` against an installed Chrome browser and an isolated Vite server; it verifies group freezing, adjacent-only preview, cancellation restoration, mouse/touch pointer capture, one final persistence write, and constrained-root scrolling. For the full non-browser client-local gate:

```sh
cd web-client
npm run validate
```

That script runs formatting, Svelte checks, Node layout/system tests, and the production build. For source-boundary, wiki trust, and wiki consolidation checks, the repo fast audit stack already includes the Domain DAG plus wiki gates:

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
