# DEOS Framework Delivery History

> Canonical complete delivery history for the current DEOS repository line
>
> This repository restarted its own release line at `0.0.0` after the move into the new DEOS monorepo. The changelog therefore focuses on achieved epics and the current shipped baseline of this repo, not on preserving every intermediate refactor step or pre-reset chronology.

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
