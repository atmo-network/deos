# DEOS Backlog

> Open framework work only; durable protocol lives in `AGENTS.md`, and completed delivery history lives in `CHANGELOG.md`.
> Completed delivery history lives in `CHANGELOG.md`; unreleased correction and release gates remain open here until their annotated tag exists.

## DEOS 0.7.11 — Router Route Truth Closure

### Canonical Prepared Route and Execution Truth

- [ ] `DEOS Router / Canonical Route Contract`: Define and implement one internal bounded `PreparedRoute` ontology used by exact-input and exact-output quote projection, route validation, per-leg protection, Oracle publication, execution, structured outcome, events, AAA integration, and weight classification; preserve only direct XYK, exact-input direct TMC mint, and Native-anchored two-leg XYK.
  - [ ] `DEOS Router / Bounded Representation`: Model intent, gross input, router fee, route input, recipient output, mechanism, bounded path, explicit bounded legs, protection, and finite weight class in one prepared value. Replace runtime route geometry based on unbounded `Vec` with bounded path/leg types and represent direct XYK, Native-anchored XYK, and direct TMC mint legs explicitly.
  - [ ] `DEOS Router / Planning Semantics`: Permit intent-specific quote algorithms but require both intents to converge to the same route ontology. Keep direct TMC mint exact-input only until TMC exposes a genuine inverse execution contract. Treat external quotes as current-state projections only; execution must prepare a fresh route against current state inside the transaction and must not accept arbitrary stale caller-supplied plans.
  - [ ] `DEOS Router / Deterministic Comparator`: Replace insertion-order selection with an explicit total comparator. Exact input orders by maximum recipient output, then minimum route cost/leg count, canonical mechanism order, and canonical encoded path; exact output orders by minimum total required input, then the same three tie-break dimensions. Add candidate-order permutation tests proving construction order cannot change selection.
  - [ ] `DEOS Router / Protection Ontology`: Make exact-input protection one minimum recipient-output bound and exact-output protection one maximum total-input bound. Evaluate reference deviation only on actual XYK legs, using each leg's directional observation and the same per-leg ontology for both intents; remove synthetic direct-pair reads for multi-hop and synthetic XYK guards for TMC mint.
  - [ ] `DEOS Router / Protection Claims`: Keep slippage, per-leg reference deviation, and System-authored policy visibly separate and document that local reference deviation proves neither external fair price, ordering protection, manipulation resistance, nor MEV protection.
  - [ ] `DEOS Router / Actual-Leg Oracle Publication`: Publish pre-execution observations in canonical leg order for exactly the executed XYK legs: direct `A → B` publishes `A → B`, `A → Native → B` publishes both directional legs in path order, and direct TMC mint publishes none. Remove request-pair and synthetic direct-pair publication.
  - [ ] `DEOS Router / Atomic Operation`: Keep fee routing, every applicable Oracle publication, AAA dirty ingress, liquidity execution, balance deltas, and Router events in one transaction; prove any fee, per-leg publication, AAA ingress, or liquidity failure restores Oracle, AAA, Router, balances, pools, and events to pre-state.
  - [ ] `DEOS Router / Canonical Outcome`: Return a structured bounded route execution outcome with intent kind, gross input, router fee, actual route input spent, recipient output, mechanism, bounded path, and weight class. Exact output must expose actual total input spent; exact input must expose the actual recipient balance delta. Quote, outcome, and event fields must retain identical economic meanings, with no redundant per-leg Router events when liquidity pallets already emit execution evidence.
  - [ ] `DEOS Router / AAA Adapter Boundary`: Remove quote, route/path validation, Router reference protection, and re-planning logic duplicated by `TmctolDexOps`. AAA may supply only System-specific authored slippage, reference-deviation bound, freshness requirement, and retry/failure policy; Router owns discovery, fee math, preparation, validation, execution, and canonical outcome through a safe single-transaction API that cannot execute externally stale plans.
  - [ ] `DEOS Router / Failure Taxonomy`: Expose stable Router error classes suitable for exhaustive AAA Temporary/Permanent classification, preserve unknown failures as Permanent, and test each accepted error variant. Remove genuinely unused variants such as any unowned route/liquidity error, or add a falsifying path that proves its ownership.
  - [ ] `DEOS Router / Weight Classes`: Define the finite classes `ExactInputDirectXyk`, `ExactInputDirectTmc`, `ExactInputMultiHopNative`, `ExactOutputDirectXyk`, and `ExactOutputMultiHopNative`; bind each prepared route and execution outcome to exactly one class and make AAA task admission cover the maximum class permitted by its swap task.
  - [ ] `DEOS Router / Hot-Path Cleanup`: After canonicalization, eliminate duplicate quotes, pool lookups, route preparation, path allocation, synthetic direct-pair reads, and other repeated work where one prepared value can safely carry the verified fact through the transaction; measure before retaining any additional indirection.
  - [ ] `DEOS Router / API and Storage Invariants`: Make low-level `quote_price` private or rename it as an explicitly one-pool XYK primitive so it cannot compete with canonical Router quote truth. Strengthen `try_state` to validate bounded fee policy, canonical LP-pair ordering, reverse-index consistency, and LP-token collision freedom; add registration and corruption tests.
  - [ ] `DEOS Router / Compatibility Identity Decision`: Explicitly retain `pallet_axial_router` as a documented stable Rust/runtime compatibility identity or perform a deliberate full migration across crate, runtime, metadata, tests, weights, client, docs, and audits; reject another partial rename.

### Adversarial Route Corpus and Invariants

- [ ] `DEOS Router / Contract Validation`: Add package and runtime matrices for every route family and intent, protection boundary, tie, actual-leg publication set/order, structured outcome/event equality, AAA adapter use, rollback point, maximum bounded path, and absence of any new route family. Exit invariant: `quote projection = prepared route = protected route = executed route = event route = weight class`.
  - [ ] `DEOS Router / Adversarial Route Scenarios`: Extend the accepted reactive corpus format with Router-triggered Oracle publication followed by swap failure; first- and later-leg publication rejection; fee and AAA-ingress rejection; direct XYK, TMC, and Native-anchored liquidity failure; stale external quote followed by fresh transactional preparation; candidate-order permutations; exact-input recipient-delta and exact-output total-spend checks; and maximum route bounds.
  - [ ] `DEOS Router / Atomicity Invariants`: Assert Oracle publications equal the exact ordered set of executed XYK legs; multi-hop never publishes a synthetic direct pair; TMC mint publishes no XYK observation; any fee, publication, AAA ingress, or execution failure restores Router, Oracle, AAA, balances, pools, and events; and every accepted outcome names one measured route class.

### Production Evidence, Documentation, and Release Acceptance

- [ ] `DEOS 0.7.11 / Production Benchmarks`: Benchmark all five Router route classes under production Wasm with accepted steps/repeats and execution mode, regenerate Router and affected AAA/Oracle weights, return actual post-dispatch weight where route-class dispatch permits it, and prove AAA admission covers its maximum permitted route class.
  - [ ] `DEOS 0.7.11 / Canonical Evidence`: Define the 0.7.11 evidence owner and bind every accepted Router route-class weight identity, affected AAA/Oracle weight identity, exact runtime/metadata/Wasm/client evidence, benchmark parameters, and candidate commit without independent accepted hashes.
  - [ ] `DEOS 0.7.11 / Documentation Sync`: Synchronize Router README/package architecture/runtime APIs, AAA and Oracle integration, AAA control-plane and client route projections, benchmark evidence, compatibility identity, wiki projections where owners changed, `CHANGELOG.md`, and canonical release evidence.
  - [ ] `DEOS 0.7.11 / Validation`: Pass Router package/runtime matrices, adversarial route corpus, AAA adapter and admission tests, Oracle publication/rollback tests, production benchmark freshness, generated weights, try-runtime/`try_state`, package/workspace tests, Clippy with `-D warnings`, client/control-plane tests, documentation checks, completion gate, and release-line audit.
  - [ ] `DEOS 0.7.11 / Definition of Done`: Close only when `quote projection = prepared route = protected route = executed route = event route = weight class`; Oracle publications match exactly the executed XYK legs; both intents share one route ontology; AAA duplicates no Router route logic; all route failures roll back atomically; tie-breaking ignores insertion order; every route class has production evidence; one canonical evidence owner binds release identity; and no new route family ships.

### DEOS 0.7.11 Non-Goals

- No arbitrary graph routing, unrestricted path lengths, external DEX aggregation, generalized intent marketplace, CoW or frequent-batch settlement inside Router, solver competition, or new market family.
- No new AAA task, condition, authority, scheduler, retry, or history surface beyond adapter and admission synchronization required by canonical Router outcomes.

## Deferred AAA Possibilities

- [~] `Batch Settlement`: Consider intent or frequent-batch settlement with one clearing rule only as a DEX-level later design after the `0.7.6` service and loss envelopes close; scheduler priority alone does not remove order-based extraction.
- [~] `Probabilistic Trigger Extension`: Consider probability only as a future append-only progressive trigger extension after a concrete deterministic and financially secure entropy capability exists, has an owned runtime ingress/security model, and carries production ProofSize/weight evidence; `0.7.2` contract contraction does not permanently reject the capability.
- [~] `Immutable Continuation`: Consider `RetryLater` for Immutable actors only after a concrete constitutional need defines non-intervention, cancellation, permanent adapter failure, terminal handling, and upgrade semantics beyond the validated Mutable-only baseline.
- [~] `AAA 1.0 Declaration Gate`: Consider the append-only `1.0` line only after maintainers explicitly choose a stability declaration using the completed `0.7.3` independent-runtime evidence; any newly discovered breaking correction must revise the pre-`1.0` candidate and repeat the gate.

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
- [ ] `DEOS Staking Reward Source Abstraction`: Evolve staking reward ingress so distribution logic is separated from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow. Exit criteria: specify and prototype a minimal runtime/pallet interface for `ExternallyFundedPot`-style reward sources, epoch snapshot timing, pot denominator fixing, and compatibility with current auto-compound claim flows.
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

- [ ] `L2 Parameter Expansion`: Only after a genuinely delegated/domain-owned parameter surface exists, add the next `L2ParameterChange` path beyond the DEOS Router pair
- [ ] `Execution Observability Expansion`: Only when a new payload family or failure-state slice ships, broaden per-kind observability beyond the current bounded detail/events
- [ ] `Browser Composition Expansion`: Only when runtime-signed submission authority expands beyond advisory plus tactical treasury invoices, add the next composition surface
- [ ] `Governance Archive Integration`: Only when a materialized/indexed governance backend is selected, connect the reserved archive boundary to live archive search and ballot timelines

### Block reward source policy

> Only actionable when the launch economy selects a concrete block subsidy / issuance source instead of assuming one exists.

- [ ] `Block Subsidy Activation`: Only after the reference economy defines a concrete block-reward source and amount policy, decide whether issuance enters the Fee Sink or the future security-reward budget; do not revive immediate author payout by default.

### Native staking LP donation route policy

> Only actionable if AAA policy needs route choice beyond deterministic `$NTVE -> stNTVE` stake acquisition.

- [ ] `Native DEOS Staking Acquisition Routes`: Only if pool-ratio divergence makes deterministic acquisition insufficient, add router quote comparison, slippage bounds, and fallback behavior

### Relay-beacon replacement path

> Only actionable if a real parachain-consumable per-block beacon appears upstream.

- [ ] `Relay-Beacon Replacement Contract`: Only if a new parachain-consumable per-block protocol beacon exists, define the replacement contract against that actual surface
- [ ] `Relay-Beacon Proof Ingestion`: Only if that future per-block beacon exists, design a weight-accounted `ConsensusHook` snapshot finalized against the real upstream surface
- [ ] `AAA Relay-Beacon Integration`: Only if that future per-block beacon exists, wire AAA to it and measure proof-size and weight impact
- [ ] `Permissionless Collator Activation`: Only after a production-ready per-block relay/protocol beacon exists, design and prototype activation instead of reviving a local threshold line
