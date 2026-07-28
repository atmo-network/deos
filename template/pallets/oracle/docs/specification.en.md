# DEOS Oracle Specification

## 1. Purpose and Boundary

`pallet-oracle` owns bounded current scalar observation truth for deterministic runtime consumers. It admits typed feeds, authorizes typed producers, applies one immutable aggregation rule, stores the current published value and revision, and emits one transactional change notification.

The pallet does not own sample generation, market interpretation, strategy execution, subscriber lists, fanout, historical series, archive search, external networks, off-chain workers, quorum consensus, fair-price claims, or MEV protection. Producers own sample generation. AAA owns subscriptions and reactions. Indexed providers own history.

The package MUST remain independently reusable and MUST NOT depend on AAA, DEOS Router, DEOS asset topology, TMCTOL policy, an off-chain worker, or an external service.

## 2. Typed Public Model

The host runtime supplies bounded SCALE types for `FeedId`, `ProducerId`, `Meaning`, and `Provenance`. Each type MUST implement the ordering, encoding, metadata, and maximum-encoded-length contracts required for bounded FRAME storage. Arbitrary bytes, strings, JSON, and runtime-decoded expressions are forbidden.

```rust
pub type OracleValue = u128;
pub type Revision = u64;

pub enum Aggregation<BlockNumber> {
  LastValue,
  Ema { half_life_blocks: BlockNumber },
}

pub enum ZeroPolicy {
  Allow,
  Reject,
}

pub enum FeedLifecycle {
  Active,
  Paused,
  Deactivated,
}

pub struct FeedConfig<ProducerId, Meaning, Provenance, BlockNumber> {
  pub producer: ProducerId,
  pub meaning: Meaning,
  pub provenance: Provenance,
  pub scale: u8,
  pub aggregation: Aggregation<BlockNumber>,
  pub zero_policy: ZeroPolicy,
  pub lifecycle: FeedLifecycle,
}

pub struct Observation<BlockNumber> {
  pub value: OracleValue,
  pub updated_at: BlockNumber,
  pub revision: Revision,
}
```

`scale` is an immutable base-10 decimal exponent used by consumers for interpretation and formatting. It does not transform submitted integers inside the pallet. The runtime MUST configure a finite maximum scale no greater than its supported `u128` formatting/arithmetic envelope.

## 3. Immutable Feed Identity

Registration binds one `FeedId` permanently to its producer, meaning, provenance, scale, aggregation, zero policy, and initial lifecycle. Registration MUST reject duplicate IDs, zero EMA half-life, unsupported scale, exhausted global capacity, and exhausted producer capacity before mutation.

Producer, meaning, provenance, scale, aggregation, zero policy, and EMA half-life are immutable. A semantic change requires a newly admitted `FeedId`. Pause, resume, and deactivation change lifecycle only and MUST NOT reinterpret existing values or authored consumer strategies.

Deactivation is terminal for the admitted ID. It retains bounded configuration and current-state truth for deterministic reads but rejects future publication and resume. The package performs no deletion/reuse that could let one ID acquire a new meaning.

## 4. Bounded Admission and Storage

The package MUST expose explicit `MaxFeeds` and `MaxFeedsPerProducer` constants or equivalent tighter bounds. Ordinary publication MUST perform O(1) map access and MUST NOT iterate feeds or producers.

Canonical storage consists of:

- `FeedCount`: bounded admitted-feed cardinality.
- `FeedIds`: bounded duplicate-free forward registry used for reconciliation.
- `Feeds[FeedId]`: immutable semantics plus lifecycle.
- `ProducerIds`: bounded duplicate-free producer registry.
- `ProducerFeeds[ProducerId]`: bounded duplicate-free IDs owned by one producer.
- `Observations[FeedId]`: absent before first accepted sample; otherwise exactly `{ value, updated_at, revision }`.

The pallet stores no sample history, revision-indexed values, subscriber set, arbitrary metadata, or time series. Try-state MUST reconcile feed count, per-producer membership, uniqueness, referenced feed existence, producer equality, lifecycle validity, observation revision nonzero, and configured cardinality bounds.

## 5. Authority

A configured registration origin admits feeds and lifecycle transitions. A configured publication origin MUST resolve to one typed `ProducerId`; signed-account shape alone does not imply producer authority unless the host deliberately defines that mapping.

The package also exposes a narrow runtime producer port accepting `(producer, feed, sample)`. Runtime composition may call this port from a trusted producer adapter, but it MUST pass the same immutable `ProducerId` checked by dispatch publication. Unknown feed, wrong producer, paused feed, deactivated feed, invalid zero, invalid aggregation, and arithmetic/revision overflow fail explicitly.

No publication path may infer authority from `FeedId`, asset direction, account derivation, or caller-controlled provenance.

## 6. Publication and Aggregation

Publication validates the complete transition before writing state or depositing events. A rejected sample leaves configuration, observation, revision, events, and hooks unchanged.

For `LastValue`, the resulting published value equals the submitted sample.

For `Ema`, the first accepted sample initializes the published value directly. Subsequent updates preserve Router parity:

```text
elapsed = max(current_block - updated_at, 1)
alpha = elapsed / (half_life_blocks + elapsed)
result = floor(alpha * sample) + floor((1 - alpha) * previous)
```

The implementation MUST use deterministic integer arithmetic equivalent to the runtime's `Perbill` calculation and MUST reject, rather than saturate, any conversion, denominator, intermediate, or final overflow. No floating-point arithmetic is permitted. Extraction MUST pin exact vectors against the current Router implementation before ownership moves.

A zero sample is accepted only when immutable `ZeroPolicy::Allow` applies. Under `Reject`, zero fails before aggregation. A first accepted zero remains a valid initialized observation and MUST NOT be confused with absent storage; initialization depends on `Observations` presence, not value.

## 7. Revision Semantics

The first accepted sample stores revision `1`. Every accepted sample refreshes `updated_at` to the current block.

Revision increments exactly when the resulting published scalar differs from the stored value. If aggregation produces the same scalar, the pallet refreshes `updated_at` without changing revision, emitting a changed-value event, or calling the change hook.

Revision increment from `u64::MAX` fails closed before any state/event/hook mutation. Consumers receive latest-state revisions, not a promise that every intermediate revision will be delivered.

## 8. Current Read Contract

Consumers query one feed with a nonzero authored `max_age_blocks`. The read result is typed:

```rust
pub enum ObservationState<BlockNumber> {
  Unavailable,
  Uninitialized,
  Fresh(Observation<BlockNumber>),
  Stale(Observation<BlockNumber>),
}
```

`Unavailable` covers unknown or deactivated feed identity. `Uninitialized` covers an admitted active or paused feed without an accepted sample. `Fresh` requires `current_block - updated_at <= max_age_blocks`; the equality boundary is fresh. Older observations are `Stale`.

Paused feeds retain their current Fresh/Stale classification but reject publication. Consumers decide whether paused provenance is acceptable outside shared oracle truth. A zero `max_age_blocks` is invalid consumer input and MUST fail at the consumer's admission boundary rather than becoming an oracle freshness convention.

The pallet exposes bounded current truth only. Historical revision lookup, charts, search, and replay belong to materialized providers and MUST NOT grow consensus storage.

## 9. Transactional Change Hook

The package exposes `OnObservationChanged(feed, revision) -> DispatchResult`. It calls the hook exactly once after computing a changed published scalar and before committing the transition.

Hook work MUST remain O(1), bounded, and independent of subscriber count. The hook reports its conservative Weight separately, and dispatch publication adds that bound to the measured oracle path. Hook failure rolls back observation state, revision, oracle event, and every mutation enclosed by the producer's outer transaction. Equal-output refreshes do not call the hook.

AAA integration may use this hook only for bounded dirty-feed marking. It MUST NOT iterate subscribers or execute actors in producer context.

## 10. Lifecycle Transitions

- `register`: absent to Active/Paused as explicitly admitted; no observation exists.
- `pause`: Active to Paused; idempotent repetition is rejected.
- `resume`: Paused to Active; current observation remains unchanged.
- `deactivate`: Active/Paused to Deactivated; terminal and non-reusable.
- `publish`: Active only; validates producer and sample, then applies aggregation.

Lifecycle calls MUST have constant bounded storage work. They MUST NOT scan producer feeds or observations.

## 11. Events and Errors

The package emits typed events for registration, lifecycle transitions, first publication, changed publication, and refresh without revision change. Events include `FeedId` and the minimum transition fields needed for deterministic consumers; they MUST NOT duplicate arbitrary metadata or historical series.

Errors distinguish duplicate/unknown feed, capacity exhaustion, unauthorized producer, invalid scale, invalid half-life, invalid zero, paused/deactivated lifecycle, invalid transition, arithmetic overflow, revision overflow, and hook failure propagation.

## 12. Weight and Atomicity

Every dispatch and runtime producer path MUST use generated production `WeightInfo` covering RefTime and ProofSize. Benchmark components cover bounded producer index length where registration cost depends on it. Publication weight distinguishes LastValue, first EMA, changed EMA, equal-output refresh, and hook-bearing worst cases where their storage/proof paths differ.

Registration prevalidates every fallible condition before cardinality or reverse-index mutation or executes transactionally. Publication plus producer-side enclosing effects MUST be transactional whenever later producer work can fail. Required hooks fail closed; optional integrations cannot silently weaken revision or rollback semantics.

## 13. Metadata and Compatibility

Before first production genesis, the package uses one coherent canonical SCALE/storage contract without migration ceremony or compatibility shadows. Variant order, field order, storage hashers, defaults, and metadata fixtures are pinned by tests.

After a downstream network launches, that network owns monotonic migrations and compatibility. Semantic feed changes still require a new feed ID; storage migration does not authorize meaning reuse.

## 14. Validation Contract

Completion requires:

- Default, no-std, runtime-benchmark, and try-runtime builds.
- Independent external runtime fixture with no AAA, Router, or DEOS topology dependency.
- SCALE/metadata/storage contract pins.
- Capacity, duplicate, producer-density, and try-state tests.
- Authority, lifecycle, zero-policy, and invalid-configuration tests.
- LastValue and EMA first/equal/change/elapsed/rounding/overflow vectors.
- Revision initialization, refresh-only, increment, and overflow tests.
- Hook cardinality and transactional rollback tests.
- Generated weights bound through the reference runtime with no `()` placeholder.
- Router parity and enclosing failed-swap rollback before Router storage deletion.

The full 0.7.8 gate additionally requires reactive AAA, control-plane, metadata, Wasm, wiki, context, and release evidence owned by their respective contracts.
