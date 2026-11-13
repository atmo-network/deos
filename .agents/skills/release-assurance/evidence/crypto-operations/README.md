# RUSTSEC-2026-0387 caller and primitive fixture

## Current candidate and ownership

This private acceptance fixture now selects the **primitive-delegation** root vendor candidate, not the previous neg_mod-only adaptation. Root `template/vendor/crypto-bigint/SECURITY-BACKPORT.md` owns the integration/support decision and operation-specific source scope. The root candidate uses registry ctutils **0.4.8**, no default features, and cmov **0.5.4**; the supporting library raises the minimum Rust requirement to **1.87**. Validation here uses Rust **1.94.1**, not an MSRV test. It is a local compatible integration into bigint 0.5.5, not an upstream-maintained 0.5 release or advisory-wide clearance.

Root Cargo/lock, security notes, shared evidence references, backlog/changelog and integration gates remain coordinator-owned. This fixture and its evidence are deletable Skill infrastructure, not project build/CI dependencies. Root vendor source, accepted PVF/diagnostic workspaces, workers and pins were not modified by this pass. Root lock at capture: `6b987eadec11a98cb96f1801ef1099a1efcf8bd4099f2659499e94bc76198ccc`.

Official advisory SHA256 remains `7c9f3f944105638e613e83e97690035198e453d701e56506ee911e1fbfcb46f3`; upstream patched requirement is >=0.7.6. `/tmp/deos-0387-closure-upstream/{REPORT.md,advisory.md}` records the current primary-source and supporting-library investigation. No blanket package ignore or advisory exemption follows from these tests.

## Executed caller contract

`src/bin/native.rs` accepts **two exactly 64-digit ASCII hexadecimal arguments**, a complete 32-byte scalar and an independent complete 32-byte prehash. Scalars must be nonzero and less than both curve orders; values are not truncated, seeded from a u64, or silently reduced for signing. Hashes accept the complete 256-bit domain. Use synthetic fixture inputs only: the program prints synthetic derived PKCS8 bytes for byte comparison and must never receive real node/account keys.

The CLI exercises actual P256 and K256 key construction, public-point derivation, deterministic RFC6979 ECDSA prehash signing and verification, and emits uncompressed public point, fixed signature and DER signature bytes. It also executes K256 lincomb on runtime-input scalars, including its actual decomposition path. Black-boxed function pointers retain the invoked sbb/count/negation and signing/derivation witnesses as linked callable bodies with CLI-loaded inputs. This does not retain every library wrapper: `selected_u256_sbb`, `selected_p256_sub` and `selected_k256_sub` are absent from both Linux ELFs. `selected_sub_mod` has a standalone symbol only in release; production inlines its live computation into main. Separately emitted library assembly can show standalone bodies without proving linked reachability. Actual curve operations remain live; unused wrapper bodies are not evidence of their own execution.

`selected_webrtc_derive` mirrors **sc-network 0.60.0 `src/webrtc.rs:121–143`**: HMAC-SHA256 of the 32-byte node-secret representation over `substrate-webrtc-p256-v1 || counter`, the same `0u8..255u8` search, explicit zero/order check and `SigningKey::from_slice`. The fixture uses the same P256 SEC1 point, `PrehashSigner<DerSignature>` and PKCS8 APIs as `derive_certificate`; it additionally compares DER signing with fixed-signature conversion. The fixture's independent prehash stands in for the SHA256 TBS digest. It does **not** construct the X509 TBS certificate, perform Ed25519 object conversion, load DTLS, or execute a complete networking node.

Native P256 enabled behavior matches the rooted std/ecdsa/pem/pkcs8/arithmetic feature closure (root additionally names the `default` alias); K256 matches rooted std/serde/ecdsa/arithmetic. Bigint selects generic-array/rand_core/zeroize. `fixture-metadata.json` and `root-curve-features.txt` in the capture bind these selections. HMAC 0.12.1 and SHA2 0.10.9 were already transitive packages; this fixture adds only their direct edges. Its lock adds only ctutils/cmov and bigint's ctutils edge, retaining the previous serde/libc/syn versions.

Root metadata selects libp2p-identity 0.2.14 with ed25519/peerid/rand, **not secp256k1**. Root sp-core 43.0.0 `src/ecdsa.rs:29–42` selects secp256k1 for std signing; K256 signing is the conditional no-std/full_crypto alternative. Therefore K256 signing here is a conditional-role regression, not proof of a selected native identity signer or a secret signer in normal runtime Wasm. The actual enabled secret native caller motivating this fixture is WebRTC's P256 derivation/signing path.

All panics/assertions/expect calls in this private fixture are acceptance-oracle failures: malformed input, rejected keys, exhaustion, signing/verification failure or divergent DER invalidates the run. They authorize no production bypass, account action or security-success finding. CLI parse/domain failures exit nonzero.

## Compiler and target matrix

| Capture | Compiler / inputs | Evidence class |
| --- | --- | --- |
| Linux native `production` | Rust 1.94.1 `e408947bfd200af42db322daf0fadfe7e26d3bd1`, LLVM 21.1.8; x86_64-unknown-linux-gnu default generic CPU; opt3, fat LTO, cgu1, panic unwind; std,signing | Linked ELF, emitted assembly/IR, actual CLI execution and matched pristine baseline |
| Linux native `release` | Same compiler/CPU/features; opt3, no LTO, cgu16, panic unwind | Linked ELF/disassembly, emitted binary and fixture-library assembly/IR, actual CLI execution and matched pristine baseline |
| Apple ARM `production` | Same Rust/LLVM; aarch64-apple-darwin default CPU; opt3, fat LTO, cgu1, panic unwind; std,signing | Exported-caller **staticlib** assembly/IR/archive only; no Apple executable, host execution or matched Apple baseline |
| Wasm `production` | Same Rust/LLVM; wasm32v1-none and wasm32-unknown-unknown; opt3, fat LTO, cgu1, panic abort; signing only; live_neg/live_lincomb/live_sign exports | Linked narrow no-std fixture plus assembly/IR; no full runtime features, new engine execution or native-lowering proof |

No target-cpu/native override, CPU-feature sweep, physical timing/load/thermal experiment or host configuration change was used. Cargo jobs are capped at two. No claim quantifies over every compiler or CPU. ctutils' ordinary native count intrinsics and best-effort barriers are **not universal constant-time proofs**.

The Apple Rust std target was fetched using `rustup target add --toolchain 1.94.1 aarch64-apple-darwin`. A normal binary build emits code but fails at the absent Apple linker/SDK. `apple-staticlib-codegen.sh` records the exact subsequent rustc invocation with crate type staticlib, preserving opt3/fat-LTO/cgu1/features and emitting the exported signing/derivation and primitive bodies without a system executable linker. Its unused CLI helper warnings are expected under this alternate artifact kind. `apple-codegen-only.sh` separately succeeds for per-crate binary IR/assembly **without whole-program LTO**; do not confuse it with the staticlib's post-LTO bodies or an executable.

## Validation and input partitions

The initial primitive capture is under `/home/llb/.cache/deos-0387-security/` (`C` in the historical reproduction commands below). Its native artifacts predate the parser correction. The reviewed capture is exclusively `/home/llb/.cache/deos-0387-security-reviewed/`; neither earlier capture nor its manifests/reports was modified. See the reviewed binding section below.

- Byte-identical standalone root vendor source/tests: **218 library + 2 upstream_primitives tests pass**, release, generic-array/rand_core/zeroize. The extra library test versus the upstream scratch candidate is the retained old neg_mod boundary regression. `vendor-source.SHA256SUMS`, `vendor-copy-diff.txt` and `final-comparison-receipt.log` bind the copy. Cargo does not expose a dependency's own dev tests through this fixture.
- Retained private production suite: **3 library + 1 native input-domain test pass**. Borrow-mask coverage is 150 combinations, including noncanonical incoming words with MSB semantics. Domain tests cover malformed length, nonhex/non-ASCII input, full-width top-bit preservation, zero and out-of-order keys.
- Five valid full-width CLI vectors pass sign/verify and match pristine registry 0.5.5 byte-for-byte in **both** native profiles: one; top-bit-plus-low-bit; independently patterned 256-bit key/hash; P256 order-minus-one; alternating-bit key/hash. Hashes include zero and all ones. `vectors.txt` pins exact inputs; each output has 10 records, including derived PKCS8. These finite witnesses validate full-width input handling, not every possible scalar.
- Native fixture Clippy `-D warnings` passes. Inherited vendor sidefuzz/unused-import warnings remain dependency warnings. Workspace Clippy and completion are deliberately deferred to coordinator integration while fixture source is owned here.
- Both narrow Wasm builds pass. This pass does not reuse old Wasm engine results as evidence for the changed vendor.

`capture.log` retains vendor tests and initial fixture tests/Clippy. Its initial CLI vector above the P256 order is correctly rejected; the vector was corrected to order-minus-one. `resume.log` captures the successful initial native comparisons and Wasm builds. `final-native.log` captures fresh tests/Clippy and both comparisons after adding the exact DER signing API; an incidental tail-slice command-composition error occurs **after** those comparisons, not in Rust validation. `final-comparison-receipt.log` separately records successful final comparisons and copy equality. Initial lock-resolution drift was restored before final captures; an earlier bad function-pointer inference was corrected before passing tests. No failed command is counted as a pass.

## Finite branch and dataflow findings

| Inspected region | Finding and classification |
| --- | --- |
| x86 fat-LTO primitive body shapes | Inspected standalone emitted sbb/Uint sbb/sub_mod/scalar sub/neg bodies contain no conditional jumps, using shifts/bitwise arithmetic, opaque loads and masks/setcc/cmov. Emission alone is not linked liveness: Uint sbb and scalar sub wrappers are absent from both ELFs; production sub_mod is an inlined live computation, not a retained symbol. Invoked sbb/count/negation wrappers are retained linked callable bodies. |
| Matched x86 K256 negation | Pristine `selected_k256_neg` has a zero-conditioned `je`; corrected body has none. Production neg_mod itself is otherwise the original algorithm: the changed shared primitives supply the difference. This is the new primitive candidate, not reuse of the earlier neg_mod-only ELF. |
| x86 counts | `selected_limb_clz` lowers to initialized `bsrq` plus xor; ctz to initialized `rep bsfq` (decoded as tzcnt on BMI CPUs). Uint ctz uses four fixed-limb counts and masks/cmov. No conditional jumps in these bodies; instruction latency/zero behavior across CPUs is not established by that observation. IR still contains llvm.ctlz/cttz, not a universal hardened software count. |
| Actual K256 fat-LTO lincomb | Both matched bodies retain **10 conditional jumps**. Inspection of the decomposition/negation prefix finds mask/select arithmetic, not a scalar-zero early branch. Remaining jumps concern public xks/tables/digits slice lengths, bounds and fixed 32-digit traversal. This is local dataflow classification; called projective/table/field bodies and the whole signer are not certified by counting these jumps. No fat-LTO lincomb improvement is claimed. |
| Native no-LTO call boundary | Linked disassembly retains selected_sbb/count bodies and calls from scalar negation into actual curve/bigint bodies (K256 lincomb calls bigint neg_mod). The inspected 113-line linked neg_mod callee has no conditional jump or call. Do not infer this from the wrapper alone. Both profile outputs match the baseline; no blanket no-LTO signer branch clearance follows. |
| Actual P256 signing | The emitted signer calls RFC6979, curve scalar multiplication and field arithmetic. Its scalar inverse exponent branches use literal public order-minus-two constants and fixed loop counters (`p256 scalar.rs:140–186`), not the secret base. Other branches test key validity, nonce validity/invertibility, signature r/s nonzero and Result propagation. Those are conditional protocol/error boundaries, **not all public branches**. Their presence prevents a blanket branch-free signer claim. |
| WebRTC derivation | HMAC-derived candidate range/zero rejection and counter exhaustion depend on candidate material, exactly as the existing caller specifies. The fixture preserves that policy; it does not label the whole derivation constant-time or alter it to hide rejections. |
| Apple ARM exported primitives | Post-LTO staticlib sbb/sub/selection witnesses have shifts/bitwise/cset/csel and no conditional branch; clz is `clz`, ctz is `rbit; clz`. This is compiled-target inspection, not Apple runtime/CPU evidence. Full Apple signer dataflow is not cleared. |
| Wasm negation | Current wasm32v1-none `live_neg` wrapper calls real K256 reduction/negation; the inspected 323-line negate callee has neither `br_if` nor calls. The no-std exports retain runtime pointer inputs for signing and lincomb. This narrow lowering says nothing about a production Wasm engine's native code. |

`native-selected.s.txt`, `native-select-sub.s.txt`, `k256-lincomb.s.txt`, `p256-sign-prehash.s.txt`, the matched baseline extracts, `candidate-{production,release}.disasm`, `apple-selected.s.txt`, `release-negmod-callee.s.txt`, `wasm-live-neg.s.txt` and `wasm-negate-callee.s.txt` provide inspectable regions. Complete assembly and IR remain beside their compiled artifacts.

Count exclusions are **caller-specific**: `elliptic-curve scalar/primitive.rs:68–71` passes public `C::ORDER` through `ScalarPrimitive::random` to bigint `random_mod`; bigint `uint/rand.rs:36` computes that modulus's bits_vartime. This is not secret count input. The exercised WebRTC derivation does not call random_mod at all. It does not justify declaring count operations harmless for arbitrary secret operands.

Bigint's old secret-divisor division remains explicitly variable-time; generic inversion and separated Montgomery algorithms are not universally repaired or proved by delegation. P256 scalar multiplication uses bigint wide multiplication plus curve-owned Barrett reduction, and its field Montgomery code is curve-owned; K256 has its own scalar/field implementations. Thus the selected secret curve paths are not evidence of using bigint's generic secret-divisor division, generic modular inversion or generic Montgomery engine. Do not generalize this source exclusion to unrelated consumers, alternate features or absent targets such as Thumb-1.

## Historical reproduction recipe

The commands below describe the pre-review capture only; do not rerun them into the now read-only original cache. For the reviewed correction, use the reviewed cache's `validate.sh`, `apple-refresh.sh` and `reuse-proof.sh` as recorded commands, selecting another newly owned output directory before reproducing. Keep target directories fresh for a new capture identity. The historical cache retains expanded commands, vectors, copied baseline manifest/lock and `key-artifacts.SHA256SUMS`; its whole manifest is no longer a current-source validation command.

```bash
F=.agents/skills/release-assurance/evidence/crypto-operations
C=/home/llb/.cache/deos-0387-security
CARGO_BUILD_JOBS=2 cargo +1.94.1 test --offline --locked --manifest-path "$F/Cargo.toml" --target-dir "$C/candidate-target" --profile production --features std,signing --lib --bin live-native
CARGO_BUILD_JOBS=2 cargo +1.94.1 clippy --offline --locked --manifest-path "$F/Cargo.toml" --target-dir "$C/candidate-target" --features std,signing --lib --bin live-native -- -D warnings
bash "$C/native-compare.sh"
bash "$C/apple-staticlib-codegen.sh"
sha256sum -c "$C/key-artifacts.SHA256SUMS"
```

For a fresh matched pristine baseline, copy this fixture into a new owned cache directory, change only its bigint patch path to the unmodified registry 0.5.5 source, and resolve **only** that package (`cargo +1.94.1 update --offline -p crypto-bigint --manifest-path ...`). The retained baseline lock differs only by removal of ctutils/cmov and bigint's ctutils edge. Use identical source files, compiler, profiles, features and input vectors; `native-compare.sh` records both builds and `cmp` operations. Do not substitute the previous u64-seeded fixture's binaries.

For each TARGET in wasm32v1-none and wasm32-unknown-unknown:

```bash
CARGO_BUILD_JOBS=2 RUSTFLAGS='-C panic=abort -C link-arg=--export=live_neg -C link-arg=--export=live_lincomb -C link-arg=--export=live_sign' cargo +1.94.1 rustc --offline --locked --manifest-path "$F/Cargo.toml" --target-dir "$C/candidate-wasm" --target "$TARGET" --profile production --features signing --bin live-wasm -- --emit=asm,llvm-ir
```

## Pre-parser-correction artifact identities

The original `key-artifacts.SHA256SUMS` binds the pre-parser-correction capture, not the current complete source tree. Its live native/README paths now intentionally fail verification; do not rewrite that historical manifest or claim it is current. Frozen `pre-review-source/` copies preserve its source bytes, including native SHA256 `d2481a8bc676c37c382ba2cb6e5ee52ecece73f910feffe4631d3f033f793192`. The following native ELFs and Apple outputs remain historical; unaffected Wasm evidence is reused only through the explicit equivalence below. `vendor-source.SHA256SUMS` binds all selected vendor Rust source/tests/manifests (manifest digest `4293416eb68565ee8a904046b4adc400183fabb0caec4d4dc9e3c7c0b4d381af`); the corresponding pristine registry source manifest digest is `64633c98462ae57b2713f2abffb2021079f3652b291fc287837410979c564b41`. Paths are part of these manifest identities.

| Final artifact | SHA256 |
| --- | --- |
| Fixture Cargo.lock | `298077ae30cf67f7e43f3760329d2e71b211a94a5961fe96e1fa651c76575920` |
| Full-width native source | `d2481a8bc676c37c382ba2cb6e5ee52ecece73f910feffe4631d3f033f793192` |
| Corrected Linux production ELF | `7e77484b1e613723225c45491a214ba50897a2da1612a8fb7e4a8377cce43e73` |
| Pristine Linux production ELF | `04db01ac0f7614b11b9a07fcbc388e8a7230b51343a17454df0d09d103ffc049` |
| Corrected Linux release ELF | `2e128d8cc4ea110ef87cbabbcc20d9722da63ca186b2f12beb7b3213b7496c69` |
| Pristine Linux release ELF | `a68e7d6d262568592320d19104ed2381086492178103598c161a4ffa7dd8cde6` |
| Corrected production assembly / IR | `9faf1c07cf99d9cc2cfc675a2f47a3e16d17c55ccc8803e1f56e7b6cec53708f` / `134491d9002e292ed880f0c0e28ed8dd6486c5cae34646dcc55a164cf888e0ce` |
| Apple ARM staticlib assembly / IR | `65693a6c298c9595a5ef4ac7b25b966fdd5ca5b608c52fcc708df58f36a85252` / `ccbf21c39b2a3d340e91364f39a48f6966012ba42c7f05638b43ce8f85b6fae1` |
| wasm32v1-none / wasm32-unknown-unknown | `66244ba243fcec06b0cc67efefe53277a07c0396743ae078784e82e9af0bbea9` / `bac89d9747614d4c047876f369ace7f13ab7c9529cea1081b6dd0e7c3f694b6e` |

## Reviewed parser correction and current bindings

`parse_hex` now requires every byte to be an ASCII hex digit before pair conversion; `from_str_radix` alone also accepted `+1` pairs. The single native domain test rejects that pair at all 32 positions in the parser shared by key/hash. No crypto function or dummy liveness call was added.

The reviewed cache contains `validate.sh`, `validation.log`, five pinned vectors and both profiles' fresh candidate/pristine native ELFs, assembly/IR, linked disassembly and symbol tables. Both profiles match all five valid vectors byte-for-byte; all four candidate CLI executions with `+1` in key/hash exit nonzero. The changed native test passes (1 test), and changed-bin Clippy with `-D warnings` passes. Existing dependency warnings are not new fixture warnings. Compiler, generic CPU, features and profiles match the matrix above; targets were independent reflink-capable copies, not hardlinks or writes into the old cache.

| Reviewed artifact | SHA256 |
| --- | --- |
| Native source | `5716521738c456125431b7dfd42fd7894ca178d2bbc7adeb5fa69b179c173117` |
| Candidate production ELF | `0a67d28bd8e3cf25b9986c5f8a0fc12808604c570bf0f4bff06760aadb0a1cdd` |
| Pristine production ELF | `c2f2cf91bcce67de5e6dbaa8642d6796d2b25e10cd2a4f6c9c31691d652604f2` |
| Candidate release ELF | `5c5c907c9d4c053267e19da0b7fbcaa4aa8e944330b7bbf017e398fded1fcc79` |
| Pristine release ELF | `047f1d01a61bb2611533090a5701a47ad6a4f703d399e8b2e555e8ef47b9e87f` |
| Refreshed Apple staticlib assembly / IR | `811a6cc521f7445ff68019b4bc6533ba9c45ba65e9cb73f033f03128da56afc7` / `5894dd4420f106d58c272f0b0d48d6223c2ca6b62a262af0579638f51ee094a3` |

`apple-refresh.sh` recompiles the corrected native source into a post-fat-LTO staticlib/archive plus assembly/IR, using the unchanged old Apple dependency rlibs read-only. It creates no Apple executable and executes no Apple CLI. Unused CLI helpers are not retained by this artifact kind; old Apple evidence does not transfer whole-CLI qualification to the new source. Fresh Linux selected-body disassembly retains the sbb/count/negation body shapes (no conditional jumps in production); linked symbol tables establish the wrapper distinctions above, not universal constant-time behavior.

`current-artifacts.SHA256SUMS` in the reviewed cache binds self-contained frozen current fixture sources, baseline sources/manifests, commands, outputs, logs and fresh codegen. `reuse-proof.log` records exact source comparisons: unchanged fixture Cargo/lock/lib/Wasm, complete vendor source manifest and pristine registry manifest, and the prior artifact identities. The original 218+2 vendor tests, 3 private library tests, both Wasm builds and root metadata are reused, not rerun. These depend on identical source/manifest/compiler/profile/feature inputs; they do not depend on the corrected native-only parser. Root Cargo/lock identities are separately checked. Unchanged artifacts retain their original hashes; historical native ELFs are never substituted for the refreshed ones. Baseline native source equals the corrected candidate; its manifest/lock retain the original pristine-only dependency difference.

## Historical capture, not current qualification

The following identities belong **only** to the earlier neg_mod-only adaptation and u64-seeded fixture under `/tmp/deos-0387-current-*`. They are not regenerated identities or evidence for the current primitive candidate. That candidate's previous support rejection is historical; the present user authorization and reviewed primitive mechanism are materially different. No old completion-gate result is inherited by this source change.

| Historical artifact | SHA256 |
| --- | --- |
| Old corrected native ELF | `8a47942b72fde6e5f0c60b5805c5f3c492ac3e64af1d3a6392acd7ada0e37f5b` |
| Old corrected native assembly | `78296ffaf96fe5ffb9b95b0945dc21dbac7e6f70f7096a3a7a67ac9d15ef71e1` |
| Old corrected native IR | `df0c9027842b3527774a1b6b238af84d1f06d61d655bb117032abbf364aa3915` |
| Old matched pristine native assembly | `1ecb619852093182cdf4bdfe5122b2cedaa67d8ef8fefafc30da59a44bb6e084` |
| Old matched pristine native IR | `f29ffb9f8e45276c6535a550f6ec6e3eecd3d901e6e016d3f620bc1a29605bbe` |
| Old corrected wasm32v1-none | `d5bad4f45ac36c074a78a4fbb515e9d33bfb73b5a1e9136fb22c639137560e5b` |
| Old corrected wasm32-unknown-unknown | `43f39b41ad7f69045d43d6f328700f19ee1ec2b4c98da80d0694791434cf40a3` |

## Remaining integration boundary

This pass supplies concrete root-selected vendor regression, full-width functional compatibility and finite compiler/caller evidence. It does not prove universal constant-time behavior, practical exploitability, advisory-wide correction, complete DTLS-node behavior, Apple executable behavior, production-runtime/engine lowering, or unchanged published worker/Omni artifacts. The source-aware advisory disposition, independent final review, root validation/Clippy/completion and exact deployed artifact-role closure remain coordinator-owned. No root full Clippy, completion gate, PVF rebuild, worker rerun, publication, staging, commit or history mutation was performed. Native/Wasm bin sources are already tracked; no force-add or ignore-rule change is needed.
