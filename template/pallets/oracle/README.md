# pallet-deos-oracle

Reusable DEOS Oracle package for bounded typed scalar observations.

## Purpose

The package admits immutable typed feeds, authorizes typed producers, stores one current scalar observation, applies LastValue or deterministic EMA aggregation, tracks change-only revisions, and invokes one transactional O(1) change hook.

It does not depend on Actors, DEOS Router, DEOS topology, off-chain workers, external networks, subscriber iteration, or historical storage. See [`docs/specification.en.md`](./docs/specification.en.md) for the normative contract, [`docs/architecture.en.md`](./docs/architecture.en.md) for the reusable package implementation map, and [`docs/embedding.md`](./docs/embedding.md) for host obligations.

Concrete reference-runtime composition belongs to [`docs/oracle.integration.en.md`](../../../docs/oracle.integration.en.md).

## Validation

```bash
cargo test -p pallet-deos-oracle
cargo check -p pallet-deos-oracle --no-default-features
cargo check -p pallet-deos-oracle --features runtime-benchmarks
cargo check -p pallet-deos-oracle --features try-runtime
cargo clippy -p pallet-deos-oracle --all-targets -- -D warnings
cargo test -p pallet-oracle-embedding-fixture
cargo check -p pallet-oracle-embedding-fixture --no-default-features
```

Production integration additionally requires generated weights, an independent runtime fixture, maximum-density/try-state evidence, metadata pins, and transactional producer rollback tests.
