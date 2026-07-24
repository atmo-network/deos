# Independent AAA Embedding Runtime

This workspace member provides executable portability evidence for `pallet-aaa`. It is a minimal runtime fixture, not a second DEOS product or a downstream economy template.

The fixture deliberately uses:

- Local `u64` accounts, `u32` asset identifiers, and native balances.
- Zero genesis System AAAs.
- A two-slot User actor policy and smaller queue/wakeup bounds than DEOS.
- Native asset operations and fee collection implemented through `pallet-balances`.
- A runtime-local transaction extension proving successful and failed Executive transfer ingress without event scanning.
- Default-deny funding authority and deterministic unsupported DEX, staking, and liquidity-donation adapters in the default profile.
- An opt-in `dex-fixture` profile with one fixed-rate exact-output pair and no imported pool topology.
- No DEOS primitives, TMCTOL topology, governance catalog, Axial Router, TMC, or staking pallet.

Run its focused evidence from `template/`:

```bash
cargo test -p aaa-embedding-runtime
cargo test -p aaa-embedding-runtime --features dex-fixture
cargo check -p aaa-embedding-runtime --no-default-features
cargo clippy -p aaa-embedding-runtime --all-targets --all-features -- -D warnings
```

Additional S11 capability, ingress, lifecycle, metadata, and optional-adapter evidence belongs in this crate so failures expose pressure on the public AAA embedding contract rather than borrowing DEOS runtime helpers.
