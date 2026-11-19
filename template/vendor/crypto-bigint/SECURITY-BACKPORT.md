# crypto-bigint 0.5.5: maintained-primitive security backport

## Source and support classification

This is a locally maintained compatibility backport, not an upstream-patched or upstream-maintained 0.5 release. The official [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) patched range remains `>=0.7.6`. The selected elliptic-curve 0.13.8 requires `^0.5`; current non-yanked crypto-bigint 0.7.11 changes its array, randomness, integer and Choice contracts. Changing that version alone is not a compatible parent update.

The baseline is the published crypto-bigint 0.5.5 archive, SHA256 `0dc92fb57ca44df6db8059111ab3af99a63d5d0f8375d9972e319a379c6bab76`, source commit `98f29002552211cbd37dd09d354d1514d84158c4`. Apache-2.0/MIT license texts and notices remain. Cargo extraction markers and archive VCS metadata are not shipped. Using maintained supporting primitives supplies provenance for this mechanism; it does not imply upstream endorsement of this integration or complete advisory remediation.

## Maintained dependencies and prerequisites

| Published source | SHA256 / VCS identity |
| --- | --- |
| crypto-bigint 0.7.11, reference implementation | Archive `914c195c4d7fbc5ed7f88d14cbf85914d5c664967658d43404b263913a477e82`; VCS `6447487610c531bc0d956812fc6e1f6f3f3e3c64` |
| ctutils 0.4.8, exact normal dependency | Archive `261af4dc7a06ea8a527a8717d8cbba3417530d6ea14e1edb3de6b67806be474f`; VCS `ddefdcacb3414980b5105a967a1413eaaf385541` |
| cmov 0.5.4, resolved ctutils dependency | Archive `0c9ea0ac24bc397ab3c98583a3c9ba74fa56b09a4449bbe172b9b1ddb016027a`; VCS `5c7e4f9bb31af81bf766360e836b6d633b84dbff` |

[Bigint PR1360](https://github.com/RustCrypto/crypto-bigint/pull/1360), ending at `18637b595667bda5ee8d8a56f098593789a1efd1`, delegates choice/counting/division support to ctutils. [PR1384](https://github.com/RustCrypto/crypto-bigint/pull/1384), commit `18dda908e47f15ea9723757d8f37f6b2f88bcd13`, raises its ctutils floor to 0.4.8. The complete unmodified ctutils release includes its comparison, mask and target-specific companion corrections, including utils PR1577/1580/1582; this package does not copy an isolated first patch fragment.

Both manifests raise MSRV from upstream 1.65 to 1.87. The reference compiler is Rust 1.94.1. ctutils is required as exact `=0.4.8` with default features disabled; its cmov dependency is resolved by the embedding lock. Both dependencies are Apache-2.0 OR MIT and `no_std`; no allocation or foreign arithmetic library is added. Existing generic-array, rand_core, zeroize, subtle and allocation-facing feature declarations remain.

## Adaptation and compatibility

- Internal `CtChoice` constructors, comparisons, boolean composition and selection delegate to `ctutils::Choice`. Conversion to subtle Choice preserves boolean meaning. Its private representation changes from a word mask to a byte-backed Choice: size/alignment and derived Debug output are not preserved. The old type specifies neither fixed layout nor an encoding; this is source/API compatibility, not representation identity.
- `Limb::sbb` delegates to maintained `borrowing_sub_mask_u32/u64`. For every incoming word, only its most significant bit is subtracted; lower bits are ignored. The returned borrow remains zero or an all-ones word. No novel subtraction algorithm is introduced.
- Limb zero/one counting and internal predicates use maintained primitives. Division's private u32 comparison/selection helpers and reciprocal normalization use the same owners. Existing const entrypoints remain const on both supported pointer widths.
- `neg_mod` retains the original algorithm and source-level zero-mask expression; it benefits from the shared Choice/subtraction owners. There is no standalone local black-box adaptation. Its existing local const/boundary regression is retained alongside `tests/upstream_primitives.rs`.

| Adapted or additional file | SHA256 |
| --- | --- |
| `Cargo.toml` | `8c0f17bf56a9474db1d0b1c0f27771e2ea5cbb38a530f6c08de2c54e601ebe6c` |
| `Cargo.toml.orig` | `10dc030ad19a549b6b28869d87052e51506d9cb70a1a51a5fc38814533a32d09` |
| `src/ct_choice.rs` | `57414da2e7caf7881f053abbaa5ec452ed6505fb934ff065fde51b9942ccc494` |
| `src/limb/sub.rs` | `7f68394917564982a4a01c7a0cba11c404b6f8994cc98ab21bdf2ac85f4cb218` |
| `src/limb/bits.rs` | `d257b15c32c85cc7f63d6e1061f89035432ee3d4b806067ce291501fcfb1bfd2` |
| `src/limb/cmp.rs` | `d2f9619d825bca40d177c54028563b1a9b50b10494dafc739c26bbc4787b4c8e` |
| `src/uint/div_limb.rs` | `321f4d81f3688444ab8ae2c55c25b8f7470a21db88ffa8c2acdf2d7af7e1734a` |
| `src/uint/neg_mod.rs`, original algorithm plus local regression | `59611f8b22690255683a8d3e0ea7790d801b3c2944df8a651db1d9efa3c6e41d` |
| `tests/upstream_primitives.rs` | `020502ee777c76288297f96415e2273bb6194163f5b3fc4dae0ae5bc19db93d1` |

Other archived Rust source files are unchanged. The archive, these files and normal published dependencies reproduce this adaptation without any private agent tooling.

## Qualification boundary

The regressions cover borrow-mask interpretation, carries, const evaluation, zero/one/modulus boundaries and all U256 single-bit count positions. Functional correctness, a particular compiler's emitted code, supplied-artifact reachability and broader constant-time evidence are distinct obligations. Source barriers and passing arithmetic/signature tests are not universal timing guarantees or practical exploit evidence.

ctutils deliberately uses native counting intrinsics on configurations including x86/x86_64 and aarch64. Delegation does not universally timing-qualify counts. Old full-width division remains variable-time in its divisor; bounded inversion retains its bound-dependent contract. The old separated wide-multiply/Montgomery reduction algorithm is not replaced by the newer integrated implementation. UintRef and BoxedMontyForm APIs are absent, which excludes their literal defects, not every old comparison/inversion concern. Later Thumb-1 multiplication/carry corrections are not silently transplanted or declared covered.

An embedding consumer must establish the applicable secret operations and caller/feature/target/compiler domain, including its actual generated artifacts. This source treatment alone does not qualify an entire native node, runtime Wasm, worker distribution or release. Dependency, source, caller, feature, compiler, target or artifact changes invalidate affected qualification; version matching can still flag this patched older package, so a source-aware audit must retain that finding rather than ignore the package/advisory.
