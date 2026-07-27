# Typed Observation Oracle Architecture

## Purpose and Current Boundary

`template/pallets/oracle` implements the reusable bounded scalar observation contract. The DEOS runtime mounts it at pallet index `52` and admits both typed directions for every canonically indexed local pool. Axial Router publishes pre-execution reserve samples into these feeds and consumes their EMA values. Router-local EMA and tracked-asset storage no longer exist.

The package depends only on FRAME/runtime primitives. It has no AAA, Router, DEOS topology, network, off-chain worker, history, subscription, or strategy dependency. `template/pallets/oracle/embedding-runtime` proves independent composition with unrelated sensor meanings and provenance.

## Code Map

| Surface | Owner | Anchor |
| --- | --- | --- |
| Public scalar, aggregation, lifecycle, configuration, observation, and read-state types | Oracle package | `template/pallets/oracle/src/lib.rs` |
| Admission, lifecycle, publication, EMA, revision, freshness, and hook transitions | Oracle package | `template/pallets/oracle/src/lib.rs` |
| FRAME benchmark setup and branch postconditions | Oracle package | `template/pallets/oracle/src/benchmarking.rs` |
| Generic weight contract | Oracle package | `template/pallets/oracle/src/weights.rs` |
| Independent host fixture | Oracle package | `template/pallets/oracle/embedding-runtime/src/lib.rs` |
| DEOS feed identity, meaning, and provenance types | Shared primitives | `template/primitives/src/oracle.rs` |
| DEOS bounds, origins, canonical pool-feed identity, and idempotent admission | Reference runtime | `template/runtime/src/configs/oracle_config.rs` |
| Canonical LP-pair plus directional-feed registration | Reference runtime | `template/runtime/src/configs/assets_config.rs` |
| Top-level pool-index and feed-admission Weight envelope | Reference runtime | `template/runtime/src/configs/pool_index.rs` |
| Production-generated weights | Reference runtime | `template/runtime/src/weights/pallet_oracle.rs` |
| Runtime metadata and pallet index | Reference runtime | `template/runtime/src/lib.rs` |

## Runtime Types and Authority

`OracleFeedId` identifies ordered `asset_in` and `asset_out`, `LocalPoolObservationMethod`, immutable aggregation identity including EMA half-life, and scale. Its canonical constructor and explicit `reverse()` preserve every semantic dimension while swapping direction; no admission path infers reverse truth from a forward value. `OracleMeaning` repeats the typed semantic direction for explicit review, while `OracleProvenance` identifies Axial Router pre-execution reserves.

The runtime binds `ProducerId = AccountId`. Root admits feeds and lifecycle changes. Signed publication resolves the signer as producer, then the pallet checks exact equality with the immutable registered producer. Canonical pool indexing admits an EMA feed at scale `12` for each direction with the Axial Router pallet account as producer, pre-execution-reserve provenance, zero rejection, and active lifecycle. Repeated indexing accepts only an exact immutable configuration match.

The runtime bounds global feeds at `1,024`, per-producer feeds at `1,001`, and scale at `18`. Two feeds per pool bound Axial Router admission to `500` complete directional pairs. Capacity prevalidation rejects before mutation when both identities do not fit; a conflicting second identity rolls back the first feed and LP reverse index. The top-level pool transaction extension charges two worst-case generated registration envelopes plus its LP-index database work.

## Storage Topology

| Storage | Shape | Bound and role |
| --- | --- | --- |
| `FeedCount` | Scalar | Admitted-feed cardinality |
| `FeedIds` | Bounded vector | Duplicate-free forward registry, maximum 1,024 |
| `Feeds` | Map by typed feed ID | Immutable semantics plus lifecycle |
| `ProducerIds` | Bounded vector | Duplicate-free producer registry, maximum 1,024 |
| `ProducerFeeds` | Map to bounded vector | Exact reverse ownership, maximum 1,024 feeds per producer |
| `Observations` | Map by typed feed ID | Optional current `{ value, updated_at, revision }` only |

Registration prevalidates duplicate, global capacity, scale, half-life, and producer capacity. One transactional mutation updates forward/reverse registries, configuration, count, and event. Deactivation retains identity and current truth; it never permits semantic ID reuse.

Try-state walks only bounded registries and reconciles cardinality, uniqueness, forward/reverse existence, producer equality, nonempty producer indexes, and nonzero initialized revisions.

## Publication Flow

Publication loads one feed, verifies the exact producer and Active lifecycle, applies immutable zero policy, and computes LastValue or EMA. EMA uses `elapsed = max(current - updated_at, 1)` and Router-compatible `Perbill` floor arithmetic. Presence of observation storage, not scalar value, distinguishes initialization.

The first accepted sample stores revision `1`. A changed published scalar increments revision with checked arithmetic and invokes `OnObservationChanged`. Equal output refreshes `updated_at` without hook or revision increment. The complete path is transactional, so hook failure leaves observation and events unchanged.

The hook reports a conservative independent Weight. Dispatch publication charges the maximum measured oracle branch plus that hook bound. The reference runtime binds `AaaObservationChangeIngress`, which transactionally coalesces one subscribed feed's latest revision in AAA-owned dirty state without reading subscribers or marking actors ready.

Axial Router direct routes validate against the previous standalone EMA, collect the fee, publish the pre-execution reserve sample, and then execute. Missing feeds skip publication without implicit admission so a valid User swap preserves its prior outcome. A failed execution rolls back the observation value/block/revision, oracle event, fee movement, pool effects, and recipient movement.

The System AAA market guard consumes only Fresh nonzero directional observations through its authored 100-block age boundary. Unavailable, Uninitialized, and Stale states fall back to direct reserves and then fail Temporary when no reserve reference exists. User swaps remain independent of standalone initialization during extraction, preserving the prior valid-swap outcome until the Router producer and consumer move atomically.

## Read Surface

`observation_state(feed, max_age)` returns Unavailable, Uninitialized, Fresh, or Stale. Unknown and deactivated IDs are unavailable. Paused feeds remain readable but reject publication. Equality at the nonzero authored maximum-age boundary is fresh.

No historical revision lookup exists. Archive, charts, search, and replay remain materialized-provider responsibilities.

## Production Weight Evidence

Weights were generated through `scripts/benchmarks.sh pallet_oracle` with 50 steps and 20 repeats against the production benchmark runtime on 2026-07-27. RefTime and estimated ProofSize remain separate claims.

| Path | RefTime model | Estimated ProofSize | Reads | Writes |
| --- | ---: | ---: | ---: | ---: |
| Register for existing producer at maximum occupancy | 138,358,000 | 20,532 | 4 | 4 |
| Register new producer at maximum occupancy | 206,174,000 | 44,420 conservative bridge | 5 | 5 |
| Pause | 18,648,000 | 3,551 | 1 | 1 |
| Resume | 18,788,000 | 3,551 | 1 | 1 |
| Deactivate | 18,648,000 | 3,551 | 1 | 1 |
| Publish LastValue with AAA hook | 35,759,000 | 3,551 | 4 | 1 |
| Publish changed EMA with AAA hook | 37,575,000 | 3,551 | 4 | 1 |
| Publish equal EMA refresh | 26,750,000 | 3,551 | 2 | 1 |

Registration uses fixed worst-case Weight equal to the component-wise maximum of two measured storage topologies. The existing-producer case fills its producer index and the global feed registry. The new-producer case fills the producer and feed registries. Its measured ProofSize was `44,420`, above the generated `34,255` estimate, so the runtime weight file conservatively overrides ProofSize to the measured value.

Changed publication measures the subscriber-independent AAA hook's two additional reads without fanout or actor execution. Equal EMA refresh invokes no change hook and retains the two-read path. Accepted weights SHA-256 is `c8d713849740ff6346571336befd9900ee4b6b53f84269327c826310526d8762`; no row implies publication or subscriber throughput.

## Falsification and Validation

Package tests pin SCALE variant order, storage names, duplicate/capacity rejection, lifecycle transitions, producer authorization, zero behavior, freshness boundaries, revision overflow, hook rollback/cardinality, exact EMA elapsed vectors, extreme arithmetic, and try-state corruption detection.

Runtime tests pin pallet index `52`, generated-weight binding, direction/aggregation/scale non-aliasing, root registration, signed producer publication, Fresh revision `1`, exact bidirectional pool admission, idempotent re-indexing, independent directional values, the `500`-pair bound, rejection when only one producer slot remains, and first-direction rollback on reverse-identity collision. The independent fixture passes default, no-std, runtime-benchmark, and try-runtime builds without DEOS types.

Router extraction regressions pin elapsed/rounding vectors and failed-swap rollback across oracle state/event/revision, fee, pool, payer, and recipient surfaces. The hook-composed failed-swap regression installs a real AAA subscriber and proves rollback across AAA dirty state/slot/cursor, Oracle state/revision/event, fee balances, pool reserves, payer, and recipient effects. Reactive AAA remains falsified if any producer or hook path iterates subscribers or executes actors synchronously.
