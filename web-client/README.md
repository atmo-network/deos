# DEOS Web Client

Repository-local SvelteKit workspace for the browser-facing DEOS reference client and generated-wiki reader.

The web client is **not** the protocol source of truth. It is the reference product surface that presents bounded chain state, signed actions, generated documentation, and execution feedback for the current DEOS/TMCTOL framework line.

For the full architecture contract, read [`../docs/web-client.architecture.en.md`](../docs/web-client.architecture.en.md).

## Current Product Shape

The client is an on-chain-first reference UI.

It provides:

- live wallet/account selection for local dev signers and injected wallets;
- chain-backed wallet balances, bounded tracked-asset transfers, and receive-address surfaces;
- Axial Router swap quotes and signed swap submission;
- staking views/actions for the current native `stNTVE` / `NTVE-stNTVE` launch-line model;
- governance proposal viewing, voting, advisory submission, tactical treasury invoice submission, preimage review, and runtime-upgrade relay guidance;
- automation and system-status views from the live adapter snapshot;
- chart/session history and route diagnostics with explicit provenance limits;
- centralized transaction, receipt, finalization, and live-network feedback through `LogWidget`;
- generated wiki browsing from `../wiki` with metadata-backed navigation, aliases, graph links, page state, source provenance, and trusted markdown rendering.

The client should read as one coherent reference product, not as a sequence of intermediate refactors. Release targeting belongs in `../CHANGELOG.md` and status/roadmap surfaces, while this README describes the current workspace contract.

## Truth and Read-Model Rule

The client follows the repository-wide read-model contract:

- **canonical-chain** surfaces are bounded runtime/query/storage projections suitable for live user flows;
- **materialized** surfaces are indexed/archive/search/analytics views and must be labeled as such;
- browser realization is separate from truth class: a view may be `direct`, `session-cache`, `session-derived`, or `provider` backed.

Do not make archive/search/dashboard behavior look like canonical chain truth. If a future feature needs an indexer, model that dependency explicitly.

## Architecture Boundaries

### Domain slices

- `src/lib/market/` — swap direction, quotes, execution orchestration, live price/session history.
- `src/lib/portfolio/` — balances, bounded asset projection, transfer/deposit bridging.
- `src/lib/staking/` — staking-facing contracts and UI types.
- `src/lib/governance/` — governance store, labels, payload helpers, review helpers, and UI-facing projections.
- `src/lib/automation/` — automation-facing contracts and UI types.
- `src/lib/log/` — account log, live network feed, transaction progress, and receipts.
- `src/lib/wallet/` — wallet session state, signer discovery, address validation, local dev signer routing.
- `src/lib/system/` — chain snapshot, refresh ownership, endpoint/session wiring, adapter runtime context, browser persistence.
- `src/lib/wiki/` — trusted repo-local wiki markdown loading/rendering helpers.

Reusable domain contracts live with their owning slices. Do not recreate a generic `shared/` bucket.

### Adapters

- `src/lib/adapters/contract.ts` is the live aggregate UI adapter contract plus named capability interfaces.
- `src/lib/adapters/blockchain/` is the concrete PAPI-backed chain adapter implementation behind a stable facade.
- `src/lib/adapters/governance/` owns typed governance read/write providers.
- `src/lib/adapters/materialized-history/` is the explicit boundary reserved for future materialized governance/archive history providers.

Concrete adapters receive shell/session facts through `system/adapter-context.ts`; they should not import wallet stores or endpoint state directly.

### Layout and widgets

- `src/lib/layout/` owns the workspace frame, center tile tree, pane hosts, tabs, split handles, header, footer, sidebar, reserved lane specs, and mobile linearization.
- `src/lib/widgets/` owns user-facing product surfaces such as Swap, Wallet, Staking, Governance, Chart, Statistics, Automation, Log, Wiki, Account, Settings, Status, and AccountChip.
- Reserved edge lanes are developer-configured shell zones, not user-reorderable economic panes.
- Widgets should adapt to pane width/height and keep the main action readable before exposing secondary diagnostics.

### UI Kit

`src/lib/ui/` is the local UI Kit. Its local contract is documented in [`src/lib/ui/README.md`](src/lib/ui/README.md). It owns reusable presentation primitives and safe interaction defaults:

- `Button`, `IconButton`, `SelectableTile`
- `Card`, `SectionCard`, `StatCard`, `DetailRow`, `Notice`, `Badge`
- `TextField`, `NumberInput`, `TextArea`, `SelectField`
- `PopoverPanel`, `SidePanelDialog`, `ReadModelBadge`, `Sparkline`
- `class.ts` and `format.ts`

UI Kit primitives stay foundation-only and must not import product domains. Repeated raw controls should graduate into UI Kit instead of being rebuilt inside widgets.

### Domain DAG

`domain-dag.json` is the client-local architecture gate. It checks:

- local import cycles;
- missing ownership headers;
- forbidden reach-through edges;
- generic shared-bucket drift;
- widget-to-concrete-adapter imports;
- UI-kit-to-domain imports;
- calibrated widget size/callback surface pressure.

Surface-pressure warnings are triage signals, not folder-theater mandates.

## Generated Wiki Boundary

`WikiWidget` renders repo-local generated wiki markdown from `../wiki`.

The wiki is trusted reviewed repository content, not arbitrary user input. Safety belongs to repository validation through:

```sh
npm run validate:wiki
```

That guard rejects raw HTML blocks, dangerous URL schemes, inline DOM event handlers, and malformed wiki frontmatter before content is rendered in the browser.

The widget consumes:

- `../wiki/_meta/navigation.json`
- `../wiki/_meta/aliases.json`
- `../wiki/_meta/graph.json`
- `../wiki/_meta/state.json`
- `../wiki/_meta/locales.json`

## Local Development

Install dependencies and start the dev server:

```sh
npm install
npm run dev
```

`npm install` runs `papi generate` from the committed `.papi/metadata/deos.scale` snapshot. When runtime query/view surfaces change, refresh metadata with:

```sh
../scripts/export-papi-metadata.sh
```

For local native-staking demos, regenerate the chain spec with current runtime presets, start the local network, then run:

```sh
../scripts/07-seed-web-client-state.sh
```

That script is local-dev-only.

Optional browser-open mode:

```sh
npm run dev -- --open
```

## Validation

Use the smallest meaningful validation first. For full client validation, run:

```sh
npm run validate
```

That command runs Prettier, Svelte checks, and the production build. For architecture-boundary changes, also run the Domain DAG gate:

```sh
npm run validate:dag
```

`validate:dag` resolves the validator through `DOMAIN_DAG_VALIDATOR`, `SKILL_DIR`, or the current user's default pi skill path. It preserves the default web-client root when forwarding extra validator args, and the Domain DAG config includes `scripts/` so these package launchers stay under the same source-boundary/header gate. Run `npm run validate:dag -- --help` for launcher options.

For wiki-rendering/content changes, also run:

```sh
npm run validate:wiki
```

`validate:wiki` resolves the validator through `WIKI_TRUST_VALIDATOR` or the repo-local wiki-sync skill path. It preserves the default repo wiki directory when forwarding extra validator args. Run `npm run validate:wiki -- --help` for launcher options.

To run every configured client-adjacent gate:

```sh
npm run validate:all
```

## Production Build

```sh
npm run build
npm run preview
```
