# Offline PVF Validation

This non-publishable isolated-workspace tool owns finite acceptance of exported PVF fixtures through the SDK ValidationHost and real version-checked prepare/execute workers. It is not a runtime dependency, custom node, relay client, scheduler, performance benchmark or production certificate.

The result API returns validation output/errors, not worker CPU duration. Optional `--cpu-trace FILE` captures the existing SDK assignment/success events; execution histograms and caller elapsed time are never substituted. The [SDK integration owner](../../../docs/deos/actors.integration.en.md#sdk-dependency-line) records timing ownership and the current evidence boundary. Capture is diagnostic, not a calibrated performance certificate.

## Inputs and Results

- `src/main.rs` binds exported ValidationParams, persisted validation data, exactly one sealed block, expected head and complete ValidationResult, using opaque extrinsics rather than importing the DEOS runtime. Bundled multi-block candidate data refuses; this tool's one-block boundary is narrower than Cumulus's general validation input.
- The caller discovers existing `*.params.scale` fixtures and fails on an empty directory, inconsistent inputs, trailing SCALE bytes or any output-field mismatch.
- By default, each fixture has valid, corrupt-seal, mismatched-parent-root and corrupt-compact-proof cases. Only typed `WorkerReportedInvalid` satisfies a negative; timeout, preparation, cancellation, infrastructure or unexpected success fail.
- Parent-root mutation also breaks parent-head continuity; the separate compact-proof case keeps headers/seals unchanged and exercises proof decoding.
- Receipt PVD/PoV/code/head/commitment fields are locally bound. Relay parent, core, session and erasure context are synthetic V2 inputs, not authenticated relay provenance.
- Default SDK ExecutorParams, 360-second preparation and two-second Backing limits are retained. Owned short cache paths avoid Unix-socket limits; host handles are dropped and task termination is checked.

## Case Selection

The public script and direct host accept explicit `--positive-only`. It requires exactly one discovered `*.params.scale` fixture and selects only its complete positive check. Rust fixes that fixture name before worker-version probes, trace installation, artifact-cache creation or ValidationHost startup; zero/multiple fixtures refuse instead of selecting the first. New directory entries cannot expand the fixed selection. The selected files still pass the unchanged complete input and expected-result checks.

Positive-only constructs no corrupt inputs. It prints `PVF_SELECTION_V1 ... full_suite=false`; optional trace session/completion records add `case_selection: positive_only_single_fixture` without changing the CPU event contract. Success proves only the one positive candidate, not negative-suite acceptance, ancestry, physical bounds or tariff sufficiency. Without the flag, discovery, four-case order/negative flags, executor parameters, secure mode and full-result/typed-negative checking remain unchanged.

The wrapper still performs its declared prerequisites and host build before Rust fixture admission; this is not a dry-run option. `./scripts/pvf-assurance.sh --self-test` separately checks argument refusal and exact argv forwarding using owned temporary fixtures/stubs, without Cargo, binary checks, workers or candidate execution. Tool unit tests cover selection with synthetic codec-only inputs, not real PVF acceptance.

## Running

Generate candidates with the runtime-owned `actor_finalization_close --pvf-fixture WASM NEW_DIR`, current `actor_wide_close --pvf-fixture WASM NEW_DIR`, or applicable `actor_retention_close` export. These Close fixtures retain their User-admission prerequisites; do not enable unsupported capacity to run them.

For an independent genesis-only diagnostic when User admission is unavailable, run from `template/`:

```bash
SKIP_WASM_BUILD=1 cargo run --locked --release -p deos-runtime \
  --example actor_finalization_close -- --genesis-only --pvf-fixture /absolute/diagnostic.wasm /absolute/new-export
```

This selects both named runtime presets, directly from runtime-owned genesis with no setup mutations, and authors one inherent-only block per preset. The SDK uses actual selected Wasm (no native fallback) for plain/recording authoring and Core recording/replay imports, including complete-state late-invalid-root rollback. An empty proof-size host-query sequence is permitted and reported; it does not demonstrate Actor-service query coverage. Export uses the existing Aura seal/native state oracle and synthetic relay proof. This is **not finalization Close, retention, lifecycle, authenticated ancestry, corrected-node or resource qualification**. `--genesis-only --sdk-wasm WASM` runs the same SDK checks without export; `--genesis-only` uses compiled Rust only. Native default-production closure equivalence to the selected artifact is a caller prerequisite, not established by matching runtime versions alone. Model-only authority still refuses Wasm/PVF requests.

Then run from the repository root:

```bash
DEOS_BINARY_DIR=/absolute/pinned/binary/bundle \
  ./scripts/pvf-assurance.sh --fixtures /absolute/export --para-id 2000
```

The parachain ID is a caller input, not tool-owned genesis policy. The default Wasm is the canonical production-profile output; `--wasm PATH` selects another explicit artifact. The public script verifies the exact prepare/execute worker checksums through `01-download-binaries.sh --check --only`, builds this package and preserves the selected Wasm identity. No unrelated node/bencher binary or full-bundle marker is required; the host still checks both worker versions against its SDK. Its `--help` declares prerequisites and side effects.

For a fresh diagnostic artifact without replacing canonical production output, use `scripts/03-build-runtime.sh --output /absolute/new.wasm`, then select that exact file in both the exporter and `pvf-assurance.sh --wasm`. This retains the production compilation profile and canonical build root but creates only a new destination; it does not update production identity pins or regenerate metadata/client artifacts. Current source/host/Wasm/input equivalence and actual worker acceptance still require evidence. A historical Wasm or successful file-publication self-test cannot supply it.

## Canonical SDK Source and Reproducibility

This directory is the sole host package/build owner. Its own `Cargo.toml` and `Cargo.lock` select the SDK umbrella from `https://github.com/paritytech/polkadot-sdk` at commit `9448fe4f252936d241b8d2663506813b884f94f3`, not the runtime workspace's registry umbrella. This is tag `polkadot-stable2609-2`, annotated object `e8161a399c97802e696ff3563aa0efa7b0583696`, tree `97a1f73479286fdbd94e4bed0c5062999fab3073`; the source node version is1.25.2. The parent workspace excludes this directory so Cargo cannot unify its host graph with the runtime graph. No second host engine, runtime SDK upgrade or registry-host fallback exists.

The current lock has1,471 package identities, including314 SDK packages at their exact Git revision. It is not the entire upstream SDK lock. Compatible maintenance selects Wasmtime36.0.20 with Cranelift0.123.20 and corrected DNS/TLS, IO, compression and logging dependencies. Eight existing project-owned backports are reused from `template/vendor`; the complete local `vendor/tracing-subscriber-0.3.19` backports the upstream formatter sanitizer while preserving the SDK's exact version requirement. Each local owner documents its mechanism and limitations; a local version is not an upstream fixed-version claim.

The inspected default Linux normal/build graph has836 packages; the all-feature/all-target preservation graph has945. Neither is the 1,471-package full lock inventory, nor a declaration that every dependency function was semantically reviewed. The maintenance changes preserve all314 SDK identities, all five host Rust files and declared transport/platform capabilities. Historical host executable acceptance does not transfer to this changed source graph; current-source build and real-worker results require separate qualification.

The `dev` profile is unoptimized with `debug=0`, `incremental=false`; the template's ancestor `rust-toolchain.toml` selects Rust1.94.1. Package metadata/lints are explicit in this separate workspace; release tooling must synchronize its package version with `template/Cargo.toml` without changing the SDK revision.

From this directory, the ordinary reproducible build and package validation routes are:

```bash
SKIP_WASM_BUILD=1 cargo build --locked -p deos-pvf-validation
SKIP_WASM_BUILD=1 cargo test --locked -p deos-pvf-validation
SKIP_WASM_BUILD=1 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Cargo may acquire the locked Git/registry sources through its normal source cache; after acquisition, add `--offline` to disallow network use. No cache under a particular home directory or Skill is a build prerequisite. Full local source caches may be reused, but their availability is not a clean-room reproducibility claim. Source checkout, compiler, graph/features, profile, environment and binary hashes must be retained separately when qualifying a particular artifact; nominal version equality is insufficient.

The public wrapper changes into this directory before Cargo to honor the pinned toolchain, supplies this manifest explicitly and derives the compiler's native host triple from `rustc -vV`. It passes that triple as Cargo's explicit `--target`, overriding inherited `CARGO_BUILD_TARGET` and Cargo `build.target`, then executes only `<target-root>/<native-triple>/debug/deos-pvf-validation`. The default target root is this directory's ignored `target/`; an absolute `CARGO_TARGET_DIR` selects another root consistently for build and execution. Neither unqualified `target/debug` path is a fallback. Build failure stops execution.

Worker checksum verification still precedes build, application preflight checks each worker version, and SDK startup still passes/enforces the host's actual source node version. Default secure mode and complete-result/typed-negative semantics are unchanged. The offline wrapper self-test supplies a foreign inherited target and executable stubs in the native-qualified and stale unqualified paths; only the native-qualified stub may run.

For direct execution, first verify both workers through `scripts/01-download-binaries.sh --check --only polkadot-prepare-worker --only polkadot-execute-worker`, then invoke this workspace's built binary with `WASM FIXTURES WORKERS PARA_ID`. Direct execution does not replace the wrapper's checksum gate. Historical diagnostic acceptance remains bound to its original artifact/input identities; source selection or a build alone does not requalify that evidence or close release/security-policy gates.

## Optional CPU Trace

Add `--cpu-trace /absolute/new.jsonl` to the public script. The parent directory must exist; the tool atomically creates a new file and never overwrites an existing one. The default acceptance path installs no subscriber and produces no CPU trace.

- `src/cpu_trace.rs` accepts only the SDK execute-queue target/module and exact assignment/success messages. Each sequential case binds the source-defined ArtifactId Debug form, preparation-parameter hash and assigned worker. Missing, duplicate, out-of-order or mismatched events fail closed; subscriber initialization, poisoned state and write failures do not fall back to wall timers.
- JSONL retains session code/host/worker hashes, OS/architecture, default ExecutorParams encoding, requested/available security, fresh artifact-cache declaration, case input hash, selected raw event fields and accepted outcomes. Input hash covers the SCALE tuple `(ValidationParams, PersistedValidationData, expected ValidationResult)`. The timing-contract commit identifies the inspected SDK source contract, not a reproducible-binary attestation.
- `child_cpu_us` is emitted only after full positive acceptance. Negative outcomes have null CPU; zero placeholders, unsupported duration syntax, fractional microseconds and overflow refuse. This measures whole-job child user+system CPU, including setup/instance/result handling, not isolated root work or persistent database commit.
- Only a final `complete` record after all selected cases and graceful host shutdown marks a complete run. Positive-only completion retains its explicit narrow case-selection annotation; it never certifies the omitted negative cases. Failures retain partial diagnostics; an earlier per-case accepted row is not whole-run assurance. OS caches, machine calibration and environmental interference are uncontrolled, and a new artifact directory is not proof of a cold CPU/cache state.
- Event layout is SDK-source-bound, not a stable telemetry API. Synthetic subscriber tests prove routing/field handling and refusal behavior, not actual worker emission, current Wasm acceptance, a worst-case bound or fee backing. Exact current code/input pairing and real-worker trace validation remain separate gates.

## Offline Proof-Size Umbrella Pilot

`proof_umbrella_decoded_universe_and_optional_closed_inputs` tests a candidate bound for recorder bytes, not CPU or production Weight. For a decoded canonical witness `P`, its candidate universe is `P` plus the implicit V1 null node; the bound sums each node vector's SCALE-encoded size. It is conditional on every charged node/value belonging to that universe. Compact-proof bytes, full `StorageProof` bytes, native recorder bytes and PoV remain separate quantities.

The ordinary package test uses fixed empty, inline and hashed-value tries, checked-root compact decoding, repeated reads and missing-node/wrong-root controls. The empty witness exposes an implicit-node counterexample; hashed nodes expose compact/full-size inequality. These are deterministic native SDK primitive contracts, not execution of Actor code, block validation or a physical benchmark. Test-only `sp-trie` support resolves through this isolated workspace's SDK/Cargo.lock; a co-installed registry version is not its API authority.

Run `SKIP_WASM_BUILD=1 cargo test --locked -p deos-pvf-validation proof_umbrella_decoded_universe_and_optional_closed_inputs` from this directory. `RUST_TEST_NOCAPTURE=1` exposes labelled results. Optionally set `DEOS_PVF_PROOF_UMBRELLA_FIXTURES` to an existing fixture directory: the test reads four complete/inspection filesets, checks their existing bindings, decodes the supplied witness against its parent root and repeats a covered development-account read. It never starts ValidationHost/workers or executes Wasm/blocks; this is read-only analysis, not another acceptance attempt.

Without that environment variable, the test explicitly reports `checked=0 reason=not_requested`; a passing generic test does not attest external fixtures. Actual PVF SizeOnlyRecorder event origin, private-cache/full-domain re-encoding, overlay/root coverage and a supported input-domain bound remain separate qualifications. No RefTime upper, full I/K containment, fee or production binding follows. The DEOS integration owner retains the reference pilot's exact inputs and results.

The same test now compares both decoded StorageProof DB and checked-root CompactProof DB constructors with a fresh recorder over a warm native SDK SharedTrieCache. It asserts cached-root presence, NodeOwned re-encoding membership, query-value equality, cold/warm counter equality and duplicate-read stability; the cold report is reused, not recomputed. These selected native coordinates do not attest the private PVF cache, generated-node handling, whole-block work or physical cache costs. The empty input's compact round-trip explicitly supplies its null node without changing the recorder's two-byte charge.

`proof_umbrella_generated_nodes_require_positive_transaction_origin` is a separate fixed native state-root test. It changes an inline value and a hashed blob through SDK `Backend::storage_root`, retains the positive BackendTransaction nodes, and verifies that unmasked post-state recording defeats the original input-only universe. A native `IgnoredNodes` mask from that transaction restores charged-node containment while retaining an untouched input blob. Run it through the same package test route with that exact `--test-filter`; it reads no external fixtures and executes no Actor, runtime or PVF.

This is a byte-accounting/provenance contract, not provider equivalence or free computation. The native ignored-Value branch and private PVF SizeOnlyRecorder's seen-Value branch differ in recorded-key updates; equal excluded bytes do not establish equal cache permissions or CPU. Full-domain root-generation/overlay/private-PVF origin and a uniform supported input-domain upper remain unqualified. Test-only `sp-state-machine` follows the locked SDK dependency; the normal acceptance host and production runtime gain no new mode.

The generated-node test also attaches a fresh native Recorder to the actual TrieBackend during `storage_root`. It requires nonzero observable root-generation work, recorded-node containment in the original input universe and no overlap with newly generated transaction nodes. This distinguishes root generation from later direct post-state reads; phase counters are not additive prices or CPU observations. The selected case does not qualify every root delta, child trie, cache, full-domain native externalities overlay or actual private PVF recorder.

`proof_umbrella_overlay_keeps_original_backend_node_origin` covers one native public std externalities/overlay path: set two values, calculate a V1 root, read overlay values, repeat the root and read untouched backend values without applying the transaction. A fresh Recorder must remain within the original proof-plus-null universe; the backend root and old values must remain unchanged, while the calculated root matches an independent SDK delta. TestExternalities inserts an empty `:code` key, included in the input proof. Zero recorder bytes for overlay writes/hits do not mean free CPU or allocation. This selected no-apply path does not qualify write-after-root invalidation, child tries, actual PVF recorder/cache or a uniform input-domain upper.

The actual cfg-gated SizeOnlyRecorder source has a separate conditional accounting qualification in the DEOS integration owner. Only NodeOwned/EncodedNode/Value add SCALE byte-vector sizes on a new shared hash; the other events update key history only. A global seen set plus fresh per-block counters establishes a conditional bound on the derived sum by the original input universe, provided every charged hash has an original input owner whose encoded weight dominates the charge, with normal nonoverflow arithmetic. Exact re-encoding membership is sufficient, not automatic from decoder acceptance. Source guards/paper induction are not execution of that provider; no copied provider or build-feature workaround is used. Full-origin/representation/input-domain and independent RefTime/K qualification remain separate.

`proof_umbrella_decoder_normalization_uses_original_hash_owner_weight` builds one canonical inline V1 leaf at the 63/64-byte SCALE prefix boundary and a trailing-byte variant. Cold reads record original encoded nodes; warm native cache reads record NodeOwned re-encoding. The test retains the noncanonical decoder-acceptance counterexample to exact membership, hash preservation and cold/warm counter equality, while checking normalized encoded weight is dominated by the original hash owner's weight. Roots are synthetically chosen from supplied bytes, not established chain parent roots. It changes no old canonical assertions and executes no actual private PVF recorder, Actor or block; byte differences are not CPU savings or a runtime-legal underfit.

The integration owner also records a source-level V1 decode-origin nonexpansion argument: same-owner NodePlan/owned conversion/re-encoding, valid nonoverflow arithmetic and a normally returning complete pipeline imply normalized raw length and defined SCALE weight do not exceed the original owner. Inline children must normalize strictly below32 bytes; decoder acceptance alone does not satisfy that assertion. The SDK header comment's16-bit limit is not enforced by `decode_size`. Source checks and symbolic formulas are not SDK/PVF execution, and actual callback/cache hash ownership, input-domain/emitted-code and physical RefTime/K qualification remain separate.

`proof_umbrella_old_root_alias_can_charge_generated_payload` retains a fixed native same-content alias: sparse input covers B's hash but omits its blob; writing that blob to A and calculating a root publishes the payload through SharedTrieCache. After local-cache drop, an old-root B read succeeds through a second local cache and its fresh unmasked recorder's cumulative bytes exceed the original input universe. Plain-DB B reads still fail, the backend root stays unchanged and the transaction is never applied. Run the same package route with this exact `--test-filter`; `RUST_TEST_NOCAPTURE=1 DEOS_VERBOSE=1` exposes the report.

The [DEOS integration owner](../../../docs/deos/actors.integration.en.md#pvf-first-service-observation-profile) retains exact values, source identity and applicability. This refutes unconditional native input-only recorder accounting, not a production Weight or DEOS inspection bound. Native local-drop publication is not the private Cumulus cache's no-op merge, and no actual SizeOnlyRecorder ran. A source-only review conditionally excludes the alias from the DEOS reference block, since an in-block root followed by a recorded old-root read is required; the integration owner records the reopen triggers. No Actor/block/PVF execution, generated-node margin, early-seeding or production change follows.

## Hash-Owner Input-Budget Prototype

The test-only `src/proof_input_budget.rs` makes the input-weight premise executable without copying the private Cumulus recorder. It maps each original Blake2 hash in the supplied decoded proof plus the implicit V1 null node to its maximum supplied SCALE byte-vector weight. Checked `u32` addition refuses a universe whose total exceeds wasm32. These weights are candidate budgets: decoder domination is not assumed. A first-seen charged event must name a known original hash and fit that hash's budget; otherwise certification refuses. Later occurrences of the same hash add nothing, matching the size-only counter rule, not all recorded-key/cache semantics.

For a complete, correctly attributed cost-bearing event stream and a counter starting at zero, successful certification gives `first_seen_sum <= input_budget`. The checker does not authenticate that producer, prove event completeness or establish the private PVF cache's input-origin predicate. Normalized bytes must retain the original event hash, not a recomputed normalized hash. Larger first-seen re-encodings are rejected rather than hidden in an assumed adjustment. This is a conditional one-axis method component, not a uniform input-domain maximum, serialized/compact proof or PoV bound, RefTime model or production tariff.

Run `SKIP_WASM_BUILD=1 cargo test --locked --release -p deos-pvf-validation proof_input_budget::` from this directory. Four fixed native tests cover implicit null, actual SDK cache normalization at the 63/64-byte prefix boundary, unknown/overweight first-seen inputs, duplicate handling, the exact `u32` sum boundary and SDK overlay/root work followed by backend promotion. The normalization roots are synthetically chosen from supplied bytes, not established chain parents. The canonical overlay fixture certifies its original epoch and refuses generated post-commit inputs. It executes no private Cumulus SizeOnlyRecorder, Actor block, ValidationHost, Wasm or worker, reads no external fixture directory and does not rerun existing pilots. The [source-only callback/epoch map](../../../docs/deos/actors.integration.en.md#hash-owner-callback-and-epoch-applicability) distinguishes callback identity from input membership and a fresh counter from a fresh seen set; it is not executed-provider attestation.

## Prerequisites and Safety

- Native SDK compilation requires the pinned Rust toolchain, C/C++ tools, Perl core modules including FindBin/IPC::Cmd and `protoc`; system OpenSSL development libraries or the SDK's vendored OpenSSL build must be available.
- Standard `PERL5LIB`, `PROTOC` and `LD_LIBRARY_PATH` may select an isolated local toolchain; no system package installation or executable-bundle download occurs inside assurance; Cargo may fetch locked build dependencies.
- Secure-validator mode is required by default. `--insecure-local` explicitly permits best-effort hardening for trusted local code and is reported as narrower evidence, never silently selected.
- Candidate inputs are trusted local fixture files, not an untrusted network ingest surface. No relay history, public setup, live-node composition, resource maximum or tariff sufficiency follows from acceptance.

## Validation

From the repository root, use `scripts/ci-local.sh --only tests --package deos-pvf-validation --manifest-path template/tools/pvf-validation/Cargo.toml`, and the same route with `--only clippy`, for local result-classification, CLI, trace-state, synthetic-subscriber, duration and output-safety checks. The direct isolated-workspace commands above are equivalent. Root workspace checks alone no longer include this package; comprehensive validation must compose its explicit manifest route and formatting. `scripts/pvf-assurance.sh --self-test` checks isolated manifest/target build selection and exact runtime argv with no Cargo or workers. Real-worker acceptance is the separate opt-in `pvf-assurance.sh` route and needs exported candidates and both checksum/version-verified workers. The [SDK integration owner](../../../docs/deos/actors.integration.en.md#sdk-dependency-line) owns current artifact identity and acceptance scope.
