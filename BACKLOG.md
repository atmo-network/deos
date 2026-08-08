# DEOS Backlog

> This file contains unfinished implementation work only.
>
> Normative runtime meaning belongs to pallet specifications. Release procedure, review order, churn budgets, evidence identity, merge/tag rules, and acceptance choreography belong to `docs/release-protocol.en.md`. Completed work belongs to `CHANGELOG.md`.

## DEOS 0.7.13 — Router Route Truth Closure

> Normative Router meaning must move into `template/pallets/router/docs/specification.en.md`. This backlog lists unfinished implementation surfaces and must not become the Router specification.

### Router Specification

- [ ] `Router / Canonical Specification`: Write and accept a dry Router runtime specification covering public types, supported route families, intents, fee semantics, selection, protection, Oracle publication, transaction boundaries, outcomes, errors, Weight classes, adapters, storage, and conformance.
- [ ] `Router / Backlog Contraction`: After specification acceptance, replace every semantic description below with section references and retain only implementation deltas.

### Canonical Route Representation

- [ ] `Router / PreparedRoute`: Implement one bounded internal route value shared by quote projection, transactional preparation, validation, protection, execution, events, outcomes, Oracle publication, AAA integration, and Weight classification.
- [ ] `Router / Bounded Geometry`: Replace unbounded route/path representation and transient path allocation with bounded route and leg types.
- [ ] `Router / Fresh Execution Truth`: Treat external quotes as projections only; prepare executable truth from current state inside the execution transaction.
- [ ] `Router / Deterministic Selection`: Replace insertion-order behavior with the total comparator defined by the accepted specification and add candidate-order permutation tests.

### Protection, Publication, and Atomicity

- [ ] `Router / Intent Protection`: Implement the accepted exact-input output floor and exact-output total-input ceiling without synthetic route semantics.
- [ ] `Router / Per-Leg Reference Checks`: Apply reference checks only to actual executed XYK legs and keep Router protection separate from AAA System policy.
- [ ] `Router / Actual-Leg Oracle Publication`: Publish exactly the executed XYK legs in canonical execution order; direct TMC mint publishes no XYK observation.
- [ ] `Router / Atomic Execution`: Keep fee routing, Oracle publication, AAA ingress, liquidity mutation, balance deltas, and Router events inside one rollback boundary.
- [ ] `Router / Rollback Matrix`: Cover every fee, publication, ingress, pool, TMC, balance, and event failure point.

### Outcomes, Errors, and AAA Boundary

- [ ] `Router / Canonical Outcome`: Return one bounded structured outcome whose economic fields retain identical meaning across quote projection, execution, events, and clients.
- [ ] `Router / Failure Taxonomy`: Expose stable exhaustive Router failure classes usable by AAA Temporary/Permanent mapping; unknown remains Permanent.
- [ ] `Router / AAA Adapter Contraction`: Remove quote ownership, path validation, Router protection, and replanning duplicated in `TmctolDexOps`; AAA supplies only its authored policy inputs and consumes the canonical Router outcome.
- [ ] `Router / Route Weight Classes`: Bind every supported prepared route and outcome to one measured route class; make AAA admission cover the maximum class permitted by its swap task.
- [ ] `Router / Compatibility Identity`: Decide and apply either explicit retention of `pallet_axial_router` identity or one complete pre-launch rename; add no partial alias layer.

### Hot-Path and Storage Contraction

- [ ] `Router / Duplicate Work Removal`: Delete repeated quotes, pool lookups, route preparation, path allocation, synthetic direct-pair reads, and parallel validation where `PreparedRoute` already owns the fact.
- [ ] `Router / Primitive API Boundary`: Make low-level pool quote helpers private or name them explicitly as single-pool primitives.
- [ ] `Router / Storage Invariants`: Strengthen `try_state` for fee policy, canonical LP-pair ordering, reverse-index consistency, and LP-token collision freedom.

### Executable Conformance

- [ ] `Router / Route Vectors`: Generate conformance vectors for every supported route family, intent, protection boundary, tie, publication set/order, outcome, error class, and Weight class.
- [ ] `Router / Adversarial Corpus`: Cover stale quote projection, candidate permutations, first/later publication failure, AAA ingress rejection, fee failure, direct XYK failure, TMC failure, Native-anchored leg failure, exact-input recipient delta, and exact-output total spend.
- [ ] `Router / Cross-Domain Invariant`: Prove `projected route = prepared route = protected route = executed route = event route = outcome route = Weight class`.

### Generated Surfaces and Documentation

- [ ] `Router / Production Weights`: Benchmark every accepted route class and regenerate Router plus affected AAA/Oracle weights.
- [ ] `Router / Metadata and Clients`: Regenerate runtime metadata, descriptors, client projections, and route evidence from the accepted ABI.
- [ ] `Router / Public Projection Sync`: Align Router package docs, runtime APIs, AAA/Oracle integration, web-client docs, and wiki projections with the Router specification.

### 0.7.13 Exit State

- [ ] Router runtime and generated artifacts conform to the accepted Router specification.
- [ ] One bounded `PreparedRoute` owns route truth from preparation through outcome.
- [ ] Oracle publication equals the exact ordered executed XYK legs.
- [ ] AAA duplicates no Router discovery, quote, route, or protection logic.
- [ ] Every supported route has one measured Weight class and complete rollback coverage.
- [ ] No new route family ships beyond the accepted specification.

### 0.7.13 Non-Goals

- No arbitrary graph routing, unrestricted paths, external DEX aggregation, intent marketplace, solver competition, CoW/frequent-batch settlement, or new market family.
- No new AAA task, condition, authority, scheduler, retry, or history surface except adapter synchronization required by the accepted Router contract.

## Deferred AAA Possibilities

- [~] `Batch Settlement`: Consider intent or frequent-batch settlement with one clearing rule only as a DEX-level later design after the `0.7.6` service and loss envelopes close; scheduler priority alone does not remove order-based extraction.
- [~] `Probabilistic Trigger Extension`: Consider probability only as a future append-only progressive trigger extension after a concrete deterministic and financially secure entropy capability exists, has an owned runtime ingress/security model, and carries production ProofSize/Weight evidence; `0.7.2` contract contraction does not permanently reject the capability.
- [~] `Immutable Continuation`: Consider `RetryLater` for Immutable actors only after a concrete constitutional need defines non-intervention, cancellation, permanent adapter failure, terminal handling, and upgrade semantics beyond the validated baseline.
- [~] `AAA 1.0 Declaration Gate`: Consider the append-only `1.0` line only after maintainers explicitly choose a stability declaration using the completed `0.7.3` independent-runtime evidence; any newly discovered breaking correction must revise the pre-`1.0` candidate and repeat the gate.

## Product / Client Work

### Wallet and portfolio boundary

- [~] `Wallet Portfolio Boundary`: Any expansion to a full portfolio UX remains blocked until a materialized/indexed asset-discovery surface exists beyond live chain storage.

### Web-client product stabilization

- [ ] `Reserved Edge-Lane Growth`: Only if product pressure creates another reserved left/right lane, define the concrete lane role and extend `RESERVED_LANE_SPECS` without reintroducing user-reorderable edge-lane state.
- [ ] `Governance State Separation`: Only if proposal composition or archive work grows enough to create a named ownership conflict, split the state boundary at that concrete seam.
- [ ] `Materialized Provider Boundary`: Only when a second indexed/archive provider family exists, decide whether `adapters/materialized-history/` should become a first-class `materialized/` or `providers/` slice.

## Runtime Framework Evolution

> These slices keep DEOS current with useful Polkadot SDK runtime patterns while preserving the framework boundary: adopt configuration discipline, reusable primitives, and economic mechanisms; do not import unrelated product layers such as Revive contracts by default.
>
> Source context for agents beyond their training cutoff: Polkadot SDK `stable2606` release notes.

- [ ] `Runtime Cadence Profile`: Define a cadence profile contract that derives time-sensitive runtime constants from a configurable block-duration target instead of hardcoding one block speed. Audit voting periods, AAA cooldowns/retry windows, staking epochs, cleanup windows, and docs for assumptions that break between conventional ~6s blocks and faster sub-second profiles.
- [ ] `V3 Scheduling / Block-Bundling Readiness`: Document and encode a non-enabled readiness profile for future V3 scheduling/block-bundling adoption, including runtime/operator prerequisites, benchmark margins, `on_idle`/hook pressure, message-queue/XCM budgets, and activation conditions.
- [ ] `DEOS Staking Reward Source Abstraction`: Separate staking distribution from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow.
- [ ] `Budget Recipient Primitives`: Introduce typed budget-recipient primitives or runtime helpers for framework-owned economic destinations such as staking reward pots, governance treasuries, liquidity reserves, and System AAA actors.
- [ ] `Unclaimed Reward Policy`: Make staking/native reward leftovers explicit runtime policy: rollover, Fee Sink return, burn, or treasury routing.

## Collator Economics & Fee Routing

> Phase 1 uses trusted permissioned collators, collects 100% of transaction, AAA, governance-opening, and XCM-execution fees in the Fee Sink, and distributes available native balance 50/50 into staking ingress and liquidity provisioning.
>
> A future permissionless phase may introduce equal security/staking/liquidity thirds only after bounded security-reward settlement ships; indivisible remainder stays in Fee Sink.

- [ ] `Permissionless Collator Reward Contract`: Before assigning the future security branch, define bounded active-set eligibility, contribution attribution, settlement cadence, custody, payout recipients, unclaimed leftovers, failure behavior, and read-model surfaces.
- [~] `Phase 2 Reward Routing Preparation`: Keep Phase 2 as a runtime-upgrade boundary, not a launch-time parameter.
  - [~] `Claimable LP Nomination Flow`: Activate explicit LP-nomination reward-weight provider only when permissionless collators ship.
  - [ ] `LP Nomination Activation`: Expose LP-point nomination to specific collators only when permissionless collator selection is enabled.

## Conditional / Externally Gated Work

### Governance execution expansion policy

> Only actionable when a concrete domain-owned control surface, payload family, or failure-state slice is selected beyond the current baseline.

- [ ] `L2 Parameter Expansion`: Only after a genuinely delegated/domain-owned parameter surface exists, add the next `L2ParameterChange` path beyond the Router pair.
- [ ] `Execution Observability Expansion`: Only when a new payload family or failure-state slice ships, broaden per-kind observability beyond current bounded detail/events.
- [ ] `Browser Composition Expansion`: Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next composition surface.
- [ ] `Governance Archive Integration`: Only when a materialized/indexed governance backend is selected, connect the reserved archive boundary to live archive search and ballot timelines.

### Block reward source policy

> Only actionable when the launch economy selects a concrete block subsidy/issuance source instead of assuming one exists.

- [ ] `Block Subsidy Activation`: Only after the reference economy defines a concrete block-reward source and amount policy, decide whether issuance enters the Fee Sink or the future security-reward budget; do not revive immediate author payout by default.

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Native DEOS Staking Acquisition Routes`: Only if pool-ratio divergence makes deterministic acquisition insufficient, add Router quote comparison, slippage bounds, and fallback behavior.

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Relay-Beacon Replacement Contract`: Only if a new parachain-consumable per-block protocol beacon exists, define the replacement contract against that actual surface.
- [ ] `Relay-Beacon Proof Ingestion`: Only if that future per-block beacon exists, design a Weight-accounted `ConsensusHook` snapshot finalized against the real upstream surface.
- [ ] `AAA Relay-Beacon Integration`: Only if that future per-block beacon exists, wire AAA to it and measure ProofSize/Weight impact.
- [ ] `Permissionless Collator Activation`: Only after a production-ready per-block relay/protocol beacon exists, design and prototype activation instead of reviving a local threshold line.
