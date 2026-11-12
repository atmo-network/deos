# crypto-bigint 0.5.5: scoped zero-mask hardening

## Source and choice

This package retains crypto-bigint 0.5.5 and its public arithmetic API. The crates.io archive SHA256 is `0dc92fb57ca44df6db8059111ab3af99a63d5d0f8375d9972e319a379c6bab76`, upstream source commit `98f29002552211cbd37dd09d354d1514d84158c4`. Apache-2.0/MIT license texts and upstream notices are retained. Cargo's extraction marker and archive VCS metadata are not shipped.

The official [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) patched range is >=0.7.6. The selected elliptic-curve 0.13.8 requires crypto-bigint ^0.5, so substituting 0.7.6 is not a compatible Cargo update; changing SDK/curve parents is not part of this source treatment. This package must not be classified as an upstream patched release or as generally unaffected by the advisory. It is an unintegrated candidate: independent arithmetic review accepts the narrow correction, but the source/support review rejects root integration under the current upstream-supported-backport constraint. Integration requires an upstream-supported compatible correction or explicit authorization to maintain this local adaptation, followed by complete applicable caller/artifact qualification.

## Upstream-derived correction

[Upstream PR1360](https://github.com/RustCrypto/crypto-bigint/pull/1360), ending at `18637b595667bda5ee8d8a56f098593789a1efd1`, delegates choice arithmetic to ctutils 0.4.5. The published crypto-bigint 0.7.6 archive SHA256 is `6bb08299a6046833a879ace115697f12fd2bd6c8d3e93c909ffc9e0738e7cae0`; its Cargo.toml requires ctutils 0.4.5. That ctutils archive has SHA256 `38d28fb6aa7dd680b3ffd1a3e64e115c177bdba63c0ae7f9cd48129ff05c4fa0`.

The accepted upstream opacity pattern is ctutils 0.4.5 `src/choice.rs::Choice::to_u8`, which calls `core::hint::black_box(self.0)` before mask conversion/selection. File SHA256: `6c78fe58e5b1a842cce21f9098bac24052726a443140cd35f969adea82bca671`. This is a best-effort optimization barrier, not a language-level constant-time guarantee. This adaptation is local, not an upstream-maintained 0.5 branch or a literal application of a newer algorithm patch.

Only `src/uint/neg_mod.rs` changes executable arithmetic: keep the existing `ct_is_nonzero()` result opaque before the existing subtraction/result mask. The value, subtraction, mask, public signature, const evaluation and operation order remain unchanged. No new arithmetic algorithm, dependency, global CtChoice behavior or widened subtraction implementation is introduced. A boundary/const-evaluation unit test is included. Both manifests honestly raise MSRV from 1.65 to 1.86 for const `black_box`; DEOS's selected compiler is 1.94.1.

| Changed file | SHA256 |
| --- | --- |
| `Cargo.toml` | `0e932ff32c70dbf902755a94325c046733a634997724b61736da630c9009bddf` |
| `Cargo.toml.orig` | `ba586b7095dbb8e4742cf99b515350dae3260f3b809710f778a0d1f965762718` |
| `src/uint/neg_mod.rs` | `064487b4a47119696985d6bfdf5b910926fbb0e116314e10c30fe352bcdd5647` |

All other archived source files are unchanged. The source archive plus these changes reproduces this treatment independently of any Skill or private fixture.

## Qualification and limits

Rust 1.94.1 (`e408947bfd200af42db322daf0fadfe7e26d3bd1`, LLVM 21.1.8), native x86_64 default CPU, opt3/codegen1/fat LTO/panic-unwind: a live prehash-signing fixture using selected bigint features and P256/K256 native signing/serialization features retains the real K256 lincomb caller. Its old negation wrapper contains a zero-dependent `je`; this source emits straight-line subtraction/masking without a conditional branch. Both P256/K256 prehash signing and verification execute with a command-line-derived key/hash. Native lincomb's earlier no-LTO mask branches do not reproduce in the matched fat-LTO baseline; do not claim their removal as a measured benefit.

A no-allocator wasm32v1-none signing fixture, opt3/codegen1/fat LTO/panic-abort, exports runtime-input negation, lincomb and signing. The old negation export contains `br_if`; this source removes it. That fixture omits serde/alloc and is not the full DEOS runtime, nor evidence about an execution engine's native lowering. No production runtime Wasm, node/worker build, physical timing, practical exploit, universal constant-timeness or release acceptance is attested here.

The package's 218 unit tests pass in default and optimized builds with generic-array/rand_core/zeroize. The separate three-test API fixture checks const evaluation, zero/one/high-limb/modulus-minus-one negation, borrow boundaries and curve/group identities. Functional tests do not prove side-channel resistance. The unmodified package emits existing `sidefuzz` cfg and `macros::*` import warnings under this compiler; these are not silently repaired.

## Operation-specific remaining boundary

| Advisory operation | Retained source / applicability |
| --- | --- |
| UintRef equality | Affected bigint UintRef API absent; old Uint equality XOR/OR-reduces fixed limbs. DER UintRef is a different type. |
| BoxedMontyForm inversion | Affected API absent; selected P256/K256 scalar inversions use curve-owned implementations. |
| Subtraction | Old Limb::sbb uses widened wrapping arithmetic, not the newer two-overflowing-sub defect. Native neg_mod's older zero-mask lowering is hardened here. |
| Zero counts / size rounding | Old native count primitives and CtChoice accumulators remain; this package does not fix them generally. Inspected curve random_mod counts public ORDER, not secret candidates; newer runtime nlimbs rounding implementation is absent. |
| Division | New UintRef algorithm absent. Old shift/subtract division and reciprocal normalization remain; no secret division call found in selected elliptic-curve/P256/K256/ECDSA/primeorder closure. Not qualified for secret divisors. |
| Montgomery multiplication | New integrated overflowing-add carry construction absent. Old bigint Montgomery uses mul_wide plus separate reduction; no selected signer call to its Residue/DynResidue multiplication was identified. P256 scalar multiplication uses bigint mul_wide followed by curve-owned Barrett reduction; P256 field arithmetic uses curve-owned Montgomery routines. These are distinct paths, not complete bigint exclusion. |

These are separate source/caller classifications, not a package/advisory-wide exception. Source, parent, feature, compiler, target or deployment changes require reassessing the affected classifications. Integration must bind exact source and caller prerequisites, retain the registry-identity advisory view for path dependencies, and qualify the final applicable production graph/artifacts before release claims.
