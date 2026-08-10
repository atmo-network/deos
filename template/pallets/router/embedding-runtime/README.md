# DEOS Router External-Consumer Fixture

This crate compiles `pallet-deos-router` in a minimal independent host runtime. It supplies host-owned TMC, XYK, fee-routing, and Oracle adapters without importing the DEOS runtime or its Actor topology.

## Validation

```bash
cargo check -p pallet-deos-router-embedding-fixture --all-features
cargo clippy -p pallet-deos-router-embedding-fixture --all-targets --all-features -- -D warnings
```

The fixture proves public embedding portability. Router behavior remains owned by package tests; concrete DEOS composition remains owned by runtime integration tests.
