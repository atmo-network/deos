# DEOS Backlog

> Open framework work only; durable protocol lives in `AGENTS.md`, and completed delivery history lives in `CHANGELOG.md`.
>
> Release boundary: `DEOS 0.7.3 — Progress-Preserving AAA Continuation` is the validated current framework line, including the independently consumable AAA package boundary. Completed semantics and evidence live in `CHANGELOG.md` and the owning AAA specification, package, and architecture documents.

## DEOS 0.7.4 — Verifiable Step Composition

> Release thesis: Complete AAA as a configurable, verifiable straight-line economic pipeline by making ordered step conditions, typed economic tasks and parameters, and failure policy explicit, analyzable, authorable, and mechanically protected without expanding consensus control-flow power.
>
> Hard constraints: Add only the authorized fieldless `Task::StopCycle` task variant and the package-owned non-nested `ConditionSet::{Always, All, Any}` aggregate; add no other `Task`, atomic `Condition`, `AmountResolution`, or `StepErrorPolicy` variant. `StopCycle` may complete the current logical run successfully but may not pause, cancel, close, deactivate, reschedule, or mutate the actor. A `ConditionSet` may only admit or skip its current step; it may not select a successor, task, adapter, parameter, or error policy. Add no nested groups, negation, threshold/XOR logic, branch, jump, loop, recursion, callback, nested program, generic `RuntimeCall`, second scheduler/queue/inbox/checkpoint/bitmap, graph authoring model, product-specific pallet policy, or consensus storage/runtime API without a demonstrated correctness necessity. Keep Noita as an off-chain design heuristic only.
>
> Frozen `0.7.3` comparison identity at commit `b3eb031`: SHA-256 production AAA weights `2c74e92a46727fa5ed359c9cdce14296885b22446be415fa0f0bd252ede18f38`; SHA-256 compressed runtime Wasm `b858a8747ead30a5e2cda21f174d77f56ef7e9c1e063241682084fbe605717c9`; `blake2_256` PAPI metadata `0x6d5e5c108b13aa128ed5ebfdab748c47f5c70044fda7e6e1838ab9db194dbca6`; runtime identity `(authoring=1, impl=1, system=3, spec=1, transaction=1)`. The unchanged full `./scripts/aaa-release-gate.sh` baseline, including the 10K occupancy profile, passed in 224 seconds on 2026-07-24. Recompute and compare these pins only when their owning artifacts change.
>
> Baseline ownership inventory: `template/pallets/aaa/src/types.rs` defines atomic `Condition`, `ConditionSet`, `Task` plus typed parameters, `AmountResolution`, `StepErrorPolicy`, `Step`, and `ProgramInput`; `template/pallets/aaa/src/contract.rs` exhaustively classifies every primitive; `template/pallets/aaa/src/lib.rs` owns admission and shape validation; `template/pallets/aaa/src/execution.rs` owns condition evaluation, amount resolution, task preparation/execution, adapter failure classification consumption, policy interpretation, cursor progression, Continuation state transitions, and the production executor reused by rollback-only runtime simulation; runtime adapters classify `TaskFailure` in `template/runtime/src/configs/aaa_config.rs`; `template/runtime/src/apis.rs` exposes matching-runtime simulation; `web-client/src/lib/automation/plan-artifact.ts`, `forecast.ts`, `analysis.ts`, `simulation.ts`, `matching-wasm.ts`, and `runtime-simulation-codec.ts` own metadata-bound projection, local amount/weight models, per-cursor structural analysis, adapter-local simulation, and matching-runtime verification. `docs/aaa.specification.en.md`, `docs/aaa.architecture.en.md`, and `docs/aaa-control-plane.contract.en.md` own normative, shipped, and off-chain contracts respectively.
>
> Transition ownership: Production and rollback-only matching-runtime simulation share `execute_single_cycle_traced` and the private exhaustive `resolve_step_control`; the adapter-local TypeScript simulation remains a separately proven projection rather than runtime truth.


## Post-0.7.3 AAA Possibilities

- [~] `Probabilistic Trigger Extension`: Consider probability only as a future append-only progressive trigger extension after a concrete deterministic and financially secure entropy capability exists, has an owned runtime ingress/security model, and carries production ProofSize/weight evidence; `0.7.2` contract contraction does not permanently reject the capability.
- [~] `Immutable Continuation`: Consider `RetryLater` for Immutable actors only after a concrete constitutional need defines non-intervention, cancellation, permanent adapter failure, terminal handling, and upgrade semantics beyond the validated Mutable-only baseline.
- [~] `AAA 1.0 Declaration Gate`: Consider the append-only `1.0` line only after maintainers explicitly choose a stability declaration using the completed `0.7.3` independent-runtime evidence; any newly discovered breaking correction must revise the pre-`1.0` candidate and repeat the gate.

## AAA Control-Plane Tooling

> `docs/aaa-control-plane.contract.en.md` owns the accepted off-chain artifact, provenance, simulation, governance-composition, and history boundary. Tooling must not change AAA consensus semantics or expand on-chain history.

- [~] `AAA Control Plane / Indexed History`: After a materialized backend is selected, correlate finalized actor configuration, cycle, Continuation, and funding events with available artifacts under explicit replay/reorg and missing-artifact semantics.

## Product / Client Work

### Wallet and portfolio boundary

- [~] `Wallet Portfolio Boundary`: Any expansion to a full portfolio UX remains blocked until a materialized/indexed asset-discovery surface exists beyond live chain storage

### Web-client product stabilization

- [ ] `Reserved Edge-Lane Growth`: Only if product pressure creates another reserved left/right lane, define the concrete lane role and extend `RESERVED_LANE_SPECS` without reintroducing user-reorderable edge-lane state.
- [ ] `Governance State Separation`: Only if proposal composition or archive work grows enough to create a named ownership conflict, split the state boundary at that concrete seam.
- [ ] `Materialized Provider Boundary`: Only when a second indexed/archive provider family exists, decide whether `adapters/materialized-history/` should become a first-class `materialized/` or `providers/` slice.

## Runtime Framework Evolution

> These slices keep DEOS current with useful Polkadot SDK runtime patterns while preserving the framework boundary: adopt configuration discipline, reusable primitives, and economic mechanisms; do not import unrelated product layers such as Revive contracts by default.
> Source context for agents beyond their training cutoff: Polkadot SDK `stable2606` release notes — <https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2606>.

- [ ] `Runtime Cadence Profile`: Define a cadence profile contract that derives time-sensitive runtime constants from a configurable block-duration target instead of hardcoding one block speed. Exit criteria: audit voting periods, AAA cooldowns/retry windows, staking epochs, cleanup windows, and docs for assumptions that would break when moving between conventional ~6s blocks and faster sub-second / ~500ms profiles; add a validation guard for new block-count assumptions where practical.
- [ ] `V3 Scheduling / Block-Bundling Readiness`: Document and encode a non-enabled readiness profile for future V3 scheduling / block-bundling adoption. Exit criteria: list runtime/operator prerequisites, benchmark and block-weight margin checks, `on_idle` / hook pressure review, message-queue/XCM budget considerations, and a clear condition for moving from legacy scheduling to V3-ready or V3-enabled.
- [ ] `Staking Reward Source Abstraction`: Evolve staking reward ingress so distribution logic is separated from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow. Exit criteria: specify and prototype a minimal runtime/pallet interface for `ExternallyFundedPot`-style reward sources, epoch snapshot timing, pot denominator fixing, and compatibility with current auto-compound claim flows.
- [ ] `Budget Recipient Primitives`: Introduce typed budget-recipient primitives or runtime helpers for framework-owned economic destinations such as staking reward pots, governance treasuries, liquidity reserves, and System AAA actors. Exit criteria: replace any new raw-account economic routing in touched surfaces with typed recipient derivation and decide whether a future mutable registry pallet is justified or overkill.
- [ ] `Unclaimed Reward Policy`: Make staking/native reward leftovers explicit runtime policy instead of implicit residue. Exit criteria: define rollover / return-to-Fee-Sink / burn / treasury-routing options, choose the current reference policy, and cover expiry or settlement behavior with tests.

## Collator Economics & Fee Routing

> Phase 1 uses trusted, permissioned collators, collects 100% of transaction, AAA, governance-opening, and XCM-execution fees in the Fee Sink, and distributes available native balance 50/50 into staking ingress and liquidity provisioning.
> A future permissionless phase may introduce equal security/staking/liquidity thirds only after bounded security-reward settlement ships; indivisible remainder stays in Fee Sink for a later cycle.

- [ ] `Permissionless Collator Reward Contract`: Before assigning the future equal-third security branch, define bounded active-set eligibility, contribution attribution, settlement cadence, custody, payout recipients, unclaimed leftovers, failure behavior, and read-model surfaces; do not assume that a `CollatorRewardPot` account or pallet is the final topology.
- [~] `Phase 2 Reward Routing Preparation`: Keep Phase 2 as a runtime-upgrade boundary, not a launch-time parameter
  - [~] `Claimable LP Nomination Flow`: Activate explicit LP-nomination reward-weight provider only when permissionless collators ship
  - [ ] `LP Nomination Activation`: Expose LP-point nomination to specific collators only when permissionless collator selection is enabled

## Conditional / Externally Gated Work

### Governance execution expansion policy

> Only actionable when a concrete domain-owned control surface, payload family, or failure-state slice is selected beyond the current baseline.

- [ ] `L2 Parameter Expansion`: Only after a genuinely delegated/domain-owned parameter surface exists, add the next `L2ParameterChange` path beyond the Axial Router pair
- [ ] `Execution Observability Expansion`: Only when a new payload family or failure-state slice ships, broaden per-kind observability beyond the current bounded detail/events
- [ ] `Browser Composition Expansion`: Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next composition surface
- [ ] `Governance Archive Integration`: Only when a materialized/indexed governance backend is selected, connect the reserved archive boundary to live archive search and ballot timelines

### Block reward source policy

> Only actionable when the launch economy selects a concrete block subsidy / issuance source instead of assuming one exists.

- [ ] `Block Subsidy Activation`: Only after the reference economy defines a concrete block-reward source and amount policy, decide whether issuance enters the Fee Sink or the future security-reward budget; do not revive immediate author payout by default.

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Native Staking Acquisition Routes`: Only if pool-ratio divergence makes deterministic acquisition insufficient, add router quote comparison, slippage bounds, and fallback behavior

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Relay-Beacon Replacement Contract`: Only if a new parachain-consumable per-block protocol beacon exists, define the replacement contract against that actual surface
- [ ] `Relay-Beacon Proof Ingestion`: Only if that future per-block beacon exists, design a weight-accounted `ConsensusHook` snapshot finalized against the real upstream surface
- [ ] `AAA Relay-Beacon Integration`: Only if that future per-block beacon exists, wire AAA to it and measure proof-size and weight impact
- [ ] `Permissionless Collator Activation`: Only after a production-ready per-block relay/protocol beacon exists, design and prototype activation instead of reviving a local threshold line
