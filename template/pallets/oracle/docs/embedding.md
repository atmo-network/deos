# pallet-deos-oracle Embedding Contract

## Host Types and Bounds

- Supply bounded SCALE `FeedId`, `ProducerId`, `Meaning`, and `Provenance` types; do not use arbitrary bytes, strings, or JSON.
- Set defensible `MaxFeeds`, `MaxFeedsPerProducer`, and `MaxScale` constants.
- Keep feed identity semantics immutable; semantic changes register a new ID.

## Authority and Producers

- Bind `RegisterOrigin` to the host's admitted governance/administration boundary.
- Bind `PublishOrigin` to an origin that resolves exactly one typed producer identity.
- Runtime producer adapters may use `ObservationSink`, but must pass the same immutable producer identity and enclose publication with producer effects transactionally.

## Hooks and Consumers

- `OnObservationChanged` receives the exact revision plus previous/current scalar transition and must remain O(1), bounded, and subscriber-independent. It exposes no separate Weight estimate: host-generated publication weights measure the concrete composed changed path and charge it exactly once. A hook error atomically rejects publication.
- A required hook failure must propagate and roll back the observation plus the producer's enclosing transaction.
- Consumers must author a nonzero maximum age and distinguish Unavailable, Uninitialized, Fresh, and Stale.
- History, search, charts, subscriber fanout, and strategy execution remain outside this package.

## Evidence

- Bind generated `WeightInfo`; production runtimes must not use `()` or the packaged `SubstrateWeight`. Both are hand-written estimates rather than benchmark output, and they underprice execution. A nontrivial hook requires host-composed publication benchmarks.
- Validate default, no-std, runtime-benchmark, try-runtime, metadata, maximum-density, hook rollback, and independent-runtime builds.
- Pin LastValue and EMA arithmetic, first/equal/change revision behavior, lifecycle transitions, zero policy, overflow, and SCALE/storage contracts before integration.
