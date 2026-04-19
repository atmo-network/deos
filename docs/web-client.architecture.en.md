# DEOS Web Client Architecture Notes

This note captures the current product/UI architecture rules for the repository-local web client.
It complements `../web-client/README.md` by fixing terminology, subsystem boundaries, and near-term refactor direction.

## 1. Terminology Boundary

The client uses two different concepts that must not be collapsed into one word.

### 1.1 Widget

A `widget` is an **economic-functional unit**.
It exists because it expresses one domain-facing surface for the user.

Examples:

- `ChartWidget` — price/history visualization
- `StatusWidget` — compact chain/session footer status surface hosted in the reserved footer lane
- `StatisticsWidget` — protocol/system statistics plus recent system traces
- `SwapWidget` — trade execution surface
- `WalletWidget` — balances plus a bounded tracked-asset view and receive/send for the account already selected in the reserved sidebar lane
- `AccountChip` — special widget rendered in the reserved header lane; it is the sole sidebar toggle surface and now has only that one role
- `AccountWidget` — special widget rendered inside the reserved sidebar lane to control account selection and signer onboarding
- `LogWidget` — canonical receipt/history surface with account mode plus a live finalized network-feed mode
- `AutomationWidget` — lightweight automation/actor health surface
- `GovernanceWidget` — proposals, votes, outcomes

A widget answers the question:

> what economic/user-facing thing does this surface do?

### 1.2 Layout subsystem

The dynamic tiling / tabbing / resizing machinery is **not** a widget.
It is a separate client subsystem responsible for spatial composition.

Examples:

- `WorkspaceFrame`
- `TileContainer`
- `PaneHost`
- `SplitHandle`
- Layout store / tile tree / drag-drop mechanics

These pieces answer a different question:

> how are functional widgets arranged on screen?

That is infrastructure, not domain functionality.

### 1.3 Reserved edge lanes

The header/footer/menu shell is also not a tabbed pane widget.
It is a reserved edge lane inside the unified workspace frame, outside the tabbed center pane tree, and linear by construction rather than tabbed.

Examples:

- `AppHeader`
- `AppFooter`
- `SidebarPanel`
- Auth/account-selection menu
- Global settings/reset actions

These pieces answer a third question:

> what app-level controls frame the reserved edge lanes around the center tile tree?

That is layout framing, not widget business logic and not tabbed pane machinery. Reserved-lane widget sets are config-owned by developers rather than user-reorderable, and mobile may intentionally use a different lane widget mapping than desktop.

## 2. UI/UX Postulates for the Current Stage

The current stage of the client should optimize for clarity, not density.

### 2.1 DRY

Do not repeat the same user-facing information across multiple widgets without a strong reason.
If the same transaction preview / receipt / execution-status surface appears in multiple widgets, that is a signal to extract a shared functional unit.

Current rule:

- Repeated transaction-preview logic is a smell
- If multiple widgets need the same execution feedback, prefer one shared receipt/history surface over parallel ad-hoc summaries
- Repeated micro-UI patterns (section cards, stat cards, detail rows, notices, selectable tiles, fields, icon buttons, shell dialogs, shell popovers, provenance/source badges) are also a smell and should move into `src/lib/shared/ui/` instead of being rebuilt inside each widget
- The current implementation uses `LogWidget` as that canonical receipt/history surface while local action widgets stay focused on initiation

### 2.2 KISS

Prefer the smallest readable UI that preserves truth.
Do not fill widgets with low-priority diagnostics just because the data is available.

Current rule:

- Live protocol truth beats visual cleverness
- Diagnostics should support the main action, not drown it
- Advanced detail should move behind structure, not sit inline everywhere by default
- Periodically run subtraction passes: if a block is not mandatory for the widget's domain, remove it rather than letting framework/demo noise accumulate

### 2.3 2D structure over flat stacks

The visual language already implies framed structural modules with heavy borders.
That means widgets should not behave like long flat single-column dumps.

Current rule:

- Widgets should be composed as small internal `grid`-based 2D layouts
- Prefer clear sub-panels, summary cells, and local information hierarchy
- Avoid turning each widget into one long vertical checklist unless the content is intrinsically linear

### 2.4 Separate function from frame

A widget should focus on one function.
The layout system should focus on docking, tabs, splits, and resizing.

Current rule:

- Do not let layout mechanics define widget semantics
- Do not describe layout infrastructure as if it were an economic feature
- Do not hide major UX complexity inside generic pane hosts and then call the result a widget
- Keep auth/account selection in the reserved sidebar lane instead of smearing it across economic widgets such as `WalletWidget`
- Keep sidebar visibility control in the header-lane `AccountChip` instead of turning the sidebar itself into a tabbed widget surface
- Keep widget-local visual primitives thin by extracting stable micro-UI into the shared UI-kit layer instead of proliferating local badge/card/notice/input/button clones
- Shared UI-kit primitives such as section headers, stat cards, and tab bars should wrap and compress honestly in tight panes instead of forcing every widget to solve narrow-width overflow independently

### 2.5 Organic customization

Layout manipulation is not just an operator affordance.
It is part of the design language.

Current rule:

- Widgets must adapt to arbitrary pane widths and heights instead of assuming one canonical card size
- Resizing stacks, dragging tabs, or moving between wide and narrow viewports should let the UI morph into new readable forms
- Structural morphology switches driven by pane resizing SHOULD be width-first by default; avoid width+height breakpoint coupling that can create resize-feedback oscillation when one layout shape changes the measured height of the next
- When vertical space becomes scarce, a widget may collapse into denser representations rather than pretending the original layout still fits
- When the viewport drops below the mobile-width threshold, the workspace should stop clinging to desktop split geometry and let the same tree flow into a vertical ribbon/feed of widget surfaces
- Prefer responsive morphing over brittle breakpoint-specific clones; one subsystem should learn multiple honest shapes

## 3. Layout System Rules

The dynamic workspace is a first-class subsystem.
It should evolve independently from domain widgets.

### 3.1 Allowed responsibilities

The layout subsystem may own:

- Workspace-frame composition
- Tile-tree state
- Splits and ratios
- Drag/drop tab movement
- Pane hosting
- Resize handles
- Persistence of workspace arrangement
- Reserved position-bound edge lanes around the center tile tree
- Developer-owned lane widget mapping for desktop/mobile variants

### 3.2 Forbidden responsibility drift

The layout subsystem should not become the place where product semantics are invented.
It must not decide what the economic meaning of a surface is.

### 3.3 Naming rule

Use `layout`, `pane`, `tile`, `split`, `handle`, `workspace` language for layout infrastructure.
Reserve `widget` for economic-functional surfaces.

### 3.4 Responsive morphology rule

The layout subsystem should preserve one workspace mental model across form factors.
It should not force separate mobile vs desktop products when the same tree can morph honestly.

Current rule:

- Wide viewports may use split geometry directly
- The canonical default desktop/tablet tree is `Swap | Wallet` over `Log | Statistics` in the left column and `Chart | Automation | Governance | Wiki` in the right column
- That shipped arrangement should live as an explicit named layout spec, not as ad-hoc tree assembly hidden inside store code
- The center tile tree is capped at a `4 x 4` capacity (`16` leaves) so user-customizable pane growth stays bounded even if future widget count increases
- Mobile-width viewports should be allowed to linearize the same center tree into a ribbon/feed of leaves when split geometry stops using space efficiently
- Reserved edge lanes should keep responding to their own container width during that mobile linearization rather than pretending they are ordinary tabbed panes
- Desktop and mobile may intentionally use different developer-configured widget sets inside those reserved lanes
- Tablet/desktop widths should preserve the user-customizable split-tree workspace instead of linearizing too early
- Tab bars, handles, and pane chrome must remain usable inside tight widths instead of assuming desktop-only room
- User-driven layout edits should survive that morphing rather than being discarded as an unsupported mode
- Sidebars should be modeled as the same reserved-lane class as header/footer rather than as special one-off widget containers, and header-hosted controls may toggle sidebar visibility without turning those edge lanes into tabbed panes
- Account selection/signer onboarding may live inside a sidebar-hosted widget without making that sidebar a tabbed pane

## 4. Transaction Feedback Rule

Transaction progress is a functional concern, not an incidental widget-local detail.

Current rule:

- Signed / broadcasted / best / finalized / error feedback is part of the execution UX contract
- Raw status duplication across multiple widgets should converge toward one clearer shared model
- The backing state for that shared model should live in a dedicated top-level slice (`src/lib/log/`) rather than bloating unrelated stores
- Wallet and swap now publish their shared receipt semantics into `LogWidget` rather than owning parallel local receipt blocks
- By the same logic, live market direction/history/quote/swap state should live in a dedicated top-level slice (`src/lib/market/`) rather than staying entangled with chain snapshot ownership inside `src/lib/system/`
- Likewise, balances, bounded asset projection, and transfer/deposit bridging should live in a dedicated top-level slice (`src/lib/portfolio/`) rather than sharing ownership with chain snapshot refresh logic

## 5. Near-Term Refactor Direction

These are the practical consequences of the rules above.

### 5.1 Keep execution feedback inside the log surface

Current state:

- `LogWidget` is now the canonical execution-feedback and history surface
- It runs in `account` mode for current-user activity and `network` mode for the live finalized chain-event feed captured during the current session
- `SwapWidget` and `WalletWidget` initiate actions but no longer duplicate full receipt/status blocks

Direction:

- Keep shared transaction execution feedback centralized in that log surface
- Keep local widgets focused on initiating actions, not owning the whole receipt narrative
- Avoid reintroducing a separate receipt-only widget when the log surface can hold both current-account activity and a live bounded network feed honestly

### 5.2 Keep read-model provenance honest in the client

Motivation:

- The project-wide read-model contract is still only two-class: `canonical-chain` vs `materialized`
- The browser still needs to admit how a surface is realized right now: direct chain read, session cache, bounded session-derived view, or provider-backed materialization
- Without that second axis, session-built bounded views can silently masquerade as either direct runtime projections or archive truth

Direction:

- Keep the authoritative contract class aligned to `docs/read-model.contract.en.md`
- Model client-side realization separately instead of inventing a third truth class
- Use explicit provenance metadata on ambiguous surfaces such as live runtime quotes, recent chart samples, live session event feeds, bounded client asset projections, and future materialized/archive providers
- If a future widget introduces a materialized/archive provider, the product contract for that widget MUST name the provider family (`indexer`, `archive-api`, or `analytics-api`), the scope (`historical`, `archive`, or `search`), and the provider/source reference through the same provenance vocabulary rather than inventing a parallel "archive mode" label with hidden semantics
- Materialized/archive fallback behavior must stay honest: on provider failure, the widget may show no data, switch to an explicitly smaller bounded canonical-chain slice, or expose a clearly different disabled/error mode, but it must not keep rendering stale provider-backed data under live-chain wording
- Do not keep dormant materialized/archive state inside the active viewer store when no current widget actually surfaces that contract yet; add it back only with a real archive/search UX slice
- Converge the user-facing presentation of that provenance on shared UI primitives instead of widget-local pills so the contract stays visually consistent across the workspace
- Do not wrap pure interaction state (pane layout, form drafts, slippage inputs, dialog toggles) in read-model metadata just because the helper exists

### 5.3 Keep layout infrastructure outside the widget namespace

Motivation:

- Dynamic panes/splits/handles are infrastructure
- Naming them as widgets confuses the architecture

Direction:

- Keep layout code under a dedicated subsystem
- Do not keep permanent compatibility wrappers inside `src/lib/widgets/` for layout-only pieces
- The current rule is explicit: files such as `LeafPane`, `ResizeHandle`, `TileContainer`, `WorkspaceFrame`, `AppHeader`, `AppFooter`, and `SidebarPanel` belong to `src/lib/layout/`, not to the widget namespace
- Reserved edge-lane implementation should live under `src/lib/layout/`, not under the widget namespace
- Special control widgets rendered inside those lanes may still live under `src/lib/widgets/` when they are part of the widget composition model rather than the lane infrastructure itself
- Reserved-lane widget order/content comes from layout specs, not from end-user drag/reorder behavior
- The same rule applies to header-lane widgets too: `AccountChip` is selected by the lane spec even though it is a widget file
- For tabbed center panes, outer overflow/scroll hosting belongs to `PaneHost`; widgets should render adaptive content inside that host rather than owning the top-level scroll container themselves, and special full-height panes such as `ChartWidget` should be able to rely on that host to provide a definite full-height box instead of inventing a second internal scroll region
- For reserved side lanes, outer frame and inner overflow hosting both belong to `SidebarPanel`; sidebar widgets should render adaptive content inside that host rather than depending on an outer scroll wrapper in `WorkspaceFrame`
- There is no need for a separate `frame/` subtree once the lane model is established; keep position-bound lane components directly under `src/lib/layout/`

### 5.4 Rebuild widgets as internal grids

Motivation:

- The current frame-heavy visual language wants modular 2D information arrangement
- Flat content stacks make framed widgets feel structurally dishonest

Direction:

- Refactor widgets toward internal grid layouts
- `SwapWidget`, `WalletWidget`, `StatisticsWidget`, `LogWidget`, `AutomationWidget`, `ChartWidget`, and `GovernanceWidget` already moved to coarser internal grid/panel composition; continue the same simplification pass wherever future heavy widgets or shell surfaces start drifting back toward flat stacks
- Let widgets switch representation when space changes if that preserves the same domain truth more honestly than forcing one rigid layout
- `ChartWidget` already follows this rule: it lets the chart canvas elastically consume the remaining pane height without introducing its own internal scroll host, and keeps the series toggle handles in a bottom control rail opposite the pane-stack tabs so the graph itself stays the main occupant of the widget body
- `SwapWidget` now follows the same rule: it keeps its refactor progressive via in-file snippets placed alongside the main template instead of exploding the trade form into many tiny files, and those snippets now consume explicit typed view-model arguments instead of implicitly reading the whole widget through closure. Tight panes still collapse the trade chrome into a denser compact form while preserving the same execution contract, warnings, slippage control, route truth, and Bits-UI dropdown asset selection surface
- `WalletWidget` now follows it too: tight panes shorten account presentation, turn asset selection into a horizontal ribbon, and keep the bounded tracked-asset send surface usable without assuming a desktop-only stack
- `AccountWidget` now follows the same rule inside the reserved sidebar lane: dense sidebars shorten the selected address with full-detail fallback, switch preset and injected-account lists into tighter multi-column grids once width supports them, and keep the custom-account handoff row horizontal only when the lane is actually wide enough
- `SettingsWidget` now follows it too inside the reserved sidebar lane: the endpoint field stays full-width as the primary control, the smaller `Domain ID` / `Sidebar edge` controls share one row once width supports it, and the apply action now respects lane width instead of wasting vertical space by default
- `GovernanceWidget` now follows it as well: tight panes collapse tally and vote-power detail into a denser single proposal-state block while keeping vote actions immediate, and the public submit area now carries both the bounded advisory form and the first minimal tactical treasury invoice composition slice instead of pretending every signed public payload kind fits one advisory-only form
- `AutomationWidget` now follows it too: tight panes collapse actor details into denser summary blocks instead of preserving the same roomy operator card everywhere, dense panes let the actor header stack before the status badge starts fighting the label/role for width, and the header actor-count badge also shortens in compact panes
- `StatisticsWidget` now follows it more explicitly as well: tight panes collapse the snapshot/liquidity stat grids, let the route-mix detail cards stack before those labels/details become cramped, and stack the lower route/liquidity split instead of clinging to one roomy desktop matrix
- `WikiWidget` now follows it too: it uses the same shared panel chrome, consumes the generated wiki navigation manifest instead of staying a fixed hardcoded shortcut list, renders trusted repo-local wiki markdown directly in the SPA client through `marked`, now uses the generated aliases/graph/state metadata for alias-aware matching, related-page navigation, and compact compiled provenance, keeps alias-hit context visible in filtered result rows instead of hiding why a page matched, and in non-dual reader mode now keeps the markdown body ahead of those secondary context cards so the content remains first when panes get tight; it also shows explicit repo-local wiki page paths plus featured-page marking from that metadata contract rather than forcing the user to infer which cards are the primary onboarding entrypoints, and the current trust boundary lives at repo review/validation time through the dedicated wiki-trust validator rather than through runtime sanitization
- `LogWidget` now follows it more explicitly too: outside ticker mode it collapses receipt rows and grouped block entries into stacked mobile-style cards when panes get narrow instead of preserving one desktop timeline shape
- `StatusWidget` now follows it more explicitly too: the footer strip stays as a true minimal-height full-width status lane and falls back to horizontal overflow under width pressure instead of inflating into a tall card/grid stack; the outer lane frame belongs to `AppFooter`, while `StatusWidget` itself should render only the inline status content
- Use summary cards / cells / local matrix-like composition where appropriate
- Preserve readability over ornamental complexity

## 6. Product Boundary Reminder

The web client is still early.
It is allowed to ship transitional scaffolding.
But transitional scaffolding should still obey the correct vocabulary and separation of concerns.

At this stage:

- Correctness of live execution flows matters more than polish
- But once a flow is live, unnecessary duplication and naming drift should be reduced quickly
- Product clarity now will compound into future ecosystem dApps
