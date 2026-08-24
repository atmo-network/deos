# DEOS Asset Conversion Full-Balance Backport

## Authority

- Upstream crate: `pallet-asset-conversion 31.0.0`.
- Original crates.io checksum: `9372e01886c4a9b625929c2549ac50d67cfe0f8e4daf232d884aecc843fe2557`.
- Upstream pull request: `paritytech/polkadot-sdk#12408`.
- Upstream merge commit: `408895c27a5aff4bac99e956df5426983566f8cb`.
- Parent upstream `src/lib.rs` SHA-256: `060af0ccea54c6d970a67b6f65af94c952d995f22885ed27861f9095ec54a010`.
- Fixed upstream and retained vendored `src/lib.rs` SHA-256: `cc2cd2d15e190d03b19b564c529ee06f0fe6987979c2c08fd532ca40d734c2f0`.

## Exact Patch

The retained source applies only the upstream production changes from PR #12408 to the crates.io 31.0.0 implementation:

- pool reserve reads use full `Assets::balance` rather than `reducible_balance`;
- liquidity withdrawal obtains both reserves through `get_reserves`;
- exact-input and exact-output quote paths obtain both reserves through `get_reserves`.

The upstream post-merge `src/lib.rs` and retained `src/lib.rs` have the same SHA-256 above. The upstream regression is realized at the DEOS runtime integration boundary because the crates.io package does not ship a complete independently buildable dev-dependency set.

## Reachability and Removal

- `template/Cargo.toml` patches crates.io to this directory.
- `cargo tree -p deos-runtime -i pallet-asset-conversion --edges normal` must resolve exactly one package and show this path.
- `template/Cargo.lock` must contain exactly one `pallet-asset-conversion` package and no registry source for it.
- Remove this directory and `[patch.crates-io]` entry when the selected compatible Polkadot SDK release contains PR #12408 with equivalent or stronger full-balance reserve semantics.
- Review expiry: before DEOS 0.7.25 or any Polkadot SDK dependency update, whichever occurs first.
