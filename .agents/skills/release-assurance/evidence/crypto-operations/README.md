# RUSTSEC-2026-0387 correction fixture

## Decision and ownership

Candidate: retain 0.5.5, adapt the accepted ctutils 0.4.5 opacity barrier narrowly at Uint::neg_mod's zero mask. No new arithmetic algorithm or dependency. `template/vendor/crypto-bigint/SECURITY-BACKPORT.md` owns source provenance, compatibility, per-operation scope and limitations. Root manifest/lock, advisory ledger, compiled assurance reference and delivery/context changes remain coordinator-owned. This fixture is private acceptance evidence, not project build/CI infrastructure. Its native CLI parse/key/sign/verify assertions belong to this acceptance oracle: any panic invalidates the run, never yields a successful security finding, and touches no production state. Same-line panic-owner annotations were added after compiler capture to retain the previously ignored input under the repository audit; they change comments only, not the recorded arithmetic/call graph. Recorded ELF/IR hashes remain historical capture identities, not newly rebuilt identities.

Official advisory fetched directly from RustSec main: SHA256 `7c9f3f944105638e613e83e97690035198e453d701e56506ee911e1fbfcb46f3`, patched >=0.7.6. Direct GitHub PR1311/1355/1358/1360/1365 patch captures match the earlier finite assessment exactly. Published 0.7.6 requires ctutils 0.4.5; do not mistake PR1360's intermediate borrowed-sub implementation for the accepted final correction. Elliptic-curve ^0.5 requirement prevents a compatible upgrade to 0.7.6.

## Reproduction

Run from repository root, using Rust 1.94.1 explicitly. Native profile matches template production opt3/codegen1/fat LTO/panic-unwind, default x86_64 CPU. `std,signing` selects native P256 ECDSA/PEM/PKCS8 and K256 ECDSA/serde/std; bigint selects generic-array/rand_core/zeroize. The private lock pins selected versions, including serde 1.0.228 and libc 0.2.186.

```bash
F=.agents/skills/release-assurance/evidence/crypto-operations
cargo +1.94.1 test --offline --locked --manifest-path "$F/Cargo.toml" --target-dir /tmp/deos-0387-current-target --profile production --features std,signing --lib
cargo +1.94.1 rustc --offline --locked --manifest-path "$F/Cargo.toml" --target-dir /tmp/deos-0387-current-target --profile production --features std,signing --bin live-native -- --emit=asm,llvm-ir
/tmp/deos-0387-current-target/production/live-native 123456789
cargo +1.94.1 clippy --offline --locked --manifest-path "$F/Cargo.toml" --target-dir /tmp/deos-0387-current-target --features std,signing --lib --bin live-native -- -D warnings
```

For each `TARGET` in wasm32v1-none and wasm32-unknown-unknown, build the no-allocator signing fixture:

```bash
RUSTFLAGS='-C panic=abort -C link-arg=--export=live_neg -C link-arg=--export=live_lincomb -C link-arg=--export=live_sign' cargo +1.94.1 rustc --offline --locked --manifest-path "$F/Cargo.toml" --target-dir /tmp/deos-0387-current-wasm --target "$TARGET" --profile production --features signing --bin live-wasm -- --emit=asm,llvm-ir
```

This Wasm profile deliberately omits serde/alloc; the full no-std graph cannot link standalone without an allocator. The initial full-serde fixture failed for that precise reason. No allocator was added or altered to bypass it. These Wasm results do not qualify full runtime features. Native complete curve features and real secret signers are independently exercised.

## Terminal evidence

- Vendor source copied to standalone `/tmp/deos-0387-current-vendor-test/` because Cargo cannot test a dependency's dev-dependencies through this fixture. Default and release `cargo +1.94.1 test --offline --locked --manifest-path .../Cargo.toml --target-dir /tmp/deos-0387-current-vendor-target --features generic-array,rand_core,zeroize --lib` each pass 218 tests. Standalone source matches the vendor source except Cargo-generated lock/build files. The rejected dependency `-p crypto-bigint --lib` invocation is not a passing test receipt.
- Private production-LTO API suite passes three tests: const and modular negation boundaries; 50 limb borrow boundary combinations; both curve negation/group identities.
- Native live CLI input reaches actual prehash signing and verification for both curves. A black-boxed function pointer prevents the negation witness from being discarded/inlined away. `nm` confirms both `selected_k256_neg` and the real K256 lincomb body in both old and corrected final ELF binaries. The wrapper operands and real signer's scalars are nonconstant; no physical timings collected.
- Matched old source is `/tmp/deos-0387-current-baseline/`, with the identical fixture/lock but patch path pointing at the unmodified selected registry 0.5.5. Final native old negation wrapper has a zero-conditioned `je`; corrected body has no conditional branch. Both real lincomb bodies retain ten conditional branches overall; the inspected zero-mask region has none in either body. The prior no-LTO lincomb issue is not reproduced under fat LTO, so no improvement is claimed there. Other caller branches are not proved secret-independent by this comparison.
- Corrected native sbb, Uint sbb/sub_mod and P256/K256 scalar subtraction/negation wrappers contain no conditional branch. This is finite compiler inspection, not a formal proof of the full signer or advisory-wide safety.
- Matched wasm32v1-none old `live_neg` has `br_if`; corrected export has none. Both corrected Wasm targets link with 21 exports and zero imports. Node 26.8.1 execution checks zero/one negation and actually executes K256 prehash signing successfully. This functional engine execution is not a native-lowering or timing proof.
- Scoped fixture Clippy with `-D warnings` passes. Unmodified vendor source still emits its inherited sidefuzz/unused-import warnings as a dependency. Workspace Clippy is deferred until coordinator integration, not claimed here.
- Completion gate passes with `--skip-cargo-check --skip-runtime-tests --skip-simulator`. It is a source/context audit, not root integration acceptance; unrelated concurrent runtime/governance changes remain untouched.

## Exact artifacts

Captures, terminal receipts and reusable `patch -p1` input are in `/tmp/deos-0387-current-upstream/`. `crypto-bigint-0.5.5-zero-mask.patch` applies to the official 0.5.5 archive; SHA256 `db2028357a08d42b7b6ff363b3039c9e10f3698c256c109f914bd13c69d6a749`. It includes both MSRV updates, the single arithmetic correction/regression and archive VCS-metadata removal, not this package's provenance prose.

| Final artifact | SHA256 |
| --- | --- |
| Native ELF | `8a47942b72fde6e5f0c60b5805c5f3c492ac3e64af1d3a6392acd7ada0e37f5b` |
| Corrected native assembly `live_native-969f44158e559416.s` | `78296ffaf96fe5ffb9b95b0945dc21dbac7e6f70f7096a3a7a67ac9d15ef71e1` |
| Corrected native IR | `df0c9027842b3527774a1b6b238af84d1f06d61d655bb117032abbf364aa3915` |
| Matched old native assembly `live_native-2754989154171d0c.s` | `1ecb619852093182cdf4bdfe5122b2cedaa67d8ef8fefafc30da59a44bb6e084` |
| Matched old native IR | `f29ffb9f8e45276c6535a550f6ec6e3eecd3d901e6e016d3f620bc1a29605bbe` |
| Corrected wasm32v1-none Wasm | `d5bad4f45ac36c074a78a4fbb515e9d33bfb73b5a1e9136fb22c639137560e5b` |
| Corrected wasm32-unknown-unknown Wasm | `43f39b41ad7f69045d43d6f328700f19ee1ec2b4c98da80d0694791434cf40a3` |

## Coordinator integration gate

Do not integrate under the current support constraint. Independent arithmetic/compiler review `51794bb89ab9a3044a775d8ae5478592eea7c5f760fc68d57d5fb6318f47afc4` accepts the exact narrow correction, with final deployment applicability unverified. Independent source/support review `658b2c740ea579d4028376d02f76e06182db1c06ad4120aef5e292714fdf3ea0` rejects treating this local adaptation as an authorized upstream-supported backport. An upstream-supported compatible correction or explicit user authorization to maintain this adaptation is required first; independent review cannot supply that authority. The source inventory is not full deployment-role closure. P256 scalar multiplication combines bigint wide multiplication with curve-owned Barrett reduction, unlike its curve-owned field Montgomery arithmetic.

Only after that gate and applicable source/caller qualification may the coordinator consider `crypto-bigint = { path = "vendor/crypto-bigint" }` in template's existing crates.io patch table. Regenerate the root lock under 1.94.1; require only the 0.5.5 registry source/checksum removal, unchanged version/edges/other package choices. Current root lock remains `75f20fadc68d1f80adfc99bc8dff545cf4876d845d9902244b1704fa6aafb2b3`.

Bind source/caller-specific advisory decisions to the exact vendor manifest/neg_mod hashes and selected parent/features; preserve the registry-identity advisory projection for this path package. Do not install a blanket package/advisory exemption. Native sc-network/DTLS and libp2p-identity callers, actual runtime Wasm reachability/features/host-engine behavior, root validation/workspace Clippy and any refreshed artifact identities remain coordinator integration gates. Reconcile the shared dependency-provenance reference, BACKLOG and CHANGELOG only to the scope actually accepted.

The new private `src/bin/native.rs` and `src/bin/wasm.rs` files match root's broad `bin/` ignore rule; include them explicitly with `git add -f` if retaining this fixture. No staging, commits, publication or history changes performed by this task.
