# DEOS Actors External-Consumer Embedding Fixture

This non-published workspace package provides executable portability evidence for `pallet-deos-actors`. Its flat placement beside the pallet's `src/` expresses pallet ownership without a one-item category directory, while its separate Cargo package preserves an external-consumer dependency edge. It is not a second DEOS product or a downstream economy template.

The fixture deliberately uses:

- Local `u64` accounts, `u32` asset identifiers, and native balances.
- Zero genesis System Actors.
- A two-slot User actor policy and smaller queue/wakeup bounds than DEOS.
- Native asset operations and fee collection implemented through `pallet-balances`.
- A runtime-local transaction extension proving successful and failed Executive transfer ingress without event scanning.
- Default-deny funding authority and deterministic unsupported DEX, liquidity, and staking adapters in the default profile.
- An opt-in `dex-fixture` profile with one fixed-rate exact-output pair and one explicitly Temporary exact-input fixture, with no imported pool topology.
- Mutable User and System `ActorRunState` coverage for open/finalized nonce separation, cursor and eligibility ownership, immutable Opening/funding facts, exact outcomes, cooldown, suffix resumption, concurrent Executive ingress, cancellation, pure close, and try-state integrity.
- No DEOS primitives, TMCTOL topology, governance catalog, DEOS Router, TMC, or staking pallet.

Run its focused evidence from `template/`:

```bash
cargo test -p pallet-deos-actors-embedding-fixture
cargo test -p pallet-deos-actors-embedding-fixture --features dex-fixture
cargo test -p pallet-deos-actors-embedding-fixture --features try-runtime
cargo test -p pallet-deos-actors-embedding-fixture --features dex-fixture,try-runtime
cargo check -p pallet-deos-actors-embedding-fixture --no-default-features
cargo clippy -p pallet-deos-actors-embedding-fixture --all-targets --all-features -- -D warnings
```

Capability, ingress, run-state, lifecycle, metadata, and optional-adapter evidence belongs in this crate so failures expose pressure on the public Actors embedding contract rather than borrowing DEOS runtime helpers.
