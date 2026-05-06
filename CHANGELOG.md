# DEOS Framework Delivery History

> Canonical complete delivery history for the current DEOS repository line
>
> This repository restarted its own release line at `0.0.0` after the move into the new DEOS monorepo. The changelog therefore focuses on achieved epics and the current shipped baseline of this repo, not on preserving every intermediate refactor step or pre-reset chronology.

## [Unreleased]

## [0.3.2] - 2026-05-06

### Template Workspace Hygiene

- `release`: Prepared the `0.3.2` release line by bumping Rust workspace package versions, web-client package metadata, and runtime `spec_version` to `208` for the XCM weight/config update.
- `template`: Refreshed staking workspace README wording around the current liquid `stXXX` / locked `NTVE/stNTVE` LP nomination contract and normalized the asset-conversion runtime integration test module spelling.
- `deos-runtime`: Replaced most placeholder runtime `WeightInfo = ()` bindings with concrete upstream SDK `SubstrateWeight<Runtime>` implementations for timestamp, transaction-payment, parachain-system, message queue, XCMP queue, session, and collator selection; the remaining weight-reclaim placeholder is documented as an SDK 2603 visibility constraint where the public fallback returns the same measured constant weight.
- `scripts`: Added `audit-template-readiness.sh`, a lightweight static gate for template readiness smells covering XCM fallback weights, unclassified runtime weight placeholders, stale staking aliases, and asset-conversion naming drift.
- `deos-runtime` + `scripts`: Added the correct `pallet_xcm` benchmark registration path through `pallet_xcm::benchmarking::Pallet::<Runtime>`, generated and wired runtime-local `pallet_xcm` weights, and taught benchmark normalization to use `polkadot_sdk` paths plus repository-relative generated comments.

### Collator Economics & Fee Routing

- `deos-runtime`: Added the first unified fee-collection slice for transaction fees: `RuntimeFeeSplit` routes 20% of resolved transaction-fee credit to the current author / collator and 80% to Fee Sink, with a safe fallback that sends all fees to Fee Sink when no author can be resolved.
- `pallet-aaa` + `deos-runtime`: Replaced direct AAA fee transfers with a runtime-bound `FeeRouter`, so user-AAA creation/evaluation/execution fees can use the same 20% author / 80% Fee Sink contour when the author is resolvable and otherwise safely fall back to Fee Sink.
- `pallet-staking`: Added deterministic `lp_reward_account(asset_id)` ingress alongside `pool_account(asset_id)` and `reward_account(asset_id)`, giving Phase 1 Fee Sink redistribution explicit destinations for staking-pool yield and LP donation while reserving `reward_account` for future claimable nomination rewards.
- `simulator`: Added pure reward-routing helpers and regressions for the 20/80 outer collection split, Phase 1 two-pool Fee Sink redistribution, and Phase 2 `1:1:4` Fee Sink redistribution.
- `pallet-aaa` + `deos-runtime`: Materialized Fee Sink as System AAA #1 with a Phase 1 `SplitTransfer` execution plan that fans out accumulated native fees/rewards 50/50 into the native staking pool account and `lp_reward_account(NTVE)`.
- `docs` + `BACKLOG.md`: Fixed the launch economics contract around the two-phase model: Phase 1 keeps trusted collators and only two pool reward flows, while Phase 2 is an explicit runtime-upgrade boundary for permissionless collators, LP nomination, and GovXP-weighted claimable nomination rewards.

## [0.3.1] - 2026-05-06

### Staking Specification Hardening

- `docs/staking.specification.en.md`: Merged the accepted staking proposal clarifications into the canonical contract: non-locked `NTVE/stNTVE` LP transfer isolation, the empty-pool precondition for `staking_exchange_rate`, bounded skip/defer behavior for AAA liquidity donation, and the governance-custody ordering rule that prevents double counting while `lock_until` is active.
- `docs` + `BACKLOG.md`: Removed the temporary staking proposal document after merging its accepted items and closed the corresponding backlog slice so the canonical staking specification remains the only source of truth.

### Polkadot SDK stable2603-1 Patch Migration

- `template/Cargo.lock`: Applied the `polkadot-stable2603-1` patch release by updating the corresponding 2603 patch crates in lockfile, including client/collator-protocol fixes, `frame-support`, Westend runtime, prospective-parachains, statement-store, and related node-side crates while keeping the 2603.0.0 umbrella crate baseline.
- `scripts` + docs: Retargeted local Polkadot SDK binary/tool guidance from `polkadot-v1.22.0` to `polkadot-stable2603-1` (node v1.22.1) and recorded that stable2603-1 is a patch-level lockfile/operator binary migration with no DEOS runtime API shape change required.

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
