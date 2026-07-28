# DEOS Oracle Package Architecture

## Purpose and Boundary

`pallet-deos-oracle` implements a reusable bounded scalar-observation package. It admits immutable typed feed identities, authorizes typed producers, stores only current scalar truth, applies deterministic LastValue or EMA aggregation, tracks change-only revisions, and invokes one transactional O(1) change hook.

The package has no AAA, Router, DEOS topology, network, off-chain worker, history, subscription, or strategy dependency. A host supplies feed identity and meaning types, authority origins, bounds, hook behavior, and production weights. The independent embedding fixture uses unrelated sensor semantics to falsify accidental DEOS coupling.

Concrete DEOS composition belongs to [`docs/oracle.integration.en.md`](../../../../docs/oracle.integration.en.md).

## Code Map

| Surface | Owner | Anchor |
| --- | --- | --- |
| Public scalar, aggregation, lifecycle, configuration, observation, and read-state types | DEOS Oracle package | `src/lib.rs` |
| Registration, lifecycle, publication, EMA, revision, freshness, and hook transitions | DEOS Oracle package | `src/lib.rs` |
| Benchmark setup and branch postconditions | DEOS Oracle package | `src/benchmarking.rs` |
| Generic `WeightInfo` contract and conservative fallback | DEOS Oracle package | `src/weights.rs` |
| Host-runtime obligations | DEOS Oracle package | `docs/embedding.md` |
| Package mocks and behavioral regressions | DEOS Oracle package | `src/mock.rs`, `src/tests.rs` |
| Independent host composition | Embedding fixture | `embedding-runtime/src/lib.rs` |

## Host Contract

`Config` binds generic `FeedId`, `ProducerId`, `Meaning`, and `Provenance` types. The host also provides registration and publication origins, `MaxFeeds`, `MaxFeedsPerProducer`, `MaxScale`, `OnObservationChanged`, and `WeightInfo`.

`FeedId` remains the immutable identity key chosen by the host. `FeedConfig` stores the authorized producer, explicit meaning and provenance, scalar scale, aggregation policy, zero policy, and lifecycle. The package never infers semantic equivalence, reverse direction, market meaning, or producer trust.

`RegisterOrigin` controls feed and lifecycle administration. `PublishOrigin` resolves directly to the typed producer identity checked against the immutable feed configuration. `OnObservationChanged` receives only `(feed, revision)` after a changed scalar and declares its independent weight.

## Storage Topology

| Storage | Shape | Bound and role |
| --- | --- | --- |
| `FeedCount` | Scalar | Admitted-feed cardinality |
| `FeedIds` | `BoundedVec<FeedId, MaxFeeds>` | Duplicate-free forward registry |
| `Feeds` | Map by `FeedId` | Immutable semantics plus lifecycle |
| `ProducerIds` | `BoundedVec<ProducerId, MaxFeeds>` | Duplicate-free producer registry |
| `ProducerFeeds` | Map to `BoundedVec<FeedId, MaxFeedsPerProducer>` | Exact reverse producer ownership |
| `Observations` | Map by `FeedId` | Optional current `{ value, updated_at, revision }` |

Registration prevalidates duplicate identity, global capacity, scale, EMA half-life, and producer capacity. One transactional mutation updates forward and reverse registries, immutable configuration, count, and event. Deactivation retains identity and current truth; it never permits semantic ID reuse.

Try-state walks only host-bounded registries. It reconciles cardinality, uniqueness, forward/reverse existence, producer equality, nonempty producer indexes, configured bounds, and nonzero initialized revisions.

## Publication Flow

Publication loads one feed, verifies the exact producer and Active lifecycle, applies the immutable zero policy, and computes LastValue or EMA. EMA uses `elapsed = max(current_block - updated_at, 1)` with `Perbill` floor arithmetic. Observation presence, not scalar value, distinguishes initialization.

The first accepted sample stores revision `1`. A changed published scalar increments revision with checked arithmetic and invokes `OnObservationChanged`. Equal output refreshes `updated_at` without hook or revision increment.

The complete path is transactional. Hook failure propagates its dispatch error and rolls back observation state and publication events. The package does not iterate subscribers, execute downstream work, persist history, retry a failed hook, or weaken the host's rollback semantics. Recovery is a new producer attempt after the host integration becomes available.

## Lifecycle and Read Surface

Feeds move through explicit Active, Paused, and Deactivated lifecycle states. Paused feeds retain readable current truth but reject publication. Deactivated feeds retain identity and storage lineage while becoming unavailable through the public observation-state projection.

`observation_state(feed, max_age)` returns Unavailable, Uninitialized, Fresh, or Stale. Unknown and deactivated identities are unavailable. Equality at a nonzero authored maximum-age boundary remains Fresh.

No historical revision lookup exists. Archive, charts, search, replay, and unbounded analytics belong to an external materialized provider.

## Benchmark and Weight Architecture

Package benchmarks construct bounded worst-case registration, lifecycle, LastValue, changed EMA, and equal-refresh branches. Postconditions verify the intended storage topology and whether the change hook ran.

The package owns only the `WeightInfo` interface and conservative fallback. Every production host must generate runtime-specific weights against its concrete origin, hook, database schedule, bounds, and Wasm. RefTime, measured or estimated ProofSize, reads, and writes remain separate evidence dimensions.

Registration exposes distinct existing-producer and new-producer measurements because their proof topology differs. Publication includes the host-declared hook bound only on changed output. Equal refresh retains the no-hook path.

## Falsification and Validation

Package tests pin SCALE variant order, storage names, duplicate and capacity rejection, lifecycle transitions, producer authorization, zero behavior, freshness boundaries, revision overflow, hook rollback and cardinality, exact EMA elapsed vectors, extreme arithmetic, and try-state corruption detection.

The embedding runtime passes default, no-std, runtime-benchmark, and try-runtime profiles without DEOS types. Its unrelated sensor identities and provenance prove that the package contract does not require assets, pools, prices, Router, or AAA.

A host integration remains incomplete until it supplies generated weights, validates maximum configured density, proves hook rollback at the composed transaction boundary, and classifies current versus materialized read surfaces.
