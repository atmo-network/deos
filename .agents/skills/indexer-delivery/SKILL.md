---
name: indexer-delivery
description: Drives DEOS materialized indexer capabilities from read-model contract through ingestion, finality, schema, provider API, client provenance, and operational evidence.
fmos: true
---

# Indexer Delivery

Canonical work: indexer and materialized-provider items in `BACKLOG.md`, including governance archive integration and any explicitly activated provider implementation slice.

## Mission and Scope

Deliver externally materialized read surfaces without expanding consensus state or presenting indexed data as canonical chain truth. Coordinate contract selection, ingestion, finality/reorg behavior, replay, schema evolution, bounded provider APIs, client adapters, and operational readiness.

Exclude consensus mechanics, unbounded on-chain history, hidden correctness dependencies, generic product analytics, provider credentials, live deployment, and account-affecting operations.

## Truth Owners

- `docs/read-model.contract.en.md` owns canonical-chain versus materialized classification.
- Owning subsystem specifications and architecture documents own event/storage meaning.
- `docs/web-client.architecture.en.md` owns browser provenance and provider realization.
- The selected indexer package owns ingestion, schema, checkpoints, replay, and API behavior once implementation exists.
- Client domain slices own provider adapters and presentation contracts.
- `BACKLOG.md` owns remaining work; `CHANGELOG.md` owns completed outcomes.

## Delivery Loop

1. Select an activated materialized-data use case and classify every datum against the read-model contract.
2. Define source events/storage, finality point, reorg/replay semantics, stable identity keys, retention, schema versioning, and bounded query shape before implementation.
3. Add the smallest ingestion-to-query vertical slice with deterministic fixtures and restart/replay evidence.
4. Integrate through a narrow provider contract that preserves provenance, staleness, unavailable-provider fallback, and canonical-chain separation.
5. Validate local replay and client behavior, then reconcile backlog, architecture, operations, and changelog.

## Evidence and Gates

- Require idempotent replay, duplicate suppression, checkpoint recovery, finality/reorg tests, and explicit retention.
- Require bounded pagination and query limits even though storage is external.
- Fail visibly or degrade to a narrower canonical capability when the provider is unavailable; never fabricate archive truth.
- Treat backend selection, hosted infrastructure, credentials, network deployment, and external publication as explicit gates.

## Working Memory and Handoff

Report the selected use case, source and provenance class, replay/finality evidence, provider/client contract, operational gate, and exact unblocker. Keep no copied schema catalog or shadow roadmap here.

## Evolution and Retirement

Refine this organ after real provider or replay friction. Split only if ingestion operations and product analytics become independently owned loops. Retire it if materialized delivery leaves repository scope or becomes fully owned by deterministic tooling.
