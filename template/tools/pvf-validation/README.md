# Offline PVF Validation

This non-publishable workspace tool owns finite acceptance of exported PVF fixtures through the SDK ValidationHost and real version-checked prepare/execute workers. It is not a runtime dependency, custom node, relay client, scheduler, performance benchmark or production certificate.

## Inputs and Results

- `src/main.rs` binds exported ValidationParams, persisted validation data, sealed block, expected head and complete ValidationResult, using opaque extrinsics rather than importing the DEOS runtime.
- The caller discovers existing `*.params.scale` fixtures and fails on an empty directory, inconsistent inputs, trailing SCALE bytes or any output-field mismatch.
- Each fixture has valid, corrupt-seal, mismatched-parent-root and corrupt-compact-proof cases. Only typed `WorkerReportedInvalid` satisfies a negative; timeout, preparation, cancellation, infrastructure or unexpected success fail.
- Parent-root mutation also breaks parent-head continuity; the separate compact-proof case keeps headers/seals unchanged and exercises proof decoding.
- Receipt PVD/PoV/code/head/commitment fields are locally bound. Relay parent, core, session and erasure context are synthetic V2 inputs, not authenticated relay provenance.
- Default SDK ExecutorParams, 360-second preparation and two-second Backing limits are retained. Owned short cache paths avoid Unix-socket limits; host handles are dropped and task termination is checked.

## Running

Generate candidates with the runtime-owned `actor_finalization_close --pvf-fixture WASM NEW_DIR` or applicable `actor_retention_close` export. Then run from the repository root:

```bash
DEOS_BINARY_DIR=/absolute/pinned/binary/bundle \
  ./scripts/pvf-assurance.sh --fixtures /absolute/export --para-id 2000
```

The parachain ID is a caller input, not tool-owned genesis policy. The default Wasm is the canonical production-profile output; `--wasm PATH` selects another explicit artifact. The public script verifies the exact prepare/execute worker checksums through `01-download-binaries.sh --check --only`, builds this package and preserves the selected Wasm identity. No unrelated node/bencher binary or full-bundle marker is required; the host still checks both worker versions against its SDK. Its `--help` declares prerequisites and side effects.

## Prerequisites and Safety

- Native SDK compilation requires the pinned Rust toolchain, C/C++ tools, Perl core modules including FindBin/IPC::Cmd and `protoc`; system OpenSSL development libraries or the SDK's vendored OpenSSL build must be available.
- Standard `PERL5LIB`, `PROTOC` and `LD_LIBRARY_PATH` may select an isolated local toolchain; no system package installation or executable-bundle download occurs inside assurance; Cargo may fetch locked build dependencies.
- Secure-validator mode is required by default. `--insecure-local` explicitly permits best-effort hardening for trusted local code and is reported as narrower evidence, never silently selected.
- Candidate inputs are trusted local fixture files, not an untrusted network ingest surface. No relay history, public setup, live-node composition, resource maximum or tariff sufficiency follows from acceptance.

## Validation

Use `scripts/ci-local.sh --only tests --package deos-pvf-validation` and scoped Clippy for local result-classification checks. Workspace checks include the package. Real-worker acceptance is the separate opt-in `pvf-assurance.sh` route and needs exported candidates and both checksum/version-verified workers. The [SDK integration owner](../../../docs/actors.integration.en.md#sdk-dependency-line) owns current artifact identity and acceptance scope.
