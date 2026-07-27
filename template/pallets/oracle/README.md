# pallet-oracle

Reusable bounded typed scalar observation pallet.

## Purpose

The package admits immutable typed feeds, authorizes typed producers, stores one current scalar observation, applies LastValue or deterministic EMA aggregation, tracks change-only revisions, and invokes one transactional O(1) change hook.

It does not depend on AAA, Axial Router, DEOS topology, off-chain workers, external networks, subscriber iteration, or historical storage. See [`docs/specification.en.md`](./docs/specification.en.md) for the normative contract and [`EMBEDDING.md`](./EMBEDDING.md) for host obligations.

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
