# DEOS Project Protocol

> Durable project context for contributors and agents. Detailed subsystem truth belongs in specifications, architecture documents, code, tests, and repo-local skills.

## 0. Meta-Protocol Principles

- `Reflexive Protocol`: The context system must obey the same boundedness, ownership, validation, and cleanup rules that it imposes on the project.
- `Rationalist Discipline`: Treat rationalism as an operating method: expose assumptions, separate evidence from inference and preference, make load-bearing claims falsifiable, state material uncertainty, and update conclusions when better evidence arrives.
- `Mandatory Knowledge Sync`: Every meaningful task reconciles durable rules, open work, delivery history, entrypoints, and subsystem truth when reality changes.
- `Flat Structure`: Use second-level headings and one-level lists only so rules remain scannable, addressable, and independently editable.
- `Single Ownership`: Keep each fact in one authoritative surface and replace duplicated detail with a truth-owner reference.
- `Boundary Clarity`: Keep meta-protocol, project architecture, open work, delivery history, and subsystem implementation truth in their respective layers.
- `Constraint-Driven Evolution`: Add structure only after real constraints expose the need; preserve complexity earned by invariants and delete complexity created by habit.
- `Progressive Enhancement`: Keep the smallest correct baseline independently usable, then add capability, evidence, performance, and integration layers only when their contracts and prerequisites become explicit.
- `Graceful Degradation`: When an optional tool, provider, adapter, or evidence layer is unavailable, fall back to an explicit narrower capability without weakening invariants, atomicity, safety, or truth classification; required layers fail closed instead of silently changing semantics.
- `Test-Driven Evolution`: Treat context structure, links, terminology, claims, and open-work truth as testable infrastructure rather than optional prose hygiene.
- `Context Optimization`: Evolution includes addition, consolidation, relocation, and deletion; move exploratory knowledge toward stable contracts as evidence accumulates, retire superseded explanations, and never treat growth alone as improvement.
- `Cognitive Infrastructure`: Treat docs, scripts, skills, audits, and validation gates as part of the system because they determine which abstractions contributors reproduce.
- `Completion Honesty`: A task is not done while validation fails, backlog state lies, completed work remains open, or an external gate is presented as locally verified.

## 1. Project Identity and Scope

- `Repository Type`: DEOS is a specification and reference framework for deterministic protocol economies.
- `DEOS`: The Deterministic Economic Operating System provides the runtime kernel, DEOS Actors, routing, staking, governance, consensus integration, and bounded read surfaces.
- `TMCTOL`: The Token Minting Curve plus Treasury-Owned Liquidity standard is the flagship economic configuration running on DEOS; it is not the only possible DEOS economy.
- `Goal`: Provide a forkable foundation for launching ecosystems with explicit economic mechanisms, inspectable invariants, and production-oriented validation.
- `Product Boundary`: This repository owns the framework and reference stack, not a finished branded ecosystem product.
- `First-Ecosystem Relationship`: DEOS is both reusable infrastructure and the substrate of its first intended production ecosystem; reusable contracts stay here while identity, dApps, launch narrative, and concrete product loops stay downstream.
- `Adoption Model`: Partner teams fork DEOS, configure their economy, and may contribute framework-hardening improvements back without moving product policy into the kernel.
- `Mechanism vs Policy`: DEOS owns primitives, invariants, adapters, execution safety, bounded projections, and reference patterns; downstream instances own brand, founder economics, labor culture, invoice norms, bucket names/percentages, marketing, and demand strategy.
- `Release Line`: The standalone repository release line began at `0.0.0`; `CHANGELOG.md` records this line only.
- `Pre-1.0 Launch Boundary`: No downstream product network may launch before DEOS `1.0`; every `0.x` release evolves fresh-genesis source, permits breaking ABI/storage changes, and carries no production storage lineage or migration obligation.
- `Release Branch Topology`: A dedicated version branch may accumulate reviewable checkpoint commits during development, but immediately before its release pull request to `main` it must be rewritten with lease protection to exactly one repository-style release commit above the verified `main` baseline, preserving the prepared tree and reusable tree-bound evidence; never rewrite `main`, tags, or a branch already under pull-request review.
- `Acronym Semantics`: Deterministic means explicit bounded protocol reactions for the same on-chain state, Economic names the managed capital/liquidity domain, and Operating System means a domain-specific execution substrate rather than a general-purpose OS.
- `Primary Human Entry`: `README.md` explains the framework and routes evaluators, builders, operators, and contributors to the right source of truth.

## 2. Context and Truth Ownership

- `Canonical Memory Split`: `AGENTS.md` owns durable protocol, `BACKLOG.md` owns open work, and `CHANGELOG.md` owns completed delivery history.
- `Open Work`: Start repository work from `BACKLOG.md`; add newly discovered in-scope work there and remove completed items immediately.
- `Backlog Shape`: Format open work as `- [ ] \`Domain\`: Description` with uppercase prose after the colon; track closable deliverables and explicit gates only, while evergreen disciplines belong in this protocol, subsystem docs, or skill contracts.
- `Delivery History`: Record meaningful completed outcomes and impact in `CHANGELOG.md`, not in backlog or durable protocol prose.
- `Changelog Shape`: Format delivery entries as `- \`Domain\`: Description`, using slash-separated domain qualifiers when needed; keep at most 8 single-line entries per release and at most 512 characters per entry; exclude package-marker chores, intermediate implementation diaries, and duplicated architecture explanations.
- `Spec Ownership`: Specifications own intended subsystem contracts, rationale, invariants, and public semantics.
- `Contract Closure`: Follow specification → implementation → tests → evidence-selected correction of specification and implementation → architectural description of the verified implementation. Classify discrepancies as contract, implementation, or test defects; never ratify accidental behavior by rewriting the specification. A release freeze requires these surfaces to agree, with affected evidence refreshed after corrections.
- `Package Architecture Ownership`: Package-owned architecture docs describe the shipped implementation of the independently reusable crate: storage topology, bounded algorithms, modules, generic interfaces, package evidence, and code anchors.
- `Integration Documentation Ownership`: Root `docs/*.integration.en.md` documents concrete DEOS reference composition across runtime bindings, adapters, cross-pallet flows, System actor topology, production weights, client realization, and operational watchpoints.
- `Code Ownership`: Code and tests own executable behavior; documentation claims must remain subordinate to verified implementation truth.
- `Skill Ownership`: Repo-local skills own agent workflows and audits; project build, runtime, tests, benchmarks, CI, and release tooling must work without installed skills or files under `.agents/skills`. Skills may inspect and invoke project-owned tools, never supply a required project dependency.
- `Experiment Causality`: Give each materially distinct hypothesis one experiment owner, each decision explicit downstream relations, each measurement exact evidence/source identity, and each release gate explicit evidence owners. Frozen architectural facts reopen only through their declared evidenced invariant/impossibility trigger; separate architecture selection from production measurement and release acceptance. The architecture-experiments skill owns the record method.
- `README Ownership`: Root and subtree READMEs own human orientation, setup, navigation, and current workspace purpose.
- `Read-Model Ownership`: `docs/read-model.contract.en.md` owns chain/materialized data classification; `web-client/docs/architecture.en.md` owns browser realization.
- `Actors Control-Plane Ownership`: `docs/actors-control-plane.contract.en.md` owns off-chain Actor Contract artifacts, typed projection/diff, forecast/simulation provenance, governance composition inputs, and materialized Actor history boundaries.
- `Framework Boundary Ownership`: `docs/framework-instance.contract.en.md` owns the reusable mechanism versus downstream policy contract.
- `Builder Economy Ownership`: `docs/builder-economy.contract.en.md` owns the reference composition of second-order TMCTOL, `$BLDR` governance, invoices, BLDR Anchor, BLDR Treasury, and the parent-capital bridge; TMCTOL and Governance specifications retain their underlying mathematics and governance semantics.

## 3. Repository Topology

- `/docs`: Cross-system conceptual control plane containing framework contracts, concrete DEOS integration documents, strategy notes, and the canonical documentation index; package-owned documents are linked directly rather than mirrored as redirect stubs.
- `/template`: Rust reference implementation containing the parachain runtime, pallets, primitives, weights, tests, runtime-adjacent research, and package-owned reusable specifications, implementation architecture, and embedding guidance under each pallet's `docs/` directory.
- `/web-client`: SvelteKit reference client for browser-facing DEOS and current TMCTOL flows.
- `/scripts`: Shared human/CI/skill automation; numbered scripts are independent atoms, while named scripts are deterministic compositions or admin utilities.
- `/simulator`: Historical TMCTOL hypothesis lab and authoritative mathematical reference for formulas, thresholds, conservation, floor/compression scenarios, and parameter behavior.
- `/wiki`: Generated bilingual semantic projection of canonical root and package-owned documentation for onboarding, frontend rendering, and agent navigation.
- `/.agents/skills`: Repository-local agent workflow control plane; its README owns the skill graph map, each `SKILL.md` owns one domain contract, and project-specific audits belong in `alignment`.
- `Support Priority`: Routine stabilization starts with `/docs`, then `/template`, `/web-client`, and `/scripts`; consult `/simulator` whenever tokenomics or invariant math moves.
- `Core Entry`: Start system-wide architecture work with `docs/core.architecture.en.md`.
- `Runtime Entry`: Start Rust workspace work with `template/README.md` and the owning pallet/runtime docs.
- `Client Entry`: Start browser work with `web-client/README.md` and `web-client/docs/architecture.en.md`.
- `Scripts Entry`: Start automation work with `scripts/README.md`, `_common.sh`, and the touched entrypoint's `--help` contract.
- `Wiki Entry`: Start wiki work with `/.agents/skills/wiki-sync/SKILL.md`.

## 4. Canonical Vocabulary

- `Terminology Lockstep`: Stable specs, architecture docs, runtime/API surfaces, wiki, and client copy must use one canonical term per domain atom.
- `Framework Naming`: Use `DEOS` for the framework/runtime/reference stack and `TMCTOL` only for the concrete economic standard.
- `Concrete Subsystem Branding`: Use `DEOS Router`, `DEOS Governance`, `DEOS Staking`, and `DEOS Oracle` when naming those concrete framework subsystems; preserve stable Rust crate, runtime pallet/module, source-file, and generated-weight identifiers unless a separate compatibility change lands. Concept, mechanism, and domain labels remain unprefixed when they own a broader semantic object, and distinctive names such as DEOS Actors and TMC remain unprefixed.
- `Cargo Publication Naming`: Independently publishable DEOS packages use globally conflict-resistant framework names such as `pallet-deos-*` and `deos-primitives`; never substitute `deus` or an unqualified generic package name for the DEOS framework identity. Cargo package identity may change without renaming the stable Rust library crate.
- `Cargo Release Identity`: `template/Cargo.toml` owns the lockstep DEOS workspace package version; every workspace member inherits `workspace.package.version`, and `Cargo.lock` records the resolved release identity.
- `Governance Naming`: Use `DEOS Governance` for the concrete subsystem and `Governance` for the broader domain overview; runtime implementation remains `pallet-governance`.
- `Asset Notation`: Prefix concrete asset symbols with `$` in specs and architecture prose (`$NTVE`, `$VETO`, `$BLDR`); keep bare labels for vote options and non-asset semantics.
- `Actors Abstraction`: Describe automation by System Actor role and execution-plan family rather than legacy manager/farmer names.
- `Actor Casing`: In prose, title-case established role names such as `Burn Actor`, `Liquidity Actor`, and `System Actor`; keep ordinary descriptions lowercase and code identifiers idiomatic.
- `Legacy Names`: Keep manager names only for historical orientation or explicit compatibility aliases at public boundaries.
- `TMC`: The unidirectional issuance engine implementing the configured minting curve.
- `TOL`: The asset-scoped topology through which a treasury owns protocol liquidity and related strategic capital; anchor, bucket, and treasury counts are configuration, with the TOL component of reference first-order Native/foreign TMCTOL using A/B/C/D plus paired B/C/D treasuries and the TOL component of second-order `$BLDR` TMCTOL using BLDR Anchor plus BLDR Treasury.
- `Bucket Types`: The reference first-order aliases are Bucket Anchor (`A`), Bucket Builder (`B`), Bucket Capital (`C`), and Bucket Dormant (`D`). Anchor is the reusable immutable bucket type; the second-order `$BLDR` TOL component uses the short human name `BLDR Anchor` because it has no sibling lettered family. Implementation identifiers belong to their owning code surface and never to the economic vocabulary.
- `BLDR Anchor`: The sole immutable Anchor-type bucket owning protocol-created `$NTVE/$BLDR` LP; never qualify its human name with a bucket letter and never call it Builder Bucket. Bucket Builder (`B`) is the distinct first-order spendable/buyback lane, while BLDR Anchor communicates the second-order immutable role without the `BB` ambiguity.
- `DEOS Router`: The fee-burning execution gateway selecting the candidate with maximum recipient output across XYK, TMC, and bounded Native-anchored routes.
- `DEOS Oracle`: The bounded current typed observation owner; runtime implementation remains `pallet-oracle`.
- `DEOS Staking`: The reference staking subsystem; runtime implementation remains `pallet-staking`.
- `System Actors`: Runtime-owned Actor instances executing bounded protocol economic flows.
- `Burn Actor`: The System Actor role that processes configured balances into burn flow.
- `Liquidity Actor`: The System Actor role family that provisions liquidity for configured pools or lanes.
- `Omnivorous Intake`: Balance-driven ingress semantics that react to assets arriving at an actor account rather than one bespoke extrinsic.
- `Resilience`: Retry and cooldown behavior protecting actors during oracle, liquidity, or market unavailability.
- `Runtime-as-Config`: Generic pallets receive economic and runtime policy through traits and adapters rather than hardcoded ecosystem logic.
- `Omni Node`: The deployment architecture; this repository does not carry a custom node crate.

## 5. Framework Architecture Invariants

- `Framework Forkability`: Changes under `/template` must preserve generic utility and avoid hardcoding downstream ecosystem identity or business policy.
- `Reusable Pallet Packaging`: Treat a reusable pallet as an independently consumable package; keep its public host contract and separate external-consumer fixtures under the pallet ownership boundary, while concrete DEOS adapters and topology remain in the reference runtime composition.
- `Deterministic Mechanics`: Runtime-managed economic reactions must use explicit triggers, typed payloads, bounded state, and weight-accounted execution.
- `Actors Progress Preservation`: Keep `RetryLater` Mutable-only and Temporary-only, with one sparse scalar-cursor Continuation on the canonical FIFO/wakeup substrate; preserve committed prefixes without compensation, whole-plan rollback, duplicate scheduler state, or off-chain correctness dependencies. `template/pallets/actors/docs/specification.en.md` owns the full contract.
- `Actors Authority Simplicity`: Keep consensus authority at immutable `ActorType::{User, System}` until a concrete shipped need proves two types insufficient; every actor receives bounded service through one undifferentiated paged FIFO under one scheduler, one global ticket allocator/cutoff, one actor-local ticket, and one wakeup/Continuation/lifecycle owner, where actor class never changes service order, not an Actor-id whitelist, owner-authored priority, a generic policy object, or pre-extrinsic execution.
- `Actors Market Loss Bounds`: Every market task carries its owning spend/output bound; generic Actors must not impose an asset-agnostic raw-balance System cap. Every System swap additionally obeys the typed reference-deviation guard, and temporary market rejection uses one capped retry/wakeup path rather than accepting an adverse fill.
- `Actors Observation Reactions`: Derive duplicate-free subscriptions from admitted typed trigger sources into actor-owned reusable slot-addressed pages; DEOS Oracle change context may mark bounded dirty state only, deferred fanout may set the existing pending latch, and the existing scheduler alone executes actors. One shared Trigger instance defines initial causal cohortability independently of Contract length; mixed-length Actors share Step-0 materialization/reaction without loading unreachable tails, then every surviving Actor continues independently under Q1.
- `Actor Fee Boundaries`: Keep Creation Fee, useful-readiness Trigger Fee, per-admitted-cycle complete bounded Pipeline Machine Fee, and pay-as-attempted Action execution fee as four independent owners. A Trigger charges its generated detection/matching/materialization owner only for `pending_signal: false -> true`; while latched, redundant occurrences perform no Actor-specific Trigger evaluation or fee collection and create no additional activation. Later consumption charges complete bounded Pipeline control—including retry, continuation, RunFrame, completion, and minimal-apoptosis work—but never future Action effects. Actor lifetime creates no recurring rent.
- `Trigger Latching`: Once `pending_signal` becomes true, disable or remove the Actor from family-specific detector topology where practical until Pipeline Opening consumes the latch; otherwise reject redundant work before Actor-specific evaluation. Re-arm from current authoritative state, intentionally discard latched-period causal history, and preserve at most one active plus one deferred Pipeline.
- `User Cycle Admission`: Trigger processing and Pipeline admission are separate transitions. One paid `false -> true` Trigger transition may latch readiness while a Pipeline remains Running/Suspended; only Idle consumption tests and charges complete Pipeline Machine plus cleanup capacity and opens one cycle. Running/Suspended Q1 Steps reuse paid machine authority, while each Action attempt independently admits and pays its actual effect. Insufficient Pipeline admission selects custody-neutral minimal apoptosis; the committed useful-readiness Trigger fee remains final.
- `Actor Close and Custody`: Minimal apoptosis and every other close delete process semantics, release lifecycle metadata/slot/state resources, and perform no economic unwind or custody transfer. Deterministic sovereign custody survives close and exact-slot recreation; exit/recovery uses ordinary authored Steps, and `Transfer(AllAvailable)` receives no lifecycle source-exhaustion privilege.
- `Zero-Step and Immutable`: Admit `0..=MaxContractSteps`; a zero-Step activation is a bounded FIFO-ordered Opening/completion control cycle with no Action fee, and `AtTime { after_ticks }` supplies first-class one-shot temporal readiness. User Immutable rejects owner close and every premature mutation/destruction path, while mandatory authored/temporal terminal semantics and insufficient-activation minimal apoptosis remain independently effective.
- `Token-Driven Coordination`: Prefer asset movement and runtime hooks over privileged signed calls when token ingress itself defines the event.
- `Bounded Consensus State`: Every storage collection, iteration, history surface, retry path, and projection must have a defensible bound.
- `Read-Model Honesty`: Public data must be classified as bounded authoritative on-chain truth or externally indexed/materialized truth; canonical UX must not hide an indexer dependency.
- `On-Chain Projection`: Keep canonical bounded state and projections on-chain; route archive, search, and unbounded analytics to materialized providers.
- `Mechanism-Over-Policy`: DEOS Router compares viable candidates by maximum recipient output; price impact and fee fields remain informational quote metadata.
- `Transactional Mutation`: Entry points that can fail after touching multiple storage locations must prevalidate fallible conditions or use transactional semantics.
- `Reverse-Index Preference`: Persist bounded reverse mappings when live bijectivity, inverse conversion, or lookup weight forms part of the contract.
- `Unified Primitives`: Keep shared asset taxonomy and ecosystem constants in `template/primitives`; avoid duplicated magic numbers.
- `AssetKind`: Preserve bitmask-based O(1) classification and dedicated staked-local/staked-foreign namespaces.
- `Arithmetic`: Use `Perbill`/`Permill` for ratios and `U256` intermediates where curve arithmetic can overflow native widths.
- `Logical-First Naming`: Name stable abstractions by role before representation; a time-ordered wakeup index need not promise a particular storage structure. Project identifiers and code comments describe domain mechanisms, not Skill names, experiment IDs, candidate labels, or implementation-session history; keep experimental provenance in its research owner, pointing to project code rather than making code depend on it.
- `Cadence`: Keep block-duration assumptions explicit, benchmarked, and configuration-driven rather than fixing DEOS to one block speed.
- `Protected Complexity`: Preserve complexity earned by real constraints and invariants; remove accidental complexity and speculative indirection.
- `No Premature Optimization`: Prefer contract correctness and honest product flows over speculative loading, bundle, storage, or scheduler indirection.
- `Pre-Fork Storage Lineage`: Through DEOS `0.x`, reset fresh-baseline storage versions and remove historical migration ceremony; the first downstream production genesis may occur only from `1.0` or later, after which each deployed fork owns monotonic versions and migrations.
- `Genesis-Complete Runtime Presets`: Runtime code owns every genesis-state value for each declared profile, and its named preset must generate a complete, internally coherent, builder-verified ChainSpec without post-generation mutation. Operator scripts may select the runtime preset and outer ChainSpec metadata only; they must not duplicate or override economic policy, authorities, accounts, para identity, or other genesis truth. Do not expose a production-like profile before the runtime owns its complete production preset.
- `Pre-Launch Contract Coherence`: Before any network launches and before a stability declaration, prefer one semantically ordered canonical SCALE/API contract over append-only compatibility, legacy aliases, or migration ceremony; group fields by domain meaning and hierarchy, then regenerate metadata, control-plane, client, tests, weights, and Wasm evidence together.

## 6. TMCTOL Economic Invariants

- `Unidirectional Minting`: TMC issues along a curve and does not expose reserve redemption as a protocol promise.
- `Economic Physics`: Parameters defining launch-time Economic Physics default to immutable unless a stronger constitutional contract explicitly delegates them.
- `Gravity Well`: Treat the emergent liquidity state as a studied standard property, not an unconditional market guarantee.
- `Elasticity Inversion`: Use the term only for the expanding-supply zero-slope threshold where supply growth stops worsening the effective floor.
- `Compression Terminology`: Every compression claim identifies its analysis axis and metric; do not conflate inversion, relative parity, absolute-gap compression, and arbitrage overtake.
- `Economic Claim Honesty`: Never state market immunity or unconditional guarantees beyond shipped runtime behavior and falsifiable evidence.
- `TOL Accounting`: Keep reserve scope, bucket state, supply basis, sellable-pressure assumptions, and governance conditions explicit in floor claims.
- `Bucket Policy`: Treat anchor, bucket, and treasury topology, order, and percentages as TMCTOL/reference-instance policy rather than mandatory DEOS kernel law; TOL order names collateral dependency, not execution hierarchy.
- `Anchor Strong Immutability`: Reference Bucket A and BLDR Anchor are sealed dormant Immutable System Actors with no Contract, hot state, scheduler work, funding state, or fee dependency. Their LP-namespace balances admit inbound LP but expose zero reducible balance to ordinary, admin-forced, and runtime-internal transfer or burn paths; the LP namespace cannot enter asset destruction while this consensus freezer exists. Only an explicit runtime upgrade or fork may revise this guarantee.
- `Layered TMCTOL Classification`: First- and second-order classify complete TMCTOL economies because issuance begins in TMC; TOL names only their liquidity and strategic-capital component. The first-order Native/foreign TMCTOL and second-order `$BLDR` TMCTOL references share only the anchor-directed issuance invariant of approximately one third of total issuance; collateral allocation is topology-specific, with half entering first-order Bucket A and all `$NTVE` collateral directed to BLDR Anchor.
- `Burn Liveness`: Burn effects depend on funded, configured, schedulable execution; do not describe fee capture as automatic supply reduction before the Burn Actor completes it.
- `Liquidity Liveness`: Liquidity effects depend on healthy pools, configured execution plans, bounded slippage, and valid reserve accounting.
- `Simulator Authority`: Use the simulator for economic math and hypotheses, not as a shadow runtime for storage, weights, Actors, governance, XCM, or client parity.
- `Simulator Minimalism`: Keep simulator models and scenarios readable, elegant, and economics-centered; test formulas, invariants, and explicit hypotheses rather than defensive implementation machinery, and retain lifecycle detail only when an economic question requires it.
- `Deterministic Simulation`: Use fixed cases or explicit seeded PRNGs in correctness suites and keep wall-clock measurement in benchmark tooling.

## 7. Runtime Subsystem Contracts

- `FRAME`: Use FRAME v2 pallets, typed configuration, and `frame_benchmarking::v2` patterns.
- `Asset Registry`: Persist bidirectional `Location <-> AssetId` mappings; derive IDs only at governance registration and preserve balance identity across location updates.
- `Token Bootstrap`: Asset-registration and curve-creation hooks must remain deterministic and idempotent.
- `Sovereign Liquidity`: Foreign assets enter local `pallet-assets` through XCM reserve-transfer assumptions; DEOS does not delegate its liquidity accounting to foreign chains.
- `Liquid Staking`: Keep one staking pallet, `stXXX` receipts, and native `stNTVE`; do not add a parallel nomination-token tier without evidence.
- `Native Security`: Native collator backing uses explicit locked `$NTVE/stNTVE` LP custody rather than liquid receipt ownership.
- `Actors Staking Portability`: Keep `Stake` and `Unstake` tasks generic; runtime adapters decide native, non-native, or local representation behavior.

## 7A. Governance and Reward Contracts

- `Governance Domains`: Model governance as explicit domain-scoped primary/protection track pairs rather than proposal-id conventions or actor-profile hacks.
- `Governance Shape`: Prefer `GovernanceDomain + CadenceMode + ProposalPayloadKind`; add richer proposal classes only after measured pressure.
- `Urgent Policy`: Fast-track eligibility defaults deny and must be opted in per domain/payload combination.
- `Strategic Governance Ingress`: A protocol-domain `L1RootAction` may enter only through the existing signed proposal surface gated by runtime-defined nonzero primary-track power, retaining ordinary fee/cap/rollback semantics; this grants no direct Root dispatch, gives `$VETO` no agenda-setting authority, and leaves Root-equivalent enactment inside the existing bounded payload executor. `template/pallets/governance/docs/specification.en.md` owns the full contract.
- `Builder Invoice Settlement`: The Governance specification owns `BaseFloorCapped` settlement semantics: an above-base scalar target may clip only to enactment-time capacity at or above the complete base amount, while below-floor capacity produces typed execution failure and zero payout; targets at or below base require their complete amount. The Builder Economy contract selects that policy and binds each invoice to a bounded canonical IPFS CID plus one governance-approved Mutable System Actor treasury account, leaving its Actor Contract and lifecycle unchanged. Builder governance gains domain-local debit authority over that declared custody, not arbitrary Actor or Root authority.
- `L2 Parameters`: Treat delegated parameter changes as explicit bounded domain-owned surfaces, not permission to call arbitrary admin setters.
- `Safety Bias`: Protection governance may fail closed; `$VETO` is negative constitutional power rather than a second positive-governance path.
- `Governance Reward Memory`: Keep windows, expiry buckets, uniqueness, retention, and proposal maturity bounded; avoid full-account or full-proposal scans.
- `Reward Sparsity`: Preserve sparse, touch-driven, one-epoch-lagged reward snapshots and explicit truncation signals.
- `Reward Sources`: Keep reward distribution separable from origin so externally funded or treasury-budgeted pots remain possible.
- `Unclaimed Rewards`: Treat leftovers as explicit runtime policy rather than accidental residue.

## 7B. Fee and Security Contracts

- `Liquidity Slippage`: Derive Liquidity Actor swap tolerance from current reserve depth and clamp it between explicit runtime bounds.
- `Fee Collection`: Keep DEOS Router trading fees on the Burn Actor path; collect 100% of transaction, Actor-execution, governance-opening, and XCM-execution fees into the Fee Sink System Actor independently of actor execution liveness, and name the generic Actors boundary by collection rather than trading-route semantics. Keep collection ledger-only and let one 120-tick cadence under the 500 ms consensus clock own allocation, so no fee source fabricates a privileged or parallel queue signal.
- `Fee Allocation Phases`: Each Fee Sink cycle processes 10% of its current spendable native buffer only when the processed amount can fund at least one ED to every configured allocation leg. While collators remain permissioned, the processed amount splits 50/50 to staking ingress and liquidity provisioning; a future equal-thirds security/staking/liquidity plan requires permissionless collators and an explicit bounded security-reward contract, with the unprocessed buffer and indivisible remainder retained for later cycles.
- `Native Flow Anchors`: The DEOS runtime endows System Actor, custody, and staking-ingress accounts admitted for arbitrarily small native flows with one persistent free-balance ED, preserves that anchor through authored spend resolution, and converts only newly received value. Actor close remains balance-neutral and grants no source-exhaustion exception. Generic Actors follows exact host-ledger consequences and does not promise provider-only or reserved-only zero-free sub-ED ingress.
- `Collator Reward Gate`: Treat `CollatorRewardPot` as an unresolved design placeholder, not an accepted pallet or storage topology, until eligibility, contribution accounting, settlement cadence, custody, payout, leftovers, and failure behavior have explicit owners and bounds.

## 8. Engineering and Validation

- `Validation Layers`: Mathematical truth lives in the simulator, behavioral truth in pallets/tests, systemic truth in runtime integration tests/XCM, client truth in `web-client`, project/release composition in root scripts and workflows, and agent-method validation inside its owning Skill.
- `Validation Ownership`: Each validation and test stays with the surface whose truth it decides: package/domain checks with that package or project domain, client checks in `web-client`, CI checks in workflows/root scripts, development profiles in root scripts, and Skill-method checks inside the Skill. The project MUST expose one project-owned comprehensive validation entrypoint that may compose all project surfaces but never Skill internals; if it needs a check currently inside a Skill, move that check to the project owner rather than introducing the dependency.
- `Validation Scope`: Run the smallest meaningful changed-scope check first and escalate only when the diff crosses boundaries.
- `Tool Self-Validation`: Keep a repository tool's own regression checks in its explicit self-test mode rather than adding a separate test tool; reuse that mode from the owning validation flow and isolate fixture mutations from project state.
- `Stateful Tests`: Use realistic stateful mocks for AMM, TMC, and cross-component mechanism verification.
- `Benchmark Metrics`: Measure both RefTime and ProofSize with explicit bounded components and worst-case setup.
- `Weight Bridge`: Generate pallet weight templates and bind runtime-specific implementations under `template/runtime/src/weights`.
- `Production Weights`: Runtime configs must use real `WeightInfo`; do not ship `()` placeholders.
- `Idle Safety`: Preserve block-weight headroom for `on_idle` work and account for hook pressure in scheduling changes.
- `Operational Reserve`: The reference runtime intentionally carries no dedicated Operational weight reserve while no concrete critical Operational extrinsic consumes it; introducing such a call requires a measured reserve and an explicit dispatch/`on_idle` rebalance in the same change.
- `Rust Imports`: Prefer direct `polkadot_sdk`, `frame_support`, and `sp_runtime` paths over compatibility shims unless a macro/generated boundary requires them.
- `Rust Warnings`: Maintain zero Clippy warnings across workspace/all targets.
- `Workspace Lints`: Keep Substrate cfg allowances and the upstream-aligned lint set honest.
- `WASM Builder`: Keep `substrate-wasm-builder` aligned with the current Polkadot SDK line.
- `Runtime Version`: Through DEOS `0.x`, keep `authoring_version = 1`, `spec_version = 1`, `impl_version = 1`, `system_version = 3`, and `transaction_version = 1`; package versions may advance without simulating on-chain compatibility. After the first downstream production genesis from `1.0` or later, that deployed runtime owns monotonic compatibility bumps under SDK semantics.
- `Source Headers`: Do not add license or copyright headers to source files.
- `Suppressions`: Avoid broad JS/TS/Svelte lint and type suppressions; narrow and justify unavoidable exceptions.
- `Complexity Feedback`: Treat compilation and integration failures as architectural feedback; simplify abstractions before adding compatibility layers.
- `File Mutation`: Use repository edit/write tools for file changes; use shell commands for inspection, execution, and validation.
- `Long-Running Processes`: Do not start foreground servers or watchers in the primary agent flow unless explicitly requested.

## 9. Documentation Contract

- `Spec Purity`: Specifications define intended source-of-truth contracts only; implementation status, migration notes, and rollout caveats belong in architecture docs.
- `TMCTOL Specification Boundary`: `docs/tmctol.specification.en.md` is the project-independent mathematical entry to the standard; keep concrete asset tickers, named project tokens, Actor topology, invoice semantics, and instance product flows in the Builder Economy, framework-instance, or integration owners.
- `Delivery Sequence`: For non-trivial subsystems, refine specification, implement and validate code, then update package architecture and any affected DEOS integration document from shipped truth.
- `Paired Docs`: Non-trivial reusable pallets should have a specification and a separate package implementation architecture map; add a root integration document only when concrete DEOS composition crosses the package boundary.
- `Embedding Docs`: Reusable host-runtime obligations live at `template/pallets/<name>/docs/embedding.md`; do not create uppercase package-root `EMBEDDING.md` aliases or redirect shims.
- `Workspace Architecture`: A self-contained workspace owns its implementation map at `<workspace>/docs/architecture.en.md`; root `/docs` retains cross-system contracts, integration, strategy, and framework-wide architecture only.
- `Package Purity`: Package architecture and embedding docs must remain useful to an independent host runtime; concrete DEOS actor catalogs, addresses, pallet indices, adapters, runtime parameters, production bindings, and cross-system product realization belong in root integration docs.
- `Integration Test`: Place a claim in package docs only if it remains true when the crate is embedded in an unrelated runtime; otherwise route it to the owning root integration document.
- `Doc Filenames`: Use full dotted forms such as `name.specification.en.md`, `name.architecture.en.md`, `name.integration.en.md`, `name.contract.en.md`, and `name.strategy.en.md`.
- `Markdown Tables`: Use exactly one padding space inside every cell boundary and compact delimiter rows such as `| --- | --- |`, preserving alignment only when meaningful with `| :--- | ---: |`.
- `Architecture Neutrality`: Architecture docs describe current implementation truth without embedding release-number rhetoric.
- `Architecture Readability`: Keep each architecture-doc prose paragraph, list item, or table row independently addressable and at most 1024 characters; decompose mixed claims rather than hiding growth through hard line wrapping.
- `Dimensional Tables`: Use a compact table when multiple peer entities repeat the same dimensions or attributes; keep cells fact-dense and short, and place rationale, exceptions, or procedural flow outside the table instead of turning cells into prose paragraphs.
- `README Neutrality`: Entrypoint READMEs explain current purpose, setup, navigation, and validation; release history belongs in `CHANGELOG.md`.
- `Canonical Consolidation`: Merge extension specs into stronger canonical contracts when ownership converges; delete empty redirect stubs and route readers directly through the owning README or documentation index.
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

- `Human-Callable Script Classes`: Root numbered scripts run independently from any working directory, document inputs/outputs/side effects, validate prerequisites, and perform one operation; contiguous numbering follows the local-network evidence ladder from pinned node binaries through Cargo tools, runtime, ChainSpec, network, liveness, basic mutation, temporal consensus, and composed economics, while named scripts provide deterministic compositions or admin utilities without agent judgment. Remove or insert an atom only by reconciling the whole sequence, callers, help, Wiki, and clean-machine bootstrap ownership.
- `Script Language Boundary`: Use Bash for public root orchestration, process lifecycle, environment/toolchain control, and command composition; use JavaScript ES modules only for deterministic structural transformation or validation where JSON, metadata, graphs, exact integer handling, or testable data semantics would be unsafe or obscure in shell. Keep JavaScript support leaves behind the owning Bash or package entrypoint, never create a second orchestration layer, and do not add Python automation.
- `Skill/Script Placement`: Skills own strategy, interpretation, coordination, handoff, and their private capability validation; reusable deterministic project operations used by humans, packages, GitHub Actions, CI, or root compositions belong in root `/scripts`. Skills may invoke public project scripts, but project code, packages, workflows, root validation, and independent skills must not depend on another skill's internal scripts. Release-assurance dependency review, comparative Weight ledgers, and cross-system threat checklists remain private Skill evidence rather than project validation or global documentation.
- `Skill Deletability`: Removing `/.agents/skills` must leave project build, tests, packages, CI, release validation, and runtime behavior unchanged; Skill checks support development only and never become project acceptance dependencies.
- `Script Entrypoint Contract`: Full named/admin implementations follow `usage -> parse_args -> check_prerequisites/plan -> main` on `_common.sh`; every entrypoint exposes honest `--help`, and agent-specific leaves compose public root scripts when shared execution is required.
- `Compact Command Output`: Shared script-harness steps suppress successful child-command output by default, report concise timing, and retain full failure logs while printing a bounded tail; `DEOS_VERBOSE=1` restores live full output for diagnosis.
- `Canonical Validation Routing`: Owning skills declare the narrowest changed-scope route, exclusions, and escalation triggers through repository scripts; never default to full gates or build a shadow harness/raw command inventory, and extend the canonical route when it lacks precision.
- `Audit Ownership`: Project-specific agent audits live in the repo-local `alignment` skill and run only through that Skill; code/test/release validation remains project-owned and does not orchestrate Skill audits.
- `Skill Domain DAG`: Keep skills independently portable and acyclic: orchestration routes to documented capability contracts, never sibling internals; split only for a distinct owner, trigger, exclusion boundary, or reusable capability that reduces context/interface pressure, not for folder theater or file size.
- `Diff-Aware Gates`: Audits default to changed scope and reserve full-tree or network-backed checks for explicit release/all modes.
- `Durable Ledgers`: Record reusable hallucinations, ambiguities, dead ends, and boundary drifts only; bare tool failures remain transient output.
- `Wiki Role`: `/wiki` is an isolated strict OKF v0.2 bundle and concise, provenance-aware learning lens over canonical root and package-owned project truth, not a release-note mirror or docs dump; its root `index.md` declares the bundle while unrelated repository Markdown remains outside strict OKF scope.
- `Wiki Owner Naming`: Name each page after the semantic object it owns, not after its directory or implementation package. Before naming, identify whether the owner is a domain, concept, mechanism, concrete subsystem, or product surface; state what readers expect there; test whether the title survives an implementation replacement; and split subsystem from mechanism only when both have enough independent content. The `overview/` directory marks an entry role, not a mandatory `DEOS X` title pattern.
- `Wiki Locales`: Human pages use explicit locale suffixes and mirrored page IDs/topology; shared metadata represents localized fields; Russian prose prefers natural Russian terminology and minimizes English borrowings except canonical identifiers, code symbols, and terms whose translation would reduce precision.
- `Wiki Navigation`: Articles explain established facts and current capability boundaries, never backlog tasks, implementation plans, or unverified completion. Link to Wiki peers or concrete non-document resources, never to source documentation in the repository. Keep source-document provenance in non-rendered structured metadata, not article links or source-path panels. New pages need provenance, related links, locale mirrors, and graph/index reachability; merge weak leaflets into stronger owner pages.
- `Wiki Trust`: Treat repo-local wiki Markdown as reviewed content and enforce the trust boundary with the wiki validation skill.
- `Wiki Growth`: Large wiki expansion requires consolidation; provenance, graph reachability, and explicit page status must drive merge/remove work rather than decorative freshness dates or subjective scores.

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
- `Inspect and Scope`: Read implementation, call sites, tests, and git diff before mutation; make targeted changes and exclude unrelated cleanup unless the task explicitly includes consolidation.
- `Sync Backlog`: Close, narrow, split, retarget, or gate the canonical open-work item as reality changes.
- `Validate Locally`: Run focused checks through the owning route; Rust completion needs targeted checks/tests plus workspace Clippy with `-D warnings`.
- `Surface Validation`: Run simulator tests for math/invariant changes, relevant client checks for browser contracts, syntax/help/smoke checks for scripts, and trust/sync/consolidation checks for wiki changes.
- `Sync Docs`: Update specifications only when contracts change and architecture docs only when shipped implementation truth changes.
- `Sync Context`: Update `AGENTS.md` only for durable patterns, `BACKLOG.md` only for open work, and `CHANGELOG.md` only for completed outcomes.
- `Completion Gate`: After repository changes and knowledge sync, run `./.agents/skills/alignment/scripts/completion-gate.sh`; a failing gate means not done.
- `Garbage Collection`: Consolidate stale, duplicated, resolved, or over-detailed context whenever growth obscures the durable contract.
- `External Gates`: Do not publish, deploy, sign, submit, mutate accounts, or cross destructive/approval boundaries without explicit user authorization.
- `Done`: Report changed paths, validation evidence, remaining gates, and exact unblockers concisely.
