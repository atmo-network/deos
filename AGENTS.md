# Project Context & Meta-Protocol

> Capturing layered abstractions, implementation insights, and evolutionary optimization
>
> Canonical project-memory ABC-split:
>
> - `AGENTS.md`: Durable protocol and context layer.
> - `BACKLOG.md`: Open backlog / active roadmap.
> - `CHANGELOG.md`: Completed delivery history.

## 0. Meta-Protocol Principles

> This section governs how the context system itself evolves. It stays separate so meta-rules about memory, validation, and protocol upkeep do not get mixed with project-specific conventions.

This document is a living protocol for continuous, intelligent self-improvement and optimal knowledge evolution. Its core principles govern how this context system manages itself and are universally applicable to any project:

- `Mandatory Self-Improvement`: Every task ends with context updates, creating self-reinforcing knowledge accumulation. Protocols enforce their own effectiveness through positive feedback loops.
- `Protocol Evolution`: Rules improve when better workflows emerge. Feedback loops create emergent intelligence; context embodies the design principles it mandates.
- `Test-Driven Evolution`: Comprehensive validation enables clean evolution through systematic patterns and self-correcting feedback loops.
- `Context Optimization`: Systematically prevent infinite growth through proactive cleanup. Transform tactical experiences into strategic wisdom. Evolution includes both addition AND consolidation.
- `Delivery History Rotation`: Fixed entry limits through intelligent consolidation. Newer entries provide tactical details, older entries preserve strategic patterns inside `CHANGELOG.md` while durable rules stay out of rolling delivery history.
- `Non-Duplication Enforcement`: Information exists in only one authoritative section. Create hierarchical navigation instead of duplicating content.
- `Decreasing Abstraction Structure`: Organize from general to specific, mirroring optimal cognitive processing patterns.
- `Validation Infrastructure`: Automated validation of structure, cross-references, and information architecture. Pre-task preparation and completion protocols ensure quality gates are never skipped.
- `Living Hierarchical Documentation`: Documentation mirrors system architecture and evolves with implementation. Design intent stays synchronized with actual behavior.
- `Boundary Clarity`: Meta-principles govern context evolution; project conventions govern domain. Protocol ≠ project; distinct evolutionary pathways with cognitive firewalls preventing contamination.
- `Emergent Elegance`: Multiple iterations reveal constraints guiding toward patterns; complexity reduction emerges from understanding, not premature simplification.
- `Progressive Enhancement`: At 95%+ quality, targeted additions beat wholesale replacement; incremental improvement surpasses architectural revolution.
- `Emergent Property Validation`: Component interactions require explicit testing/documentation. Test organization mirrors architecture; structural symmetry creates multiplicative quality.
- `Morphological-First Decision Making`: Analyze solution space before implementing—map extremes, identify trade-offs. Dual-phase analysis reveals dimensional intersections invisible to single-phase.
- `Specification Maturity`: Documentation evolves exploratory → consolidated. Each abstraction layer enables deeper insight through progressive comprehension frameworks.
- `Constraint-Driven Evolution`: Evolution follows constraint discovery; constraints are catalysts not limitations. Evolved architectures solve discovered constraints; defend against simplification discarding hard-won insights.
- `Cognitive Infrastructure`: Docs, scripts, skills, and validation gates are part of the system architecture because they determine which abstractions replicate correctly through humans and agents. Treat them as first-class coordination substrate, not auxiliary tooling.

---

## 1. Concept

> This section answers what the project is in conceptual terms. It stays separate from topology and implementation guidance so identity, scope, and product boundary are fixed before directory-level or operational detail appears.

The project is a `Specification & Reference Framework`.

`The Operating System (DEOS)`: A Deterministic Economic Operating System providing the runtime kernel, automated actor scheduling (`pallet-aaa`), routing, and consensus infrastructure. It acts as the immutable physics engine for any economic model deployed on it.

`The Standard (TMCTOL)`: A rigorous economic specification for a Token Minting Curve combined with Treasury-Owned Liquidity. It runs on top of DEOS and defines the mathematical laws of a self-sustaining economy.

`Goal`: Provide a "Foundation in a Box" (DEOS) for launching independent ecosystems with mathematically guaranteed liquidity infrastructure (TMCTOL), achieving production-ready reliability through test-driven evolution with 100% validation.

`Product Boundary`: This repository is the framework/substrate layer, not the finished user-facing ecosystem product. This does not mean ecosystem work is deferred: the framework is being carved out from the intended first production ecosystem so reusable economic/runtime/client/validation components stay clean before concrete names, dApps, launch configs, and product loops crystallize downstream.

`First-Ecosystem Relationship`: DEOS is both a reusable framework and the protocol substrate of its first intended production ecosystem. Treat generic mechanics, invariants, adapters, validation, and reference-client honesty as framework-owned; treat ecosystem identity, concrete dApps, launch narrative, user loops, and business policy as downstream product-owned unless they reveal a reusable framework contract.

`Adoption Model`: DEOS is meant to be forked by partner teams launching their own ecosystems on top of the shared framework. Those downstream ecosystems, including the first production instantiation, may contribute stabilizations, client/indexer/read-model improvements, and other framework-hardening work back into DEOS/TMCTOL, while business-product logic stays downstream rather than accreting inside the core repo.

`Release-Line Reset`: After the move into the standalone DEOS repository, the canonical framework release line restarts at `0.0.0`. `CHANGELOG.md` SHOULD therefore summarize achieved epics and the present shipped baseline of this repo line rather than preserving every intermediate implementation step or inherited pre-reset chronology.

## 2. Identity & Naming Contract

> This section freezes project vocabulary and brand boundaries. It exists separately because naming drift creates system-wide confusion across docs, runtime surfaces, client copy, and backlog state.

- `Terminology Lockstep`: Stable specs, architecture docs, and runtime/API surface MUST use the same canonical term for the same domain atom. Do not keep prose aliases once a runtime term becomes canonical.
- `Framework-vs-Standard Naming`: Use `DEOS` for the repository/framework/reference-stack/runtime-substrate identity. Use `TMCTOL` only for the concrete tokenomic standard (spec, math, current flagship economic configuration) that runs on DEOS. Do not resolve framework-brand drift with blind search/replace across standard-specific contracts, and do not let standard-specific docs imply that TMCTOL is the only possible DEOS configuration.
- `Governance Layer Naming`: Public docs MAY refer to the governance model as `DEOS Governance` when emphasizing that the shipped bounded dual-track governance contract is DEOS-specific and distinct from Polkadot `OpenGov`. Keep runtime implementation names generic (`pallet-governance`, `Governance` UI section/widget) unless a separate rename epic explicitly targets those technical surfaces.
- `DEOS Acronym Semantics`: `Deterministic` names explicit bounded protocol-managed economic reactions for the same on-chain state, typed payloads, and token flows; it does not mean the whole market becomes predictable. `Economic` names the managed domain of capital, liquidity, treasury, staking, and governance-conditioned flows. `Operating System` should be read as a domain-specific execution substrate / system-services layer for protocol economies, not as a claim that DEOS is a general-purpose OS.
- `Artifact-Name Tail Policy`: Completed in `0.1.2`. The rename epic migrated runtime and pallet crate names (`tmctol-runtime` → `deos-runtime`, `pallet-tmctol-*` → `pallet-deos-*`), runtime `spec_name`/`impl_name`, script identifiers, web-client adapter types, local-storage keys, and PAPI generated descriptors (`tmctol` → `deos`). The `@polkadot-api/descriptors` package namespace (`deos`) now matches the current runtime contract. Standard-specific identifiers (TMCTOL spec filenames, `tmctol_integration_tests` module, axial-router TMCTOL integration test functions) remain as conscious legacy names because they name the standard under test, not the framework. Do not reintroduce `tmctol-*` identifiers in new framework code; if legacy references surface in stale docs or downstream forks, treat them as rename-debt to be cleaned in the same pass.
- `AAA Abstraction Lift`: When automation previously described as a special-case `manager` is now realized as one or more System AAA actors, rewrite docs/context upward to the AAA role / execution-plan-family abstraction. Keep legacy manager names only as historical orientation aids, not as the primary mental model.
- `Compression Metric Discipline`: Any spec, simulator test, or architecture note that says `compression` MUST name both the analysis axis (`burn-time` vs `expanding-supply`) and the metric (`C/F` relative spread vs `C - F` absolute gap`) unless local context already fixes both. Do not collapse `Elasticity Inversion`, `Relative Compression Parity`, `Absolute-Gap Compression`, and `Arbitrage Reversal / Overtake` into one point.
- `Token Ticker Notation`: In specs/architecture docs, prefix concrete token symbols with `$` when they mean actual assets or token-scoped domains/examples (`$NTVE`, `$VETO`, `$BLDR`). Keep bare uppercase labels only when they are vote options, track actions, or other non-asset semantics (`Veto`, `Pass`, `Nay`)

---

## 3. Project Topology

> This section maps the concrete repository surfaces and their maintenance order. It stays separate from concept so the authoritative directory map can evolve operationally without redefining project identity.

- `Current Support Priority`: Day-to-day maintenance and framework-stabilization work currently center on `/docs` → `/template` → `/web-client` → `/scripts`. `/simulator` remains authoritative when tokenomics, formulas, thresholds, or invariants change, but it is no longer treated as a co-equal default entrypoint for unrelated work.
- `/docs/`: The Knowledge Base and current conceptual control plane. Architecture guides, specs, mathematics, and the primary explanation surface for the framework.
- `/template/`: `The Reference Implementation` and main runtime kernel.
  - `/template/runtime/`: The Parachain assembly.
    - `/weights/`: The Bridge between generated benchmarks and runtime configuration.
  - `/template/pallets/`: Runtime pallets (`aaa`, `asset-registry`, `axial-router`, `governance`, `staking`, `tmc`).
  - `/template/primitives/`: Unified types (`assets.rs`, `ecosystem.rs`).
  - `/template/research/`: Runtime-adjacent local experiments and evidence harnesses that are not part of the production runtime contract.
  - `/template/README.md`: Local orientation entrypoint for the Rust workspace. The repository snapshot is runtime-centric and expects Omni Node deployment rather than an in-repo custom node crate.
- `/web-client/`: Repository-local SvelteKit frontend workspace for the browser-facing DEOS reference client and visualization flows for the current TMCTOL standard. Its `src/lib/` tree keeps first-class product subsystems and shared infrastructure rather than hiding heterogeneous state under a generic `entities/` umbrella.
- `/scripts/`: Automation layer. Numbered scripts are atomic leaf operations; named scripts are orchestrators/admin utilities. Named/admin entrypoints should follow a shared shell skeleton (`usage -> parse_args -> check_prerequisites/plan -> main`) on top of `_common.sh`. All script entrypoints should expose `--help` and keep declared environment/behavior contracts honest. `scripts/README.md` is the canonical map. Do not park project-specific validation/audit knowledge here by default; `/scripts` may orchestrate gates, but durable audit leaves belong in the repo-local skill layer.
- `/simulator/`: Historical proving ground for the composite tokenomic primitive and still-authoritative mathematical reference surface. Consult it when changing tokenomics, formulas, thresholds, or invariants; do not treat it as the default entrypoint for routine runtime, frontend, or tooling work.
- `/.agents/skills/`: Repository-local agent skill layer. This is cognitive infrastructure, not convenience tooling: it shapes how contributors and agents perceive the system. Skill-local automation scripts are first-class entrypoints too: they SHOULD mirror the `/scripts` shell skeleton, expose `--help`, and reuse `/scripts/_common.sh` through a local bridge that restores repo-root paths plus skill-local `SCRIPT_DIR` after sourcing the root helper. The `alignment` skill owns project-specific audit/validation leaves; when a new drift class needs validation, evolve that skill and have operational scripts call it instead of accumulating validation logic in `/scripts`. Generic validators that project package scripts depend on, such as `domain-dag`, should live repo-locally so validation does not depend on one operator's global agent installation. Project skills MUST stay independently portable: a skill may expose its own scripts and contracts, but it SHOULD NOT call sibling skill internals directly; cross-skill orchestration belongs in `AGENTS.md`, package scripts, root scripts, or other project-level entrypoints.

---

## 4. Core Entities

> This section names the durable domain atoms before discussing implementation choices. It exists separately so ontology stays distinct from architecture, policy, and workflow rules.

### 4.1 Abstract Economic Actors (The Specification)

_Defined in `/simulator`, agnostic of blockchain framework._

- `TMC (Curve)`: The unidirectional emission engine ($P = P_0 + slope \cdot s$). Controls token emission through a deterministic economic machine.
- `TOL (Liquidity)`: The multi-bucket accumulator ensuring a rising price floor.
- `Gravity Well`: The emergent state where TOL accumulation stabilizes volatility (~15% MarketCap).
- `Elasticity Inversion`: The zero-slope threshold where supply expansion stops worsening the effective floor. Distinct from relative-compression parity, absolute-gap compression, and arbitrage reversal / overtake.

### 4.2 Concrete Implementation Entities (The Framework)

_Implemented in `/template/pallets`, bound by Substrate logic._

- `Parachain Runtime`: The aggregator of pallets, utilizing `Runtime-as-Config` adapters and modern FRAME patterns.
- `Axial Router`: The execution gateway enforcing fee burning and optimal routing. It acts as an economic coordination actor determining "Efficiency Score" to arbitrate between Market Liquidity (XYK) and Protocol Liquidity (TMC).
- `System AAA Actors`: Runtime-owned AAA instances executing the protocol's autonomous economic flows. The stable contract is the actor role / execution-plan family, not one legacy subsystem name.
  - `Actor Role Families`: The current reference line uses multiple System AAA actor families, including burning / buyback actors, liquidity-provisioning actors, splitter / distribution actors, and bucket / treasury actors.
  - `Liquidity-Provisioning Actors`: What older context called `Zap Manager` is one actor family implemented as multiple System AAA instances for specific pools or lanes (for example Native/foreign provisioning and `NTVE-BLDR` provisioning).
    - `Opportunistic Liquidity Provisioning`: A policy trait of some liquidity-provisioning actors: add liquidity at the current pool ratio without mandatory pre-balancing.
    - `Patriotic Accumulation`: A policy trait of some Native-sovereign liquidity-provisioning actors: prefer retaining Native surplus over selling it for Foreign assets.
  - `Shared AAA Execution Semantics`: Balance-driven intake and retry / cooldown behavior are reusable execution semantics that may appear across multiple actor families, not separate actor kinds.
  - `"Omnivorous Intake"`: Balance-driven ingress semantics that react to assets arriving at the actor account rather than relying on one bespoke extrinsic path.
  - `Resilience`: Retry / cooldown behavior protecting actors during oracle or market unavailability.
- `Asset Conversion`: Uniswap V2-like DEX for automated market making, utilizing `AssetKind` with Bitmask-based classification.
- `Omni Node`: Primary deployment architecture eliminating node boilerplate.

---

## 5. Architectural Decisions

> This section records the durable design choices and launch-line constraints that shape the shipped reference line. It stays separate from entities so rationale, trade-offs, and protected architecture do not get buried inside the ontology list.

### 5.1 Runtime & Platform Architecture

- `Omni Node`: Deployment architecture eliminating node boilerplate. Node-level features (DHT bootnode discovery, `ParachainTracingExecuteBlock`, `collator_peer_id`) are handled by the Omni Node binary — no custom `node/` directory.
- `Frame V2`: Strictly typed `#[frame::pallet]`, `frame_benchmarking::v2`.
- `Token-Driven Coordination`: State transitions are triggered by asset movement (Substrate hooks), not Signed Extrinsics. This ensures origin-agnostic security.
- `Runtime-as-Config`: Business logic (Pallets) is generic. Configuration (Runtime) injects specific behavior via runtime traits / adapter implementations following SDK patterns for clean separation.
- `On-Chain vs Indexed Read Model`: The default product mental model MUST classify every public datum as either `(a)` bounded authoritative on-chain state/projection intended for raw client or light-client consumption, or `(b)` externally indexed/materialized data intended for archive/search/analytics. Do not make indexers a silent dependency for canonical user flows when a bounded on-chain projection is the real contract, and do not force unbounded history or dashboard aggregations into consensus state just to avoid off-chain tooling.
- `Unified Primitives`: `primitives/src/ecosystem.rs` is the single source of truth for constants, avoiding magic numbers. `AssetKind` uses bitmask classification for O(1) type inspection.
- `Type System Discipline`: Enforced strict `sp_arithmetic::Perbill` for ecosystem parameters. Adopted `sp_core::U256` for bonding curve calculations to prevent intermediate overflows.
- `Logical-First Naming`: Stable docs/specs SHOULD name scheduler/storage abstractions by role first and reference-runtime representation second. Example: `time-ordered wakeup index` is the contract; block-bucketed map, tree-backed index, or other representation is an implementation choice until benchmark evidence promotes it to stable architecture.
- `assets-common Rejected`: The `assets-common` crate (Location-as-AssetId, TrustBacked, ERC20/pallet-revive) is designed for Asset Hub pattern. Incompatible with TMCTOL's `u32` bitmask + `pallet-asset-registry` architecture. `pallet-asset-registry` already provides `MaybeEquivalence<Location, AssetId>`.
- `Reverse-Index Preference`: When a subsystem needs bounded reverse lookup, bijectivity enforcement, or inverse conversion as part of the live contract, persist the reverse mapping as first-class state instead of recomputing the inverse relation through scans, ad hoc indexes, or weight-time storage reads.
- `Stateful Testing`: Mocks use `RefCell<BTreeMap>` for realistic AMM simulation and TMC behavior, enabling "Mechanism Verification" over simple policy checks.

### 5.2 Constitutional & Product-Surface Architecture

- `Liquid Staking Architecture`: Staking shares are evolving towards tokenized yield-bearing assets. Native/local receipts use `TYPE_STAKED = 0x5000_0000`, foreign staking receipts use the dedicated `TYPE_STAKED_FOREIGN = 0x6000_0000` namespace, and `XXX / stXXX` pairs remain technically composable even though such pools do not yet have a protocol-assigned role. This keeps receipt derivation collision-free under the current 32-bit `[type:4 | index:28]` contract without shrinking local or foreign index space. The next reward evolution keeps `pallet-staking` and `pallet-governance` separate: backing inflow raises share price through the pool, governance-conditioned reward inflow stays in a second sovereign channel and compounds into fresh same-asset `stXXX`, while the exact governance-weight formula stays runtime-configured rather than hardcoded into the staking pallet.
- `Unified Native Staking`: `pallet-staking` remains the single staking pallet. Non-native assets stay generic and economic-only. Native `$NTVE` mints liquid `stNTVE` without choosing a collator; nomination/security backing is explicit locked `NTVE/stNTVE` LP custody. `stNTVE` stays the only native staking receipt; no separate `nomXXX` token tier or standalone `pallet-delegation` is introduced unless later evidence proves it necessary. Higher-level automation SHOULD keep staking portable: AAA exposes generic `Stake { asset, amount }` / `Unstake { asset, shares }`, while runtime adapters decide whether a runtime-defined asset representation maps to native liquid staking, non-native staking, or another local staking primitive. Do not encode DEOS-specific collator selection, LP custody, or receipt naming in the portable AAA task enum.
- `Governance Domain Hierarchy`: Model governance as explicit domain-scoped `primary track + protection track` pairs. Current launch line: protocol / network governance = `Native + $VETO`; canonical tactical governance (`$BLDR`) = `$BLDR + Native`. Do not encode this constitutional topology in proposal-id prefixes or actor-profile hacks.
- `Governance Decomposition Discipline`: Prefer the smallest explicit governance shape that still keeps execution authority honest. For the current line, favor `GovernanceDomain + CadenceMode + ProposalPayloadKind` over a separate `ProposalClass` ontology unless measured implementation pressure later proves that a richer class layer is truly necessary.
- `Urgent Fast-Track Opt-In Discipline`: Treat urgent fast-track as an explicit runtime policy per `(domain, payload_kind)` combination, not as a default capability of all protected proposals. On the current line, urgent eligibility SHOULD default deny until a constitutional/runtime contract explicitly opts specific combinations in.
- `L2 Parameter Surface Discipline`: Treat `L2ParameterChange` as a domain-owned/delegated control-surface contract, not as permission to reuse any convenient admin setter already present in the runtime. Mutable delegated parameters SHOULD carry explicit bounds that preserve the economic assumptions they affect. On the current launch line, canonical `$BLDR` governance stays invoice-centric; if a mechanism still belongs to System AAA / L1, tactical governance SHOULD use `L2SignalToL1` until that control surface is explicitly delegated into the L2 domain.
- `Launch Physics Immutability`: Parameters that define launch-time economic physics SHOULD default to immutable post-launch unless a stronger constitutional contract explicitly says otherwise. On the current line, TMC curve launch parameters are treated as fixed after `create_curve`; do not reintroduce post-launch slope mutation casually through pallet admin surfaces, governance executors, or product wording.
- `Safety-over-Liveness Protection Bias`: `$VETO` is a negative constitutional power surface, not a second ordinary-governance token. It intentionally inverts the classic technical-committee pattern: proposals may open publicly first, then protection acts on-chain after submission. If `$VETO` concentration goes bad, the intended worst case is fail-closed L1 paralysis / slower evolution that hardens physics against hasty rewrite, not an alternate positive-governance path that can actively seize L0/L1 economics.
- `Bootstrap-to-Protection Transfer`: Launch-time `Root` / `Sudo` bootstrap authority is normal for parachain rollout. Judge decentralization by the explicit handoff path from bootstrap superuser into public primary/protection governance, not by genesis absence of a superuser. Early team concentration of `$VETO` is acceptable only as staged protection-power transfer and SHOULD later diffuse toward aligned builders, stakers, and community defenders rather than remain a permanent founding-team committee in token form.

### 5.3 Current Launch-Line Constraints

- `Trusted-Collator Simplification Phase`: Until a relay-beacon randomness path is production-ready, the active collator set stays permissioned (`Invulnerables` / trusted collators), native binding targets stay restricted to that set, and local probabilistic consumers may fall back to previous-block-hash sampling rather than resurrecting a separate local entropy subsystem. Existing epoch-scale relay randomness items are not accepted as the replacement contract; only a future parachain-consumable per-block protocol beacon qualifies.

### 5.4 Economic Architecture

- `Unidirectional Minting`: A mathematical "Ratchet" preventing reserve extraction.
- `Bidirectional Compression`: Burn-time claim: burning lowers the ceiling while TOL raises or stabilizes the floor, so the price corridor compresses directly. Distinct from expanding-supply threshold analysis, where post-inversion floor recovery does not by itself imply compression.
- `Multi-Bucket Strategy`: 4-bucket system (50/16/16/16) ensures ~100% capital utilization while preserving governance segmentation.
- `Canonical Bucket → Treasury Pairing`: Each TOL Bucket (B, C, D) has a dedicated paired Treasury AAA (`aaa_id=7..9`) that accumulates its unwound LP, preserving governance segmentation across downstream economic functions.
- `Mechanism-Over-Policy`: The Router acts as a pure mechanism (XYK vs TMC) rather than a policy engine, reducing attack surface.

---

## 6. Engineering Conventions

> This section governs how code-facing work should be validated, benchmarked, and evolved. It exists separately so engineering discipline stays tighter than broader documentation and operational process rules.

### 6.1 The Three-Layer Validation

Truth for protocol and economic changes is established in three layers. Use every layer when a change crosses the economic contract; otherwise validate the smallest affected surface first and escalate only when the diff crosses layer boundaries.

- `Simulation (Mathematical Truth)`: `/simulator`, JavaScript/BigInt, `PPB`; verifies formulas before runtime logic is changed.
- `Simulator Determinism`: Correctness tests in `/simulator` SHOULD use fixed cases or explicit seeded PRNGs rather than host-time or ambient random sources. Performance timing belongs in dedicated benchmark tooling, not in the deterministic correctness suite.
- `Implementation (Behavioral Truth)`: `/template/pallets`, Rust, `Perbill`, unit tests, benchmarks; verifies runtime code matches the math and block-weight constraints.
- `Integration (Systemic Truth)`: `/template/runtime`, integration tests, XCM; verifies components coordinate correctly.
- `Operational Priority`: Routine support work still biases toward `/docs`, `/template`, `/web-client`, and `/scripts` in that order; `/simulator` becomes mandatory when tokenomics or invariant math moves.

### 6.2 Benchmarking Standard

- `Syntax`: `frame_benchmarking::v2::*`.
- `Metrics`: Mandatory measurement of `RefTime` (Computation) AND `ProofSize` (Storage Access).
- `Complexity`: Explicit `Linear<Min, Max>` components.
- `Hygiene`: No assumptions. Mock the worst-case state (full storage) in `SETUP` using `whitelisted_caller()`.
- `Stateful Benchmarking`: Use `BenchmarkHelper` traits to bridge mock runtimes with benchmarking requirements.

### 6.3 Rust / Runtime Coding Standards

- `Zero Warnings`: Maintain zero clippy warnings. Resolve redundant pattern matching, collapsible ifs, useless conversions.
- `Clean Imports`: Use unified `polkadot_sdk::*` imports over fragmented crate-specific imports.
- `No Compatibility-Shim Imports By Default`: In pallet/runtime source, prefer direct `polkadot_sdk`, `frame_support`, and `sp_runtime` imports/trait names over `frame::deps::*` compatibility shims unless a macro boundary or generated surface specifically requires the shim path.
- `No License Headers`: Do not include license headers or copyright notices at the beginning of source files.
- `Broad Suppression Ban`: Avoid broad lint/type suppressions (`@ts-ignore`, `@ts-expect-error`, `eslint-disable`, `as any`, `@ts-nocheck`) in active JS/TS/Svelte and helper-script surfaces. If a suppression becomes unavoidable, make it narrow, justified, and update the suppression audit intentionally rather than hiding it inline.
- `Complexity Resolution`: When facing integration challenges, simplify abstractions progressively. Substrate compilation failures are architectural feedback.
- `Antifragile Simplicity`: Default to the simplest deterministic rule that preserves invariants. Add complexity only when a concrete failure mode or constraint proves it necessary.
- `Transactional Mutation Discipline`: Any extrinsic or runtime entrypoint that can fail after mutating multiple storage locations SHOULD either pre-validate all fallible conditions first or use transactional semantics so capacity / late-guard failures cannot strand partial state.
- `Workspace Lint Hygiene`: `Cargo.toml` must declare `unexpected_cfgs` with Substrate-specific cfg values (`substrate_runtime`). Clippy lints must track upstream parachain template — currently 25 rules.
- `SDK Version Tracking`: `substrate-wasm-builder` version must match the polkadot-sdk umbrella (`2603.0.0 -> 32.0.0` on the current line). Current operator binary tag is `polkadot-stable2603-3` (node v1.22.3); the 2603 patch release is represented by targeted `Cargo.lock` patch updates when upstream crate patch versions move rather than a new umbrella crate version. `system_version` in `RuntimeVersion` must be `3` or higher on SDK 2603 to activate the staged `:pending_code -> :code` runtime-upgrade path from RFC-123.

### 6.4 Evolution Principles

- `Forkability`: Changes in `/template` must maintain generic utility. Do not hardcode ecosystem-specific logic into the generic framework components.
- `Pre-Fork Storage-Lineage Discipline`: This repository is a forkable framework with no implicit deployed-chain lineage of its own. Until a downstream fork launches a concrete chain, treat pallet storage versions as fresh-baseline schema markers and delete/reset historical `OnRuntimeUpgrade` migration ceremony instead of preserving in-repo upgrade archaeology. Once a forked chain exists, storage migrations become fork-owned delivery work and must then be handled explicitly.
- `Emergent Complexity`: Features like "Gravity Well" are `evolved complexity`. They are protected. Complicated spaghetti code is `accidental complexity`. It is destroyed.
- `Evidence-First Vocabulary`: Do not let architectural vocabulary outrun measured necessity. If a structure (`tree`, `heap`, specialized index) is still exploratory, keep it as roadmap/architecture discussion until insert/extract/proof-size evidence justifies making it part of the stable contract.
- `Premature Optimization Discipline`: While the project is still architecturally raw, prefer business-facing correctness, product flows, and contract honesty over speculative lazy-loading, bundle-shaping, or other micro-optimizations. Do not add performance indirection unless tests, validation tooling, profiling, or real user/operator pain identify a concrete hotspot. Measure first, optimize second, and revert complexity that fails to produce meaningful wins.
- `Framework Maturity Bias`: Once the core DEOS/TMCTOL framework contract is found, default the next evolution steps toward stabilization, crystallization, forkability, reference-client quality, and honest read-model/indexer support rather than speculative feature growth for its own sake.

---

## 7. Operational Conventions

> This section covers cross-cutting documentation, frontend provenance, and autonomous coordination rules. It stays separate from engineering conventions because these disciplines shape project truth and operator behavior beyond code style alone.

### 7.1 Documentation, Read-Model & Contract Evolution

- `Canonical Spec Consolidation`: If a standalone subsystem spec becomes a pure extension of a stronger canonical contract, merge the normative content into the canonical spec and retire the old file as a redirect stub rather than maintaining parallel contracts
- `Read-Model Classification`: New subsystem specs, runtime query surfaces, and product-facing data contracts MUST explicitly classify client-facing data as either bounded authoritative on-chain projections or externally indexed/materialized views. Do not leave canonical UX silently dependent on indexers, and do not move unbounded archive/dashboard workloads into consensus state just to avoid off-chain tooling.
- `Specification as Broad Contract`: A specification is the broad contract surface for a subsystem: it defines the concept, rationale, constraints, base model, and any explicit extension-of-base structure needed to understand what is being built. It is not a thin API checklist or a post-hoc implementation note.
- `Pallet Mirror Docs`: First-class runtime pallets SHOULD keep two documentation layers once their behavior is non-trivial: a contract/spec layer for the intended model and a separate architecture doc for the currently shipped implementation, runtime bindings, storage topology, migration state, and operational watchpoints.
- `Doc Filename Contract`: First-class paired subsystem docs SHOULD use the full-form dotted filename contract `name.specification.<lang>.md` for contract/spec layers and `name.architecture.<lang>.md` for shipped implementation maps. Other typed doc classes SHOULD follow the same dotted pattern (`name.strategy.<lang>.md`, `name.contract.<lang>.md`, `name.insights.<lang>.md`) when a stable type distinction exists. Use canonical spelling `architecture`, not `archicture`, and do not regress back to short-form `*.spec.*` / `*.arch.*` or older hyphenated filename patterns once a document has moved to the dotted contract.
- `Generated Wiki Locale Contract`: Human-facing files under `/wiki` SHOULD use explicit locale suffixes (`index.<locale>.md` and taxonomy directories directly under `/wiki` such as `overview/<slug>.<locale>.md` or `concepts/<slug>.<locale>.md`) with `en` as the default compatibility locale on the current line. Avoid an extra `/wiki/pages/` wrapper unless a future renderer contract proves it necessary. Do not maintain separate `wiki/log.<locale>.md` files by default; completed wiki delivery history belongs in `CHANGELOG.md`. Non-default locale pages SHOULD mirror the same page ids and relative path topology as the default locale, while `/wiki/_meta/*.json` SHOULD stay shared and express localization through language-keyed field objects plus an explicit locale map such as `_meta/locales.json` when frontend/agent consumers need stable discovery. Russian wiki prose SHOULD read as natural Russian: keep English only for canonical protocol/module/token/API names, code identifiers, explicit UI labels, or terms intentionally stabilized in the project vocabulary; otherwise adapt generic words such as workflow, surface, boundary, focus, status, baseline, tooling, gate, rendering, and provenance into Russian phrasing instead of leaving unnecessary Anglicisms.
- `Generated Wiki Semantic Lens`: `/wiki` is a living, semi-automatically maintained hyperlinked learning lens over the current project implementation and state, not a release-note mirror or dumping ground for current-version status. It serves humans and agents: pages, metadata, graph links, confidence, and consolidation ledgers should make project truth easy to explore without forcing full `/docs` reads for every question. Token savings are a useful side effect, not the goal; optimize for truthful, navigable, provenance-aware knowledge. Keep it concise, atomized, low-repetition, and synchronized after significant runtime, client, docs, or tooling changes; route package/runtime versions to package metadata, `Cargo.toml`, commits, and `CHANGELOG.md`. Large wiki growth requires an explicit refactor/compression slice rather than incremental water accumulation.
- `Wiki Leaflet Discipline`: New wiki pages need a clear navigation role, source provenance, related links, locale mirrors, and graph/index reachability. Prefer merging short audience/checklist/route leaflets into stronger owner pages unless a standalone page improves onboarding, frontend navigation, or agent map traversal. Low `confidence` is not decorative metadata: treat it as an improve / merge / remove signal before adding more pages in the same neighborhood, and use graph-connected low-confidence clusters to find weak knowledge regions rather than isolated page smells. Keep consolidation candidates in `_meta/consolidation.json` and protect the boundary through the wiki consolidation guard rather than blocking useful prose with brittle style lint.
- `Wiki Table Preservation Bias`: Do not replace tables with lists merely because a validator reports a small width warning. Keep tables when comparison/matrix structure carries meaning; first try shorter labels, tighter phrasing, or column-scope reduction. Convert to bullets only when the table is genuinely unreadable on mobile or prose-heavy enough that tabular structure no longer helps.
- `Self-Contained Wiki Product`: `/wiki` is derived from docs but should read as an autonomous explanation product. Main wiki prose SHOULD keep readers inside wiki-internal links and domain routes instead of requiring outward jumps to `/docs` for comprehension. Source/provenance references may remain in metadata, but human-facing wiki paths should explain concepts directly, choose one canonical wiki owner per concept, and replace repeated explanations with dense cross-links to that owner.
- `Trusted Wiki Render Contract`: When the browser renders repo-local wiki markdown directly, treat `/wiki/**/*.md` as trusted reviewed repository content rather than as user input. The safety boundary MUST then move to repository validation: reject raw HTML tag blocks, dangerous URL schemes, inline DOM event-handler attributes, and extra value-side colons in wiki frontmatter key-value lines pre-merge (currently via `./.agents/skills/wiki-sync/scripts/validate-wiki-trust.sh`) instead of pretending runtime sanitization is the canonical defense. Wiki markdown frontmatter is parsed as simple TOML-like `key: value`; do not put another colon in a `summary`: or other scalar metadata line.
- `Spec Purity`: Contract/spec documents MUST define the intended source-of-truth model only. Do not narrate current implementation status, shipped divergence, migration state, or rollout caveats inside the spec; those belong in the paired architecture doc.
- `Spec -> Code -> Architecture Sequencing`: For non-trivial subsystem work, the default delivery order is `(1)` establish or refine the specification contract, `(2)` implement and validate the code against that contract, and only then `(3)` write or update the architecture document to describe the shipped realization in that specific implementation. Architecture docs are downstream of implementation truth; they do not replace the normative spec and should not be written as if unimplemented architecture were already the contract.
- `Architecture Doc Release Agnosticism`: Architecture documents describe implementation truth, runtime/storage topology, subsystem boundaries, operational watchpoints, and the contracts actually used by the shipped implementation. Do not tie architecture docs to monorepo release numbers, patch targets, or consolidation-line labels; those belong in `CHANGELOG.md`, release notes, package metadata, or status/roadmap surfaces. If an architecture note needs a date, use it only as a freshness marker, not as the source of semantic version truth.
- `Entrypoint README Release Neutrality`: Human entrypoint README files should describe the current workspace/product contract, local orientation, and validation commands. Avoid embedding release-target rhetoric in README bodies; point readers to `CHANGELOG.md` for release history/targets and to `BACKLOG.md` or generated status pages for current open work.
- `Readiness Layer Timing`: While the runtime is still in active architectural evolution, avoid standalone readiness layers (dedicated rollout runbooks, report-generating readiness scripts, umbrella release gates for unstable surfaces). Keep rollout notes close to the relevant architecture/spec docs until the launch line stabilizes.
- `Rename Gate`: When a public runtime/domain atom is renamed, completion MUST include a stale-alias grep across `/template`, `/docs`, `BACKLOG.md`, `CHANGELOG.md`, and `AGENTS.md`, with runtime, tests, benchmarks, and docs all updated in the same pass.

### 7.2 Frontend Architecture Discipline

- `Frontend Read-Model Provenance`: In `/web-client`, keep the project-wide read-model contract two-class (`canonical-chain` vs `materialized`) and model browser-side realization separately (`direct`, `session-cache`, `session-derived`, `provider`) so session-built bounded views cannot masquerade as direct runtime projections or archive/indexed truth.
- `Named UX Topology Specs`: Canonical user-facing workspace/layout defaults with product significance SHOULD live in explicit named specs/constants plus migration matchers rather than ad-hoc store assembly. Mobile-only topology linearization SHOULD align to design-system breakpoints unless evidence justifies divergence. In `/web-client`, reserved edge-lane topology and lane widget composition are developer-configured rather than user-reorderable state, and mobile may intentionally map a different widget set into those designated lanes than desktop does.
- `Frontend Slice Honesty`: In `/web-client/src/lib`, do not keep heterogeneous stateful slices hidden under a generic umbrella once their roles diverge. Promote them to first-class top-level modules when that makes shell, layout, domain state, and execution-feedback boundaries more truthful. Avoid generic `shared/` wrapper buckets for durable frontend code; prefer explicit foundation modules (`ui`, `read-model`, `format`, `persistence`, `economics`) and domain-owned contract files inside the owning slice.
- `Execution-Feedback Slice Separation`: Account log, live network feed, and tx-progress / receipt state SHOULD live in a dedicated frontend slice (`log`) rather than bloating broader market/system state stores.
- `Adapter Purity`: Frontend adapter layers (`src/lib/adapters/*/`) SHOULD remain transport-oriented and SHOULD NOT accumulate domain types, constants, or UI contracts that outlive a single transport implementation. Domain types, shared constants, materialized-provider contracts, and domain-specific UI building blocks SHOULD live in the corresponding domain slice (`src/lib/<domain>/`) instead, so the adapter layer can be replaced or collapsed without dragging unrelated domain surface along with it.

### 7.3 Autonomous Execution & Backlog Discipline

- `BACKLOG Concreteness`: `BACKLOG.md` SHOULD track closable deliverables, explicit externally gated work, or bounded epics with clear exit criteria. Do not leave evergreen maintenance disciplines as unchecked backlog items; move standing rules into `AGENTS.md` or the relevant architecture/spec docs instead.
- `Fragility Routing`: When reviewing project `fragilities`, put actionable implementation/adoption/readiness risks into `BACKLOG.md` as concrete slices, but keep evergreen semantic, identity, documentation-purity, and process-discipline fragilities in `AGENTS.md` as durable rules rather than immortal backlog checkboxes.
- `While-True Backlog Sync`: Autonomous `while true` execution MUST stay anchored to `BACKLOG.md`. If implementation discovers a new in-scope slice that is not represented there, add or retarget a concrete BACKLOG entry in the same pass, and close/remove stale entries immediately once their exit criteria land.
- `Diff-Aware Validation Gates`: Agent-local auditors and loop gates SHOULD validate the changed scope by default and run the smallest meaningful validation set for the touched layer (for example shell syntax for changed scripts, simulator for math-coupled work, cargo for Rust workspace changes). Reserve full-tree sweeps for explicit `--all` / release-gate flows so new work is judged against its delta rather than unrelated historical debt.
- `Durable Skill Memory > Run Telemetry`: Agent-local skills that keep memory SHOULD prefer high-order durable ledgers over per-run execution traces. Canonical memory should capture reusable `hallucinations`, `ambiguities`, `dead ends`, and `boundary drifts`; raw run telemetry belongs in transient logs unless it crystallizes into a repeatable coordination failure pattern.
- `Low-Signal Ledger Ban`: Canonical ledgers MUST reject entries that only restate a bare gate/tool failure (`Knowledge Sync Gate Failed`, `Compilation Gate Failed`, etc.) without scoped pattern, affected surface, and remedy. If the agent cannot explain `what failed`, `where`, and `what to do differently`, the event stays in transient output rather than durable memory.
- `Foreground Service Trap`: Do not launch long-running dev servers, watchers, or similar blocking processes in the primary agent conversation flow unless the user explicitly asks for a live service session. Prefer bounded probes (`--help`, build, lint, test, static reads), or start the service in the background only with explicit ownership of log path, readiness check, and shutdown behavior.

---

## 8. Integration Protocols

> This section captures the concrete seams where the reference runtime binds into weights, network/XCM, and upstream SDK realities. It exists separately so integration contracts are visible as first-class architecture rather than hidden inside generic conventions.

### 8.1 Runtime Integration Protocol

- `The Weight Bridge`: Generated weights in `/pallets/*/src/weights.rs` are templates; real weights live in `/runtime/src/weights/`, where runtime-specific imports/bounds are adapted into the pallet `WeightInfo` implementation.
- `Configuration-as-Code`: Runtime `configs/*.rs` must point to `crate::weights::pallet_name::WeightInfo`. Never leave `()` or placeholder implementations in production.
- `On_Idle Safety`: Verify `BlockWeights` configuration leaves sufficient margin (e.g., 75% Dispatch Ratio) for `on_idle` tasks (automatic cleanup/swapping).
- `Relay-Beacon Ingress Pattern`: If a future parachain-consumable per-block relay/protocol beacon is adopted, prefer a weight-accounted `ConsensusHook` wrapper that materializes one compact per-block snapshot into pallet storage. Hot-path consumers (e.g. AAA entropy resolution) SHOULD read that snapshot instead of rebuilding `RelayChainStateProof` on demand.
- `Governance Reward Memory Discipline`: Governance-linked staking reward weight SHOULD use bounded sliding windows with runtime-configured caps and zero-sum eviction. If an account's winning-vote rolling sum reaches zero, its governance reward-memory entry SHOULD be deleted rather than kept as inert state. Prefer sparse per-account windows plus epoch expiry buckets that touch only expiring accounts over broad full-account scans. Item-scoped uniqueness SHOULD be enforced inside the live reward-memory horizon so the same governance item cannot be counted twice for one account while it is still relevant to reward weight. When temporary admin-side ingress is still required, prefer bounded per-item account batches over unbounded winner injection surfaces. Interim proposal policy (vote-weight surface, voting period, approval threshold, turnout floor) SHOULD stay runtime-configured so narrow governance slices can evolve without pallet rewrites. For the current launch line, prefer the simplest bounded governance policy that already preserves invariants: same-domain stake-value weighting, narrow admin recovery, and bounded recent finalized-outcome retention. When proposals must mature automatically, prefer epoch-keyed maturity buckets over scanning all active proposals each block. If finalized proposal outcomes are retained for UX/admin observability, they SHOULD use bounded expiry buckets rather than indefinite archival growth inside the kernel pallet.
- `Sparse Reward Snapshot Line`: The current staking reward-weight path SHOULD remain one-epoch-lagged, sparse, and touch-driven. Keep active per-account snapshots sparse, aggregate same-block reward touches/inflows in memory before touching staking storage, fix the epoch denominator when reward inflow is recorded, and emit an explicit truncation signal if bounded ingress or touched-account caps are exceeded. Native `$NTVE` nomination rewards are driven by explicit collator-LP/governance touch hooks plus epoch reward-account reconciliation, not by `$NTVE` reward-account or `stNTVE` transfer-event scanning. Legacy non-native event ingress remains compatibility support and SHOULD run from weight-accounted `on_idle`; truncated epochs are ineligible for settlement. On already-live chains with pre-existing untouched holders, require an explicit bootstrap or warm-up step before enabling reward ingress; the bootstrap path may materialize the live snapshot only before that epoch's denominator is frozen. Generic non-native rewards settle through same-asset auto-compound `claim_reward` / `claim_reward_batch`; native nomination rewards use dedicated liquid/compound claim surfaces.
- `Reserve-Aware Liquidity-Actor Slippage`: Builders for the current TMCTOL standard on DEOS SHOULD derive liquidity-provisioning actor foreign→native swap tolerance from current native reserve depth and clamp it between explicit lower/upper bounds. Avoid reusing the generic System AAA 5% swap ceiling as a flat liquidity-provisioning policy once pools become deep enough that tighter bounds materially reduce MEV exposure.
- `Upstream Sync Protocol`: Classify parachain-template changes as `SDK-standard` (adopt), `Ecosystem-pattern` (evaluate against project architecture), or `Business-logic` (skip).
- `Monorepo Source of Truth`: For upstream feature/history/PR audits in the current Polkadot SDK era, use `paritytech/polkadot-sdk` as the authoritative repository. Do not cite archived `paritytech/substrate` PRs/issues as current-status evidence.

### 8.2 Network Integration Protocol

- `Hybrid Registration Protocol`: Generate `AssetId` via `Blake2(Location)` only at first governance registration, then persist a bidirectional `Location <-> AssetId` registry in `pallet-asset-registry` so later XCM-version key changes can update the location side without breaking balances while reverse lookup and bijectivity stay O(1).
- `Token-Domain Bootstrap Protocol`: Runtime glue hooks on Asset Registry registration and TMC curve creation must remain idempotent and deterministic. Preferred default mapping is `tol_id = token_asset_id` for non-LP assets, with governance override available via explicit binding extrinsics.
- `Sovereign Liquidity`: The Parachain treats itself as a sovereign entity. It does not trust foreign chains to manage its liquidity; it pulls assets into its own local `pallet-assets` registry via XCM Reserve Transfers.

---

## 9. Pre-Task Preparation Protocol

> This section is the pre-flight checklist for safe, aligned work. It stays separate so task orientation remains a stable ritual instead of being scattered across topical sections.

`Before executing any task, the Agent must`:

1.  `Classify Surface`: Is the task primarily about `/docs`, `/template`, `/web-client`, `/scripts`, or `/simulator`?
2.  `Locate Truth`:
    - `/docs`: Review `/docs/README.md` plus the relevant spec or architecture note first
    - `/template`: Review the relevant `/docs` contract, then `/template` patterns and runtime/pallet conventions
    - `/web-client`: Review `/docs/web-client.architecture.en.md`, `web-client/README.md`, and the touched slice boundaries
    - `/scripts`: Review `scripts/README.md`, `_common.sh`, and the touched entrypoint contract
    - `/simulator`: Consult when tokenomics, formulas, thresholds, or invariants are being changed or re-validated
3.  `Support Priority Check`: Default maintenance focus is `/docs` → `/template` → `/web-client` → `/scripts`; treat `/simulator` as the authoritative math surface, not as a co-equal default entrypoint for unrelated work.
4.  `Context Check`: Ensure mental model aligns with current architecture and current launch-line constraints.

---

## 10. Task Completion Protocol

> This section defines what counts as done, including validation, hygiene, knowledge sync, and repo-local gates. It exists separately so completion criteria stay explicit, auditable, and harder to skip under pressure.

`The sequence for "Done"`:

1.  `Validation`:
    - Run the smallest meaningful validation set for the touched surface first
    - `/docs` or `AGENTS.md`: terminology, cross-reference, and stale-alias sanity for the changed contract
    - `/web-client`: run the relevant workspace check/build commands when UI or client contracts changed
    - `/scripts`: run shell syntax, `--help`, and bounded smoke checks for the touched entrypoints
    - `/template`: run targeted `cargo` checks/tests for the touched crate or runtime surface; escalate to workspace-wide checks when the diff crosses crate/runtime boundaries
    - `/simulator` or tokenomics/invariant changes: `node ./simulator/tests.js`
    - If a change crosses the economic contract, implementation, and integration boundaries, re-run the full three-layer path instead of stopping at a local smoke check
2.  `Hygiene`:
    - Rust changes: zero Clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`) before calling the Rust layer done
    - Keep touched code and docs formatted and style-consistent in their native workspace
3.  `Knowledge Sync`:
    - Update `/docs` if logic changed.
    - Update `BACKLOG.md` if open backlog changed; close/remove stale entries and split any newly discovered in-scope slice into a concrete backlog item in the same pass.
    - Update `AGENTS.md` if _patterns_ or _wisdom_ evolved.
    - If any public term was renamed, run a stale-alias audit before finishing so spec/docs/runtime/tests/benchmarks keep one canonical vocabulary.
    - Record completed delivery work in `CHANGELOG.md`.
4.  `Repo-Local Completion Gate`:
    - When a pass changes repository state and you are operating autonomously (including `while true` loops), run `./.agents/skills/alignment/scripts/while-true-gate.sh` after local validation and knowledge sync.
    - Treat a failing gate as `not done`.
    - Keep `/.agents/skills/alignment/SKILL.md` as the canonical detail/flags reference instead of duplicating its internal phases here.
5.  `Garbage Collection` (if `AGENTS.md` exceeds 360 lines):
    - Trigger garbage collection phase
    - Analyze bloat sources: prune verbose sections outside active protocol content (redundant references, over-detailed patterns)
    - Preserve: architectural decisions rationale, philosophical foundations, active conventions
    - Remove: implementation minutiae superseded by code, resolved open questions, dated references
