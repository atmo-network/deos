# DEOS Project Protocol

> Durable project context for contributors and agents. Detailed subsystem truth belongs in specifications, architecture documents, code, tests, and repo-local skills.

## 0. Meta-Protocol Principles

- `Reflexive Protocol`: The context system must obey the same boundedness, ownership, validation, and cleanup rules that it imposes on the project.
- `Mandatory Knowledge Sync`: Every meaningful task reconciles durable rules, open work, delivery history, entrypoints, and subsystem truth when reality changes.
- `Flat Structure`: Use second-level headings and one-level lists only so rules remain scannable, addressable, and independently editable.
- `Single Ownership`: Keep each fact in one authoritative surface and replace duplicated detail with a truth-owner reference.
- `Boundary Clarity`: Keep meta-protocol, project architecture, open work, delivery history, and subsystem implementation truth in their respective layers.
- `Constraint-Driven Evolution`: Add structure only after real constraints expose the need; preserve complexity earned by invariants and delete complexity created by habit.
- `Test-Driven Evolution`: Treat context structure, links, terminology, claims, and open-work truth as testable infrastructure rather than optional prose hygiene.
- `Context Optimization`: Evolution includes addition, consolidation, relocation, and deletion; growth alone does not count as improvement.
- `Progressive Consolidation`: Move exploratory knowledge toward stable contracts as evidence accumulates, then retire parallel or superseded explanations.
- `Cognitive Infrastructure`: Treat docs, scripts, skills, audits, and validation gates as part of the system because they determine which abstractions contributors reproduce.
- `Completion Honesty`: A task is not done while validation fails, backlog state lies, completed work remains open, or an external gate is presented as locally verified.

## 1. Project Identity and Scope

- `Repository Type`: DEOS is a specification and reference framework for deterministic protocol economies.
- `DEOS`: The Deterministic Economic Operating System provides the runtime kernel, Account Abstraction Actors, routing, staking, governance, consensus integration, and bounded read surfaces.
- `TMCTOL`: The Token Minting Curve plus Treasury-Owned Liquidity standard is the flagship economic configuration running on DEOS; it is not the only possible DEOS economy.
- `Goal`: Provide a forkable foundation for launching ecosystems with explicit economic mechanisms, inspectable invariants, and production-oriented validation.
- `Product Boundary`: This repository owns the framework and reference stack, not a finished branded ecosystem product.
- `First-Ecosystem Relationship`: DEOS is both reusable infrastructure and the substrate of its first intended production ecosystem; reusable contracts stay here while identity, dApps, launch narrative, and concrete product loops stay downstream.
- `Adoption Model`: Partner teams fork DEOS, configure their economy, and may contribute framework-hardening improvements back without moving product policy into the kernel.
- `Mechanism vs Policy`: DEOS owns primitives, invariants, adapters, execution safety, bounded projections, and reference patterns; downstream instances own brand, founder economics, labor culture, invoice norms, bucket names/percentages, marketing, and demand strategy.
- `Release Line`: The standalone repository release line began at `0.0.0`; `CHANGELOG.md` records this line only.
- `Acronym Semantics`: Deterministic means explicit bounded protocol reactions for the same on-chain state, Economic names the managed capital/liquidity domain, and Operating System means a domain-specific execution substrate rather than a general-purpose OS.
- `Primary Human Entry`: `README.md` explains the framework and routes evaluators, builders, operators, and contributors to the right source of truth.

## 2. Context and Truth Ownership

- `Canonical Memory Split`: `AGENTS.md` owns durable protocol, `BACKLOG.md` owns open work, and `CHANGELOG.md` owns completed delivery history.
- `Open Work`: Start repository work from `BACKLOG.md`; add newly discovered in-scope work there and remove completed items immediately.
- `Backlog Shape`: Format open work as `- [ ] \`Domain\`: Description` with uppercase prose after the colon; track closable deliverables and explicit gates only, while evergreen disciplines belong in this protocol, subsystem docs, or skill contracts.
- `Delivery History`: Record meaningful completed outcomes and impact in `CHANGELOG.md`, not in backlog or durable protocol prose.
- `Changelog Shape`: Format delivery entries as `- \`Domain\`: Description`, using slash-separated domain qualifiers when needed; exclude package-marker chores, intermediate implementation diaries, and duplicated architecture explanations.
- `Spec Ownership`: Specifications own intended subsystem contracts, rationale, invariants, and public semantics.
- `Architecture Ownership`: Architecture docs own shipped implementation maps, runtime bindings, storage topology, operational watchpoints, and code anchors.
- `Code Ownership`: Code and tests own executable behavior; documentation claims must remain subordinate to verified implementation truth.
- `Skill Ownership`: Repo-local skills own specialized workflows and audits; do not duplicate their internal procedures here.
- `README Ownership`: Root and subtree READMEs own human orientation, setup, navigation, and current workspace purpose.
- `Read-Model Ownership`: `docs/read-model.contract.en.md` owns chain/materialized data classification; `docs/web-client.architecture.en.md` owns browser realization.
- `Framework Boundary Ownership`: `docs/framework-instance.contract.en.md` owns the reusable mechanism versus downstream policy contract.

## 3. Repository Topology

- `/docs`: Conceptual control plane containing specifications, contracts, architecture maps, strategy notes, and the canonical documentation index.
- `/template`: Rust reference implementation containing the parachain runtime, pallets, primitives, weights, tests, and runtime-adjacent research.
- `/web-client`: SvelteKit reference client for browser-facing DEOS and current TMCTOL flows.
- `/scripts`: Operator/developer automation; numbered scripts are atomic leaves and named scripts are orchestrators or admin utilities.
- `/simulator`: Historical TMCTOL hypothesis lab and authoritative mathematical reference for formulas, thresholds, conservation, floor/compression scenarios, and parameter behavior.
- `/wiki`: Generated bilingual semantic projection of `/docs` for onboarding, frontend rendering, and agent navigation.
- `/.agents/skills`: Repository-local cognitive and validation infrastructure; project-specific audits belong in the `alignment` skill.
- `Support Priority`: Routine stabilization starts with `/docs`, then `/template`, `/web-client`, and `/scripts`; consult `/simulator` whenever tokenomics or invariant math moves.
- `Core Entry`: Start system-wide architecture work with `docs/core.architecture.en.md`.
- `Runtime Entry`: Start Rust workspace work with `template/README.md` and the owning pallet/runtime docs.
- `Client Entry`: Start browser work with `docs/web-client.architecture.en.md` and `web-client/README.md`.
- `Scripts Entry`: Start automation work with `scripts/README.md`, `_common.sh`, and the touched entrypoint's `--help` contract.
- `Wiki Entry`: Start wiki work with `/.agents/skills/wiki-sync/SKILL.md`.

## 4. Canonical Vocabulary

- `Terminology Lockstep`: Stable specs, architecture docs, runtime/API surfaces, wiki, and client copy must use one canonical term per domain atom.
- `Framework Naming`: Use `DEOS` for the framework/runtime/reference stack and `TMCTOL` only for the concrete economic standard.
- `Governance Naming`: Public prose may say `DEOS Governance`; runtime implementation remains `pallet-governance` and client UI remains `Governance` unless a dedicated rename lands.
- `Asset Notation`: Prefix concrete asset symbols with `$` in specs and architecture prose (`$NTVE`, `$VETO`, `$BLDR`); keep bare labels for vote options and non-asset semantics.
- `AAA Abstraction`: Describe automation by System AAA role and execution-plan family rather than legacy manager/farmer names.
- `Actor Casing`: In prose, title-case established role names such as `Burn Actor`, `Liquidity Actor`, and `System AAA Actor`; keep ordinary descriptions lowercase and code identifiers idiomatic.
- `Legacy Names`: Keep manager names only for historical orientation or explicit compatibility aliases at public boundaries.
- `TMC`: The unidirectional issuance engine implementing the configured minting curve.
- `TOL`: The protocol-owned liquidity accumulator and bucketed reserve topology of the TMCTOL standard.
- `Axial Router`: The fee-burning execution gateway selecting the candidate with maximum recipient output across XYK, TMC, and bounded Native-anchored routes.
- `System AAA Actors`: Runtime-owned AAA instances executing bounded protocol economic flows.
- `Burn Actor`: The System AAA role that processes configured balances into burn flow.
- `Liquidity Actor`: The System AAA role family that provisions liquidity for configured pools or lanes.
- `Omnivorous Intake`: Balance-driven ingress semantics that react to assets arriving at an actor account rather than one bespoke extrinsic.
- `Resilience`: Retry and cooldown behavior protecting actors during oracle, liquidity, or market unavailability.
- `Runtime-as-Config`: Generic pallets receive economic and runtime policy through traits and adapters rather than hardcoded ecosystem logic.
- `Omni Node`: The deployment architecture; this repository does not carry a custom node crate.

## 5. Framework Architecture Invariants

- `Framework Forkability`: Changes under `/template` must preserve generic utility and avoid hardcoding downstream ecosystem identity or business policy.
- `Deterministic Mechanics`: Runtime-managed economic reactions must use explicit triggers, typed payloads, bounded state, and weight-accounted execution.
- `Token-Driven Coordination`: Prefer asset movement and runtime hooks over privileged signed calls when token ingress itself defines the event.
- `Bounded Consensus State`: Every storage collection, iteration, history surface, retry path, and projection must have a defensible bound.
- `Read-Model Honesty`: Public data must be classified as bounded authoritative on-chain truth or externally indexed/materialized truth; canonical UX must not hide an indexer dependency.
- `On-Chain Projection`: Keep canonical bounded state and projections on-chain; route archive, search, and unbounded analytics to materialized providers.
- `Mechanism-Over-Policy`: The Axial Router compares viable candidates by maximum recipient output; price impact and fee fields remain informational quote metadata.
- `Transactional Mutation`: Entry points that can fail after touching multiple storage locations must prevalidate fallible conditions or use transactional semantics.
- `Reverse-Index Preference`: Persist bounded reverse mappings when live bijectivity, inverse conversion, or lookup weight forms part of the contract.
- `Unified Primitives`: Keep shared asset taxonomy and ecosystem constants in `template/primitives`; avoid duplicated magic numbers.
- `AssetKind`: Preserve bitmask-based O(1) classification and dedicated staked-local/staked-foreign namespaces.
- `Arithmetic`: Use `Perbill`/`Permill` for ratios and `U256` intermediates where curve arithmetic can overflow native widths.
- `Logical-First Naming`: Name stable abstractions by role before representation; a time-ordered wakeup index need not promise a particular storage structure.
- `Cadence`: Keep block-duration assumptions explicit, benchmarked, and configuration-driven rather than fixing DEOS to one block speed.
- `Protected Complexity`: Preserve complexity earned by real constraints and invariants; remove accidental complexity and speculative indirection.
- `No Premature Optimization`: Prefer contract correctness and honest product flows over speculative loading, bundle, storage, or scheduler indirection.
- `Pre-Fork Storage Lineage`: Before a downstream chain launches, reset fresh-baseline storage versions and remove historical migration ceremony; deployed forks own their migrations.

## 6. TMCTOL Economic Invariants

- `Unidirectional Minting`: TMC issues along a curve and does not expose reserve redemption as a protocol promise.
- `Economic Physics`: Parameters defining launch-time Economic Physics default to immutable unless a stronger constitutional contract explicitly delegates them.
- `Gravity Well`: Treat the emergent liquidity state as a studied standard property, not an unconditional market guarantee.
- `Elasticity Inversion`: Use the term only for the expanding-supply zero-slope threshold where supply growth stops worsening the effective floor.
- `Compression Terminology`: Every compression claim identifies its analysis axis and metric; do not conflate inversion, relative parity, absolute-gap compression, and arbitrage overtake.
- `Economic Claim Honesty`: Never state market immunity or unconditional guarantees beyond shipped runtime behavior and falsifiable evidence.
- `TOL Accounting`: Keep reserve scope, bucket state, supply basis, sellable-pressure assumptions, and governance conditions explicit in floor claims.
- `Bucket Policy`: Treat bucket topology and percentages as TMCTOL/reference-instance policy rather than mandatory DEOS kernel law.
- `Burn Liveness`: Burn effects depend on funded, configured, schedulable execution; do not describe fee capture as automatic supply reduction before the Burn Actor completes it.
- `Liquidity Liveness`: Liquidity effects depend on healthy pools, configured execution plans, bounded slippage, and valid reserve accounting.
- `Simulator Authority`: Use the simulator for economic math and hypotheses, not as a shadow runtime for storage, weights, AAA, governance, XCM, or client parity.
- `Deterministic Simulation`: Use fixed cases or explicit seeded PRNGs in correctness suites and keep wall-clock measurement in benchmark tooling.

## 7. Runtime Subsystem Contracts

- `FRAME`: Use FRAME v2 pallets, typed configuration, and `frame_benchmarking::v2` patterns.
- `Asset Registry`: Persist bidirectional `Location <-> AssetId` mappings; derive IDs only at governance registration and preserve balance identity across location updates.
- `Token Bootstrap`: Asset-registration and curve-creation hooks must remain deterministic and idempotent.
- `Sovereign Liquidity`: Foreign assets enter local `pallet-assets` through XCM reserve-transfer assumptions; DEOS does not delegate its liquidity accounting to foreign chains.
- `Liquid Staking`: Keep one staking pallet, `stXXX` receipts, and native `stNTVE`; do not add a parallel nomination-token tier without evidence.
- `Native Security`: Native collator backing uses explicit locked `NTVE/stNTVE` LP custody rather than liquid receipt ownership.
- `AAA Staking Portability`: Keep `Stake` and `Unstake` tasks generic; runtime adapters decide native, non-native, or local representation behavior.
- `Governance Domains`: Model governance as explicit domain-scoped primary/protection track pairs rather than proposal-id conventions or actor-profile hacks.
- `Governance Shape`: Prefer `GovernanceDomain + CadenceMode + ProposalPayloadKind`; add richer proposal classes only after measured pressure.
- `Urgent Policy`: Fast-track eligibility defaults deny and must be opted in per domain/payload combination.
- `L2 Parameters`: Treat delegated parameter changes as explicit bounded domain-owned surfaces, not permission to call arbitrary admin setters.
- `Safety Bias`: Protection governance may fail closed; `$VETO` is negative constitutional power rather than a second positive-governance path.
- `Governance Reward Memory`: Keep windows, expiry buckets, uniqueness, retention, and proposal maturity bounded; avoid full-account or full-proposal scans.
- `Reward Sparsity`: Preserve sparse, touch-driven, one-epoch-lagged reward snapshots and explicit truncation signals.
- `Reward Sources`: Keep reward distribution separable from origin so externally funded or treasury-budgeted pots remain possible.
- `Unclaimed Rewards`: Treat leftovers as explicit runtime policy rather than accidental residue.
- `Liquidity Slippage`: Derive Liquidity Actor swap tolerance from current reserve depth and clamp it between explicit runtime bounds.
- `Fee Routing`: Preserve the unified author/Fee Sink collection contract and explicit unresolved-author behavior.

## 8. Engineering and Validation

- `Validation Layers`: Mathematical truth lives in the simulator, behavioral truth in pallets/tests, and systemic truth in runtime integration tests/XCM.
- `Validation Scope`: Run the smallest meaningful changed-scope check first and escalate only when the diff crosses boundaries.
- `Stateful Tests`: Use realistic stateful mocks for AMM, TMC, and cross-component mechanism verification.
- `Benchmark Metrics`: Measure both RefTime and ProofSize with explicit bounded components and worst-case setup.
- `Weight Bridge`: Generate pallet weight templates and bind runtime-specific implementations under `template/runtime/src/weights`.
- `Production Weights`: Runtime configs must use real `WeightInfo`; do not ship `()` placeholders.
- `Idle Safety`: Preserve block-weight headroom for `on_idle` work and account for hook pressure in scheduling changes.
- `Rust Imports`: Prefer direct `polkadot_sdk`, `frame_support`, and `sp_runtime` paths over compatibility shims unless a macro/generated boundary requires them.
- `Rust Warnings`: Maintain zero Clippy warnings across workspace/all targets.
- `Workspace Lints`: Keep Substrate cfg allowances and the upstream-aligned lint set honest.
- `WASM Builder`: Keep `substrate-wasm-builder` aligned with the current Polkadot SDK line.
- `Runtime Version`: Keep `system_version >= 3`; bump `spec_version` for runtime-upgrade-visible behavior.
- `Source Headers`: Do not add license or copyright headers to source files.
- `Suppressions`: Avoid broad JS/TS/Svelte lint and type suppressions; narrow and justify unavoidable exceptions.
- `Complexity Feedback`: Treat compilation and integration failures as architectural feedback; simplify abstractions before adding compatibility layers.
- `File Mutation`: Use repository edit/write tools for file changes; use shell commands for inspection, execution, and validation.
- `Long-Running Processes`: Do not start foreground servers or watchers in the primary agent flow unless explicitly requested.

## 9. Documentation Contract

- `Spec Purity`: Specifications define intended source-of-truth contracts only; implementation status, migration notes, and rollout caveats belong in architecture docs.
- `Delivery Sequence`: For non-trivial subsystems, refine specification, implement and validate code, then update architecture documentation from shipped truth.
- `Paired Docs`: Non-trivial pallets should have a specification and a separate architecture map.
- `Doc Filenames`: Use full dotted forms such as `name.specification.en.md`, `name.architecture.en.md`, `name.contract.en.md`, and `name.strategy.en.md`.
- `Markdown Tables`: Use compact delimiter rows such as `|---|---|`, preserving alignment only when meaningful with `|:---|---:|`.
- `Architecture Neutrality`: Architecture docs describe current implementation truth without embedding release-number rhetoric.
- `README Neutrality`: Entrypoint READMEs explain current purpose, setup, navigation, and validation; release history belongs in `CHANGELOG.md`.
- `Canonical Consolidation`: Merge extension specs into stronger canonical contracts when ownership converges; retire old files as redirect stubs when necessary.
- `Economic Claims`: Load-bearing architecture claims require code anchors and falsification tests that would fail if behavior regressed.
- `Read-Model Classification`: New specs and public query surfaces state whether each client-facing datum is canonical-chain or materialized.
- `Rename Gate`: Public domain renames must update runtime, tests, benchmarks, docs, wiki, context, and stale-alias audits in one pass.
- `Release Readiness`: Avoid standalone readiness layers while the runtime remains architecturally fluid; keep rollout notes near owning docs until the launch line stabilizes.

## 10. Frontend Contract

- `Frontend Provenance`: Keep canonical-chain vs materialized truth separate from browser realization such as direct, session-cache, session-derived, and provider.
- `Frontend Ownership`: Promote meaningful domains to top-level `src/lib` slices; avoid generic shared buckets that hide state ownership.
- `Adapter Purity`: Keep transport adapters transport-oriented; domain types and durable UI contracts belong to their owning slices.
- `Execution Feedback`: Keep account log, network feed, transaction progress, and receipts in the dedicated `log` slice.
- `UX Topology`: Store product-significant workspace defaults in named specs/constants with migration matchers; do not reintroduce user-reorderable reserved edge lanes.
- `Visible UI`: Use semantic markup, accessible interaction states, responsive layouts, and the established UI system for all user-facing changes.
- `Data Density`: Optimize reference-client surfaces for scanning, provenance, transaction feedback, and bounded truth rather than decorative dashboard density.
- `Indexer Honesty`: Do not present session-derived or cached browser state as direct runtime projection or archive truth.
- `Performance`: Measure before adding lazy-loading, bundle shaping, caching indirection, or speculative client optimization.

## 11. Scripts, Skills, and Wiki

- `Script Classes`: Numbered scripts perform atomic leaf operations and must not orchestrate other numbered scripts; named scripts own orchestration or composite administration.
- `Script Skeleton`: Named/admin entrypoints follow `usage -> parse_args -> check_prerequisites/plan -> main` on `_common.sh`.
- `Script Help`: Every script entrypoint exposes `--help` and accurately declares environment and behavior.
- `Audit Ownership`: Project-specific audit leaves live in the repo-local `alignment` skill; root scripts may orchestrate but should not duplicate audit knowledge.
- `Skill Portability`: Repo-local skills must remain independently portable and must not call sibling skill internals directly.
- `Diff-Aware Gates`: Audits default to changed scope and reserve full-tree or network-backed checks for explicit release/all modes.
- `Durable Ledgers`: Record reusable hallucinations, ambiguities, dead ends, and boundary drifts only; bare tool failures remain transient output.
- `Wiki Role`: `/wiki` is a concise, provenance-aware learning lens over current project truth, not a release-note mirror or docs dump.
- `Wiki Locales`: Human pages use explicit locale suffixes and mirrored page IDs/topology; shared metadata represents localized fields.
- `Wiki Navigation`: New pages need provenance, related links, locale mirrors, and graph/index reachability; merge weak leaflets into stronger owner pages.
- `Wiki Trust`: Treat repo-local wiki Markdown as reviewed content and enforce the trust boundary with the wiki validation skill.
- `Wiki Growth`: Large wiki expansion requires consolidation; confidence and graph metadata must drive merge/remove work rather than decorative bookkeeping.

## 12. Network, XCM, and Upstream Integration

- `Trusted-Collator Phase`: Until a parachain-consumable per-block relay beacon exists, the active collator set remains permissioned and previous-block-hash sampling remains the accepted local fallback.
- `No Local VRF Revival`: Do not resurrect the retired local commit/reveal randomness subsystem without new evidence that the relay-beacon path cannot satisfy the contract.
- `Relay Beacon`: Adopt future randomness only against a real parachain-consumable per-block protocol beacon.
- `Beacon Ingress`: Prefer a weight-accounted consensus hook that materializes one compact per-block snapshot for hot-path consumers.
- `XCM Safety`: Keep asset conversion, reserve checks, barriers, and location mappings covered by runtime integration tests.
- `Runtime Upgrade`: Preserve authorized-upgrade, pending-code, version, and rollback assumptions in runtime tests and operator tooling.
- `V3 Scheduling`: Keep disabled until operator prerequisites, weight margin, hooks, message queues, and XCM budgets have an explicit readiness profile.
- `Block Rewards`: Do not imply an issuance source exists; activate routing only after the reference economy selects a concrete source and amount policy.
- `Indexer Boundary`: Never solve archive/search needs by growing unbounded consensus state.
- `Deployment Boundary`: Omni Node owns node-level discovery, tracing, collator identity, and execution-block integration.
- `Upstream Source`: Use `paritytech/polkadot-sdk` as the authoritative repository for current SDK evidence, not archived Substrate history.
- `Upstream Classification`: Classify upstream changes as SDK-standard, ecosystem-pattern, or business-logic before adopting them.

## 13. Task Lifecycle

- `Classify`: Identify whether the task primarily touches docs, template, web-client, scripts, simulator, wiki, or context.
- `Locate Truth`: Read the owning specification, architecture doc, README, code, and tests before editing.
- `Check Boundary`: Identify framework versus instance policy, trigger, storage/weight bound, truth surface, and rejected shortcut for non-trivial changes.
- `Inspect First`: Read current implementation, call sites, tests, and git diff before mutation.
- `Keep Scope`: Make targeted changes and exclude unrelated cleanup unless the task explicitly includes consolidation.
- `Sync Backlog`: Close, narrow, split, retarget, or gate the canonical open-work item as reality changes.
- `Validate Locally`: Run focused tests, checks, formatting, and audits for the touched surface.
- `Validate Rust`: Run targeted Cargo checks/tests and workspace Clippy with `-D warnings` before calling Rust work complete.
- `Validate Math`: Run `node ./simulator/tests.js` whenever tokenomics, formulas, thresholds, or invariants change.
- `Validate Client`: Run relevant web-client checks/builds when UI, adapters, read models, or client contracts change.
- `Validate Scripts`: Run shell syntax, `--help`, and bounded smoke checks for touched entrypoints.
- `Validate Wiki`: Run wiki trust/sync/consolidation checks for touched wiki surfaces.
- `Sync Docs`: Update specifications only when contracts change and architecture docs only when shipped implementation truth changes.
- `Sync Context`: Update `AGENTS.md` only for durable patterns, `BACKLOG.md` only for open work, and `CHANGELOG.md` only for completed outcomes.
- `Completion Gate`: After repository changes and knowledge sync, run `./.agents/skills/alignment/scripts/completion-gate.sh`; a failing gate means not done.
- `Garbage Collection`: Consolidate stale, duplicated, resolved, or over-detailed context whenever growth obscures the durable contract.
- `External Gates`: Do not publish, deploy, sign, submit, mutate accounts, or cross destructive/approval boundaries without explicit user authorization.
- `Done`: Report changed paths, validation evidence, remaining gates, and exact unblockers concisely.
