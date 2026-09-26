# DEOS Oracle Integration

## Purpose and Ownership

This document maps how the DEOS reference runtime composes `pallet-deos-oracle` with canonical pool admission, DEOS Router production and consumption, pull-time Actor observation reads, bounded browser inspection, generated weights, and transactional runtime evidence.

The reusable package contract and implementation remain in [`template/pallets/oracle/docs/specification.en.md`](../template/pallets/oracle/docs/specification.en.md) and [`template/pallets/oracle/docs/architecture.en.md`](../template/pallets/oracle/docs/architecture.en.md). This document owns only concrete DEOS composition.

## Integration Code Map

| Surface | Anchor |
| --- | --- |
| DEOS feed identity, meaning, and provenance | `template/primitives/src/oracle.rs` |
| Runtime bounds, origins, pool-feed identity, and unit hooks | `template/runtime/src/configs/oracle_config.rs` |
| Canonical LP pair plus directional-feed registration | `template/runtime/src/configs/assets_config.rs` |
| Atomic pool/LP/feed lifecycle and admission Weight | `template/runtime/src/configs/assets_config.rs`, `template/runtime/src/weights/pallet_deos_router.rs` |
| DEOS Router production and consumption | `template/runtime/src/configs/deos_router_config.rs`, `template/pallets/router/src/lib.rs` |
| Runtime-generated DEOS Oracle weights | `template/runtime/src/weights/pallet_oracle.rs` |
| Pallet index and metadata composition | `template/runtime/src/lib.rs` |
| Runtime integration evidence | `template/runtime/src/tests/oracle_integration_tests.rs`, `template/runtime/src/tests/deos_router_integration_tests.rs` |
| Canonical browser inspection | `web-client/src/lib/observation/`, `web-client/src/lib/adapters/blockchain/observations.ts` |

## Feed Identity and Authority

`OracleFeedId` identifies ordered `asset_in` and `asset_out`, `LocalPoolObservationMethod`, immutable aggregation identity including EMA half-life, and scale. Its constructor and explicit `reverse()` preserve every semantic dimension while swapping direction. DEOS never infers reverse truth from a forward value.

`OracleMeaning` repeats the typed semantic direction for review. `OracleProvenance` identifies DEOS Router pre-execution reserves. Feed identity changes require a new identity rather than mutable semantic reuse.

The runtime binds `ProducerId = AccountId`. Root admits feeds and lifecycle changes. Signed publication resolves the signer as producer; the package then checks exact equality with the immutable producer stored for that feed.

## Runtime Bounds and Mounting

The DEOS runtime mounts DEOS Oracle at pallet index `52`. It bounds global feeds at `1,024`, per-producer feeds at `1,001`, and scalar scale at `18`.

Canonical pool indexing admits one EMA feed at scale `12` for each ordered direction. Both use the DEOS Router pallet account as producer, pre-execution-reserve provenance, zero rejection, and Active lifecycle. Repeated indexing succeeds only when the complete immutable configuration matches.

Two feeds per pool bound Router admission to `500` complete directional pairs. The permissionless DEOS pool lifecycle prevalidates the expected LP token, bounded reverse-index capacity and collisions, both feed identities, and producer capacity before underlying creation. Pool, actual LP verification, reverse binding, and both feeds share one transactional rollback owner; liquidity mutation never repairs topology.

Canonical pool creation is measured as one Router call at `144,923,000 / 34,255` with 13 reads and 10 writes. No pool admission path performs an unbounded feed scan. `PoolIndexExtension` is removed from the signed transaction format.

## Router Production and Consumption

For a direct XYK route, DEOS Router validates the candidate against the previously stored directional EMA, collects its fee, publishes the current pre-execution reserve sample, and only then executes the swap.

Missing feeds skip publication without implicit admission. This preserves a valid User swap outcome while feed creation remains an explicit governance/runtime-composition action.

A failed direct execution rolls back observation value, block, revision, DEOS Oracle event, fee movement, pool effects, payer balance, and recipient movement. Router-local EMA, tracked-asset governance state, and observation history do not exist.

The System Actors market guard consumes only Fresh nonzero directional observations through its authored age bound. Unavailable, Uninitialized, and Stale states fall back to direct reserves and then classify failure as Temporary when no reserve reference exists. User swap validity does not depend on prior DEOS Oracle initialization.

## Actors Consumption

The reference runtime binds both `OnObservationChanged` and `OnFeedStateChanged` to `()`. DEOS Oracle publication therefore creates no Actors ingress, subscription, dirty state, Pending obligation, or Trigger cause, and publication Weight carries no Actors hook component. Actors reads current observations only through `TmctolObservationProvider` when an authored observation predicate or the System swap reference guard is evaluated at execution time. Oracle writes remain O(1) and independent of Actor population.

## Canonical and Materialized Read Surfaces

The canonical browser inspector reads the bounded feed registry and selected Oracle keys at one finalized hash and classifies scalar state as Fresh, Stale, Uninitialized, or Unavailable.

Current feed configuration, scalar, block, and revision are canonical-chain truth. Historical revisions, charts, search, replay, and unbounded analytics remain materialized-provider responsibilities under [`read-model.contract.en.md`](./read-model.contract.en.md).

The client must not reconstruct history from session observations or present cached/provider values as direct runtime projection.

## Generated Weight Ownership

`template/runtime/src/weights/pallet_oracle.rs` owns the executable DEOS Oracle methods. Production generation must use the reference benchmark runtime and preserve RefTime, measured or estimated ProofSize, reads, and writes as separate evidence.

Registration measures existing-producer and new-producer storage topologies separately, including first-source carrier insertion. Lifecycle and prepared publication paths measure coalesced carrier membership; unprepared changed publication measures first insertion. Publication binds one generated maximum across empty, Primary first/existing, Secondary first/existing, combined, capacity-rejection, and equal-refresh branches. Each branch measures the concrete bound hooks in place; no independent hook Weight is added.

Any change to Oracle hook composition, runtime bounds, producer identity, or pool admission invalidates composed publication evidence even when the reusable Oracle algorithm remains unchanged.

## Accepted Production Weight Evidence

Production-Wasm `50 × 20` generation on 2026-09-15 produced the following runtime methods while both hooks were still bound to Actors ingress. They remain conservative upper bounds for the unit-hook composition until the coordinated Weight regeneration replaces them. RefTime excludes runtime database charges; reads and writes expose those charges separately.

| Path | RefTime | ProofSize | Reads | Writes |
| --- | ---: | ---: | ---: | ---: |
| Register existing producer | 181,241,000 | 20,532 | 10 | 9 |
| Register new producer | 242,912,000 | 45,174 conservative bridge | 11 | 10 |
| Pause / resume / deactivate maximum | 40,020,000 | 3,551 | 5 | 2 |
| Publish LastValue empty | 56,292,000 | 3,587 | 10 | 2 |
| Publish changed EMA empty | 65,163,000 | 3,587 | 13 | 7 |
| Publish Primary first | 67,328,000 | 3,587 | 11 | 5 |
| Publish Primary existing | 65,862,000 | 3,587 | 10 | 4 |
| Publish Secondary first | 70,052,000 | 6,636 | 13 | 5 |
| Publish Secondary existing | 70,192,000 | 6,636 | 13 | 5 |
| Publish combined | 79,550,000 | 6,636 | 14 | 8 |
| Reject at Secondary capacity | 48,261,000 | 6,636 | 8 | 0 |
| Publish equal EMA refresh | 47,353,000 | 3,551 | 6 | 2 |

The new-producer benchmark measured `45,174` ProofSize above its generated `34,255` estimate, so normalization retains the measured conservative bridge. Combined publication is the successful maximum.

These values bound configured operations only; they imply no publication or actor throughput.

## Falsification and Validation

Runtime tests pin pallet index `52`, generated-weight binding, direction/aggregation/scale non-aliasing, Root registration, signed producer publication, Fresh revision `1`, bidirectional pool admission, idempotent re-indexing, independent directional values, the `500`-pair bound, one-slot capacity rejection, and reverse-identity rollback.

Router regressions pin failed-swap rollback across DEOS Oracle state, event, revision, fee, pool, payer, and recipient surfaces.

Integration fails if publication reaches Actors, admits only one direction, accepts mutable semantic reuse, or presents current reserves as archive or unconditional fair-price truth.
