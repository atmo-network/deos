# Compatible DTLS Read correction

## Source and input identity

- Published package: `str0m-openssl 0.6.0`, archive `https://static.crates.io/crates/str0m-openssl/str0m-openssl-0.6.0.crate`.
- Archive SHA256: `ae33ce33dcd4dd2d9b6c01e7e425b604be9b811f69511b4387e76d5a36fe606b`, matching the selected frozen010 SDK lock entry. The archive was read from the existing Cargo cache, not fetched anew.
- Published VCS metadata: repository `https://github.com/algesten/str0m`, commit `9b159720be773693dd561dec0fb20c6d7e4aa323`, path `crypto/openssl`. This is the archive's declaration, not independent source-to-binary attestation.
- Original `src/dtls_ossl.rs` SHA256: `1cb1a5bad2ea428a057107ac97556a7985cb18455e05bb73a96c03c596bb66fe`.
- Corrected `src/dtls_ossl.rs` SHA256, including seven regressions: `f583a3c550be2b12979dff2fc2459b52f861ba0bb2c31b8007721dee7cba7e5d`.
- Source finding report SHA256: `71a77f96bb02c7e1ac286141ab8690e2f9144af1cadc853f0bda3d6169510f3d`; independent confirmation SHA256: `f190b90f591c3f4f0ae27cacc4582ae792fb413d8b84c37ee98ea393ec684f41`.
- The published README, `Cargo.toml.orig`, license expression `MIT OR Apache-2.0`, and all source files are retained. The archive contains no separate license text files. Only `.cargo_vcs_info.json` extraction metadata is omitted; its identity is recorded above. No copyright or license text was removed.

## Behavior and compatibility

- The only non-test Rust change is `Ok(n)` → `Ok(max)` in `IoBuffer::read`: report exactly the bytes copied and drained, never more than the destination length.
- With incoming `ab` and a one-byte destination, the original reports two after copying one; with a zero-byte destination it reports two after copying none. The correction reports one and zero respectively and preserves the remaining queue.
- The empty-queue branch still returns `WouldBlock`, including an empty destination. Drain ordering, outgoing data, protocol/cipher policy, public Rust API, dependency requirements, package version and C-facing declarations are unchanged. This is source-level API/ABI preservation, not a compiled native ABI comparison.
- OpenSSL BIO forwards the trait's reported length to C. Provider relevance is confirmed, but actual C read-size/network reachability and exploit consequences are not established. The MTU comment and 30000-byte assertion do not bound every BIO read capacity. No unsafe C or network counterexample was executed.
- A local `[workspace]` isolates this package's regression owner, following existing standalone vendor packages. There is no root Cargo patch, workspace membership, lock update, runtime version change or consumer selection.

## Regression owner and validation

- Seven tests live beside private `IoBuffer` in `src/dtls_ossl.rs::io_buffer_tests`: short, equal and larger destinations; repeated partial reads; zero destination with pending bytes; and empty queue with positive or empty destinations.
- The package-local lock pins OpenSSL `0.10.79`, openssl-sys `0.9.115`, openssl-src `300.6.1+3.6.3` and str0m-proto `0.7.0`, matching selected provider versions. It is a standalone test resolution, not the complete SDK graph. Original dependency requirements are unchanged; the archive's newer OpenSSL resolution is not used.
- From the repository root: `CARGO_NET_OFFLINE=true RUSTUP_TOOLCHAIN=1.94.1 CARGO_TARGET_DIR=/tmp/deos-010-dtls-fix/target ./scripts/ci-local.sh --only tests --manifest-path template/vendor/str0m-openssl/Cargo.toml --package str0m-openssl --features vendored --test-filter io_buffer_tests`.
- Full selected-version provider tests pass through the project CI route with seven discovered regressions; all-target package Clippy with `-D warnings` passes. Existing ABI-matched user-cache Perl modules supply FindBin/IPC::Cmd/Time::Piece; no system-OpenSSL substitution or weakening flags are used. Initial missing-FindBin and items-after-test-module failures remain preserved; moving unchanged tests to EOF resolves the latter without suppression or production changes.
- Package `cargo fmt --check` passes. A local four-space rustfmt setting preserves upstream archive formatting instead of inheriting the reference runtime's two-space style and rewriting unrelated source; the initial inherited-style failure is retained.
- Narrow fallback: Rust `1.94.1`, edition 2024, compiled the verbatim `IoBuffer` definition, inherent methods, Read implementation and seven tests extracted into scratch. All seven pass. Reverting only `Ok(max)` to `Ok(n)` in scratch produces three failures: short destination (2 versus 1), repeated partial read (5 versus 2), zero destination (2 versus 0). Four unaffected cases still pass.
- This fallback is direct compiled Rust trait evidence, not full provider/OpenSSL integration, a native ABI test, whole-package Clippy clearance, an exploit or the earlier JavaScript arithmetic model. Extraction-only dead-code warnings concern the unused outgoing field/method; they were not suppressed.
- Local original archive/extraction, one-line negative control and first-failure records remain under `/tmp/deos-010-dtls-fix/`; full-provider and final corrected test/Clippy logs are under `/tmp/deos-010-dtls-validation/`. Required tests remain project-owned in this package and do not depend on either scratch path or any Skill.

## Consumption boundary

- Frozen010 remains unchanged and does not consume this correction. Its separate Crypto acceptance is neither invalidated nor satisfied by the trait test result; this new material source finding retains its own first-failure mechanism and counterexample.
- Before promotion, a consumer must select this correction in its own graph/lock, refresh the affected native build/artifact and integration evidence, and obtain independent review. The existing minimal-compatible-fix authorization permits this bounded source work; canonical selection, support promises and publication remain separately controlled. No new native candidate, SDK/node rebuild, root Wasm, release or version is produced here. Parent integration, shared documentation and backlog remain separately owned.
