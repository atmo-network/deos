# DEOS Framework Delivery History

> Canonical complete delivery history for the current DEOS repository line
>
> This repository restarted its own release line at `0.0.0` after the move into the new DEOS monorepo. The changelog therefore focuses on achieved epics and the current shipped baseline of this repo, not on preserving every intermediate refactor step or pre-reset chronology.

## [0.5.0] - 2026-05-17

### Wiki Positioning

- `wiki`: Added `DEOS in 60 Seconds`, `Who DEOS Is For`, `Partner Pitch`, `Executive Summary`, `Partner Evaluation Route`, and `DEOS vs DAO Treasury` entry/comparison pages, then moved the project meme, audience map, partner-facing why-it-matters story, shipped/not-shipped executive summary, five-page fork/adoption route, and committee-treasury-vs-circuit-treasury contrast into the top-level wiki, first-steps route, framework overview, root README, and external-evaluator reading path so external readers see the core value proposition before the full runtime/domain graph; strengthened the root README opening and entrypoint links around deterministic economic circuits, partner evaluation, and the TMCTOL first-standard shape, tightened Russian prose in the new entry/positioning pages while preserving stable protocol terms and naturalizing unnecessary generic Anglicisms, removed separate wiki log files so delivery history stays consolidated in the project changelog, and updated the repo-local wiki-sync skill/context rules so future wiki work no longer recreates local mutation logs.

### Web Client Maintenance

- `release`: Bumped the Rust template workspace package versions (`deos-runtime`, `primitives`, and DEOS pallets) plus `template/Cargo.lock` package entries to `0.5.0`, aligning runtime-side package metadata with the web-client package version.
- `web-client`: Bumped frontend dependencies to their latest available package versions and verified the updated dependency graph with `npm run check` and `npm run build`. Static adapter output now writes SPA fallback to `200.html` instead of overwriting `index.html`, removing the adapter fallback warning while preserving static-site routing fallback behavior. Added `npm run validate`, `npm run validate:dag`, and `npm run validate:all` client scripts so Prettier/Svelte/build checks and the Domain DAG gate have compact package entrypoints, with README and web-client architecture validation guidance updated to use them. The Domain DAG package script now uses a small client-local Node launcher that resolves `DOMAIN_DAG_VALIDATOR`, `SKILL_DIR`, or the current user's default pi skill path instead of hardcoding one absolute user path; the launcher also exposes `--help` with path-neutral defaults and forwards explicit validator arguments while preserving the default web-client root unless `--root` is supplied. Added a matching `npm run validate:wiki` launcher for the trusted wiki markdown gate and included it in `npm run validate:all` so browser-rendered wiki safety has a package entrypoint; its forwarded-args path similarly preserves the default repo wiki directory unless `--wiki-dir` is supplied, and its help output uses repo-relative default labels. The package validation launchers now write CLI messages through stdout/stderr directly instead of `console.*`, keeping browser-console/debug greps clean across `src` and `web-client/scripts`; `web-client/domain-dag.json` now includes the client `scripts/` directory so validation launchers carry Domain DAG ownership headers and remain inside the source-boundary gate. Sidebar skeleton ownership wording now avoids generic placeholder language in Domain DAG headers.
- `web-client`: Replaced the recursive `TileContainer.svelte` self-import with a local recursive Svelte snippet, preserving the split-tree layout behavior while making the local source import graph acyclic under the Domain DAG validator.
- `web-client`: Flattened the old `src/lib/shared/` bucket into explicit support owners (`ui/`, `system/`, `economics.ts`, `read-model.ts`) and moved reusable contracts from the temporary `domain-types.ts` bucket into owning domain slices (`market`, `portfolio`, `log`, `automation`, `staking`, `system`).
- `web-client`: Added a calibrated `domain-dag.json` gate for the client so import cycles, generic shared buckets, domain-to-widget imports, UI-kit-to-domain imports, adapter-to-widget imports, and widget-to-concrete-adapter imports are checked with project-local boundaries; the configured Domain DAG validation now passes with zero warnings.
- `web-client`: Strengthened frontend domain naming by renaming the live adapter contract from `adapters/types.ts` to `adapters/contract.ts` and moving signer/address ownership from `adapters/blockchain/signer.ts` into `wallet/signer.ts`.
- `web-client`: Removed concrete blockchain-adapter imports of `walletStore` and `system/endpoint`; `system` now passes endpoint, selected address, and dApp name through an explicit adapter runtime context.
- `web-client`: Decomposed the live adapter contract into named lifecycle/read/write/feed capabilities (`SystemReadAdapter`, `MarketAdapter`, `PortfolioAdapter`, `StakingAdapter`, `AutomationAdapter`, `LogFeedAdapter`) while preserving the aggregate `Adapter` facade.
- `web-client`: Promoted adapter runtime-context construction into `system/adapter-context.ts`, keeping endpoint, selected-address, and dApp-name wiring owned by the system composition slice.
- `web-client`: Split the blockchain adapter monolith behind the preserved facade by extracting runtime asset helpers into `adapters/blockchain/runtime-assets.ts`, snapshot building into `adapters/blockchain/snapshot.ts`, connection lifecycle into `adapters/blockchain/connection.ts`, transaction submission into `adapters/blockchain/transactions.ts`, staking actions into `adapters/blockchain/staking-actions.ts`, quote helpers into `adapters/blockchain/quotes.ts`, and chain event / transaction-log formatting into `adapters/blockchain/events.ts`. Blockchain transaction/event helpers now use explicit unknown-safe event normalization instead of local `any` signatures.
- `web-client`: Reduced root `src/lib` foundation drift by moving presentation formatting helpers to `ui/format.ts` and browser local-storage helpers to `system/persistence.ts`, leaving root-level files for public exports and broad contracts only. System refresh failures now route through the execution log slice instead of writing directly to the browser console. The materialized-history adapter is documented as an explicit future-provider boundary rather than a generic placeholder/stub, with README, architecture, component-header, and backlog wording aligned.
- `web-client` + `domain-dag`: Added warning-only widget surface-pressure checks for oversized Svelte widgets and broad widget callback/function surfaces, then resolved the initial `GovernanceWidget.svelte` size warning by extracting governance label/projection helpers into `governance/labels.ts`.
- `web-client`: Polished Swap asset selection by fixing the malformed selected-state Tailwind class on dropdown items, restoring the intended selected asset border/background styling.
- `web-client`: Tightened button semantics in wallet/log/layout reusable surfaces by adding explicit non-submit button types to action controls and the selectable-tile UI primitive.
- `BACKLOG`: Promoted two high-priority frontend architecture tasks: establishing a named UI Kit and adding Domain DAG ownership headers to web-client source files.
- `web-client`: Established UI Kit as the named `src/lib/ui/` project UI kit with local README ownership notes, Bits UI wrapping policy, and default non-submit semantics for `Button` / `SelectableTile` primitives.
- `web-client`: Completed Domain DAG ownership headers across `src/lib`, `src/routes`, and app typing surfaces, then enabled `requireHeaders` in `domain-dag.json`; coverage includes UI Kit primitives and public barrel, the full layout layer, market/portfolio/log/system/wallet state stores, governance contracts/store/payload helpers/review components, browser persistence, wiki trust helpers, governance/materialized-history adapters, widgets, the blockchain adapter split, the live adapter contract, and root foundation contracts with owns/excludes/zone comments. Removed the remaining layout-layer `any` escape hatches by typing lazy Svelte component slots and the tab flip animation function explicitly; governance label helpers now use governance contract types instead of local `any` signatures. Replaced layout persistence and legacy-normalization casts with typed guards and a split-layout builder. Extracted active proposal cards, finalized proposal cards, archive-boundary rendering, and authorized-runtime-upgrade relay guidance into governance-owned components, bringing `GovernanceWidget.svelte` comfortably under the calibrated Domain DAG size threshold.
- `web-client`: Polished `WikiWidget` interaction safety by adding explicit non-submit button semantics to its specialized raw wiki navigation, search, copy, related-page, and source controls, and promoted repeated small search/back/copy actions to UI Kit `Button` while preserving widget-local markup for wiki lists.
- `web-client`: Promoted `WalletWidget` repeated copy/fill-max affordances to UI Kit `Button`, preserving wallet-specific styling while centralizing safe button defaults.
- `web-client`: Promoted `LogWidget` account/network segmented controls to UI Kit `Button`, keeping the local segmented-control styling while removing another raw-button pocket.
- `web-client`: Promoted `SwapWidget` fill-max, flip-direction, and execute controls to UI Kit `Button` / `IconButton`, preserving swap-specific responsive styling while centralizing interaction defaults. Swap execution error handling now uses `unknown` plus an explicit fallback message instead of a catch-local `any`, swap success logs avoid non-null assertions on route-specific output fields, and swap numeric-input handlers read DOM values through an `HTMLInputElement` guard instead of inline event-target assertions. Svelte widget rune typing now avoids generic `$derived.by<T>()` and `$derived<T>()` syntax so parser/formatter tooling sees valid component markup, and SwapWidget avoids a Prettier/Svelte parser edge around literal dollar-sign badge data inside rune-adjacent script code. Governance advisory/treasury payload parsers now use explicit nullable parse-result types instead of return-local `null as ...` assertions, trusted wiki markdown imports now use Vite's typed `import.meta.glob<string>` overload instead of a `Record<string, ...>` assertion, and blockchain event formatting now reads unknown event payloads through `Reflect.get` guards instead of `Record<string, unknown>` assertions. Governance write-surface merging now iterates an explicit typed operation list instead of casting `Object.keys` and result objects. Injected-wallet discovery now uses the ambient browser `Window.injectedWeb3` contract from `polkadot-api/pjs-signer`, the package-provided `getInjectedExtensions()` helper, and the typed `InjectedExtension` pjs-signer contract instead of local window declarations, `Object.keys` over injected runtime state, or inline runtime extension assertions; built-in dev signer presets now use an explicit readonly spec type instead of `as const` literal assertion. Runtime asset enum helpers now call PAPI `Enum` with explicit generics instead of casting constructed asset kinds, and runtime-account helper tables now use explicit readonly contracts instead of `as const` literal assertions. Reference-client System AAA account labels, current context/backlog/wiki baseline wording, generated script-layer wiki wording, AAA ingress/staking/router/asset-registry/TMC/core architecture, former-adapter cleanup notes, staking diagrams, and SDK-insight actor wording now use actor-role wording and lifecycle wording (`Burn Actor`, `Liquidity Actor`, `liquidityActorBuffers`, `BlockchainConnectionSession`) instead of primary legacy manager/farmer names. Account and wallet notice derivations now use explicit notice-state return types bound to the UI Kit `NoticeVariant` contract instead of local literal unions or `as const` literal narrowing in returned objects. Browser persistence now returns parsed JSON as `unknown` directly, and trusted wiki rendering uses Marked's synchronous parse overload instead of result casts. Blockchain automation-trigger labels now guard timer payloads instead of casting, and governance vote-tally mapping models optional extended fields in the input contract instead of casting tally objects.
- `web-client`: Promoted `ChartWidget` legend toggle controls to UI Kit `Button`, preserving chart-specific pill styling while centralizing safe interaction defaults. Chart rendering now uses typed series guards for router points, tooltips, and latest-series rows instead of non-null assertions on optional prices, and it guards D3 extent / tooltip element lookup without type assertions.
- `web-client`: Promoted the header-lane `AccountChip` toggle to UI Kit `Button`, preserving compact account/balance styling while centralizing safe button semantics.
- `web-client`: Promoted `PaneTopChrome` tab controls to UI Kit `Button`, preserving drag/drop tab semantics and local active/projection styling. Pane drag/drop hotspots now use accessible button-based drop surfaces instead of `svelte-ignore` a11y suppressions, the overlay drop target is promoted through the UI Kit `Button`, the remaining raw pane-grip button is documented as a direct DOM-bind exception for drag geometry, and pane DOM bind state now stays nullable instead of relying on non-null initialization assertions.
- `web-client`: Promoted the remaining `WikiWidget` row-as-button controls to UI Kit `Button`; widget/layout/governance surfaces no longer keep raw `<button>` pockets outside UI Kit primitives.
- `web-client`: Hardened UI Kit class handling with a shared `ui/class.ts` helper so `Button` and `SelectableTile` flatten Svelte-style string/array/object class values instead of stringifying arrays as comma-separated text; documented the UI Kit class-value policy.
- `web-client`: Added a real `.scrollbar-none` utility so compact horizontal rails such as the Wallet transfer asset selector hide scrollbars consistently instead of relying on an undefined class, then reused it for pane tabs and footer lane rails instead of ad-hoc scrollbar utilities.
- `web-client`: Extended UI Kit class-value hardening across shared presentation primitives (`Card`, `SectionCard`, `StatCard`, `Badge`, `ReadModelBadge`, `Notice`, `DetailRow`, `TextField`, `NumberInput`, `Sparkline`, `PopoverPanel`, and `SidePanelDialog`) so UI Kit consistently accepts Svelte-style class arrays/objects through the same merge helper.
- `web-client`: Promoted Swap widget numeric amount and slippage controls to the UI Kit `NumberInput` primitive while preserving compact sizing and transparent amount-field styling.
- `web-client`: Promoted Governance widget signed-submission text fields to the UI Kit `TextField` primitive, added `TextArea` for readonly payload-hex previews, and added `SelectField` for native governance selectors so repeated raw form controls are fully centralized in UI Kit.
- `web-client`: Replaced random generated form-control ids in UI Kit fields with Svelte hydration-safe `$props.id()` ids, removing potential SSR/client label-target drift.
- `wiki` + `web-client`: Regenerated and expanded the `Reference Client` wiki overview in both locales so the newcomer-facing wiki now explains UI Kit, Domain DAG ownership gates, adapter/runtime-context split, responsive lane/widget discipline, and the trusted wiki-reader boundary; added focused `UI Kit and Domain DAG`, `Generated Wiki`, and `Reading Paths` pages in both locales, refreshed `Newcomer FAQ` onboarding answers, expanded `Core Terms` frontend/wiki vocabulary, and updated wiki index, navigation/state/graph/alias/locales metadata, and wiki logs to match.
- `web-client` + `docs`: Consolidated `web-client/README.md` and `docs/web-client.architecture.en.md` around the current final product/architecture shape for the 0.5.0 line, replacing accumulated intermediate refactor chronology with stable status, product role, ownership boundaries, UI Kit / Domain DAG contracts, generated-wiki rendering, validation, and local-development guidance.
- `wiki`: Refreshed the development status pages and metadata around the 0.5.0 consolidation line so newcomers see current shipped baseline, active focus, open boundaries, and future-gated work without reading release/refactor history first.
- `wiki`: Consolidated the generated wiki into a self-contained, domain-first knowledge graph for the 0.5.0 line: index, reading paths, first steps, FAQ, contributing, glossary, status, and generated-wiki routes now keep readers inside wiki-owned explanations instead of visible docs-as-reading-path sections while preserving provenance in metadata.
- `wiki`: Added localized domain and explanation pages for Domain Map, End-to-End Flows, Architecture Diagrams, Token Surfaces, Economic Thresholds, Economic Claim Levels, Invariant Map, Threat Model, What DEOS Is Not, TOL Bucket Scenarios, Forking DEOS, Minimal Fork Profile, Validation Troubleshooting, Agent Coordination, Parachain Context, and Wiki Graph Metadata, covering swaps, actor wakeups, bucket/treasury lanes, token roles, Gravity Well / Elasticity Inversion, claim-strength levels, invariant ownership, threat boundaries, negative product scope, fork choices, Polkadot/XCM/collator context, validation triage, agent workflow, and graph metadata semantics.
- `wiki`: Compressed duplicated prose into canonical owner pages for AAA system/actor, UI Kit and Domain DAG, Reference Client, Generated Wiki, Read-Model Split, governance overview/domains, newcomer FAQ, token surfaces, staking pools, and end-to-end flows, then strengthened related-page graph routes across economics, tooling, validation, client, repository-structure, and future-gate clusters.
- `wiki`: Continued Russian-locale naturalization across fresh and high-traffic wiki pages while preserving stable protocol/module/token/API terms; updated navigation, state, graph, aliases, locales, and wiki logs for the new domain graph, and backfilled state/graph metadata for implementation, math, usage, development, and contributing pages that were already present in navigation/locale discovery.
- `docs`: Corrected the web-client architecture document and docs index wording to stay release-agnostic: architecture docs describe implementation truth and contracts, while release targeting remains in changelog/status surfaces.
- `docs` + `context`: Added durable architecture-doc release-agnosticism guidance and removed stale release-version metadata from architecture docs so implementation mirrors do not masquerade as release notes.
- `web-client`: Made the workspace README release-agnostic too: it now describes the current client contract while release targeting remains in changelog/status surfaces.
- `context`: Added README release-neutrality guidance so human entrypoints stay focused on current workspace/product contracts while release targets and open-work state remain in the ABC split.
- `wiki`: Added newcomer FAQ guidance for release/status/document boundaries so readers understand that architecture docs describe implementation contracts while release history, targets, and open work live in the ABC/status surfaces.
- `BACKLOG.md`: Retargeted the broad web-client UI architecture simplification bucket into closable product-stabilization slices with explicit gating and exit criteria.
- `wiki`: Aligned the development status pages with the refined backlog language so open client work is described as named hotspot slices rather than open-ended polish.
- `wiki`: Added a status/release-history reading path so newcomers route through Development Status, FAQ, BACKLOG, and CHANGELOG instead of looking for release targeting inside architecture docs.
- `wiki`: Expanded the core glossary with architecture document, BACKLOG, CHANGELOG, and Development Status terms to make the documentation/status boundary discoverable from vocabulary lookup, and refreshed wiki metadata to expose that boundary in navigation/graph discovery.
- `BACKLOG.md` + `wiki` + `context`: Captured the medium-priority generated-wiki semantic compression/refactor task, added durable guidance that wiki is a concise living hyperlinked learning lens over current implementation rather than a release-note/version surface, and removed current-version rhetoric from wiki status metadata/prose.
- `web-client`: Closed named staking/governance/wallet/settings input hotspots by moving staking epoch/amount controls, governance item-id / payout-asset / base-amount controls, wallet send amount, settings domain id, and sidebar edge selection to UI Kit field primitives; `NumberInput` now owns repeated label/helper wiring while domain parsing remains in owning slices, UI Kit numeric/select fields now fill their host width by default, and UI Kit class props/helpers now use Svelte's `ClassValue` type directly instead of a local alias with README coverage. `IconButton` now mirrors that explicit `ClassValue` class contract when forwarding to `Button`, `RichSelect` now centralizes the rich dropdown used by the swap asset selector so widgets no longer import Bits UI directly, UI Kit class-like props consistently accept `null` alongside `ClassValue`, and recursive class flattening no longer needs an element-level type assertion.

## [0.4.0] - 2026-05-15

### AAA Kernel Hardening

- `pallet-aaa` + `docs`: Crystallized the AAA lifecycle contract around normal cycles, close tails, lifecycle touchpoints, scheduler liveness, canonical `[Noop]` close plans, tracked funding, manual-trigger semantics, timer re-arm behavior, and close-tail finality without expanding the AAA feature surface.
- `pallet-aaa` + `deos-runtime`: Added System Immutable AAA support for hard protocol anchors; TOL Bucket A (`aaa_id=3`) and BLDR Bucket A (`aaa_id=12`) are immutable in the reference topology and cannot be mutated, paused, closed, or reopened by runtime extrinsics, including governance/root.
- `tests`: Added durable lifecycle and System Immutable conformance coverage; the pallet suite passes with `181` tests and the runtime suite passes with `305` tests.

### TMCTOL Contract Hardening

- `docs` + `simulator`: Hardened the TMCTOL public guarantee contract around conditional floor claims, canonical reported floor inputs, the TOL Anchor Invariant, bucket/LP accounting states, burn-liveness classification, Zap postconditions, bounded router-fee mutability, conservation rules, and conformance status; the simulator suite covers 65 passing vectors.
- `primitives` + `deos-runtime`: Added a storage-free `TmctolReadModelApi` that exposes bounded live guarantee state from existing AAA, TMC, Assets, and Asset Conversion state without adding a new pallet or consensus analytics storage; native burn liveness, reference BLDR buyback/burn liveness, and Zap postcondition status are reported as separate domains.
- `pallet-axial-router` + `deos-runtime`: Bounded governance-settable router fees with an explicit `MaxRouterFee` contract so Root or governance parameter changes cannot silently invalidate TMCTOL burn-liveness or conservation assumptions.
- `deos-runtime` + `docs`: Removed runtime analytics/dashboard scaffolding from the runtime source tree and codified the boundary that dashboards, trends, alerts, and historical metrics belong in test helpers, external indexers, operator tooling, or product services.

### Release Line Readiness

- `release`: Prepared the `0.4.0` release line by bumping Rust workspace package versions, web-client package metadata, runtime `spec_version` to `210`, and refreshed committed runtime metadata / PAPI descriptors for the new runtime API and economic hardening surfaces.
- `deos-runtime`: Completed the Phase 1 Fee Sink to Native Staking LP Farmer bridge, including staking-yield reconciliation into pool truth and runtime regression coverage for reserve strengthening without donor LP minting.
- `template` + `scripts` + `docs`: Advanced the operator/tooling baseline to Polkadot `stable2603-2` / node `v1.22.2` while keeping the SDK `2603.0.0` umbrella crate baseline and updating only the patch crates present in this workspace lockfile.
- `scripts`: Repaired stale runtime WASM artifact paths and hardened local Zombienet block-stability probing against startup/onboarding latency.
- `BACKLOG.md`: Removed completed hardening sections so the backlog now tracks only remaining open work while this changelog records delivered release content.

## [0.3.2] - 2026-05-06

### Template Workspace Hygiene

- `release`: Prepared the `0.3.2` release line by bumping Rust workspace package versions and web-client package metadata, with runtime `spec_version` now at `209` after the XCM weight/config update plus Phase 1 Fee Sink bridge fix.
- `template`: Refreshed staking workspace README wording around the current liquid `stXXX` / locked `NTVE/stNTVE` LP nomination contract and normalized the asset-conversion runtime integration test module spelling.
- `deos-runtime`: Replaced most placeholder runtime `WeightInfo = ()` bindings with concrete upstream SDK `SubstrateWeight<Runtime>` implementations for timestamp, transaction-payment, parachain-system, message queue, XCMP queue, session, and collator selection; the remaining weight-reclaim placeholder is documented as an SDK 2603 visibility constraint where the public fallback returns the same measured constant weight.
- `scripts`: Added `audit-template-readiness.sh`, a lightweight static gate for template readiness smells covering XCM fallback weights, unclassified runtime weight placeholders, stale staking aliases, and asset-conversion naming drift.
- `deos-runtime` + `scripts`: Added the correct `pallet_xcm` benchmark registration path through `pallet_xcm::benchmarking::Pallet::<Runtime>`, generated and wired runtime-local `pallet_xcm` weights, and taught benchmark normalization to use `polkadot_sdk` paths plus repository-relative generated comments.

### Collator Economics & Fee Routing

- `deos-runtime`: Added the first unified fee-collection slice for transaction fees: `RuntimeFeeSplit` routes 20% of resolved transaction-fee credit to the current author / collator and 80% to Fee Sink, with a safe fallback that sends all fees to Fee Sink when no author can be resolved.
- `pallet-aaa` + `deos-runtime`: Replaced direct AAA fee transfers with a runtime-bound `FeeRouter`, so user-AAA creation/evaluation/execution fees can use the same 20% author / 80% Fee Sink contour when the author is resolvable and otherwise safely fall back to Fee Sink.
- `pallet-staking`: Added deterministic `lp_reward_account(asset_id)` ingress alongside `pool_account(asset_id)` and `reward_account(asset_id)`, preserving separate staking-pool yield, liquidity-pool donation, and future claimable reward channels while the current Phase 1 Fee Sink plan routes native LP-donation funding directly into AAA #14.
- `simulator`: Added pure reward-routing helpers and regressions for the 20/80 outer collection split, Phase 1 two-pool Fee Sink redistribution, and Phase 2 `1:1:4` Fee Sink redistribution.
- `pallet-aaa` + `deos-runtime`: Materialized Fee Sink as System AAA #1 with a Phase 1 `SplitTransfer` execution plan that fans out accumulated native fees/rewards 50/50 into the native staking pool account and the Native Staking LP Farmer AAA #14.
- `docs` + `BACKLOG.md`: Fixed the launch economics contract around the two-phase model: Phase 1 keeps trusted collators and only two pool reward flows, while Phase 2 is an explicit runtime-upgrade boundary for permissionless collators, LP nomination, and GovXP-weighted claimable nomination rewards.

## [0.3.1] - 2026-05-06

### Staking Specification Hardening

- `docs/staking.specification.en.md`: Merged the accepted staking proposal clarifications into the canonical contract: non-locked `NTVE/stNTVE` LP transfer isolation, the empty-pool precondition for `staking_exchange_rate`, bounded skip/defer behavior for AAA liquidity donation, and the governance-custody ordering rule that prevents double counting while `lock_until` is active.
- `docs` + `BACKLOG.md`: Removed the temporary staking proposal document after merging its accepted items and closed the corresponding backlog slice so the canonical staking specification remains the only source of truth.

## [0.3.0] - 2026-04-25

### Native Staking Launch Contract

- `Native staking contract`: Shipped the liquidity-backed `$NTVE` launch model as one coherent baseline: `$NTVE` stakes into liquid yield-bearing `stNTVE`, the canonical `NTVE/stNTVE` AMM becomes the security-liquidity surface, collator nomination uses explicitly locked canonical LP, governance power comes only from explicit custody sources, and native nomination rewards settle through bounded epoch accounting rather than transferable-balance event tracking.
- `pallet-staking` + `deos-runtime`: Replaced the obsolete native-binding security path with locked `NTVE/stNTVE` LP nomination, including lock/unlock/withdraw/redelegate lifecycle, conservative native-equivalent LP valuation, governance-lock custody enforcement, collator-only nomination reward base separation, and trusted-collator-phase session behavior that no longer depends on liquid `stNTVE` balances or native-binding cache repair.
- `pallet-staking` + `pallet-governance`: Landed the full `NativeVotePower` custody model: locked `$NTVE`, locked `stNTVE`, standalone locked `NTVE/stNTVE` LP, and collator-locked LP feed frozen ballot power through explicit runtime valuation, while already-cast governance weights remain immutable across later exchange-rate, AMM-reserve, or custody changes.
- `pallet-staking`: Shipped native nomination reward settlement as a dedicated surface: generic `claim_reward` / `claim_reward_batch` reject the native staking asset, native rewards are recognized by reward-account balance reconciliation during bounded resumable epoch rollover, `claim_nomination_reward` and `claim_nomination_reward_batch` pay liquid `$NTVE`, and `claim_and_compound_nomination_reward(epoch, operator)` compounds into new `NTVE/stNTVE` LP locked to the explicit target operator.
- `pallet-aaa` + `deos-runtime`: Kept AAA staking portable by removing the DEOS-specific `StakeNative` task and routing native/liquid staking through generic `Task::Stake { asset, amount }`; added generic `DonateLiquidity` plus the Native Staking LP Farmer System actor so protocol-owned `$NTVE` can deterministically strengthen `NTVE/stNTVE` reserves without minting LP to the donor.
- `deos-runtime` + Asset Conversion: Shipped the deterministic stake-vs-donate native LP-farming path, canonical LP namespace seeding, balanced donation helper, native nomination reward compound helper, guarded Native Staking LP Farmer activation, and runtime coverage proving donation increases reserves per LP token while ordinary compounding mints LP and locks it transactionally.
- `pallet-staking` + `deos-runtime` weights: Added benchmark coverage for the expanded native LP custody, governance custody, rollover, claim, batch-claim, and compound paths; restored benchmark compilation and regenerated production staking runtime weights from the expanded benchmark set.

### Native Staking Read Models, Client, and Operations

- `pallet-staking` + `pallet-governance` + PAPI: Exposed bounded canonical views for native staking exchange rate, canonical `NTVE/stNTVE` reserves and LP issuance, account locked-LP/governance-custody detail, known-epoch nomination reward claimability, and account governance power/frozen-ballot facts; refreshed committed metadata/descriptors through the reproducible runtime metadata export path.
- `web-client`: Split native staking into a dedicated `StakingWidget` with canonical-chain provenance, exact 12-decimal amount parsing, signer-gated reward claim/compound actions, collator LP nomination lifecycle actions, governance-only NativeVotePower custody actions, bounded wallet balances, safe-max helpers, pending unlock detail, and grouped collapsible action sections.
- `scripts` + local bootstrap: Added local native staking bootstrap support, plan-only production/operator call preparation, JSON-safe readiness probing, and guarded activation sequencing for the canonical `NTVE/stNTVE` pool and Native Staking LP Farmer actor.

### Governance and Product Baseline Stabilization

- `Governance v1`: Closed the current domain/cadence/payload-kind rollout baseline as shipped and moved wider `L2ParameterChange`, per-kind execution observability, browser composition, and archive growth behind explicit future policy/provider gates.
- `web-client`: Added an explicit governance archive boundary so full archive search and ballot timelines remain materialized-provider work instead of being implied by bounded on-chain recent-finalized retention.

### Documentation, Wiki, and Context

- `docs/staking.specification.en.md` + `docs/staking.architecture.en.md`: Aligned the NativeStaking contract and shipped architecture around the simpler final baseline: unlock requests remove active backing immediately while custody withdrawal remains delayed, nomination reward compounding uses an explicit operator target, donation acquisition defaults to deterministic stake-vs-donate, generic native-asset calls are allowed instead of mandatory wrapper extrinsics, and architecture wording reflects the trusted-collator phase plus regenerated weights.
- `docs` + `wiki` + `BACKLOG.md`: Repaired stale native-binding and `StakeNative` wording, regenerated affected wiki projections, documented production/operator bootstrap sequencing, closed the native staking migration and current governance-product rollout baselines, and reclassified swap/mixed-route LP donation acquisition plus broader governance/archive growth as conditional future work.

## [0.2.0] - 2026-04-23

### Runtime Hardening

- `pallet-staking` + `deos-runtime`: `on_idle` staking maintenance now respects the caller-provided remaining-weight budget across native-delegation event ingress, dirty-cache repair, and reward-event ingress instead of overshooting tiny budgets on the first scanned event or first repair item.
- `deos-runtime`: Added regression coverage for tiny-budget native-delegation ingress, tiny-budget reward ingress, and sub-item-budget dirty-cache repair so future hardening passes keep the bounded `on_idle` contract honest.
- `pallet-tmc`: `create_curve()` now rejects nonexistent/self-paired asset kinds, `mint_with_distribution()` preflights asset existence, and the collateral-transfer + mint-distribution path is now transactional so late mint failures roll the whole mint back.
- `pallet-tmc`: Added deterministic user-leg / sink-leg mint failure hooks in the mock runtime plus rollback regressions covering both local-collateral and native-collateral failure paths.
- `deos-runtime`: Added runtime TMC success/failure assertions for both shipped sink topologies (default Zap Manager sink and BLDR splitter sink), and strengthened router `DirectMint` coverage to assert fee routing, net collateral delivery, and total mint-output conservation end-to-end.
- `pallet-tmc`: Benchmark coverage now includes `mint_with_distribution`, and `runtime/src/weights/pallet_tmc.rs` was regenerated from the expanded benchmark set.
- `pallet-staking` + `deos-runtime`: Delegation-weighted collator ranking now reads cached per-operator delegated-native shares refreshed from native binding and `stNTVE` balance-change ingress during `on_idle`, falls back to exact read-derived backing if that bounded ingress truncates, repairs the dirty cache through a bounded two-phase clear/rebuild loop, auto-retires stale zero-exposure bindings on cache refresh, immediately updates clean-cache rebinding / clear-to-passive deltas without waiting for later event replay, and now has runtime regressions for larger candidate-set ordering/top-N behavior, equal-backing/equal-deposit cutoff ties, and a deterministic probe proving clean cached ranking stays candidate-count-bound even with many bindings.
- `template/runtime/src/weights/pallet_staking.rs`: Regenerated staking weights after the native-delegation cache lifecycle hardening so `bind_native` / `clear_native_binding` reflect the extra cache-refresh storage touches.
- `pallet-staking` + `deos-runtime`: Reward ingress now receives the real post-native-maintenance `on_idle` budget, stops when projected scan/touch/inflow work would exceed that remaining weight, the runtime suite now covers low-idle-budget truncation plus finite-budget aggregation, scan-cap truncation, and governance-touch pressure, receipt-driven reward events now resolve `stXXX -> base asset` through a live reverse index maintained on receipt creation/backfill, governance-driven reward touches now resolve `domain -> reward assets` through a bounded index maintained at staking-asset registration time, `register_staking_asset` / `initialize_staked_asset` now run transactionally around those index updates, deterministic runtime probes now prove the indexed reward-ingress path stays event/domain bound even with many unrelated pools, direct runtime assertions now pin the exact returned reward-ingress weight for bounded inflow and governance-touch mixes, the larger-set NativeBindings suite now also proves that immediate clean-cache rebinding can flip top-N membership without waiting for `on_idle`, and the shipped staking weights were regenerated after the new indexed storage writes landed.
- `deos-runtime` XCM hardening: The runtime test suite now captures the origin-conversion baseline for `Parent`, `sibling`, `AccountId32`, and `XcmPassthrough`, covers representative barrier behavior for paid sibling plus explicit unpaid parent / parent-executive / sibling paths, proves relay/sibling/signed origins cannot reach XCMP queue-controller extrinsics while Root still can, and the runtime now narrows sovereign `Transact` to the truthful empty call matrix via `SafeCallFilter = Nothing`.
- `docs/core.architecture.en.md` + governance docs: The repo now explicitly documents that sovereign XCM `Transact` ships fail-closed on the current line and that tactical `L2TreasurySpend` currently has one bounded funding-source topology only (`BldrTreasury` -> tactical domain treasury sovereign), with wider treasury source families left as future opt-in work.
- `pallet-axial-router`: `execute_swap_for()` now pre-validates gross input affordability, collects router fees inside one transactional flow before route execution, and keeps fee-exempt system accounts on the same debit-order contract with a zero-fee branch.
- `pallet-axial-router`: Added deterministic forced-fee-failure hooks plus regressions covering `DirectXyk`, `DirectMint`, and `MultiHopNative`, proving fee-routing failures cannot execute the route or mutate pool/user balances.
- `deos-runtime`: Axial Router integration tests now assert concrete balance deltas, fee-sink deltas, event payloads/order, native-input semantics, repeated fee accumulation, and failed-swap no-fee behavior instead of API-compatibility placeholders.
- `pallet-axial-router`: The swap benchmark now asserts gross input debit, fee-sink credit, and non-zero output on the hardened transactional path.
- `runtime/src/weights/pallet_axial_router.rs`: Regenerated pallet weights from the hardened benchmark path; `swap()` now reflects the extra AAA ingress and transactional fee-routing storage touch pattern.
- `pallet-aaa`: Raised the coarse `dex_swap` task weight bucket so AAA swap-task upper bounds stay above the refreshed Axial Router benchmark weight.
- `scripts/benchmarks.sh`: Runtime-WASM discovery now prefers the renamed `deos-runtime` artifact and falls back to the legacy TMCTOL path, fixing local weight-generation after the rename.
- `docs/axial-router.architecture.en.md`: Swap-flow documentation now reflects the gross-debit preflight and fee-before-execution transactional ordering.

## [0.1.2] - 2026-04-22

### Framework Identity — TMCTOL → DEOS Rename Epic

- `tmctol-runtime` → `deos-runtime`: runtime crate, `spec_name`/`impl_name`, and all build artifact paths.
- `pallet-tmctol-governance` → `pallet-deos-governance`, `pallet-tmctol-staking` → `pallet-deos-staking`: pallet crate names and workspace aliases.
- Scripts updated: `03-build-runtime.sh`, `04-generate-chain-spec.sh`, `06-zombienet-e2e.sh`, `release-local.sh`, `validate-local.sh`, `aaa-release-gate.sh` — all `tmctol-runtime`, `tmctol-dev`, `tmctol-local`, `tmctol` chain ID references migrated.
- Web-client adapter layer: `TmctolPapiConnection` → `DeosPapiConnection`, `TmctolChainSnapshot` → `DeosChainSnapshot`, `TmctolChainConnectionState` → `DeosChainConnectionState`, `connectTmctolSigner` → `connectDeosSigner`, `DEFAULT_TMCTOL_*` → `DEFAULT_DEOS_*`.
- Web-client local-storage keys: `tmctol-tile-layout`, `tmctol-workspace-frame`, `tmctol.wallet.selected-address` → `deos-*`.
- PAPI descriptors regenerated: entry `tmctol` removed, `deos` added from new runtime WASM metadata; generated namespace now uses `deos`.
- Weight file comments updated to reference `deos-runtime` artifact paths.
- Runtime integration test module **preserved** as `tmctol_integration_tests`: it tests the TMCTOL economic standard (TMC, TOL, Router, Splitter, Zap Manager, Bucket) on top of DEOS runtime, so the `tmctol` prefix correctly identifies the standard under test, not the framework.

## [0.1.1] - 2026-04-22

### Runtime

- `pallet-governance`: Extracted `CoreResolutionOutcome` as the single source of truth for proposal resolution policy, eliminating duplication between the execution path and the view/status path.
- `pallet-governance`: Added `build_vote_tally` — a pure storage-free tally builder consumed by both execution and query surfaces, removing redundant storage reads.
- `pallet-governance`: Added 10 isolated unit tests covering all branches of the core resolution policy (zero turnout, below threshold, ties, approval not met, Binary Aye/Nay wins, Invoice positive wins, equal-weight last-wins).
- `pallet-governance`: Fixed a latent bug where `proposal_resolution_state` incorrectly returned `VoteTie` for Invoice proposals with no Nay votes due to a duplicated pre-family tie check.

### Web Client

- Decoupled `GovernanceProviderState` from `TmctolChainConnectionState`; governance domain types are now independent of the concrete blockchain adapter.
- Removed `walletStore` import from `GovernancePapiProvider`; `getWriteSurfaceAvailability` now accepts `accountId` as a parameter, making the adapter fully stateless.
- Introduced `GovernanceProposalDescriptor` named type; `GovernancePanelProposal` and `GovernanceRetainedFinalizedProposal` now compose via intersection, eliminating duplicated hydration logic.
- Extracted `loadProposalCommonFields` helper shared by active and retained proposal loaders.
- Relaxed `hasBuiltInDevSigner` signature to accept `string | null`, removing the `?? ""` hack.

## [0.1.0] - 2026-04-22

### Framework & Runtime

- `pallet-asset-registry`: Implemented O(1) bidirectional reverse index (`AssetId -> Location`) removing the need for bounded capacity scans.
- `pallet-asset-registry`: Regenerated benchmark weights reflecting the O(1) lookup architecture.
- `pallet-governance`: Cleaned up legacy `StorageVersion` migration lineage, resetting baseline schema to `1`.
- `pallet-governance`: Deduplicated terminal resolution paths and extracted shared helpers.

### Web Client

- Deduplicated governance responsive UI rendering snippets.
- Extracted provider write-surface logic into dedicated bounded files (`write-surface.ts`).
- Restructured `constants.ts` and `types.ts` to strictly separate definitions from execution logic.

## [0.0.0] - Delivered Baseline

### Runtime Platform Crystallization

- `Polkadot SDK 2603 baseline is fully landed`: the runtime, CI, docs, and local tooling now align on Polkadot SDK `2603` / node `1.22.0`, Omni Node deployment, and the current runtime/system contract.
- `Framework identity is explicit`: DEOS vs TMCTOL naming, the forkable-framework boundary, and the repo entrypoint graph are now coherent across root docs, subsystem docs, and generated knowledge surfaces.

### Core Economic Kernel

- `AAA is now a first-class deterministic actor runtime`: bounded scheduling, event-driven triggers, runtime-owned system actors, execution-plan semantics, and the current reference topology are all implemented and documented as one coherent contract.
- `Axial Router and TMC launch physics are stabilized`: routing stays mechanism-over-policy, tracked-asset/oracle coordination is live, and TMC launch parameters are treated as immutable launch physics on the current line.
- `Asset Registry baseline`: foreign assets use deterministic registration plus persistent `Location -> AssetId` identity.

### Governance and Staking

- `Governance v1 is landed on the current line`: DEOS governance now ships the bounded dual-track domain model, public ordinary cadence, payload-kind/cadence metadata, bounded active/finalized query surfaces, signed advisory submission, strategic runtime-upgrade authorization, and live tactical invoice-native treasury governance.
- `Share-vault staking is landed as the canonical staking substrate`: multi-asset `stXXX` receipts, native `stNTVE` operator-aware binding, sparse reward ingress, same-asset auto-compound settlement, and governance-conditioned reward export now form one coherent staking baseline.

### Browser Reference Client

- `Governance client became domain-first`: governance types, constants, read/write contracts, and UI semantics now live in the governance slice instead of the adapter layer, while the browser reflects bounded runtime truth for proposal status, timing, execution detail, and submission semantics.
- `Wallet and swap UX are hardened`: safe-max enforcement, tracked-asset transfers, route/provenance honesty, draft-keyed in-flight behavior, and clearer execution feedback now match the current runtime contract.
- `Pane layout and wiki client matured`: pane chrome/layout adaptation is more resilient across narrow surfaces, and the generated wiki now has explicit discovery, provenance, and browser integration.

### Documentation and Knowledge System

- `The docs plane is typed and coherent`: first-class subsystem docs now follow the specification/architecture taxonomy, DEOS terminology is aligned, and subsystem contracts are easier to navigate as a stable framework memory layer.
- `The generated wiki is now a real repo-local knowledge surface`: `/wiki` became a provenance-aware newcomer-facing projection of `/docs`, with navigation metadata, localized pages, and direct browser consumption.
- `Context files now describe the current framework line instead of inherited history`: root memory, README entrypoints, and durable protocol rules are aligned to the `0.0.0` DEOS repo baseline and its forkable-framework posture.

### Tooling and Validation

- `Benchmarking, CI, and local probes now match the live system`: benchmark lanes, runtime benchmark compilation, web-client probes, and operator/developer scripts were tightened so validation reflects the current runtime and browser contract more honestly.
- `Runtime and frontend hardening is now evidence-driven`: zero-warning Rust validation, pallet/runtime benchmark bridges, and explicit browser provenance/read-model discipline now form part of the shipped engineering baseline.
