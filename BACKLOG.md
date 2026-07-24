# DEOS Backlog

> Open framework work only; durable protocol lives in `AGENTS.md`, and completed delivery history lives in `CHANGELOG.md`.
>
> Release boundary: `DEOS 0.7.3 — Progress-Preserving AAA Continuation` is the validated current framework line, including the independently consumable AAA package boundary. Completed semantics and evidence live in `CHANGELOG.md` and the owning AAA specification, package, and architecture documents.

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

- [ ] `Governance State Separation`: Only if proposal composition or archive work grows enough to create a named ownership conflict, split the state boundary at that concrete seam.

### Reference indexer and integrated deployment

> Planned for a post-`0.8` release line; choose the exact milestone only when scope and delivery evidence support it. Subsquid is a candidate implementation stack, not a committed dependency.

- [ ] `Reference Indexer Architecture`: Select and document the indexer stack, top-level `/indexer` workspace boundary, canonical-chain ingestion model, materialized schema ownership, bounded replay/recovery behavior, provider APIs, deployment topology, observability, and upgrade compatibility without moving archive/search state into consensus storage.
- [ ] `Reference Indexer Service`: Add the configured reference indexer under top-level `/indexer` with reproducible schema generation, deterministic local fixtures, chain synchronization, migrations, health/readiness surfaces, archive/search queries, tests, and production-oriented configuration.
- [ ] `Materialized Read-Model Delivery`: Implement the first indexed surfaces needed by wallet asset discovery, governance history, activity/receipt history, and bounded analytics while preserving explicit canonical-chain versus materialized provenance in every public response.
- [ ] `Web-Client Indexer Integration`: Promote the selected materialized provider boundary into a first-class frontend slice, connect indexed wallet/governance/history experiences with stale/error/unavailable states, and keep direct chain reads authoritative for canonical current state.
- [ ] `Integrated Stack Deployment`: Extend repository-owned local and production deployment flows so node/runtime dependencies, indexer storage, indexer service, and web client can start from one documented configuration with health ordering, persistent volumes, migrations, backup/restore guidance, and end-to-end smoke validation.

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
