# Embedding DEOS Router

## Host Contract

An independent runtime configures native and fungible ledgers plus host-owned adapters for TMC minting, single-pool XYK execution, fee routing, and directional price observations. The host also supplies governance origin, bounded constants, account identities, production weights, and benchmark setup.

The Router does not require DEOS Actor accounts or concrete DEOS market policy. A host may use any account topology and adapters that preserve the public traits and transactional facts.

## Required Guarantees

- `AssetConversionApi` identifies and executes exactly one canonical pool per call.
- Execution reports measured recipient output and, for exact output, measured caller spend.
- `TmcInterface` reports recipient allocation rather than total issuance.
- `FeeRoutingAdapter` participates in the Router transaction.
- `PriceOracle` validates directional references and publishes pre-execution observations.
- `MaxLpPairs` bounds the reverse LP index.
- `WeightInfo` covers the host's worst-case bounded execution. The packaged `SubstrateWeight` and `()` are hand-written placeholders that report zero ProofSize and no database access, while the DEOS reference runtime measures a direct XYK swap at 13998 ProofSize with 25 reads and 12 writes. Generate weights against your own runtime and bind those.

## Evidence

`../embedding-runtime` compiles the pallet in an independent minimal runtime with host-local adapters and no dependency on `deos-runtime` or `pallet-deos-actors`.

```bash
cargo check -p pallet-deos-router-embedding-fixture --all-features
cargo clippy -p pallet-deos-router-embedding-fixture --all-targets --all-features -- -D warnings
```

Package tests own reusable behavior. Concrete DEOS pools, Oracle wiring, Actor policy, fees, runtime weights, and rollback evidence belong to the root integration surfaces and runtime tests.
