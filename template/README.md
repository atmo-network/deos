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

## Native Quiet Pilot

The existing default-native full-Executive quiet test emits one `ACTORS_QUIET_PILOT_ROW_V1` line per preset/block and one final `ACTORS_QUIET_PILOT_SUMMARY_V1`. Run from the repository root:

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUST_TEST_NOCAPTURE=1 \
  ./scripts/ci-local.sh --only tests --package deos-runtime \
  --default-features --release \
  --test-filter tests::production_block_replay::public_quiet_fee_sink_complete_scheduler_phase_repeats
```

The fixed window is blocks 5–34 of each declared preset's existing native fixture. The report separates empty-workload counts from global System/Fee Sink events, exact native fee strings, lifecycle-derived block latency and both-axis booked declarations. The summary hashes the JSON array formed by the row payloads in emission order. Physical metrics are explicitly absent; this is not funded distribution, ordinary-Wasm/PVF execution or a safe physical price. The [integration owner](../docs/deos/actors.integration.en.md#native-quiet-pilot-observation) owns the scope and exclusions.

The default-only `tests::production_block_replay::quiet_pilot::quiet_row_coverage_rejects_missing_duplicate_and_out_of_domain_rows` test rejects missing, duplicate and out-of-domain row sets. An all-feature runtime does not compile this quiet fixture; it is not an alternative invocation.

## Native H2 Parked Intake-to-Delivery Pilot

Run the supported ordinary Parked Balance workflow through the existing default-native full-Executive driver:

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUST_TEST_NOCAPTURE=1 \
./scripts/ci-local.sh --only tests --package deos-runtime \
--default-features --release \
--test-filter tests::production_block_replay::transfer_baseline::h2_pilot::parked_intake_then_native_delivery_reparks_without_repeat
```

`ACTORS_H2_PARKED_DELIVERY_V1` contains all seven blocks2..8: one issuer's real signed Local intake at3, fresh-ledger due wake4/B+1 eligibility5, one Native ED delivery/completion5 and re-park without repetition6..8. It reports observed Creation/derived Trigger/Pipeline/Action fees separately from the actual signed transaction fee, all money/resource ledgers and retained state. No Manual occurrence, financial episode, synthetic source event or callback is added.

Preparation enables User creation through the public Root test surface and prefunds before birth1. It is not authenticated public genesis, ordinary-Wasm/PVF, a complete funding upper, physical pricing, capacity or ROI. Direct setup's signature/extension cost is excluded; the signed intake's actual fee includes its tip exactly once. Logical storage/physical metrics are explicitly uncollected. The [integration owner](../docs/deos/actors.integration.en.md#native-h2-parked-intake-to-delivery-observation) owns scope and exclusions; all-feature execution does not compile this default-only leaf.

## Native H3 Route-to-Delivery Pilot

Run one prepared-default-native heterogeneous economic workflow through the existing full-Executive author/import driver:

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUST_TEST_NOCAPTURE=1 \
./scripts/ci-local.sh --only tests --package deos-runtime \
--default-features --release \
--test-filter tests::production_block_replay::transfer_baseline::h3_pilot::foreign_swap_then_native_delivery_completes_once
```

`ACTORS_H3_ROUTE_DELIVERY_V1` reports one Mutable User Actor: four-predicate Foreign→Native `SwapOut` at block3, predicate-free Native Transfer/completion at4, and a no-repeat block5. Setup has birth1/Manual signal2, explicit finite prefunding and an already-live recipient. It uses real market adapters and reconciles actual Creation/Trigger/Pipeline/Action and Foreign Router fees, retained hold/Contract/generation, paid Running authority, target effects and global booked resources. Ordinary transaction costs are excluded from direct setup; there is no signed User demand in the observed blocks.

This is a default-only prepared-native diagnostic with Root-enabled test admission and synthetic relay context, not authenticated public genesis, ordinary-Wasm/PVF, a complete funding upper, physical pricing, capacity or ROI. The [integration owner](../docs/deos/actors.integration.en.md#native-h3-route-to-delivery-observation) defines its scope. All-feature execution is not a substitute.

## PVF-First Service Fixture

The default-native `pvf_first_service_scenarios_are_natively_reachable` test checks one paid User AddressEvent pre-admission resource refusal and one complete zero-Step Cycle on both presets through actual Executive/Drain. It uses typed/public preparation, signed shared-budget demand and the existing proof-bound PVF exporter; it does not inject a resource ledger or collect physical timing. Run from the repository root:

```bash
env -u DEOS_PVF_FIRST_WASM -u DEOS_PVF_FIRST_OUTPUT \
  SKIP_WASM_BUILD=1 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 \
  ./scripts/ci-local.sh --only tests --package deos-runtime --default-features \
  --test-filter pvf_first_service_scenarios_are_natively_reachable
```

With both `DEOS_PVF_FIRST_WASM` and `DEOS_PVF_FIRST_OUTPUT`, the same test optionally exports four five-file candidates plus a final hash-bound `profile.json` to a new directory; unset both for native correctness only. Export requires a fresh source-matched explicit diagnostic Wasm and is separate from real-worker execution. The [integration contract](../docs/deos/actors.integration.en.md#pvf-first-service-observation-profile) owns the finite host/channel/budget procedure, missing worker prerequisites and the distinction between Prepared method, verified native reachability, PVF observation and unqualified physical I/K prices.

## Governance No-Clock Root-Carrier

The opt-in native gate checks one prepared Governance hook followed by V1 root materialization, retaining the original witness before commit. It authenticates raw/compact original inputs, independently reconstructs the complete post-state, and rejects stale-root, missing-value and reset-witness substitutions. Run from the repository root with cached prerequisites:

```bash
CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 SKIP_WASM_BUILD=1 \
  ./scripts/ci-local.sh --only tests --package deos-runtime --all-features --release \
  --ignored --test-filter tests::governance_root_carrier::hook_root_before_commit_preserves_original_witness
```

The [gate contract](../docs/deos/actors-performance-assurance.specification.en.md#governance-no-clock-root-carrier-gate) owns the exact prepared source and exclusions. This does not run a timer, issue Wasm, qualify production Weight or establish admitted history/validation-host maxima.

## Complete Native Fixed-Obligation Caller

Run the default-native complete-Executive capture without collecting timing samples:

```bash
env -u DEOS_RESOURCE_METER_MEASURE \
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 RUST_TEST_NOCAPTURE=1 \
./scripts/ci-local.sh --only tests --package deos-runtime --default-features --release \
--test-filter complete_fixed_obligation_caller_preserves_success_and_late_refusal
```

The existing meter-attribution driver uses one SDK BenchmarkingState/overlay/recorder through Executive, the actual budget-provider entry, complete Prepass/Actor Drain bodies, finalization/root checks, outer import rollback and commit. `FIXED_OBLIGATION_CALLER_V1` reports no-clock boundaries, logical operations, original-root witness union and separate pre/post-commit DB counts. Plain/observed TOP/proof/counters agree; compact proof-only replay accepts the valid block even with the late-refusal witness. Empty/stale-root proof-backend construction refuses. Logical event counting ends before SDK commit; commit effects have separate counter snapshots.

`FIXED_PREFIX_ENTRY_RECEIPT_V1` adds a separate diagnostic arm using the existing guarded entry normalizer at the real provider callback. It removes only the actual Prepass call declaration, retains its base/encoded-byte charge, checks observation-to-provider charge stability and preserves the receipt through late import rollback. Its additional reads/repeat counters are not part of the primary capture. A declared prefix is not a physically qualified P; complete R remains unknown and no replacement allocation is installed.

The existing meter opt-in `DEOS_RESOURCE_METER_MEASURE=1` also wires raw CPU/elapsed spans through eight sequential caller regions and one enclosing capture. The enclosing span retains readout/transition gaps; never add it to the region spans. These clocks include observer glue, exclude host setup and independent verification, and have not been run or calibrated for this consumer. Do not enable them while the reference-host/profile gate is blocked; the switch is not profile admission, native/target transfer or permission to fit coefficients.

This is one development-preset first block with empty CODE and synthetic relay context, not production-Wasm execution or a complete legal-domain bound. The runtime's dev dependency enables passive native-only notices without enabling runtime-benchmarks; ordinary production feature edges exclude them. SDK counts are not physical disk I/O; the complete caller includes Actor work and must not be priced wholesale as fixed work. Outstanding fixed reserves, native/production transfer and conservative both-axis P/R ownership remain unqualified. No timing, coefficient, budget credit or system-quarter repair follows.

## Actor Service Host Observation

The native `complete_service_opening_host_observation` test invokes the real public Actor Prepass, including the resource-aware Service pass, quote/RetryLater preparation, LP Task, late fee boundary, SDK rollback, pass reconciliation and resource-state publication. It compares an observed and an unwrapped host for success and controlled late refusal, then verifies original/compact witnesses and replays the complete caller on proof-only storage.

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 \
  ./scripts/ci-local.sh --only tests --package deos-runtime \
  --features runtime-benchmarks --release \
  --test-filter tests::actor_service_observation::complete_service_opening_host_observation
```

The companion `complete_service_inspection_refusal_host_observation` test constructs real paused/resumed System continuations, a previously paid User prefix and a fresh User target through public Actor calls. Actual LP effects exhaust Base ProofSize before the target's paired booking; the test checks exact Encounter rollback, paid activation/prefix preservation, both-axis declared inspection retention and compact proof-only caller replay.

```bash
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 \
  ./scripts/ci-local.sh --only tests --package deos-runtime \
  --features runtime-benchmarks --release \
  --test-filter tests::actor_service_observation::complete_service_inspection_refusal_host_observation
```

Run from the repository root with the pinned native prerequisites. Optional `DEOS_SERVICE_OBSERVATION_OUTPUT=/absolute/new-directory` retains parent/post SCALE state, raw/compact witnesses and tagged operation/transaction traces; the directory must not exist. The source is a synthetic-capacity reference Runtime genesis without the reference System population; the refusal fixture adds only its publicly constructed System cohort. Neither fixture is signed/full Executive history. Instrumented native wall time includes observer/probe/fault work; SDK key counters are not physical disk I/O. Phase witness increments are not additive ProofSize bounds. No conservative I/K price, ordinary-Wasm/PVF transfer or system-quarter qualification follows. The [integration owner](../docs/deos/actors.integration.en.md#admission-certificate-authority) retains the qualification limits.

The focused `tests::actor_service_observation::calibration::no_callback_capture_preserves_caller_and_commit_suffix` test uses the same runtime-benchmarks/release route and existing LP/I fixtures. It checks the no-callback/no-journal arm against diagnostic TOP, original mixed-delta and compact full-caller replay, with separate commit and inclusive-capture spans. Journal/fault requests are rejected before capture invocation; both capture and compact proof replay reject an outer observer before invocation. Optional output uses the same new-directory variable. Native phase scopes and SDK proof tracking remain compiled; inclusive capture contains proof snapshots/glue. This is functional boundary qualification, not calibration, a ProofSize maximum, production-Wasm transfer or a price.

Output preserves `sdkDbBeforeCommit` and adds cumulative `sdkDbAfterCommit`, captured after commit/timing endpoints and before post-state enumeration. The original witness freezes before commit resets the SDK recorder. These tracked-key counters are not physical disk I/O or net TOP-change counts.

The module filter `tests::actor_service_observation::calibration::` also runs constructed clock-coordinate/conversion checks. The four primary body/root/commit/inclusive spans reject zero/backward intervals and unrepresentable integer ns-to-RefTime conversion. Legacy per-phase diagnostic clocks remain unqualified; these functional checks are not latency, drift or numerical-policy acceptance.

The ignored `ordinary_wasm_prepass_framing_probe` compares native Executive and ordinary SDK Wasm block-builder APIs from the literal refusal fixture cut before its last helper initialization. It generates actual ordered inherents after initialization and stops at the first trap or encoded refusal, retaining phase roots, TOP states, economic/ledger comparisons and original-root witnesses. Compact-decoded proof-only storage repeats the reached Wasm API sequence. This is a diagnostic, not an ordinary-I acceptance test; `framingComplete=false` is a blocked result, even when the probe itself passes. It never repairs the parent or binds external code by inserting it.

```bash
DEOS_ORDINARY_WASM=/absolute/hash-bound/default-runtime.wasm \
DEOS_SERVICE_NATIVE_REFUSAL_POST=/absolute/retained/inspection-refusal/post.scale \
DEOS_SERVICE_OBSERVATION_OUTPUT=/absolute/new-directory \
SKIP_WASM_BUILD=1 CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 \
  ./scripts/ci-local.sh --only tests --package deos-runtime \
  --features runtime-benchmarks --release --ignored \
  --test-filter tests::actor_service_observation::ordinary_wasm_prepass_framing_probe
```

The retained native post input must authenticate the original accepted refusal root. The report compares actual framed Prepass state/economics/ledger/events with that post separately from native/Wasm phase equality; successful dispatch does not imply original-refusal equivalence.

The selected ordinary image must equal the existing accepted production hash; this route does not rebuild or re-attest it. The capacity parent has empty `:code`, not that image, and is not full Executive ancestry. Relay input uses its actual SelfParaId without changing storage. Rejected-call residual roots are diagnostics, not committed block roots; native unwind and Wasm trap boundaries differ. The [existing assurance](../.agents/skills/architecture-experiments/tracks/actors/assurance.md#complete-service-opening-native-host-consumer) owns the executed phase and remaining source-admission gate.

The ignored `retained_prepass_event_suffixes_match_typed` independently decodes the retained native pre/post and framed before/after-Prepass TOP states with current Rust event codecs. Set `DEOS_SERVICE_NATIVE_REFUSAL_PARENT`, `DEOS_SERVICE_NATIVE_REFUSAL_POST`, `DEOS_SERVICE_FRAMED_BEFORE_PREPASS` and `DEOS_SERVICE_FRAMED_AFTER_PREPASS` to the authenticated retained SCALE files, then select that exact test through the same `--ignored --features runtime-benchmarks --release` route. It checks unchanged prefixes, payload/order/topics, real framed phase and the sole extra Executive success receipt; it does not edit either event buffer or price execution.

The separately labeled signed User source starts at unchanged development genesis with historical accepted CODE, executes the complete native signed Governance history and User preparation, then checks one final block in code-bound ordinary Wasm/compact proof-only replay. Run `DEOS_PRODUCTION_WASM=/absolute/hash-bound/historical-runtime.wasm DEOS_ADMITTED_REFUSAL_OUTPUT=/absolute/new-directory SKIP_WASM_BUILD=1 ./scripts/ci-local.sh --only tests --package deos-runtime --default-features --release --ignored --test-filter tests::production_block_replay::admitted_refusal::signed_genesis_user_liquidity_inspection_refusal` from the root with pinned native prerequisites and a sufficient `DEOS_TEST_TIMEOUT_MINUTES` (45 was used). It exports actual native PVF inputs without running a worker. This source test does not qualify exact I/K branches, physical prices, all-predecessor historical-Wasm execution or current-tree issuance; the integration/assurance owners retain those boundaries.

The terminal-cohort variant is `tests::production_block_replay::admitted_paired::signed_genesis_terminal_user_refusal`. Use the same default/release ignored route and historical image, but set `DEOS_TERMINAL_USER_OUTPUT=/absolute/new-directory`. It constructs128 residents,108 real zero-LP resolution-skips and72 terminal User LP completions with Action-fee receipts, preserving a fresh paid target. Public funding retains the paired asset's actual ED. Outputs `parent.scale`, `post.scale` and `selected/development.sealed-block.scale` belong to this new source, not the older observer's fixed input pins. Selected historical-code Wasm/compact replay passes. Its separately bound observer now reaches real Base paired-booking refusal and an earlier Drain Control guard; this supplies no physical price or complete I/K maximum.

Use `tests::actor_service_observation::admitted_source::retained_terminal_user_block_classifies_refusal_boundary` through the runtime-benchmarks/release ignored route. Set `DEOS_ADMITTED_SOURCE_PARENT` to this variant's `parent.scale`, `DEOS_ADMITTED_SOURCE_POST` to `post.scale`, `DEOS_ADMITTED_SOURCE_BLOCK` to `selected/development.sealed-block.scale`, and `DEOS_SERVICE_OBSERVATION_OUTPUT` to a new directory. The terminal and resident tests have separate SHA pins. Terminal coverage includes72 paid Running membership-retirement/fee/authentication paths and exact Base Encounter rollback, not Actor destruction or conservative maxima.

The genuine next-block K source is `tests::production_block_replay::admitted_paired::retained_terminal_source_fresh_opening_successor` on the default/release ignored route. Set `DEOS_TERMINAL_SOURCE_PARENT`, `DEOS_TERMINAL_SOURCE_POST`, and `DEOS_TERMINAL_SOURCE_BLOCK` to the terminal producer's immutable `parent.scale`, `post.scale`, and sealed block; set `DEOS_TERMINAL_OPENING_OUTPUT` to a new directory. It verifies the predecessor's Aura seal, recovers the exact native header and authors one ordinary successor without rebuilding genesis or patching state. Its `parent.scale`, `post.scale`, and `selected/development.sealed-block.scale` feed `tests::actor_service_observation::admitted_source::opening::retained_fresh_opening_reaches_retirement` through the same observer environment and runtime-benchmarks/release ignored route. This covers finite fresh Opening/retirement and exact fees/custody, not physical K maxima or current-tree issuance.

`tests::production_block_replay::admitted_paired::running_retry_source_declaration_preflight` is a non-ignored default/release declaration check, with no artifact inputs. It selects a conditional Base-only paid-Running LP candidate with RetryLater policy and explicitly rejects transfer of the prior Drain booking claim. It does not construct paid Run state or qualify an actual retry/continuation, source history or physical price.

`tests::production_block_replay::admitted_priming::retained_successor_public_lp_priming` uses the default/release ignored route. Bind `DEOS_PRIMING_SOURCE_PARENT`, `DEOS_PRIMING_SOURCE_POST` and `DEOS_PRIMING_SOURCE_BLOCK` to the fresh-opening producer's immutable parent, post and sealed packet; `DEOS_PUBLIC_PRIMING_OUTPUT` must name a new directory. Signed owner updates, funding and authored LP Transfers create a zero-preservable LP checkpoint without clearing raw balances. The output retains native packets/history and the final `parent.scale`, `post.scale`, `parent-header.scale` and `selected/` PVF files for a later genuine successor. It does not itself restore the LP Contracts or prepare a Running target.

The default/release ignored `tests::production_block_replay::admitted_running::retained_priming_paid_running_refusal_source` is that follow-up producer. Bind `DEOS_RUNNING_SOURCE_PARENT`, `DEOS_RUNNING_SOURCE_POST` and `DEOS_RUNNING_SOURCE_BLOCK` to the public-priming producer's final parent, post and sealed packet; `DEOS_PAID_RUNNING_OUTPUT` must be a new directory. It restores/funds real Contracts and captures a paid Running/RetryLater-policy target after genuine zero-resolution Steps. The selected native/historical-Wasm/compact replay and target-preservation assertions do not identify the exact refusal guard or execute an actual retry. Retain its exact packet/state outputs for the separate boundary observer instead of repeating its source history.

`tests::actor_service_observation::admitted_source::retained_paid_running_block_classifies_refusal_boundary` consumes those outputs through the shared `DEOS_ADMITTED_SOURCE_PARENT/POST/BLOCK` and new `DEOS_SERVICE_OBSERVATION_OUTPUT` directory on the runtime-benchmarks/release ignored route. It verifies the real parent Opening receipt, Running authority, Base Booking rollback and earlier Drain caller-Control refusal, with no source reconstruction or additional Pipeline charge. This finite policy-case conformance is not an actual RetryLater transition or a conservative I/K maximum.

The default/release ignored `tests::production_block_replay::admitted_running::retained_paid_running_successor_k` consumes the paid-Running producer's immutable final parent/post/packet through `DEOS_RUNNING_K_SOURCE_PARENT/POST/BLOCK`; `DEOS_RUNNING_K_OUTPUT` is a new directory. Its ordinary next block retains paid authority and completes the same LP Task without a new Opening. Feed its outputs through the standard observer environment to runtime-benchmarks/release ignored `tests::actor_service_observation::admitted_source::opening::retained_paid_running_k_reaches_retirement` for phase/proof qualification. This I→K successor relation is not a matched-state performance comparison or a physical K maximum.

The default/release ignored `tests::production_block_replay::admitted_retry::retained_successor_temporary_lp_paid_retry` consumes that K producer's parent/post/sealed packet through `DEOS_RETRY_SOURCE_PARENT/POST/BLOCK`, with a new `DEOS_PAID_RETRY_OUTPUT` directory. Public owner authoring/funding and ordinary blocks create a positive committed Transfer prefix, actual Temporary LP-output rejection, real Deadline wait/wakeup and paid retry. `first-failure/`, `wakeup/` and `retry-failure/` retain independent parent/post/header/PVF packets for exact observer reuse; `native-blocks/` and `native-history.json` retain the native suffix. These semantic/root transfers do not price complete physical owners, execute an exhausted third attempt or establish legal-domain maxima.

`tests::actor_service_observation::admitted_source::retry::retained_temporary_retry_packets_qualify_complete_caller` consumes all three stage packets on the runtime-benchmarks/release ignored route. Set `DEOS_RETRY_OBSERVATION_SOURCE` to the Temporary producer's `raw/` directory and `DEOS_SERVICE_OBSERVATION_OUTPUT` to a new directory. It verifies the actual failure/placement/fee/authentication chains, target-scoped Task root/transaction rollback and Control-only wakeup, preserving the source's prefix and paid Run authority. Existing source/caller tests need not be rerun to reconstruct inputs; whole-caller witness/counter conformance is not an isolated Deadline price or a domain maximum.

The default/release ignored `tests::production_block_replay::admitted_retry::retained_retry_exhaustion_and_owner_close` consumes the retry-failure packet through `DEOS_RETRY_CLOSE_PARENT/POST/BLOCK`, writing a new `DEOS_RETRY_CLOSE_OUTPUT`. Four ordinary blocks cover waiting, wakeup, the third Temporary attempt with mandatory `RetryAttemptsExhausted` destruction, and a separate cold User's signed owner close. It checks exact Action/state-hold accounting, canonical identity/slot release and custody-neutral destruction. `terminal-wakeup/`, `terminal-failure/` and `owner-close/` retain parent/post/header/PVF packets for exact observer reuse; these finite native/historical-Wasm/compact qualifications are neither complete physical cleanup owners nor maxima.

`tests::actor_service_observation::admitted_source::lifecycle::retained_exhaustion_and_close_packets_qualify_complete_caller` consumes those three packets on the runtime-benchmarks/release ignored route. Set `DEOS_RETRY_CLOSE_OBSERVATION_SOURCE` to that producer's `raw/` directory and `DEOS_SERVICE_OBSERVATION_OUTPUT` to a new directory. It verifies target-scoped rollback, actual destruction writes before collection/final authentication, paid-prefix and custody/hold conservation, signed transaction-fee separation and frozen resource finalization. Public close and Deadline return have complete enclosing-caller proof, not fabricated Service turns or isolated physical L/wakeup increments.

The default/release ignored `tests::production_block_replay::admitted_geometry::retained_legal_geometry_admission_preflight` consumes the owner-close packet through `DEOS_GEOMETRY_SOURCE_PARENT/POST/BLOCK`, with a new `DEOS_GEOMETRY_OUTPUT`. One ordinary signed update admits a 12-Step/four-predicate Contract with expanded funding/header geometry; it retains the real canonical head/tails, provider declarations and native/Wasm/compact packet. This is actual admission/encoding preflight, not a larger Task, Pipeline receipt, Cartesian structural maximum or physical price. Reuse its selected checkpoint for subsequent funded execution rather than rebuilding earlier history.

`tests::production_block_replay::admitted_geometry::retained_geometry_predicate_retry_exhaustion` uses the same default/release ignored route and consumes that selected packet through `DEOS_GEOMETRY_RUN_PARENT/POST/BLOCK`, writing a new `DEOS_GEOMETRY_RUN_OUTPUT`. Public funding/Manual activation creates a real prefix, three predicate-heavy LP Temporary attempts, two Deadline wakeups and exhaustion deleting the three-chunk body. `opening-prefix/`, `first-failure/`, `retry-wakeup/`, `retry-failure/`, `terminal-wakeup/` and `terminal-exhaustion/` retain native/header/PVF packets for exact caller observers. Only Steps0/1 execute; the remaining ten tails are retained then destroyed, not falsely counted as executed Steps or physical maxima.

The runtime-benchmarks/release ignored `tests::actor_service_observation::admitted_source::geometry::retained_geometry_packets_qualify_complete_callers` consumes all six packets using `DEOS_GEOMETRY_OBSERVATION_SOURCE` and a new `DEOS_SERVICE_OBSERVATION_OUTPUT`. It checks complete native caller/proof equivalence, target-bound successful/rolled-back Task scopes, actual head/three-tail deletions before fee collection, paid authority, fees/holds and resource finalization. Continuing Opening retains Service membership; Settlement encloses Collection/final Authentication, so phase windows are not additive. A Control-only target wakeup can share a block with an ordinary System effect.

The ignored `tests::actor_service_observation::admitted_source::retained_signed_user_block_classifies_refusal_boundary` consumes the original twelve-Step producer's unchanged TOP/block files without repeating genesis history; it does not accept terminal-cohort inputs. Set `DEOS_ADMITTED_SOURCE_PARENT` to `raw/activation-post.scale`, `DEOS_ADMITTED_SOURCE_POST` to `raw/refusal-post.scale`, `DEOS_ADMITTED_SOURCE_BLOCK` to `raw/refusal/development.sealed-block.scale`, and `DEOS_SERVICE_OBSERVATION_OUTPUT` to a new directory. Select the exact test through `ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks --release --ignored --test-filter NAME`. SHA guards reject substituted inputs. The actual refusal is caller Control ProofSize before paired booking; positive resident Opening/receipt observations do not qualify common retirement, complete K maxima or physical prices.

For current-pallet direct-target portability, use an installed `wasm32v1-none` Rust target and an explicitly qualified WebAssembly C frontend/archiver (`CC_wasm32v1_none`, `AR_wasm32v1_none`). Run `./scripts/ci-local.sh --only check --package pallet-deos-actors --no-default-features --manifest-path template/pallets/actors/Cargo.toml --target wasm32v1-none` from the repository root, with a separate `CARGO_TARGET_DIR` when isolating consumer graphs. A retained packaged manifest selects its own registry graph; do not transfer that result to changed workspace source. A successful check produces target metadata, not an executed or accepted production runtime.

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

Workspace dependencies select final `polkadot-stable2609-1`: exact umbrella `polkadot-sdk =2609.2.0`, Parachain System `=0.32.0`, Wasm builder `=35.0.0` and the compatible FRAME/API/Arithmetic graph in `Cargo.lock`. The client transaction-depth correction remains locally patched because this tag still contains the defect. Selected default/ordinary-benchmark checks and regressions, package and workspace all-target/all-feature Clippy pass; default-only tests remain a distinct evidence class. The [SDK dependency owner](../docs/deos/actors.integration.en.md#sdk-dependency-line) owns exact source/publication identities, changed pool/ledger behavior, selected validation and remaining gates.

The selected graph has an issued default production-profile Wasm, actual-Wasm/native v16 metadata equality and coherent generated client bindings/vectors. The two scoped reference-block/proof and funded-User-Action replay profiles pass, with independent approval limited to artifact coherence and those finite profiles. This is not production safety or complete runtime/host acceptance. Earlier secure PVF, worker and corrected-node liveness observations retain their original graph/artifact identities and do not transfer automatically. Complete owner/resource, physical pricing and release acceptance remain open. The inactive precompile byte tariff remains unqualified; ordinary XCM uses its separate instruction owner.

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
