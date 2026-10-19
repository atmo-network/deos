# DEOS Reference Implementation Workspace

This directory is the Rust/FRAME implementation workspace for DEOS.
It contains the reusable parachain runtime components that realize the living contracts described in `../docs/deos/` and, when tokenomics or invariants change, can be cross-checked against the mathematical reference in `../simulators/tmctol/`. The current flagship economic standard shipped on this runtime kernel is TMCTOL.

The pre-`1.0` reference runtime supports fresh genesis only. It is not an upgrade artifact for an earlier release, carries no deployed storage lineage, and provides no production migration contract.

## Purpose

`/template` is the runtime kernel of the repository's DEOS reference stack:

- `../docs/deos/` = living contracts and architecture notes
- `/template` = production-oriented runtime implementation
- `../simulators/tmctol/` = authoritative mathematical reference when tokenomics, formulas, thresholds, or invariants change

The workspace is intentionally runtime-centric.
It does **not** carry a custom node crate in this repository snapshot; deployment is expected through the Polkadot SDK Omni Node flow.

## Generated Actor Weight Registry

From the repository root, run `SKIP_WASM_BUILD=1 DEOS_VERBOSE=1 RUST_TEST_NOCAPTURE=1 ./scripts/ci-local.sh --only tests --package deos-runtime --default-features --test-filter generated_weight_registry`. Two focused tests validate source-range parsing and evaluate every generated Actor Weight body at its documented component endpoint corners.

The test-only probe separates coefficients and generated read/write counts, then checks the result against the compiled reference function with the runtime's actual DB Weight. It prints source-bound JSON rows, not measured CPU, backend I/O, trie proof or safe production maxima. Historical component ranges do not establish current caller reachability. The [integration owner](../docs/deos/actors.integration.en.md#generated-weight-declaration-registry) links the retained registry and its limitations.

## PVF-First Service Fixture

The default-native `pvf_first_service_scenarios_are_natively_reachable` test checks one paid User AddressEvent pre-admission resource refusal and one complete zero-Step Cycle on both presets through actual Executive/Drain. It uses typed/public preparation, signed shared-budget demand and the existing proof-bound PVF exporter; it does not inject a resource ledger or collect physical timing. Run from the repository root:

```bash
env -u DEOS_PVF_FIRST_WASM -u DEOS_PVF_FIRST_OUTPUT \
  SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 \
  ./scripts/ci-local.sh --only tests --package deos-runtime --default-features \
  --test-filter pvf_first_service_scenarios_are_natively_reachable
```

With both `DEOS_PVF_FIRST_WASM` and `DEOS_PVF_FIRST_OUTPUT`, the same test optionally exports four five-file candidates plus a final hash-bound `profile.json` to a new directory; unset both for native correctness only. Export requires a fresh source-matched explicit diagnostic Wasm and is separate from real-worker execution. The [integration contract](../docs/deos/actors.integration.en.md#pvf-first-service-observation-profile) owns the finite host/channel/budget procedure, missing worker prerequisites and the distinction between Prepared method, verified native reachability, PVF observation and unqualified physical I/K prices.

## Workspace Layout

### `runtime/`

The assembled parachain runtime.
It wires pallets together through `Runtime-as-Config` adapters and hosts:

- Pallet configuration in `runtime/src/configs/`
- Runtime tests in `runtime/src/tests/`
- Runtime weight bridges in `runtime/src/weights/`
- Benchmark registry in `runtime/src/benchmarks.rs`
- Runtime-only complete-hook diagnostics in `runtime/src/benchmarks/`, separate from consensus pallets

The `authorship_complete` extra joins actual discovery/cache, callback/Executive registration and final cache deletion; `authorship_with_initialize` additionally creates Digest with actual System initialization inside Recording.

For isolated benchmark preparation without collection, run `env -u SKIP_WASM_BUILD ./scripts/benchmarks.sh --prepare` from the repository root. The script defaults to fixture authority; `--authority-profile ordinary --prepare` uses a separate build target. Requested profile/features and output path/hash do not attest freshness, executed authority or physical bounds. The [script contract](../scripts/README.md) owns action restrictions; the [runtime authority map](../docs/deos/actors.integration.en.md#benchmark-authority-profiles) distinguishes plain Cargo `runtime-benchmarks`, optional scaffolding and preserved all-feature behavior.

The separate `authorship_initialize_root` clears the setup whitelist and includes root materialization. Native real-host checks compare hook-only and root-inclusive observations; scalar agreement is not a complete proof upper or production tariff.

From the repository root, run `SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=4 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --release --test-filter tests::authorship_benchmark::`. The observer isolates setup/metadata probes and checks six absent-recipient empty/funded × single/payload/count cells per boundary, plus existing-recipient/single-Aura coverage across all three boundaries. High classes are initialization supersets, not legal complete-block maxima. The [benchmark script contract](../scripts/README.md) owns raw-only selection and failure rules; [runtime source](runtime/src/benchmarks/authorship.rs) owns input construction and phase placement. No production Weight or budget is supplied.

For the existing-recipient control alone, use `SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=4 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --test-filter authorship_existing_recipient_preserves_complete_owner_signature`. This ordinary-authority native control proves prepared ledger presence and hook coverage, not original-state witness identity or a physical maximum.

For the ordinary empty-queue transition, run `SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=4 ./scripts/ci-local.sh --only tests --package deos-runtime --default-features --test-filter authorship_empty_queue_transition_uses_actual_reference_calls`. The [default-only test](runtime/src/tests/authorship.rs) uses both named native genesis presets, signed-origin purge and two direct reference adapter rotations, then compares retained-key and no-author hook behavior. It does not execute periodic blocks, signed transaction extensions, a chain or AuraExt validation; logical observer counts are not physical pricing.

For the SDK host lifecycle, use the same all-feature/release route with `--test-filter tests::authorship_benchmark::host_backend::`. The [test-only real backend](runtime/src/tests/authorship_benchmark/host_backend.rs) exercises actual v2 runner, whitelist/recorder/reset and pending-root boundaries; native timings are discarded. These checks do not establish physical bounds.

For the input-domain falsifier, use the same repository-root route with `--default-features --test-filter full_executive_authorship_accepts_foreign_pre_runtime_digest` (or `--all-features`). Independent native signed Aura imports admit a foreign pre-runtime item before Aura. Existing header/size-accounting limits are not physical qualification or worker acceptance.

The W6 lifecycle diagnostic reuses sixteen complete blocks and independently replays their compact proofs. From this workspace, select a qualified default Wasm with `DEOS_DIAGNOSTIC_WASM=PATH DEOS_DIAGNOSTIC_WASM_SHA256=HEX SKIP_WASM_BUILD=1 cargo test --locked -p deos-runtime --lib tests::production_block_replay::full_executive_w6_mixed_arrival_lifecycle_replays_hash_bound_diagnostic_wasm -- --ignored --exact --nocapture`. The hash is checked before setup; benchmark Wasm is outside this test's feature scope. The [validation owner](../docs/deos/actors.integration.en.md#validation-and-operations) records native preparation, proof/code boundaries and remaining production acceptance.

For a sparse signed-governance chronology diagnostic, run `DEOS_REPLAY_PROGRESS=1 DEOS_VERBOSE=1 ../scripts/ci-local.sh --only tests --package deos-runtime --default-features --release --test-filter signed_l2_actor_admission_then_parked_churn_imports_complete_blocks` from this workspace. It reports cumulative guard/author/import/check wall time to stderr at block 4, every 1,000 blocks and enactment, without disabling libtest capture globally. Full chronology, root checks and the canonical timeout remain unchanged. These test-only timings do not establish production Weight; the [SDK integration owner](../docs/deos/actors.integration.en.md#sdk-dependency-line) records outcomes.

Native utilities live in `runtime/examples/`. `actor_hole_proof` inspects one fixed, reference-configured forest of paired hole-table suffix tries, records required pre-state node payloads and replays net roots. Its output does not establish public-state reachability, maximum ProofSize or a production Weight. Run `SKIP_WASM_BUILD=1 cargo run --locked -p deos-runtime --example actor_hole_proof -- --help` from this workspace for its command contract. The [Actors Close partition](./pallets/actors/docs/architecture.en.md#close-cost-partition) owns qualification scope.

For 0.7.27, reference Actors admit one ParkedBalance watch and distribution uses individual Transfer Steps with a bounded Run snapshot. The following four-source retention corpus and its recorded executions belong to the earlier domain; configured-watch guards reject it under the current reference limit. Keep its pinned evidence for reassessment, without replaying full history. Current source-only wide Close codec tests use one watch; the [integration owner](../docs/deos/actors.integration.en.md#current-distribution-and-evidence-domain) separates those checks from complete physical costs and production binding.

`cargo run --release -p deos-runtime --example actor_retention_pages -- OUTPUT.json` generates the historical four-source fixed hash-selected page corpus for `actor_retention_proof OUTPUT.json`. Both utilities expose `--help`; the generator refuses an existing output. Population and selection are fixed, and consumers verify the exact JSON hash and configured watch domain. The native proof replays eight read roots; `actor_retention_history OUTPUT.json` independently reconciles ordered source masks/frontiers, creator capacity, per-operator counts and current coded fees without epoch replay. The [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns identities, commands and qualification; finite construction/repetition proof and physical Close remain open. Literal full-history replay is excluded. Earlier twelve-source observations retain their pinned historical scope.

`actor_retention_admission OUTPUT.json --call-footprint [--preset=NAME]` records new backend keys and cumulative encoded proof around the signed temporary deactivate/activate pair in one prepared complete author/import block. Build this diagnostic in default-native mode; it uses independent committed-backend copies and local role/root checks, while `--construction` retains the nine-block/full-try-state route when that feature is enabled. Stage logs expose large-state preparation progress. Key differences exclude repeated and overlay-only reads; the [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns scope and evidence.

`actor_retention_close_pair FIXED_PAGE_JSON [--service-phase|--review-normalized] [--preset=NAME]` executes two signed permanent Close calls from different owners over the full four-source corpus. Build with `SKIP_WASM_BUILD=1 cargo build --locked -p deos-runtime --example actor_retention_close_pair`. The default records the cold point; the modes run twenty or 9,988 complete predecessor blocks with two signed native ingresses. The longer mode consumes old reviews, audits bounded page-zero review geometry and checks paid producer publication gaps. Both modes require seven Service members, actual Actor effects and independent author/import root agreement. Stage logs report each block. Synthetic preparation does not authenticate initial chronology, repetition horizon, uniform complete admission or physical/tariff/host maxima; the [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns retained evidence.

`actor_retention_hole_domain OUTPUT.json --actor-domain` separately checks the closed recipe's finite Actor-ID hash image and reports conditional per-prefix read/net-delete inputs with typed value widths and guarded ancestors. The default-native route also records bounded synthetic V1 reads and replays full-image/last-prefix deletion roots; it does not decode or admit Actor records. Default execution retains the hole-domain calculation; `--absence-witness` retains its separate recording/replay route. `--signed-pair-input` separately composes the fixed recipe’s Actors transition storage roles, joining four-source images and Block-only review/heap inputs; it also reports currency/FRAME storage inputs after Executive/event initialization and consensus inherents. `--birth-input` separately reports the six additional typed read roles, current declaration and conservative signed length for ordinary Manual/Parked creation replacing activation, plus the shared maximum-width Close-target codec. Its first-source input adds the common allocator and explicit-slot declaration/codec. The integration owner qualifies conditional inputs for first-source and repeated initially negative stateless Parked births, including AddressEvent/window targets; actual initial-state/horizon equivalence, physical cost and other creation branches remain separate. Complete block/host composition and physical admission remain separate. Complete call costs remain separate; the [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns conditional scope and qualification.

`actor_retention_hole_domain --storage-catalog` reports current native metadata declarations, compiled `StorageInfo`, nonmetadata stems and diagnostic identities without a corpus or block execution. Both helpers guard the compiled 240-owner schema, including `AssetConversion::PoolFees`; closed retention/default-mode watch-domain guards remain unchanged. Build/run in default-native mode with `SKIP_WASM_BUILD=1 cargo run --locked -p deos-runtime --example actor_retention_hole_domain -- --storage-catalog`. The [current schema/origin and catalogue checks](../docs/deos/actors.integration.en.md#inspection-nonmembership-input-applicability) own current scope; historical output remains bound to its original epoch. The [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns catalogue reconciliation and scope; listing declarations does not prove writer reachability or whole-state repetition.

`--frame-kernel-input` separately reports eighteen direct System/Executive initialization/finalization roles for the closed block with at most five extrinsics. It qualifies empty EventTopics prefix seek through source inspection and three bounded synthetic V1 recording/replay witnesses. AllPallets/inherent/economic bodies, complete host composition and physical costs remain separate.

`--quiet-hook-input` separately reports twenty-eight ordinary nonrotating consensus, Timestamp/payment and quiet Governance/message storage roles under fixed TrustedSet and empty-state premises. The source-owned legacy XCMP alias is included; the MessageQueue mutex uses an in-memory environmental token. ParachainSystem, scheduled SessionRotation, Actors bodies, complete host composition and physical admission remain separate.

`--parachain-input` separately reports thirty-five Parachain inherent/finalization, consensus callback and empty outbound storage roles for the constructor that includes the immediate parent. Closed value shapes and six synthetic V1 overwrite/deletion root replays support its conditional input arithmetic. Full PVF/PoV, scheduled SessionRotation, Actors bodies, complete host composition and physical admission remain separate.

`--session-successor-witness` compares empty prefixed seek with global next-key lookup in two bounded synthetic V1 namespace forests. Both key proofs replay; the deep foreign spine exceeds the prefixed-only input. It demonstrates an iterator-method boundary and does not admit those synthetic keys or certify Session cost. The conditional successor/overlay inventory belongs to `--session-input`.

`--session-input` reports sixteen scheduled TrustedSet Session storage roles and a separate empty reward-pot global-successor input. A twelve-key initialization deletion superset has only one key after the reward prefix, bounding overlay skips at one and backend searches at two. The source/codec calculation retains the current ordinary 712-byte key ceiling and explicit fixed-roster, empty-security and no-extra-input premises; complete physical Weight and joint repetition remain open.

`--economic-effect-input` reports twenty-two ledger, predicate, certified-recipient and coalesced LP-source publication roles for four fixed-one LP producers and the TrustedSet Fee Sink. It includes Dormant identity/semantic authentication and absent legacy authority. Active scan targets, recipient/custody floors and the closed adapter input remain premises; scheduler/Deadline/scan/hold bodies, CPU, physical admission and joint repetition remain open.

`actor_retention_hole_domain OUTPUT.json` derives conditional proof inputs from the complete fixed-corpus hole-key images, without epoch replay. Add `--absence-witness` to separately record/replay absent typed reads in complete, sparse and empty source-prefix tries. It exposes `--help`; the [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns its key-domain conditions and evidence. This utility does not certify whole FRAME/Close, physical costs or production bindings.

`actor_retention_history --signed-call-inputs` reports complete native DispatchInfo and maximum-nonce encoded lengths for the fixed setup/reclamation call groups. It needs no corpus or chain state and performs no signed execution; the [integration owner](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) records declared User reservation arithmetic and the remaining FRAME/recorder/physical gates. The existing fixed-corpus argument retains ordered-phase and tariff arithmetic.

`actor_retention_admission` and the `actor_retention_close` CLI retain the historical four-source corpus, twelve-Step bodies and three tails with sixteen-entry allowlists. Their recorded native author/import, signed Close, hold and custody checks keep their original source/artifact domain. The current source-only `actor_retention_close` tests instead admit one watch and verify wide header/body/Run codec bounds. Each utility exposes `--help`; the [reference integration](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) owns commands, identities and qualification. Source-byte checks do not certify full current physical Close.

`actor_retention_close FIXED_PAGE_JSON --api-import` checks the same complete blocks through generated SDK Core calls and `StateMachine`, with a compiled native Executive. It compares complete top state/root and exercises late invalid-root rollback with proof recording. Use a release-profile native build for the large projected state; no Wasm build is required. Stage messages distinguish preparation, node queries, authoring and API execution. The [client transaction context](../docs/deos/actors.integration.en.md#client-runtime-api-transaction-context) owns evidence and exclusions; this mode is not a Wasm or live-node test.

`--api-builder` exercises actual SDK `sc-block-builder`, including inherent generation, explicit extrinsic transactions, rejected duplicate rollback and reused finalization. The workspace applies the local transaction-depth correction in `vendor/sp-api-proc-macro`; the fixture's host-layer guard and `sdk_client_transactions` verify restored API re-entry. Native calls require the default SDK ReadRuntimeVersionExt and ProofSizeExt. Finite fixtures cover plain/recording builders and recording/replay Core imports, matching author/import estimates and checking recording rollback versus retained replay consumption. The [integration owner](../docs/deos/actors.integration.en.md#client-runtime-api-transaction-context) records exact source/artifact scopes. The wide Close utility also accepts `FIXED_PAGE_JSON --sdk-wasm PATH` to run those builder/provider/error checks in actual Wasm, with native fallback forbidden and the complete runtime version checked. Build its host without `runtime-benchmarks` for a default Wasm API set. Native preparation and the independent expected block/state precede SDK execution; code is external to the state proof. The workspace patch does not replace the downloaded Omni Node; node providers, ignored-node state and auxiliary persistence remain separate integration owners.

The wide utility also accepts `FIXED_PAGE_JSON --pvf-fixture PATH NEW_DIR` for a selected default Wasm. It binds code in prepared parent storage and exports compact-proof-bound Aura-sealed candidates after independent native seal, complete TOP/root and validation-output reconciliation. The output directory must be new. Use the default release host; native preparation/export and real SDK PVF host/worker validation are separate operations. The [client transaction context](../docs/deos/actors.integration.en.md#client-runtime-api-transaction-context) owns qualified current-source inputs and limits; this mode does not authenticate relay/setup history or certify physical maxima.

The default-feature `native_resource_meter_full_caller_preserves_execution` test reuses the canonical signed full-block caller to compare complete state, roots and exact recorded proof nodes with test-only callback observation disabled/enabled. Run `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --default-features --test-filter native_resource_meter_full_caller` from the repository root. Ordinary execution is conformance-only. `DEOS_RESOURCE_METER_MEASURE=1` explicitly enables the finite optimized diagnostic report; use it only for a newly declared measurement question, not unchanged confidence reruns. The [resource integration owner](../docs/deos/actors.integration.en.md) distinguishes observed native callback share from uninstrumented cost, production Wasm, Actor per-Step CPU and declared Weight. The observer and clock dependency are test-only.

The large-index Governance logical-work control is opt-in because tracing full values is expensive. Run `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::finalized_cleanup_index_logical_volume_preserves_plain_hook` from the repository root only for its declared source/collector question. It checks repeated index Get/Put bytes and plain-hook state-root/Weight equality, not physical CPU, proof or a price. Ordinary CI retains the cheap observer scope tests. The [control contract](../docs/deos/actors-performance-assurance.specification.en.md#governance-retained-index-logical-work-control) owns inputs, exclusions and stop rules; do not replay it for confidence.

The independent full-index cleanup source gate is opt-in: `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::independent_full_indices_cleanup_hook_has_consistent_debt`. It prepares complete record/index/expiry bijections, not a read-only trie projection, and checks actual cleanup with Governance try-state before/after. The [source contract](../docs/deos/actors-performance-assurance.specification.en.md#independent-full-index-cleanup-source-gate) keeps the old work epoch separate from the current clock. Seeded debt is not admitted history, and the test collects no physical price or complete PoV. The small pallet route uses `--package pallet-deos-governance --all-features --test-filter independent_full_indices_cleanup_fixture`.

Use the [State Witness and Resource Accounting Contract](../docs/deos/state-witness-accounting.contract.en.md) before interpreting any benchmark proof, database count, logical traffic or Weight. It separates content sharing from repeated execution and RAM, binds original/overlay/root/baseline scope, and forbids unqualified capacity/fee/reclaim credits. Its executable-falsifier map points to existing tests; documentation does not authorize replaying closed corpora or installing new prices.

The Actor Deadline baseline gate is opt-in and ordinary-authority-only: `env -u RUST_LOG -u RUNTIME_LOG SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --ignored --test-filter tests::actor_control_benchmark::deadline_c0_caller_recording_matches_sdk_baseline`. It runs one real SDK reference plus two caller-recorded c0 bodies on one development preset, preserving the SDK's effective whitelist and separating baseline/body/verification. Custom recording calls no timer; SDK reference timings are discarded. The [integration owner](../docs/deos/actors.integration.en.md#native-deadline-sdk-baseline) explains the scalar/root/counter limits. No new Wasm, maximum cost or production-price claim follows; do not replay unchanged evidence for confidence.

The literal Actor idle-plus-finalize gate is opt-in: `env -u RUST_LOG -u RUNTIME_LOG SKIP_WASM_BUILD=1 DEOS_VERBOSE=1 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --ignored --release --test-filter tests::actor_control_benchmark::block_resource_idle_finalize_control_matches_reference_sdk_and_verification_boundary`. Its first finite gate passes four componentless SDK dispatches within one test, using two parents and verification off/on. It checks the actual breaker-frozen two-Hook workflow and SDK verification/restoration boundaries, not owner timings, all custody or complete F. The [integration owner](../docs/deos/actors.integration.en.md#literal-idle-plus-finalization-control) states its limits. Retain this route only for an explicitly changed source/recorder question; the accepted unchanged question is closed, with no confidence replay or automatic collection.

The reference Actor starvation-event source gate is opt-in and ordinary-only: `env -u RUST_LOG -u RUNTIME_LOG SKIP_WASM_BUILD=1 DEOS_VERBOSE=1 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --ignored --release --test-filter tests::actor_control_benchmark::reference_idle_starvation_events_follow_halt_and_recovery`. Its first finite native gate passes one exact test with two source-checked parent traces, actual halt observations and genuine recovery, preserving cross-block event prefixes without counter seeding. The unchanged native question is closed; select this route only for an explicitly changed source/input question, not routine CI or confidence replay. The [integration owner](../docs/deos/actors.integration.en.md#reference-starvation-event-witness) states its partial-context limits. No SDK/physical collection, timing, complete F bound or automatic campaign follows.

The Actor event-append original-read/net-delta witness is opt-in and ordinary-only: `env -u RUST_LOG -u RUNTIME_LOG -u DEOS_TEST_PROFILE SKIP_WASM_BUILD=1 DEOS_VERBOSE=1 DEOS_TEST_TIMEOUT_MINUTES=45 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --ignored --test-filter tests::actor_control_benchmark::idle_event_live_prefix_original_inputs_replay_delta`. This selects the native debug profile; no timing comparison consumes it. The first exact native test passes; independent method/source/result reviews and integrated Clippy pass. Its finite question is closed. The [integration owner](../docs/deos/actors.integration.en.md#native-event-append-retention-witness) binds one original root, original-only read/net-delta replay and post-Actors scope. Retain this route only for an explicitly changed source/input obligation, not routine CI or confidence replay. No SDK benchmark dispatch, proof bundle or production-price claim follows.

The matched Actor Deadline witness gate is opt-in: `env -u RUST_LOG -u RUNTIME_LOG SKIP_WASM_BUILD=1 DEOS_VERBOSE=1 ./scripts/ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --ignored --release --test-filter tests::actor_control_benchmark::prepass_deadline_native_prefix_inputs_replay_delta`. It executes one two-branch native pair, retaining parent-prefix inputs versus standalone Deadline, comparing the complete delta/root/settlement and authenticating the selected original retry page. Effective whitelist, overlay enumeration, repair classification and custody/peer checks belong to the [integration owner](../docs/deos/actors.integration.en.md#native-matched-deadline-witness). It emits no timing field or proof bundle. Run only for an explicit changed lineage/collector/source question, not to repeat or widen a closed pair; native success supplies no production price, history or default-host bound.

The corrected ordinary SDK returned-row gate is `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::full_index_value_classes_sdk_return_separates_optional_verification`. It calls the real generated native dispatcher/BenchmarkRecording once per verify mode on one distinct-value source, checking optional-verifier exclusion, retained strict verification and equal saved proof. Incidental SDK timings are deliberately ignored; no Wasm, CPU rate, maximum host or production-price qualification follows.

The generated v2 value-class boundary gate is `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::full_index_value_classes_v2_records_absolute_body_before_verification`. Caller-owned no-clock recording checks source-cleared discounts, baseline0, required payload membership, one start/stop and proof capture before commit/verification. Cheap metadata filter `full_index_value_classes_metadata_is_extra_only` verifies categorical q0/1 and extra-only discovery. The [emitted canary contract](../docs/deos/actors-performance-assurance.specification.en.md#value-class-emitted-sourceschema-canary) remains separate from native conformance and forbids automatic timing-series or pricing promotion.

The matched native SDK value-class gate uses `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::full_index_value_classes_reach_native_sdk_body_recorder`. It runs equal/distinct full-index sources with a checked zero proof baseline and captures actual body proof before commit/verification/root diagnostics. The [source contract](../docs/deos/actors-performance-assurance.specification.en.md#matched-full-index-value-class-source-gate) excludes timing, pricing, maximal-host domination and Wasm/admission claims. This opt-in test is not a sampler or confidence rerun of the prior original-delta gate.

The finite joint pending/full-index source uses `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::independent_full_indices_and_pending_share_old_epoch_hook`. It executes one actual hook with separate cleanup/pending verifiers, full pending membership and a lifted current clock without moving old cleanup. The [composition contract](../docs/deos/actors-performance-assurance.specification.en.md#independent-full-index-and-pending-composition-source) retains the empty-pending control's strict guard and excludes history, joint maxima, refusal/rollback and physical-price claims.

The joint full-destination refusal gate uses `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::independent_full_indices_refusal_preserves_pending_and_commits_cleanup`. It compares complete committed TOP/root with a prebuilt finite cleanup/cursor model instead of weakening the standalone pending-only root guard. This is pre-effect capacity refusal, not invoked-effect rollback, physical pricing or history. Its classification belongs to the [joint-source contract](../docs/deos/actors-performance-assurance.specification.en.md#independent-full-index-and-pending-composition-source).

The paired original-input/delta gate uses `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --ignored --test-filter tests::governance_integration_tests::independent_full_indices_original_input_replays_whole_delta`. It preserves live block context, compares plain/observed full TOP and Weight, and replays original point inputs plus original repair nodes to the exact post-root. It uses the existing SDK trie/event primitives, not a new runner. The [transfer contract](../docs/deos/actors-performance-assurance.specification.en.md#independent-full-index-original-input-and-delta-transfer) excludes minimal/general proof bounds, physical CPU, validation-host/PVF, complete PoV and admitted history. Run only for a changed transfer/collector/source question, not confidence replay.

The ignored runtime test `tests::production_block_replay::signed_l2_admission_preparation_exports_pvf_fixtures` exports the four public signed Governance preparation blocks from each named genesis. Set `DEOS_GOVERNANCE_PVF_WASM` to an absolute default-Wasm file path and `DEOS_GOVERNANCE_PVF_OUTPUT` to an absolute new directory, then run `SKIP_WASM_BUILD=1 cargo test --locked --release --target-dir target/benchmarks -p deos-runtime --lib tests::production_block_replay::signed_l2_admission_preparation_exports_pvf_fixtures -- --exact --ignored --nocapture`. Absolute paths avoid Cargo's package test working directory. Code is bound at genesis; directories `1` through `4` retain each preset's original linked Aura seal, compact proof and complete native expected validation result. Worker validation is separate. The [Governance preparation integration owner](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) records acceptance and its finite scope; this route does not execute later voting/enactment, authenticate relay history or certify resource maxima.

The ignored `tests::production_block_replay::signed_intent_cleanup_exports_single_pvf_fixture` implements the bounded two-domain NoVotes bridge. It uses the same explicit absolute `DEOS_GOVERNANCE_PVF_WASM`/`DEOS_GOVERNANCE_PVF_OUTPUT` inputs, development/Alice only, the existing persistent body and endpoint-only export; no ancestor corpus or timing/prefix campaign belongs to this profile. Do not invoke it without separate history/export authorization and a current source-bound default-Wasm tuple: file paths and nonempty CODE checks do not establish those gates. Pure routing/model tests and compilation are not chronology or PVF acceptance. Use the canonical `ci-local.sh --only tests --package deos-runtime --default-features --release --test-filter tests::production_block_replay::signed_intent_cleanup_exports_single_pvf_fixture --ignored` route with an explicit sufficient test deadline. The [bounded bridge contract](../docs/deos/actors-performance-assurance.specification.en.md#bounded-governance-lifecycle-admission-bridge) and [integration owner](../docs/deos/actors.integration.en.md#bounded-intent-fee-reconciliation-source-seam) own its limits and source-bound native/one-child acceptance; that evidence is not physical pricing or network ancestry.

For the complete finite Governance/setup/churn path, use the same absolute input variables with `SKIP_WASM_BUILD=1 cargo test --locked --release --target-dir target/benchmarks -p deos-runtime --lib tests::production_block_replay::clock_prepared_signed_parked_setup_exports_pvf_fixtures -- --exact --ignored --nocapture`. The test preserves code in every parent and exports the main-path blocks through 187814 and the ten executed successors ending each prepared gap; earlier starts remain projected. The ordinary non-export test retains independent default-path coverage. Native author/import rejects initial child storage and child/offchain/transaction-index overlay operations before export or commit; `complete_block_auxiliary_overlay_guard_rejects_unexported_writes` checks all six write/delete/index cases. The [integration owner](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) binds captured batch PVF acceptance separately from native overlay checks; fresh exports require their own worker validation. This is finite fragment evidence, not public initial-state or horizon proof.

For native code-bound parent checks without another export corpus, supply only `DEOS_GOVERNANCE_PVF_WASM` and run `SKIP_WASM_BUILD=1 cargo test --locked --release --target-dir target/benchmarks -p deos-runtime --lib tests::production_block_replay::clock_prepared_signed_parked_setup_uses_code_bound_parents -- --exact --ignored --nocapture`. Each constructed parent must match the complete pre-state root and stored block number; after initialization it also checks the canonical System hash-key window. `construction_parent_guard_rejects_code_root_and_number_mismatch` covers invalid code/root/number and sparse-window inputs.

Clock preparation preserves genesis and known hash values while filling missing window entries with opaque H256 values. It projects to ten blocks before each target, then executes/imports ten quiet successors; the target parent and current header/relay fields are actual outputs of that suffix. Omitted history and its start state remain conditional. The prior captured PVF batch used sparse windows and synthetic gap endpoints; fresh exports include executed endings and require their own worker validation. The [integration owner](../docs/deos/actors.integration.en.md#actor-state-holds-and-capacity) records finite native coherence and remaining joint admission/horizon boundaries.

`actor_wide_close` is a separate current-domain caller, not a rewritten retention corpus. It publicly prepares one parked watch and twelve wide no-policy Steps with sixteen-entry AddressEvent/funding allowlists, then executes signed owner Close through actual SDK builders and recording/replay Core imports on both presets. Run `SKIP_WASM_BUILD=1 cargo run --locked --release -p deos-runtime --example actor_wide_close -- --sdk-wasm PATH` from this workspace; omit the mode for native execution. `--pvf-fixture PATH NEW_DIR` exports code-bound Aura-sealed candidates for the separate `pvf-assurance.sh` worker route. Post-state checks remain outside proof recording. Complete TOP/root, hold release, custody, and push/decode/duplicate/invalid-root refusal are finite caller evidence, not population, lifecycle, physical/tariff or live-node qualification. The [SDK integration owner](../docs/deos/actors.integration.en.md#sdk-dependency-line) owns current identities and limits.

`actor_finalization_close` checks a fixed terminal Close on both presets through actual SDK builder finalization, fresh Core import, recording/replay providers and post-Close executor/decode errors. Default SDK execution uses compiled Rust. Build with `SKIP_WASM_BUILD=1 cargo build --locked --release --target-dir target/benchmarks -p deos-runtime --example actor_finalization_close` and run under `timeout 120s`. For an existing benchmark Wasm, add `--features runtime-benchmarks` to the host build and run `timeout 180s target/benchmarks/release/examples/actor_finalization_close --sdk-wasm PATH`; the mode forbids native fallback and checks the full embedded/Core/reference runtime versions. For default Wasm, `--pvf-fixture PATH NEW_DIR` preserves parent code and exports proof-bound Aura-sealed candidates and native expected validation results; PVF host/worker validation is a separate caller. Native preparation and the independent expected block/state precede SDK execution. `--help` describes the scope; the [client transaction context](../docs/deos/actors.integration.en.md#client-runtime-api-transaction-context) owns provenance and error distinctions. These modes do not certify maximum-state cost, public history or a live node.

The separate off-chain [PVF validation tool](./tools/pvf-validation/README.md) consumes those exported candidates through actual version-checked SDK workers. `scripts/pvf-assurance.sh --fixtures DIR --para-id ID` owns checksum verification, build and typed acceptance/refusal; it requires native SDK prerequisites and secure mode by default. This package is not a runtime dependency or custom node; relay-authenticated receipt/history and complete host/resource acceptance remain separate.

`sdk_client_transactions` checks API re-entry and failed-call overlay rollback after explicit commit, rollback and nesting. It exposes `--help`; `SKIP_WASM_BUILD=1 cargo test --locked -p deos-runtime --example sdk_client_transactions` runs its nonempty regression. Its early version/extension lookup and block-identity guards also verify skipped requested dispatch, diagnostic host rollback and retry; both component tests run through that command. The caller does not execute the runtime, record proof or certify concrete Node extensions; the wide BlockBuilder fixture owns its separate native caller witness.

`actor_retention_admission OUTPUT.json --rotation` prepares the parent clock immediately before the first scheduled Session rotation and runs the same large-state review/epoch suffix. Add `--preset=development` or `--preset=local_testnet` to run one runtime-owned preset independently. Each suffix is bounded by 65 complete blocks; progress reports distinguish preparation, author/import roots and try-state verification. The source topology and parent clock remain synthetic preparation.

### `pallets/`

Custom DEOS runtime pallets in the current reference configuration:

- [`actors/`](./pallets/actors/) — deterministic actor runtime with bounded scheduling, triggers, lifecycle, static execution plans, and sparse progress-preserving Continuation
- [`asset-registry/`](./pallets/asset-registry/) — XCM location to asset-id registry
- [`router/`](./pallets/router/) — DEOS Router routing and fee/burn execution gateway
- [`governance/`](./pallets/governance/) — bounded governance reward-memory and proposal lifecycle
- [`oracle/`](./pallets/oracle/README.md) — reusable bounded typed scalar observation and revision truth
- [`staking/`](./pallets/staking/) — share-vault staking, liquid `stXXX` receipts, locked native LP nomination, governance custody, and reward settlement
- [`tmc/`](./pallets/tmc/) — token minting curve logic

See [`pallets/README.md`](./pallets/README.md) for the pallet index.

### [`pallets/actors/embedding-runtime/`](./pallets/actors/embedding-runtime/README.md)

External-consumer Cargo fixture owned by the `pallet-deos-actors` package boundary. It proves the portable host contract in default, DEX, try-runtime, and no-std profiles while starting with zero System actors and using no DEOS helper or topology dependencies.

### `primitives/`

Shared types and constants used across the runtime:

- Asset taxonomy and `AssetKind`
- Ecosystem constants
- Pallet ids and parameter defaults

### `research/`

Repository-local experimental/runtime-adjacent research artifacts when needed.
These are not part of the production runtime contract.

## Current Architecture Notes

Workspace dependencies select final `polkadot-stable2609`: umbrella `polkadot-sdk =2609.0.0`, Parachain System `=0.31.0`, Wasm builder `=35.0.0` and the compatible FRAME/API/Arithmetic graph in `Cargo.lock`. Upstream supplies reserve and XCM withdrawal corrections; their overrides and unused vendor copies are removed. Historical reserve-backport provenance lives in the [Asset Conversion integration owner](../docs/deos/asset-conversion.integration.en.md#underlying-xyk-engine), and selected affected native regressions pass. Client transaction-depth restoration remains locally patched because the final tag still contains that defect. Default check, benchmark compilation and all-target/all-feature Clippy pass with aligned host/test dependencies and generated XCM trait methods. Current production-profile Wasm is reissued and exact-Wasm profiles are rebound; fresh reference/funded-Action proof replay and byte-identical Wasm/native/canonical V16 metadata pass. Earlier thin/current-wide SDK, secure PVF and corrected-Omni liveness/recovery observations retain their original artifact identities; do not transfer them to new bytes. Full runtime, policy/resource, composed host and release gates remain open. The inactive precompile byte tariff is not certified and must be qualified before enabling a consumer; ordinary XCM uses its separate instruction owner. See the [SDK dependency owner](../docs/deos/actors.integration.en.md#sdk-dependency-line) for scope; old graph evidence is not current acceptance.

The most relevant implementation docs live in `../docs/deos/`:

- [`core.architecture.en.md`](../docs/deos/core.architecture.en.md)
- [`actors/docs/architecture.en.md`](./pallets/actors/docs/architecture.en.md)
- [`oracle/docs/specification.en.md`](./pallets/oracle/docs/specification.en.md)
- [`oracle/docs/architecture.en.md`](./pallets/oracle/docs/architecture.en.md)
- [`tmc/docs/architecture.en.md`](./pallets/tmc/docs/architecture.en.md)
- [`router/docs/architecture.en.md`](./pallets/router/docs/architecture.en.md)
- [`asset-registry/docs/architecture.en.md`](./pallets/asset-registry/docs/architecture.en.md)
- [`staking/docs/architecture.en.md`](./pallets/staking/docs/architecture.en.md)
- [`governance/docs/architecture.en.md`](./pallets/governance/docs/architecture.en.md)

For contract/spec-level docs, start at [`../docs/deos/README.md`](../docs/deos/README.md).

## Common Commands

Run from this directory unless noted otherwise.

### Build and test

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

### Targeted runtime checks

```bash
cargo test -p deos-runtime
cargo test -p deos-runtime actor_integration_tests
cargo test -p deos-runtime staking_integration_tests
cargo check -p deos-runtime --features runtime-benchmarks
```

For the cheap current-clock/Service/Deadline discriminator, run from the repository root: `SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --default-features --release --test-filter scheduler_small_cadenced_profile_retains_progress_and_current_residences`. Use `--all-features` instead to include try-state. The [validation map](../docs/deos/actors.integration.en.md#validation-and-operations) owns scope and counter meanings; this small safety witness does not replace large-population acceptance.

### Targeted pallet checks

```bash
cargo test -p pallet-deos-actors
cargo test -p pallet-deos-governance
cargo test -p pallet-deos-staking
```

### Benchmarks

Benchmark orchestration lives at the repository level:

```bash
cd ..
./scripts/benchmarks.sh pallet_governance
./scripts/benchmarks.sh pallet_staking
```

## Implementation Conventions

This workspace follows the repository rules from `../AGENTS.md`, especially:

- Polkadot SDK 2609 patterns
- FRAME v2 pallets and benchmark style
- Runtime weight bridges in `runtime/src/weights/`
- Bounded execution/storage surfaces
- `Runtime-as-Config` instead of hardcoded ecosystem logic inside generic pallets

## What this workspace is not

- Not the living contract/architecture control plane
- Not the mathematical protocol reference surface for tokenomic changes
- Not the deployment node binary layer
- Not the frontend workspace
- Not the canonical backlog or delivery log

Those live respectively in:

- `../docs/deos/`
- `../simulators/tmctol/`
- External Omni Node flow
- `../web-client/`
- `../BACKLOG.md` and `../CHANGELOG.md`
