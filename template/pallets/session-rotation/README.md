# DEOS Session Rotation

`pallet-deos-session-rotation` is a small FRAME pallet that gives a host runtime an explicit, benchmarkable Weight owner for session rotation. It contains no calls, storage, events, validator policy, or session implementation.

- [Specification](docs/specification.en.md)
- [Architecture](docs/architecture.en.md)
- [Embedding](docs/embedding.md)

Validate with:

```bash
cargo test -p pallet-deos-session-rotation --features runtime-benchmarks
cargo clippy -p pallet-deos-session-rotation --all-targets --features runtime-benchmarks -- -D warnings
```
