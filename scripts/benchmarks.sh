#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WEIGHTS_DIR="$TEMPLATE_DIR/runtime/src/weights"
BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks"
PRODUCTION_RUNTIME_WASM="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
STEPS=50
REPEAT=20
MIN_DURATION=""
HEAP_PAGES=8192
CHAIN="dev"
INCLUDE_EXTRA_BENCHMARKS=0
PALLETS=(
    "pallet_deos_actors"
    "pallet_deos_router"
    "pallet_tmc"
    "pallet_asset_registry"
    "pallet_governance"
    "pallet_oracle"
    "pallet_session_rotation"
    "pallet_staking"
    "pallet_xcm"
)
ACTORS_REQUIRED_RUNTIME_BENCHMARKS=(
    "scheduler_actor_state_probe"
    "scheduler_service_successful_interior"
    "scheduler_service_retry_to_deadline"
    "scheduler_service_retry_to_deadline_new_key"
    "scheduler_due_deadline_to_service"
    "scheduler_due_deadline_to_service_deep_index"
    "return_due_block_deadline_to_service"
    "return_due_block_deadline_to_service_deep_index"
    "scheduler_service_late_refusal_rollback"
    "scheduler_service_terminal_retain_close"
    "scheduler_service_minimal_apoptosis"
    "service_member_publish_empty"
    "service_member_publish_populated"
    "service_member_retire_singleton"
    "service_member_retire_pair_cursor"
    "service_member_retire_interior"
    "service_member_insert_populated"
    "service_round_begin_populated"
    "service_round_probe_eligible"
    "service_round_admit_eligible"
    "dependency_publication_begun_empty_source_list"
    "dependency_publication_begun_populated_source_list"
    "dependency_publication_coalesced_active_source"
    "service_member_to_deadline_new_key"
    "transaction_extension_ingress_base"
    "transaction_extension_ingress_notify"
    "run_suspend"
    "predicate_set_evaluation"
    "predicate_asset_evaluation"
    "predicate_observation_heavy_evaluation"
)

BENCHER_MODE=""
ACTION=""
TARGET_PALLET=""
EXTRINSIC_PATTERN="*"
OUTPUT_OVERRIDE=""
JSON_OUTPUT=""
RAW_ONLY=0
COMPONENT_LOW=""
COMPONENT_HIGH=""
SKIP_BUILD=0

usage() {
    cat <<EOF2
Usage: $(basename "$0") [OPTIONS] [PALLET_NAME]

Run benchmarks and generate weight files for the current DEOS reference runtime pallets.

Options:
  --steps N       Number of steps per benchmark (default: $STEPS)
  --repeat N      Number of repetitions per benchmark (default: $REPEAT)
  --min-duration N
                  Minimum seconds per benchmark (bencher default: 10; 0 uses --repeat)
  --all           Benchmark all custom pallets
  --list          List available pallets
  --check         Run self-tests, verify benchmark compilation and generated storage names
  --self-test     Test raw-sample/build/reuse routing without Cargo or artifact changes
  --extra         Include diagnostic benchmarks excluded from production weights
  --extrinsic NAME
                  Benchmark one extrinsic (requires one PALLET_NAME)
  --output FILE   Write generated weights to FILE (requires --extrinsic)
  --json-file FILE
                  Save raw component samples (requires one exact --extrinsic)
  --raw-only      Collect JSON without regression/weight generation (requires --json-file;
                  incompatible with --output; useful for branch selectors/fixed corners)
  --low VALUES    Component minima (requires --high and --output or --raw-only)
  --high VALUES   Component maxima (requires --low and --output or --raw-only)
  --skip-build    Explicitly reuse an already-built, applicable benchmark runtime
  -h, --help      Show this help message

Arguments:
  PALLET_NAME     Specific pallet to benchmark (e.g., pallet_deos_router)
                  If omitted and --all not set, the script exits with usage.

Examples:
  $(basename "$0") --all                      # Benchmark all pallets
  $(basename "$0") pallet_deos_router        # Benchmark one pallet
  $(basename "$0") --check                    # Compile and audit generated storage names
  $(basename "$0") --extra pallet_deos_actors         # Include Actors diagnostics
  $(basename "$0") --extrinsic service_member_to_deadline_new_key --output /tmp/deadline.rs pallet_deos_actors
  $(basename "$0") --steps 100 --repeat 50 --all  # Production-quality run

Environment:
  SKIP_WASM_BUILD              Must be unset for fresh Wasm builds (even 0/empty skips upstream);
                              allowed for --check and explicit --skip-build reuse
  INCLUDE_EXTRA_BENCHMARKS=0|1
  DEOS_VERBOSE=0|1              Stream full command and benchmark output (default: 0)
  DEOS_FAILURE_TAIL_LINES=N     Failure excerpt length in compact mode (default: 80)
EOF2
    exit 0
}

parse_args() {
    ACTION=""
    TARGET_PALLET=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --steps)
                STEPS="$2"
                shift 2
                ;;
            --repeat)
                REPEAT="$2"
                shift 2
                ;;
            --min-duration)
                if [[ $# -lt 2 || ! "$2" =~ ^[0-9]+$ ]]; then
                    log_error "--min-duration requires unsigned whole seconds"
                    exit 2
                fi
                MIN_DURATION="$2"
                shift 2
                ;;
            --all)
                ACTION="all"
                shift
                ;;
            --list)
                ACTION="list"
                shift
                ;;
            --check)
                ACTION="check"
                shift
                ;;
            --self-test)
                ACTION="self-test"
                shift
                ;;
            --extra)
                INCLUDE_EXTRA_BENCHMARKS=1
                shift
                ;;
            --extrinsic)
                EXTRINSIC_PATTERN="$2"
                shift 2
                ;;
            --output)
                OUTPUT_OVERRIDE="$2"
                shift 2
                ;;
            --raw-only)
                RAW_ONLY=1
                shift
                ;;
            --json-file)
                if [[ $# -lt 2 || -z "$2" || "$2" == --* ]]; then
                    log_error "--json-file requires a file path"
                    exit 2
                fi
                JSON_OUTPUT="$2"
                shift 2
                ;;
            --skip-build)
                SKIP_BUILD=1
                shift
                ;;
            --low|--high)
                if [[ $# -lt 2 || ! "$2" =~ ^[0-9]+(,[0-9]+)*$ ]]; then
                    log_error "$1 requires comma-separated unsigned component values"
                    exit 2
                fi
                if [[ "$1" == "--low" ]]; then
                    COMPONENT_LOW="$2"
                else
                    COMPONENT_HIGH="$2"
                fi
                shift 2
                ;;
            -h|--help)
                usage
                ;;
            *)
                TARGET_PALLET="$1"
                shift
                ;;
        esac
    done

    if [[ "$EXTRINSIC_PATTERN" == *'*'* && "$EXTRINSIC_PATTERN" != "*" ]]; then
        log_error "--extrinsic accepts one exact NAME, not a glob"
        exit 2
    fi
    if [[ "$EXTRINSIC_PATTERN" != "*" && -z "$TARGET_PALLET" ]]; then
        log_error "--extrinsic requires one PALLET_NAME"
        exit 2
    fi
    if [[ -n "$OUTPUT_OVERRIDE" && "$EXTRINSIC_PATTERN" == "*" ]]; then
        log_error "--output requires --extrinsic"
        exit 2
    fi
    if [[ -n "$JSON_OUTPUT" && ( "$EXTRINSIC_PATTERN" == "*" || -n "$ACTION" ) ]]; then
        log_error "--json-file requires one exact --extrinsic and PALLET_NAME"
        exit 2
    fi
    if [[ "$RAW_ONLY" == "1" && ( -z "$JSON_OUTPUT" || -n "$OUTPUT_OVERRIDE" ) ]]; then
        log_error "--raw-only requires --json-file and excludes --output"
        exit 2
    fi
    if [[ -n "$COMPONENT_LOW$COMPONENT_HIGH" \
        && ( -z "$COMPONENT_LOW" || -z "$COMPONENT_HIGH" \
            || ( -z "$OUTPUT_OVERRIDE" && "$RAW_ONLY" != "1" ) \
            || "$EXTRINSIC_PATTERN" == "*" || -n "$ACTION" ) ]]; then
        log_error "Component bounds require --low, --high, --output or --raw-only, and one exact --extrinsic"
        exit 2
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Benchmark prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    require_directory "$WEIGHTS_DIR" "Runtime weights directory"
    hydrate_local_tool_paths
    require_commands cargo sed grep wc sort awk head cut dirname mktemp chmod mv rm sha256sum

    if ! command -v frame-omni-bencher &>/dev/null; then
        log_warning "frame-omni-bencher not found. Install the pinned SDK bundle with:"
        echo "  $SCRIPT_DIR/01-download-binaries.sh"
        echo ""
        log_info "Falling back to 'cargo test --features runtime-benchmarks' mode"
        BENCHER_MODE="cargo"
    else
        local bencher_version minimum_bencher_version
        bencher_version="$(frame-omni-bencher --version | awk '{print $2}')"
        minimum_bencher_version="0.22.0"
        if [[ "$(printf '%s\n%s\n' "$minimum_bencher_version" "$bencher_version" | sort -V | head -n1)" != "$minimum_bencher_version" ]]; then
            log_error "frame-omni-bencher $bencher_version is incompatible with the current SDK; install >= $minimum_bencher_version"
            return 1
        fi
        BENCHER_MODE="omni"
        log_success "frame-omni-bencher $bencher_version found"
    fi
}

build_benchmarks() {
    if [[ "${SKIP_WASM_BUILD+x}" == "x" ]]; then
        log_error "Fresh benchmark Wasm requires SKIP_WASM_BUILD to be unset, including value 0 or empty. Unset it or explicitly select --skip-build for an applicable artifact."
        return 1
    fi
    phase_banner "Step 2: Build benchmark runtime"
    local production_identity=""
    if [[ -f "$PRODUCTION_RUNTIME_WASM" ]]; then
        production_identity="$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)"
    fi
    run_shell_step \
        "Build deos-runtime with runtime-benchmarks" \
        "" \
        "cd \"$TEMPLATE_DIR\" && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo build --release --locked --features runtime-benchmarks -p deos-runtime" || return $?
    if [[ -n "$production_identity" \
        && "$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)" != "$production_identity" ]]; then
        log_error "Benchmark build mutated the canonical production runtime Wasm"
        return 1
    fi
}

verify_actors_required_benchmark_source() {
    local benchmark_file="$TEMPLATE_DIR/pallets/actors/src/benchmarking.rs"
    local benchmark
    local missing=()

    for benchmark in "${ACTORS_REQUIRED_RUNTIME_BENCHMARKS[@]}"; do
        if ! grep -Fq "fn ${benchmark}(" "$benchmark_file"; then
            missing+=("$benchmark")
        fi
    done
    if (( ${#missing[@]} > 0 )); then
        log_error "Actors generated-Weight contract names benchmarks absent from source: ${missing[*]}"
        return 1
    fi
}

run_self_test() (
    local fixture value build_status=0
    require_commands mktemp rm grep
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/deos-benchmark-routing.XXXXXX")"
    trap 'rm -rf -- "$fixture"' EXIT
    PRODUCTION_RUNTIME_WASM="$fixture/absent-production.wasm"
    SKIP_BUILD=0
    EXTRINSIC_PATTERN="*"
    OUTPUT_OVERRIDE=""
    JSON_OUTPUT=""
    RAW_ONLY=0
    COMPONENT_LOW=""
    COMPONENT_HIGH=""

    # Exercise actual raw-sample argument construction and publication without a bencher.
    (
        BENCHER_MODE="omni"
        resolve_runtime_wasm_path() { printf '%s\n' "$fixture/runtime.wasm"; }
        normalize_weight_file() { return 99; }
        run_command_step() {
            shift 2
            printf '%s\n' "$@" > "$fixture/raw-args"
            local json=""
            while [[ $# -gt 0 ]]; do
                if [[ "$1" == "--output" ]]; then return 98; fi
                if [[ "$1" == "--json-file" ]]; then json="$2"; shift; fi
                shift
            done
            [[ -n "$json" ]] || return 97
            [[ "$raw_behavior" != failure ]] || return 42
            if [[ "$raw_behavior" == success ]]; then printf '[{"samples":1}]\n' > "$json"; fi
        }
        parse_args --raw-only --json-file "$fixture/samples.json" \
            --extrinsic close_detach_user --low 2 --high 2 pallet_deos_actors
        local raw_behavior status
        for raw_behavior in failure empty success; do
            status=0
            run_pallet_benchmark pallet_deos_actors > "$fixture/raw-log" 2>&1 || status=$?
            if [[ "$raw_behavior" == success ]]; then
                [[ "$status" == 0 && -s "$fixture/samples.json" ]] || return 1
                grep -Fxq -- '--no-median-slopes' "$fixture/raw-args"
                grep -Fxq -- '--no-min-squares' "$fixture/raw-args"
            else
                [[ "$status" != 0 && ! -e "$fixture/samples.json" ]] || return 1
            fi
        done
    )

    # Exercise the real main/build routing; replace only external work with trace markers.
    check_prerequisites() { BENCHER_MODE="omni"; }
    run_shell_step() { printf 'build\n' >> "$fixture/trace"; return "$build_status"; }
    run_pallet_benchmark() { printf 'benchmark\n' >> "$fixture/trace"; }
    check_only() { printf 'check\n' >> "$fixture/trace"; }

    assert_route() {
        local expected_status="$1" expected_trace="$2" status=0
        shift 2
        : > "$fixture/trace"
        (main "$@") > "$fixture/log" 2>&1 || status=$?
        if [[ "$status" != "$expected_status" || "$(<"$fixture/trace")" != "$expected_trace" ]]; then
            log_error "Benchmark route failed: $* (status $status, expected $expected_status)"
            return 1
        fi
        if [[ "$expected_status" == "1" ]] && ! grep -Fq 'SKIP_WASM_BUILD' "$fixture/log"; then
            log_error "Build refusal did not identify the inherited Wasm skip variable"
            return 1
        fi
    }

    unset SKIP_WASM_BUILD
    assert_route 0 $'build\nbenchmark' pallet_deos_actors
    assert_route 0 benchmark --skip-build pallet_deos_actors
    assert_route 0 check --check
    assert_route 2 '' --raw-only --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --raw-only --json-file "$fixture/samples.json" --output "$fixture/weights.rs" --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --raw-only --json-file "$fixture/samples.json" pallet_deos_actors
    assert_route 0 benchmark --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic close_detach_user --low 2 --high 2 pallet_deos_actors
    build_status=42
    assert_route 42 build pallet_deos_actors
    build_status=0
    for value in '' 0 1 false; do
        export SKIP_WASM_BUILD="$value"
        assert_route 1 '' pallet_deos_actors
        assert_route 0 benchmark --skip-build pallet_deos_actors
        assert_route 0 check --check
    done
    log_success "Benchmark raw-sample/build/reuse routing self-test passed"
)

check_only() {
    run_self_test
    phase_banner "Step 2: Benchmark compilation check"
    verify_actors_required_benchmark_source
    local production_identity=""
    if [[ -f "$PRODUCTION_RUNTIME_WASM" ]]; then
        production_identity="$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)"
    fi
    run_shell_step \
        "Verify benchmark compilation" \
        "" \
        "cd \"$TEMPLATE_DIR\" && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo check --locked --features runtime-benchmarks"
    if [[ -n "$production_identity" \
        && "$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)" != "$production_identity" ]]; then
        log_error "Benchmark compilation mutated the canonical production runtime Wasm"
        return 1
    fi
    audit_generated_weight_storage_names
    log_success "Benchmark compilation and generated storage-name audit passed"
}

audit_generated_weight_storage_names() {
    local actor_source="$TEMPLATE_DIR/pallets/actors/src/lib.rs"
    require_commands node
    node --input-type=module - "$actor_source" "$WEIGHTS_DIR" "$@" <<'NODE'
import fs from 'node:fs';
import path from 'node:path';

const [, , actorSourcePath, weightsDirectory, ...requestedFiles] = process.argv;
const sourceLines = fs.readFileSync(actorSourcePath, 'utf8').split(/\r?\n/);
const storageNames = new Set();
let pendingStorage = false;
let explicitPrefix;
for (const line of sourceLines) {
  const trimmed = line.trim();
  if (trimmed === '#[pallet::storage]') {
    pendingStorage = true;
    explicitPrefix = undefined;
    continue;
  }
  if (!pendingStorage) continue;
  const prefix = trimmed.match(/^#\[pallet::storage_prefix\s*=\s*"([^"]+)"\]$/);
  if (prefix) {
    explicitPrefix = prefix[1];
    continue;
  }
  const type = trimmed.match(/^pub type ([A-Za-z0-9_]+)/);
  if (type) {
    storageNames.add(explicitPrefix ?? type[1]);
    pendingStorage = false;
  }
}
if (storageNames.size === 0) {
  console.error(`Could not derive Actors storage names from ${actorSourcePath}`);
  process.exit(1);
}

const files = requestedFiles.length > 0
  ? requestedFiles
  : fs.readdirSync(weightsDirectory)
      .filter((name) => name.endsWith('.rs'))
      .sort()
      .map((name) => path.join(weightsDirectory, name));
const stale = [];
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const references = new Set([...text.matchAll(/Storage: `Actors::([^`]+)`/g)].map((match) => match[1]));
  for (const name of [...references].sort()) {
    if (!storageNames.has(name)) stale.push(`${file}: Actors::${name}`);
  }
}
if (stale.length > 0) {
  console.error('Generated Weight files reference stale Actors storage names:');
  for (const entry of stale.sort()) console.error(`  ${entry}`);
  process.exit(1);
}
NODE
}

resolve_runtime_wasm_path() {
    local candidates=(
        "$BENCHMARK_TARGET_DIR/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
    )

    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    log_error "Benchmark runtime WASM not found in known wbuild output paths"
    return 1
}

# frame-omni-bencher generates files with bare `frame_system`/`frame_support` imports
# and `WeightInfo<T>` struct names. Normalize to project conventions.
normalize_weight_file() {
    local pallet_name="$1"
    local file="$2"
    sed -i 's/use frame_support::/use polkadot_sdk::frame_support::/g' "$file"
    sed -i 's/pub struct WeightInfo/pub struct SubstrateWeight/' "$file"
    sed -i 's/impl<T: frame_system::Config>/impl<T: polkadot_sdk::frame_system::Config>/' "$file"
    sed -i 's/for WeightInfo<T>/for SubstrateWeight<T>/' "$file"
    sed -i 's/ pallet_xcm::WeightInfo/ polkadot_sdk::pallet_xcm::WeightInfo/' "$file"
    sed -i "s#${TEMPLATE_DIR}#template#g" "$file"
    sed -i -E "s#template/runtime/src/weights/\\.${pallet_name}\\.weights\\.[[:alnum:]]+#template/runtime/src/weights/${pallet_name}.rs#g" "$file"
    if [[ "$pallet_name" == "pallet_oracle" ]] \
        && grep -q 'fn register_feed_new_producer()' "$file"; then
        local measured_proof
        measured_proof="$(sed -n '/fn register_feed_new_producer()/,/^[[:space:]]*}/p' "$file" \
            | awk '/Measured:/ { gsub(/`/, "", $3); print $3; exit }')"
        if [[ ! "$measured_proof" =~ ^[0-9]+$ ]]; then
            log_error "Could not read measured Oracle new-producer ProofSize"
            return 1
        fi
        sed -i "/fn register_feed_new_producer()/,/^[[:space:]]*}/ s/Weight::from_parts(0, [0-9][0-9]*)/Weight::from_parts(0, ${measured_proof})/" "$file"
    fi
    log_info "  Normalized imports, struct name, local paths, and required proof bridges"
}

verify_weight_file_contract() {
    local pallet_name="$1"
    local output_file="$2"

    audit_generated_weight_storage_names "$output_file"

    if [[ "$pallet_name" == "pallet_oracle" ]]; then
        local measured_proof
        measured_proof="$(sed -n '/fn register_feed_new_producer()/,/^[[:space:]]*}/p' "$output_file" \
            | awk '/Measured:/ { gsub(/`/, "", $3); print $3; exit }')"
        if [[ ! "$measured_proof" =~ ^[0-9]+$ ]] \
            || ! sed -n '/fn register_feed_new_producer()/,/^[[:space:]]*}/p' "$output_file" \
                | grep -q "Weight::from_parts(0, ${measured_proof})"; then
            log_error "Weight file contract check failed for pallet_oracle: measured ProofSize is not charged"
            return 1
        fi
        return 0
    fi
    if [[ "$pallet_name" != "pallet_deos_actors" ]]; then
        return 0
    fi

    local benchmark_file="$TEMPLATE_DIR/pallets/actors/src/benchmarking.rs"
    local diagnostic_benchmarks=(
        "process_remove_liquidity_indexed"
        "scheduler_on_idle_healthy_empty"
        "scheduler_cooldown_ineligible_idle"
        "scheduler_wakeup_sparse_gap_recovery"
        "close_actor_system_pure"
        "close_actor_crossing_page"
        "close_actor_crossing_tail"
        "close_actor_crossing_cursor_repair"
        "close_actor_crossing_middle"
        "close_actor_observation_change"
        "create_user_actor_crossing_existing"
        "update_contract_observation_change"
        "condition_set_all_max"
        "condition_set_observation"
    )

    for benchmark in "${diagnostic_benchmarks[@]}"; do
        if ! grep -q "fn ${benchmark}" "$benchmark_file"; then
            continue
        fi
        if ! grep -q -- '--exclude-extrinsics' "$output_file" || ! grep -q "pallet_deos_actors::${benchmark}" "$output_file"; then
            log_error "Weight file contract check failed for pallet_deos_actors: missing exclude marker for ${benchmark}"
            return 1
        fi
    done

    for benchmark in "${ACTORS_REQUIRED_RUNTIME_BENCHMARKS[@]}"; do
        if ! grep -q "fn ${benchmark}" "$output_file"; then
            log_error "Weight file contract check failed for pallet_deos_actors: missing generated ${benchmark}"
            return 1
        fi
    done

    if grep -q 'fn compatibility_ingress' "$output_file"; then
        log_error "Weight file contract check failed for pallet_deos_actors: retired compatibility ingress weight remains"
        return 1
    fi

    if ! grep -q 'The range of component `n` is `\[1, 5\]`.' "$output_file"; then
        log_error "Weight file contract check failed for pallet_deos_actors: permissionless_sweep_many must cover MaxSweepBatch=5"
        return 1
    fi

    if ! grep -q 'Storage: `AssetConversion::Pools` (r:1 w:1)' "$output_file" \
        || ! grep -q 'Storage: `AssetConversion::NextPoolAssetId` (r:1 w:1)' "$output_file"; then
        log_error "Weight file contract check failed for pallet_deos_actors: task_add_liquidity must cover missing-pool creation"
        return 1
    fi
}

run_pallet_benchmark() {
    local pallet_name="$1"
    local output_file="${OUTPUT_OVERRIDE:-$WEIGHTS_DIR/${pallet_name}.rs}"
    local exclude_args=()

    if [[ "$pallet_name" == "pallet_deos_actors" && "$EXTRINSIC_PATTERN" == "*" ]]; then
        verify_actors_required_benchmark_source
    fi

    if [[ "$pallet_name" == "pallet_deos_actors" ]]; then
        local diagnostic_benchmarks=(
            "scheduler_inner_zero_step_user_complete"
            "scheduler_inner_zero_step_user_manual_header"
            "scheduler_inner_zero_step_user_immutable_manual_header_close"
  "scheduler_inner_zero_step_user_manual_header_close"
            "scheduler_inner_zero_step_system_manual_header"
            "scheduler_inner_zero_step_system_manual_header_close"
            "scheduler_inner_zero_step_user_address_event"
            "scheduler_inner_zero_step_user_address_event_close"
            "scheduler_inner_zero_step_system_address_event"
            "scheduler_inner_zero_step_system_address_event_close"
            "scheduler_inner_zero_step_user_cadenced"
            "scheduler_inner_zero_step_user_cadenced_close"
            "scheduler_inner_zero_step_user_at_time"
            "scheduler_inner_zero_step_user_at_time_close"
            "scheduler_inner_zero_step_user_observation_change_head_relink_close"
  "scheduler_inner_zero_step_system_observation_change_head_relink_close"
  "scheduler_inner_zero_step_user_observation_change_pending_head_relink_close"
  "scheduler_inner_zero_step_system_observation_change_pending_head_relink_close"
  "scheduler_inner_zero_step_user_observation_change_pending_second_page_retain"
  "scheduler_inner_zero_step_system_observation_change_pending_second_page_retain"
  "scheduler_inner_zero_step_user_observation_change_pending_second_page_close"
  "scheduler_inner_zero_step_system_observation_change_pending_second_page_close"
  "scheduler_inner_zero_step_user_observation_change_two_page_fanout_close"
  "scheduler_inner_zero_step_system_observation_change_two_page_fanout_close"
  "scheduler_inner_zero_step_user_observation_change_two_pages_close"
  "scheduler_inner_zero_step_system_observation_change_two_pages_close"
  "scheduler_inner_zero_step_user_observation_change_full_free_page_close"
  "scheduler_inner_zero_step_system_observation_change_full_free_page_close"
  "scheduler_inner_zero_step_user_observation_change_shared_page_close"
  "scheduler_inner_zero_step_system_observation_change_shared_page_close"
  "scheduler_inner_zero_step_user_observation_change_header"
  "scheduler_inner_zero_step_user_observation_change_header_close"
  "scheduler_inner_zero_step_system_observation_change_header"
  "scheduler_inner_zero_step_system_observation_change_header_close"
  "scheduler_inner_zero_step_user_crossing_header_armed_full_page_close"
  "scheduler_inner_zero_step_system_crossing_header_armed_full_page_close"
  "scheduler_inner_zero_step_user_crossing_header_armed_vacancy"
  "scheduler_inner_zero_step_system_crossing_header_armed_vacancy"
  "scheduler_inner_zero_step_user_crossing_header_armed_new_page"
  "scheduler_inner_zero_step_system_crossing_header_armed_new_page"
  "scheduler_inner_zero_step_user_crossing_waiting_tail_close"
  "scheduler_inner_zero_step_system_crossing_waiting_tail_close"
  "scheduler_inner_zero_step_user_crossing_header_armed"
            "scheduler_inner_zero_step_user_crossing_header_armed_close"
            "scheduler_inner_zero_step_system_crossing_header_armed"
            "scheduler_inner_zero_step_system_crossing_header_armed_close"
            "scheduler_inner_zero_step_user_crossing_header_waiting"
            "scheduler_inner_zero_step_user_crossing_header_waiting_close"
            "scheduler_inner_zero_step_system_crossing_header_waiting"
            "scheduler_inner_zero_step_system_crossing_header_waiting_close"
            "scheduler_inner_zero_step_user_cadenced_header"
            "scheduler_inner_opening_system_transfer_header_max"
            "scheduler_service_system_transfer_header_max"
            "scheduler_service_system_transfer_native_fixed"
  "scheduler_inner_opening_system_burn_header_max"
  "scheduler_inner_opening_user_transfer_terminal"
  "scheduler_inner_opening_user_transfer_terminal_peers"
  "scheduler_inner_opening_user_split_late_failure"
  "scheduler_inner_running_user_transfer_burn_two"
  "scheduler_inner_opening_user_transfer_progress_two"
  "scheduler_inner_opening_user_split_header_max"
  "scheduler_inner_opening_user_burn_header_max"
  "scheduler_inner_opening_user_transfer_mixed_header_max"
  "scheduler_inner_opening_user_transfer_observation_header_max"
  "scheduler_inner_opening_user_transfer_predicated_header_max"
  "scheduler_inner_opening_user_transfer_header_max"
  "scheduler_inner_zero_step_user_immutable_cadenced_header_close"
  "scheduler_inner_zero_step_user_cadenced_header_close"
            "scheduler_inner_zero_step_system_cadenced_header"
            "scheduler_inner_zero_step_system_cadenced_header_close"
            "scheduler_inner_zero_step_user_at_time_header"
            "scheduler_inner_zero_step_user_immutable_at_time_header_close"
  "scheduler_inner_zero_step_user_at_time_header_close"
            "scheduler_inner_zero_step_system_at_time_header"
            "scheduler_inner_zero_step_system_at_time_header_close"
            "scheduler_inner_zero_step_user_at_time_preserved_latch"
            "scheduler_inner_zero_step_user_at_time_preserved_latch_close"
            "scheduler_inner_zero_step_system_at_time_preserved_latch"
            "scheduler_inner_zero_step_system_at_time_preserved_latch_close"
            "scheduler_inner_zero_step_user_crossing_armed"
            "scheduler_inner_zero_step_user_crossing_armed_pages"
            "scheduler_inner_zero_step_user_crossing_armed_new_page"
            "scheduler_inner_zero_step_user_crossing_armed_pages_close"
            "scheduler_inner_zero_step_user_crossing_armed_new_page_close"
            "scheduler_inner_zero_step_user_crossing_cursor"
            "scheduler_inner_zero_step_user_crossing_cursor_close"
            "scheduler_inner_zero_step_user_crossing_waiting"
            "scheduler_inner_zero_step_user_crossing_close"
            "scheduler_inner_zero_step_user_crossing_page"
            "scheduler_inner_zero_step_user_crossing_page_close"
            "scheduler_inner_zero_step_user_crossing_page_full"
            "scheduler_inner_zero_step_user_crossing_page_full_close"
            "scheduler_inner_zero_step_user_observation"
            "scheduler_inner_zero_step_user_observation_close"
            "scheduler_inner_zero_step_user_observation_page_close"
            "scheduler_inner_zero_step_user_observation_unlink"
            "scheduler_inner_zero_step_user_observation_feed_close"
            "pipeline_zero_step_opening_collection"
            "process_remove_liquidity_indexed"
            "scheduler_on_idle_healthy_empty"
            "scheduler_cooldown_ineligible_idle"
            "scheduler_wakeup_sparse_gap_recovery"
            "close_actor_system_pure"
            "close_actor_opening_partition"
            "close_actor_crossing_page"
            "close_actor_crossing_tail"
            "close_actor_crossing_cursor_repair"
            "close_actor_crossing_middle"
            "close_actor_observation_change"
            "create_user_actor_crossing_existing"
            "update_contract_observation_change"
            "precondition_all_max"
            "precondition_observation"
            "scheduler_wakeup_cursor_remove_upward_depth"
            "scheduler_wakeup_cursor_remove_upward_pages"
            "benchmark_monolithic_create"
            "benchmark_chunked_create"
            "benchmark_monolithic_close"
            "benchmark_chunked_close"
            "benchmark_monolithic_update"
            "benchmark_monolithic_reconstruct"
            "benchmark_chunked_reconstruct"
            "benchmark_monolithic_load_first"
            "benchmark_monolithic_load_tail"
            "benchmark_chunked_load_first"
            "benchmark_chunked_load_tail"
        )
        local benchmark
        for benchmark in "${diagnostic_benchmarks[@]}"; do
            if [[ "$EXTRINSIC_PATTERN" != "$benchmark" ]]; then
                exclude_args+=(--exclude-extrinsics "pallet_deos_actors::${benchmark}")
            fi
        done

        if [[ "$INCLUDE_EXTRA_BENCHMARKS" != "1" ]]; then
            local stress_benchmarks=(
                "circular_chain_stress"
                "circular_chain_stress_100k"
                "circular_chain_100"
                "circular_chain_1000"
                "circular_chain_10000"
            )
            for benchmark in "${stress_benchmarks[@]}"; do
                if [[ "$EXTRINSIC_PATTERN" != "$benchmark" ]]; then
                    exclude_args+=(--exclude-extrinsics "pallet_deos_actors::${benchmark}")
                fi
            done
        fi
    fi

    log_info "Benchmarking: $pallet_name (steps=$STEPS, repeat=$REPEAT)"

    if [[ "$BENCHER_MODE" != "omni" ]]; then
        if [[ -n "$COMPONENT_LOW$COMPONENT_HIGH" ]]; then
            log_error "Component bounds require frame-omni-bencher; compile-only fallback cannot select samples"
            return 1
        fi
        if [[ -n "$JSON_OUTPUT" ]]; then
            log_error "--json-file requires frame-omni-bencher; compile-only fallback cannot produce samples"
            return 1
        fi
        log_warning "Running benchmark tests (dry run without weight generation)"
        if [[ "$INCLUDE_EXTRA_BENCHMARKS" == "1" ]]; then
            log_warning "--extra requires frame-omni-bencher for actual extra-benchmark execution; cargo fallback remains compile-only"
        fi
        run_shell_step \
            "Compile benchmark tests" \
            "" \
            "cd '$TEMPLATE_DIR' && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo test --release --locked --features runtime-benchmarks -p deos-runtime -- benchmark --nocapture" || true
        log_warning "Weight files NOT updated (frame-omni-bencher required for weight generation)"
        return 0
    fi

    local template_file="$TEMPLATE_DIR/.maintain/frame-weight-template.hbs"
    local runtime_wasm output_dir staged_output="" staged_json=""
    local step_label="Generate $pallet_name weights"
    runtime_wasm="$(resolve_runtime_wasm_path)" || return 1
    output_dir="$(dirname "$output_file")"
    require_directory "$output_dir" "Weight output directory"
    if [[ -n "$JSON_OUTPUT" ]]; then
        require_commands realpath
        require_directory "$(dirname "$JSON_OUTPUT")" "Raw benchmark output directory"
        if [[ "$(realpath -m -- "$JSON_OUTPUT")" == "$(realpath -m -- "$output_file")" \
            || "$JSON_OUTPUT" -ef "$output_file" ]]; then
            log_error "Raw samples and generated weights require separate output files"
            return 1
        fi
    fi
    local bencher_args=(
        --runtime "$runtime_wasm"
        --pallet "$pallet_name"
        --extrinsic "$EXTRINSIC_PATTERN"
        "${exclude_args[@]}"
        --steps "$STEPS"
        --repeat "$REPEAT"
        --heap-pages "$HEAP_PAGES"
    )

    if [[ "$RAW_ONLY" == "1" ]]; then
        staged_json="$(mktemp "$(dirname "$JSON_OUTPUT")/.${pallet_name}.samples.XXXXXX")"
        bencher_args+=(--json-file "$staged_json" --no-median-slopes --no-min-squares)
        step_label="Measure $pallet_name raw samples"
    else
        staged_output="$(mktemp "$output_dir/.${pallet_name}.weights.XXXXXX")"
        bencher_args+=(--output "$staged_output")
    fi
    if [[ -n "$JSON_OUTPUT" && "$RAW_ONLY" != "1" ]]; then
        bencher_args+=(--json-file "$JSON_OUTPUT")
    fi
    if [[ -n "$COMPONENT_LOW" ]]; then
        bencher_args+=(--low "$COMPONENT_LOW" --high "$COMPONENT_HIGH")
    fi
    if [[ -n "$MIN_DURATION" ]]; then
        bencher_args+=(--min-duration "$MIN_DURATION")
    fi

    if [[ "$INCLUDE_EXTRA_BENCHMARKS" == "1" ]]; then
        bencher_args+=(--extra)
    fi

    if [[ "$RAW_ONLY" != "1" && -f "$template_file" ]]; then
        bencher_args+=(--template "$template_file")
    fi

    if ! run_command_step \
        "$step_label" \
        "" \
        frame-omni-bencher v1 benchmark pallet "${bencher_args[@]}"; then
        rm -f "$staged_output" "$staged_json"
        return 1
    fi
    if [[ "$RAW_ONLY" == "1" ]]; then
        if [[ ! -s "$staged_json" ]]; then
            log_error "Raw benchmark samples not generated for $pallet_name"
            rm -f "$staged_json"
            return 1
        fi
        mv -f "$staged_json" "$JSON_OUTPUT"
        log_success "$pallet_name raw samples -> $JSON_OUTPUT (no weight generation)"
        return 0
    fi
    if [[ ! -s "$staged_output" ]]; then
        log_error "Weight file not generated for $pallet_name"
        rm -f "$staged_output"
        return 1
    fi
    if [[ -n "$JSON_OUTPUT" && ! -s "$JSON_OUTPUT" ]]; then
        log_error "Raw benchmark samples not generated for $pallet_name"
        rm -f "$staged_output"
        return 1
    fi

    normalize_weight_file "$pallet_name" "$staged_output"
    if [[ "$EXTRINSIC_PATTERN" == "*" ]] \
        && ! verify_weight_file_contract "$pallet_name" "$staged_output"; then
        rm -f "$staged_output"
        return 1
    fi
    chmod 0644 "$staged_output"
    mv -f "$staged_output" "$output_file"
    log_success "$pallet_name -> $output_file"
}

run_all_benchmarks() {
    local failed=0
    local succeeded=0
    local start_time
    local end_time
    local total_duration

    log_info "Running benchmarks for ${#PALLETS[@]} pallets..."

    start_time=$(date +%s)
    for pallet in "${PALLETS[@]}"; do
        if run_pallet_benchmark "$pallet"; then
            succeeded=$((succeeded + 1))
        else
            failed=$((failed + 1))
            log_error "Failed: $pallet"
        fi
        echo ""
    done
    end_time=$(date +%s)
    total_duration=$((end_time - start_time))

    phase_banner "Summary"
    echo "  Succeeded: $succeeded / ${#PALLETS[@]}"
    echo "  Failed:    $failed / ${#PALLETS[@]}"
    echo "  Duration:  ${total_duration}s"
    echo "  Steps:     $STEPS"
    echo "  Repeat:    $REPEAT"

    if [[ $failed -gt 0 ]]; then
        log_error "Some benchmarks failed"
        exit 1
    fi

    log_success "All benchmarks completed successfully"
}

list_pallets() {
    phase_banner "Available benchmark pallets"
    echo "Available pallets for benchmarking:"
    for pallet in "${PALLETS[@]}"; do
        local weight_file="$WEIGHTS_DIR/${pallet}.rs"
        if [[ -f "$weight_file" ]]; then
            echo "  - $pallet (weights: $(wc -l < "$weight_file") lines)"
        else
            echo "  - $pallet (no weight file)"
        fi
    done
}

is_known_pallet() {
    local candidate="$1"
    local pallet

    for pallet in "${PALLETS[@]}"; do
        if [[ "$pallet" == "$candidate" ]]; then
            return 0
        fi
    done
    return 1
}

main() {
    parse_args "$@"
    phase_banner "DEOS benchmark workflow"

    if [[ "$ACTION" == "self-test" ]]; then
        run_self_test
        return
    fi

    if [[ "$ACTION" == "list" ]]; then
        list_pallets
        exit 0
    fi

    if [[ "$ACTION" == "check" ]]; then
        check_only
        exit 0
    fi

    check_prerequisites

    if [[ "$BENCHER_MODE" == "omni" && "$SKIP_BUILD" != "1" ]]; then
        build_benchmarks || return $?
    fi

    if [[ -n "$TARGET_PALLET" ]]; then
        if ! is_known_pallet "$TARGET_PALLET"; then
            log_error "Unknown pallet: $TARGET_PALLET"
            echo ""
            list_pallets
            exit 1
        fi
        phase_banner "Step 3: Run pallet benchmark"
        run_pallet_benchmark "$TARGET_PALLET"
    elif [[ "$ACTION" == "all" ]]; then
        phase_banner "Step 3: Run pallet benchmark suite"
        run_all_benchmarks
    else
        log_error "Specify a pallet name or use --all"
        echo ""
        usage
    fi
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
