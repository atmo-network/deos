---
name: oracle-delivery
description: Stable entry point for DEOS typed-observation work across the standalone Oracle pallet, producers, reactive AAA, client inspection, bounded evidence, and support.
fss: true
---

# Oracle

Canonical open work: ../../../BACKLOG.md

Use this skill as the feature entry point whenever work creates, changes, consumes, inspects, validates, or documents DEOS typed observations. It preserves ownership boundaries and routes activated work from the canonical backlog without cloning release tasks.

## Feature Boundary

Oracle owns bounded current scalar/revision truth. Producers own samples, Router owns routing and pre-execution pool sampling, AAA owns subscriptions and reactions, and materialized providers own history. This skill coordinates that feature boundary; it does not own runtime semantics, producer policy, subscriber execution, external price networks, or shared command implementations.

## Truth Owners

- `template/pallets/oracle/docs/specification.en.md` owns intended Oracle semantics and invariants.
- `template/pallets/oracle/docs/architecture.en.md` owns the reusable package implementation map; `docs/oracle.integration.en.md` owns concrete DEOS runtime, Router, AAA, client, weight, and rollback composition.
- `template/pallets/oracle/` owns executable behavior, tests, benchmarks, weights, and the independent embedding runtime.
- `template/pallets/router/docs/architecture.en.md` owns Router observation production and pre-execution ordering.
- `template/pallets/aaa/docs/specification.en.md` and `template/pallets/aaa/docs/architecture.en.md` own reactive subscription, fanout, scheduling, and execution semantics.
- `docs/aaa-control-plane.contract.en.md` and `docs/read-model.contract.en.md` own off-chain inspection and canonical/materialized provenance.
- `web-client/src/lib/observation/` and its blockchain adapter own browser realization.
- `BACKLOG.md` owns activated future work; `CHANGELOG.md` owns completed outcomes.

## Routing Protocol

1. Classify the task as Oracle package semantics, producer integration, reactive AAA, control-plane/client inspection, materialized history, or documentation/support.
2. Read the owning specification and architecture map, then inspect the affected code, tests, runtime configuration, and current diff before changing behavior.
3. Preserve one truth owner per layer: do not recreate Oracle values in Router or AAA, execute subscribers in producer context, infer reverse-direction feeds, or place archive history in consensus storage.
4. Reconcile the specification before non-trivial semantic changes. Update architecture only from shipped implementation truth.
5. Run the narrowest owning validation first and escalate when the change crosses package, runtime, AAA, client, metadata, weight, or release boundaries.
6. Reconcile open work and delivery history only to evidence actually produced, then run the repository completion gate.

## Decisive Checks

- Every feed has immutable typed meaning, producer, provenance, scale, aggregation, freshness, lifecycle, and explicit bounds; semantic changes create a new feed identity.
- Current reads and publication remain bounded. Notification work stays subscriber-independent and does not synchronously iterate subscribers.
- Equal output may refresh `updated_at` without advancing the change-only revision or notifying reactions.
- Directional pool observations require explicit admission in each direction and preserve Router pre-execution sampling plus whole-swap rollback.
- AAA alone owns bounded subscriptions, dirty coalescing, deferred fanout, the existing readiness latch, queue, wakeup, and scheduler path.
- Browser inspection reads selected bounded finalized-state keys and labels provenance, freshness, staleness, and unavailability honestly; history requires a named materialized provider.
- Local-pool observations never become claims of external fair price, manipulation resistance, MEV protection, or transaction-order guarantees.

## Evidence Routes

- Oracle package changes require focused unit/try-state tests and the applicable default, no-std, runtime-benchmark, try-runtime, and independent-embedding profiles.
- Storage, admission, lifecycle, or publication changes require maximum-density and transactional-failure evidence plus regenerated two-dimensional weights when measured work changes.
- Router producer changes require exact aggregation/parity vectors, directional registration coverage, and failed-swap rollback evidence.
- Reactive AAA changes require focused ingress/subscription/fanout tests and the shared AAA quick or full release route according to scheduler, liveness, capacity, or production-weight impact.
- Client/control-plane changes require observation and reactive-authoring tests, Svelte checks, and production build when browser realization changes.
- Metadata, runtime Wasm, generated artifacts, or release claims require their owning full validation and freshness gates; no narrower result may be promoted into release evidence.

## Evolution

Keep this mature feature skill version-neutral and compact. When verified Oracle work becomes actionable, continue in this same skill and point agents to the exact canonical backlog slice. Add guidance only for recurring feature-specific routing or evidence failures that canonical docs and shared validators do not already own; remove guidance after a stronger owner absorbs it. Split only if a genuinely independent observation feature develops its own truth owners and decisive gates.
