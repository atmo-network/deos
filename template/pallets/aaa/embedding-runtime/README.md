# AAA External-Consumer Embedding Fixture

This non-published workspace package provides executable portability evidence for `pallet-aaa`. Its flat placement beside the pallet's `src/` expresses pallet ownership without a one-item category directory, while its separate Cargo package preserves an external-consumer dependency edge. It is not a second DEOS product or a downstream economy template.

The fixture deliberately uses:

- Local `u64` accounts, `u32` asset identifiers, and native balances.
- Zero genesis System AAAs.
- A two-slot User actor policy and smaller queue/wakeup bounds than DEOS.
- Native asset operations and fee collection implemented through `pallet-balances`.
- A runtime-local transaction extension proving successful and failed Executive transfer ingress without event scanning.
- Default-deny funding authority and deterministic unsupported DEX, staking, and liquidity-donation adapters in the default profile.
- An opt-in `dex-fixture` profile with one fixed-rate exact-output pair and one explicitly Temporary exact-input fixture, with no imported pool topology.
- Mutable User and System Continuation coverage for cooldown, suffix resumption, concurrent Executive ingress, cancellation, pure close, and try-state integrity.
- No DEOS primitives, TMCTOL topology, governance catalog, Axial Router, TMC, or staking pallet.

Run its focused evidence from `template/`:

```bash
cargo test -p pallet-aaa-embedding-fixture
cargo test -p pallet-aaa-embedding-fixture --features dex-fixture
cargo test -p pallet-aaa-embedding-fixture --features try-runtime
cargo test -p pallet-aaa-embedding-fixture --features dex-fixture,try-runtime
cargo check -p pallet-aaa-embedding-fixture --no-default-features
cargo clippy -p pallet-aaa-embedding-fixture --all-targets --all-features -- -D warnings
```

Capability, ingress, Continuation, lifecycle, metadata, and optional-adapter evidence belongs in this crate so failures expose pressure on the public AAA embedding contract rather than borrowing DEOS runtime helpers.
