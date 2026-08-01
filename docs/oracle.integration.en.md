# DEOS Oracle Integration

## Purpose and Ownership

This document maps how the DEOS reference runtime composes `pallet-deos-oracle` with canonical pool admission, DEOS Router production and consumption, reactive AAA ingress, bounded browser inspection, generated weights, and transactional runtime evidence.

The reusable package contract and implementation remain in [`template/pallets/oracle/docs/specification.en.md`](../template/pallets/oracle/docs/specification.en.md) and [`template/pallets/oracle/docs/architecture.en.md`](../template/pallets/oracle/docs/architecture.en.md). This document owns only concrete DEOS composition.

## Integration Code Map

| Surface | Anchor |
| --- | --- |
| DEOS feed identity, meaning, and provenance | `template/primitives/src/oracle.rs` |
| Runtime bounds, origins, pool-feed identity, and AAA hook | `template/runtime/src/configs/oracle_config.rs` |
| Canonical LP pair plus directional-feed registration | `template/runtime/src/configs/assets_config.rs` |
| Pool-index and feed-admission weight envelope | `template/runtime/src/configs/pool_index.rs` |
| DEOS Router production and consumption | `template/runtime/src/configs/axial_router_config.rs`, `template/pallets/router/src/lib.rs` |
| Runtime-generated DEOS Oracle weights | `template/runtime/src/weights/pallet_oracle.rs` |
| Pallet index and metadata composition | `template/runtime/src/lib.rs` |
| Runtime integration evidence | `template/runtime/src/tests/oracle_integration_tests.rs`, `template/runtime/src/tests/axial_router_integration_tests.rs` |
| Canonical browser inspection | `web-client/src/lib/observation/`, `web-client/src/lib/adapters/blockchain/observations.ts` |

## Feed Identity and Authority

`OracleFeedId` identifies ordered `asset_in` and `asset_out`, `LocalPoolObservationMethod`, immutable aggregation identity including EMA half-life, and scale. Its constructor and explicit `reverse()` preserve every semantic dimension while swapping direction. DEOS never infers reverse truth from a forward value.

`OracleMeaning` repeats the typed semantic direction for review. `OracleProvenance` identifies DEOS Router pre-execution reserves. Feed identity changes require a new identity rather than mutable semantic reuse.

The runtime binds `ProducerId = AccountId`. Root admits feeds and lifecycle changes. Signed publication resolves the signer as producer; the package then checks exact equality with the immutable producer stored for that feed.

## Runtime Bounds and Mounting

The DEOS runtime mounts DEOS Oracle at pallet index `52`. It bounds global feeds at `1,024`, per-producer feeds at `1,001`, and scalar scale at `18`.

Canonical pool indexing admits one EMA feed at scale `12` for each ordered direction. Both use the DEOS Router pallet account as producer, pre-execution-reserve provenance, zero rejection, and Active lifecycle. Repeated indexing succeeds only when the complete immutable configuration matches.

Two feeds per pool bound Router admission to `500` complete directional pairs. Capacity prevalidation rejects before mutation when both identities do not fit. A conflicting reverse identity rolls back the first feed, pool index, and LP reverse index in the same top-level transaction.

The pool-index call charges two worst-case DEOS Oracle registration envelopes plus its own bounded LP-index database work. No pool admission path performs an unbounded feed scan.

## Router Production and Consumption

For a direct XYK route, DEOS Router validates the candidate against the previously stored directional EMA, collects its fee, publishes the current pre-execution reserve sample, and only then executes the swap.

Missing feeds skip publication without implicit admission. This preserves a valid User swap outcome while feed creation remains an explicit governance/runtime-composition action.

A failed direct execution rolls back observation value, block, revision, DEOS Oracle event, AAA dirty ingress, fee movement, pool effects, payer balance, and recipient movement. Router-local EMA, tracked-asset governance state, and observation history do not exist.

The System AAA market guard consumes only Fresh nonzero directional observations through its authored age bound. Unavailable, Uninitialized, and Stale states fall back to direct reserves and then classify failure as Temporary when no reserve reference exists. User swap validity does not depend on prior DEOS Oracle initialization.

## Reactive AAA Hook

`AaaObservationChangeIngress` binds `OnObservationChanged` to `AAA::note_observation_changed`. Changed publication coalesces one latest revision into AAA-owned exact active-dirty state. Equal output refresh invokes no hook.

Ingress remains subscriber-independent O(1). It does not read subscriber pages, mark actor readiness, enqueue actors, evaluate conditions, or execute plans. Deferred AAA fanout follows exact active dirty feeds and occupied subscriber pages through the existing scheduler.

DEOS Oracle publication and AAA ingress share one transaction boundary. Any revision, capacity, or reciprocal-topology failure rolls back the DEOS Oracle observation and event rather than exposing a revision without its reaction obligation.

Direct publication propagates the exact AAA dispatch error, including `DirtyObservationCapacityExceeded` and `DirtyObservationInvariant`. DEOS Router maps a rejected pre-execution publication to `InvalidOracleData`; its outer swap transaction rolls back fee, pool, payer, recipient, Oracle, event, and dirty-ingress effects.

These failures are fail-closed availability signals, not deferred work. Operators must repair or clear the bounded dirty topology or restore capacity before retrying the producer operation. A later retry re-enters the same atomic path; no rejected observation revision or notification obligation survives for replay.

The DEOS Oracle dispatch envelope adds the runtime-declared AAA ingress weight to the maximum DEOS Oracle publication branch. Independently metered fanout and actor execution never enter publication weight.

## Canonical and Materialized Read Surfaces

The canonical browser inspector reads the bounded feed registry and selected Oracle/AAA keys at one finalized hash. It classifies scalar state as Fresh, Stale, Uninitialized, or Unavailable and reactive delivery as Clean, PendingFanout, FanoutInProgress, or AwaitingCleanup.

Exact dirty age comes from AAA `dirty_since`; selected active position follows bounded predecessor links, remaining work follows occupied subscriber-page links, and identified production weights yield a conditional page/block ceiling. The surface discloses every estimate assumption and never predicts queue admission, actor execution, intermediate-revision delivery, or a general fair price.

An optional selected actor adds one exact `ActorHot` read for pending signal, type-derived lane, queue ticket or wakeup pointer, and current admission status. It performs no queue or wakeup prefix scan.

Current feed configuration, scalar, block, and revision are canonical-chain truth. Historical revisions, charts, search, replay, and unbounded analytics remain materialized-provider responsibilities under [`read-model.contract.en.md`](./read-model.contract.en.md).

The client must not reconstruct history from session observations or present cached/provider values as direct runtime projection.

## Generated Weight Ownership

`template/runtime/src/weights/pallet_oracle.rs` owns the executable DEOS Oracle methods. Production generation must use the reference benchmark runtime and preserve RefTime, measured or estimated ProofSize, reads, and writes as separate evidence.

Registration measures existing-producer and new-producer storage topologies separately. The runtime binds their component-wise conservative maximum. Publication benchmarks include the composed changed-hook topology, while the dispatch envelope separately includes the hook's declared bound.

Any change to AAA ingress storage, Oracle hook composition, runtime bounds, producer identity, or pool admission invalidates composed publication evidence even when the reusable Oracle algorithm remains unchanged.

## Accepted Production Weight Evidence

Production-Wasm `50 × 20` generation on 2026-07-28 produced the following runtime methods. RefTime below excludes runtime database charges while the reads and writes columns expose those charges explicitly.

| Path | RefTime | ProofSize | Reads | Writes |
| --- | ---: | ---: | ---: | ---: |
| Register for existing producer at maximum occupancy | 142,060,000 | 20,532 | 4 | 4 |
| Register new producer at maximum occupancy | 211,203,000 | 44,420 conservative bridge | 5 | 5 |
| Pause | 18,788,000 | 3,551 | 1 | 1 |
| Resume | 18,788,000 | 3,551 | 1 | 1 |
| Deactivate | 18,718,000 | 3,551 | 1 | 1 |
| Publish LastValue with no-subscriber hook branch | 35,829,000 | 3,559 | 4 | 1 |
| Publish changed EMA with no-subscriber hook branch | 37,575,000 | 3,559 | 4 | 1 |
| Publish equal EMA refresh | 26,750,000 | 3,551 | 2 | 1 |

The new-producer benchmark measured `44,420` ProofSize above its generated `34,255` estimate. The runtime file deliberately replaces that estimate with the measured value.

Changed publication measurement includes the composed no-subscriber AAA hook branch. Dispatch then adds the independently declared worst-branch AAA ingress envelope, currently `430,032,000 / 6,128` with runtime RocksDB charges, so publication remains safe when a clean subscribed feed appends to the active-dirty list.

The component-wise maximum DEOS Oracle publication method plus that hook declares `668,166,000 / 9,687` before execution. Equal refresh receives the same conservative dispatch envelope even though it invokes no hook.

Accepted DEOS Oracle weights SHA-256 is `ffd422bd67a6b75c8bc4e76f7ace4aad5b40a352cf2b10a70547a241e261259e`. These values bound configured operations only; they imply no publication, subscriber, or actor throughput.

## Falsification and Validation

Runtime tests pin pallet index `52`, generated-weight binding, direction/aggregation/scale non-aliasing, Root registration, signed producer publication, Fresh revision `1`, bidirectional pool admission, idempotent re-indexing, independent directional values, the `500`-pair bound, one-slot capacity rejection, and reverse-identity rollback.

Runtime regressions inject exact dirty-capacity and reciprocal-topology failures through the real AAA hook, prove no Oracle observation, event, or dirty ownership commits, restore healthy topology, and prove later publication succeeds. Router regressions separately pin hook-rejection and later failed-swap rollback across DEOS Oracle state, event, revision, fee, pool, payer, and recipient surfaces with a real subscriber.

Reactive integration fails if publication iterates subscribers, directly executes actors, admits only one direction, accepts mutable semantic reuse, commits DEOS Oracle state without dirty ingress, or presents current reserves as archive or unconditional fair-price truth.
