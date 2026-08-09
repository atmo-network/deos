# DEOS Backlog

> This file contains admitted unfinished implementation work only.
>
> Normative runtime meaning belongs to subsystem specifications. Release procedure, review order, churn accounting, evidence identity, merge/tag rules, and acceptance choreography belong to `docs/release-protocol.en.md`. Completed work belongs to `CHANGELOG.md`.

## DEOS 0.7.14 — Router Route Truth Closure

> 0.7.14 starts only after the 0.7.13 Actors identity and surface closure passes its complete release gate.
>
> Normative Router meaning belongs to `template/pallets/router/docs/specification.en.md`. This backlog contains unfinished implementation obligations and must not become the Router specification.

### Router Specification

- [ ] `Router / Canonical Specification`: Write and accept a dry Router runtime specification covering public types, supported route families, intents, fee semantics, selection, protection, Oracle publication, transaction boundaries, outcomes, errors, Weight classes, adapters, storage, and conformance.
- [ ] `Router / Backlog Contraction`: After specification acceptance, replace every semantic description below with section references and retain only implementation deltas.

### Canonical Route Representation

- [ ] `Router / PreparedRoute`: Implement one bounded internal route value shared by quote projection, transactional preparation, validation, protection, execution, events, outcomes, Oracle publication, Actors integration, and Weight classification.
- [ ] `Router / Bounded Geometry`: Replace unbounded route/path representation and transient path allocation with bounded route and leg types.
- [ ] `Router / Fresh Execution Truth`: Treat external quotes as projections only; prepare executable truth from current state inside the execution transaction.
- [ ] `Router / Deterministic Selection`: Replace insertion-order behavior with the total comparator defined by the accepted specification and add candidate-order permutation tests.

### Protection, Publication, and Atomicity

- [ ] `Router / Intent Protection`: Implement the accepted exact-input output floor and exact-output total-input ceiling without synthetic route semantics.
- [ ] `Router / Per-Leg Reference Checks`: Apply reference checks only to actual executed XYK legs and keep Router protection separate from System Actor policy.
- [ ] `Router / Actual-Leg Oracle Publication`: Publish exactly the executed XYK legs in canonical execution order; direct TMC mint publishes no XYK observation.
- [ ] `Router / Atomic Execution`: Keep fee routing, Oracle publication, Actor ingress, liquidity mutation, balance deltas, and Router events inside one rollback boundary.
- [ ] `Router / Rollback Matrix`: Cover every fee, publication, ingress, pool, TMC, balance, and event failure point.

### Outcomes, Errors, and Actors Boundary

- [ ] `Router / Canonical Outcome`: Return one bounded structured outcome whose economic fields retain identical meaning across quote projection, execution, events, and clients.
- [ ] `Router / Failure Taxonomy`: Expose stable exhaustive Router failure classes usable by Actor adapter Temporary/Permanent mapping; unknown remains Permanent.
- [ ] `Router / Actors Adapter Contraction`: Remove quote ownership, path validation, Router protection, and replanning duplicated in `TmctolDexOps`; the Actor runtime supplies only authored policy inputs and consumes the canonical Router outcome.
- [ ] `Router / Route Weight Classes`: Bind every supported prepared route and outcome to one measured route class; make Actor admission cover the maximum class permitted by each swap task.
- [ ] `Router / Compatibility Identity`: Decide and apply either explicit retention of `pallet_axial_router` identity or one complete pre-launch rename; add no partial alias layer.

### Hot-Path and Storage Contraction

- [ ] `Router / Duplicate Work Removal`: Delete repeated quotes, pool lookups, route preparation, path allocation, synthetic direct-pair reads, and parallel validation where `PreparedRoute` already owns the fact.
- [ ] `Router / Primitive API Boundary`: Make low-level pool quote helpers private or name them explicitly as single-pool primitives.
- [ ] `Router / Storage Invariants`: Strengthen `try_state` for fee policy, canonical LP-pair ordering, reverse-index consistency, and LP-token collision freedom.

### Executable Conformance

- [ ] `Router / Route Vectors`: Generate conformance vectors for every supported route family, intent, protection boundary, tie, publication set/order, outcome, error class, and Weight class.
- [ ] `Router / Adversarial Corpus`: Cover stale quote projection, candidate permutations, first/later publication failure, Actor ingress rejection, fee failure, direct XYK failure, TMC failure, Native-anchored leg failure, exact-input recipient delta, and exact-output total spend.
- [ ] `Router / Cross-Domain Invariant`: Prove `projected route = prepared route = protected route = executed route = event route = outcome route = Weight class`.

### Generated Surfaces and Documentation

- [ ] `Router / Production Weights`: Benchmark every accepted route class and regenerate Router plus affected Actors and Oracle weights.
- [ ] `Router / Metadata and Clients`: Regenerate runtime metadata, descriptors, client projections, and route evidence from the accepted ABI.
- [ ] `Router / Public Projection Sync`: Align Router package docs, runtime APIs, Actors and Oracle integration, web-client docs, and wiki projections with the Router specification.

### 0.7.14 Exit State

- [ ] Router runtime and generated artifacts conform to the accepted Router specification.
- [ ] One bounded `PreparedRoute` owns route truth from preparation through outcome.
- [ ] Oracle publication equals the exact ordered executed XYK legs.
- [ ] The Actor runtime duplicates no Router discovery, quote, route, or protection logic.
- [ ] Every supported route has one measured Weight class and complete rollback coverage.
- [ ] No new route family ships beyond the accepted specification.

### 0.7.14 Non-Goals

- No arbitrary graph routing, unrestricted paths, external DEX aggregation, intent marketplace, solver competition, CoW/frequent-batch settlement, or new market family.
- No new Actor task, condition, authority, scheduler, retry, or history surface except adapter synchronization required by the accepted Router contract.

## Runtime Framework Evolution

> These slices keep DEOS current with useful Polkadot SDK runtime patterns while preserving the framework boundary: adopt configuration discipline, reusable primitives, and economic mechanisms; do not import unrelated product layers such as Revive contracts by default.
>
> Source context for agents beyond their training cutoff: Polkadot SDK `stable2606` release notes.

- [ ] `Runtime Cadence Profile`: Define a cadence profile contract that derives time-sensitive runtime constants from a configurable block-duration target instead of hardcoding one block speed. Audit voting periods, Actor cooldowns and retry windows, staking epochs, cleanup windows, and documentation for assumptions that break between conventional ~6s blocks and faster sub-second profiles.
- [ ] `V3 Scheduling / Block-Bundling Readiness`: Document and encode a non-enabled readiness profile for future V3 scheduling/block-bundling adoption, including runtime/operator prerequisites, benchmark margins, `on_idle`/hook pressure, message-queue/XCM budgets, and activation conditions.
- [ ] `DEOS Staking Reward Source Abstraction`: Separate staking distribution from reward origin, allowing externally funded or treasury-budgeted pots alongside existing same-asset reward inflow.
- [ ] `Budget Recipient Primitives`: Introduce typed budget-recipient primitives or runtime helpers for framework-owned economic destinations such as staking reward pots, governance treasuries, liquidity reserves, and System Actors.
- [ ] `Unclaimed Reward Policy`: Make staking/native reward leftovers explicit runtime policy: rollover, Fee Sink return, burn, or treasury routing.

## Collator Economics and Fee Routing

> Phase 1 uses trusted permissioned collators, collects 100% of transaction, Actor-execution, governance-opening, and XCM-execution fees in the Fee Sink, and distributes available native balance 50/50 into staking ingress and liquidity provisioning.
>
> A future permissionless phase may introduce equal security/staking/liquidity thirds only after bounded security-reward settlement ships; indivisible remainder stays in the Fee Sink.

- [ ] `Permissionless Collator Reward Contract`: Before assigning a future security branch, define bounded active-set eligibility, contribution attribution, settlement cadence, custody, payout recipients, unclaimed leftovers, failure behavior, and read-model surfaces.
