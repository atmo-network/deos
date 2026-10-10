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
MERGE_INPUT=""
MERGE_BASE=""

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
  --self-test     Test output protection and raw/build/reuse routing in isolated fixtures
  --extra         Include diagnostic benchmarks excluded from production weights
  --extrinsic NAME
                  Benchmark one exact extrinsic (requires one PALLET_NAME and --output,
                  unless --raw-only); selected generation never merges methods
  --output FILE   Write focused weights to a separate artifact (requires --extrinsic);
                  production Weight paths and their aliases are rejected
  --json-file FILE
                  Save raw component samples (requires one exact --extrinsic)
  --raw-only      Collect JSON without regression/weight generation (requires --json-file;
                  incompatible with --output; useful for branch selectors/fixed corners)
  --low VALUES    Component minima (requires --high and --output or --raw-only)
  --high VALUES   Component maxima (requires --low and --output or --raw-only)
  --skip-build    Explicitly reuse an already-built, applicable benchmark runtime
  --merge-candidate FILE
                  Merge one already-generated method into an isolated candidate;
                  requires --base, exact --extrinsic, --output and PALLET_NAME
  --base FILE     Complete normalized generated runtime Weight file (read-only input);
                  merge mode runs no Cargo/bencher and certifies no coefficients
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
  $(basename "$0") --merge-candidate /tmp/setter.rs --base template/runtime/src/weights/pallet_deos_actors.rs --extrinsic set_user_creation_enabled --output /tmp/actors-candidate.rs pallet_deos_actors

Environment:
  DEOS_BINARY_DIR             Tool bundle directory (default: project/bin); select and
                              verify the intended SDK bundle before measured collection
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
            --merge-candidate|--base)
                if [[ $# -lt 2 || -z "$2" || "$2" == --* ]]; then
                    log_error "$1 requires an input file"
                    exit 2
                fi
                if [[ "$1" == --merge-candidate ]]; then
                    MERGE_INPUT="$2"
                    ACTION="merge"
                else
                    MERGE_BASE="$2"
                fi
                shift 2
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

    if [[ "$EXTRINSIC_PATTERN" != "*" && ! "$EXTRINSIC_PATTERN" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
        log_error "--extrinsic accepts one exact Rust NAME, not a pattern"
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
    if [[ "$EXTRINSIC_PATTERN" != "*" && "$RAW_ONLY" != "1" && -z "$OUTPUT_OVERRIDE" ]]; then
        log_error "Focused generation requires --output to a separate artifact; it does not merge production methods"
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
    if [[ "$TARGET_PALLET" == pallet_governance \
        && "$EXTRINSIC_PATTERN" == service_pending_enactments_full_topology \
        && ( "$RAW_ONLY" != 1 || "$INCLUDE_EXTRA_BENCHMARKS" != 1 ) ]]; then
        log_error "$EXTRINSIC_PATTERN requires --extra --raw-only --json-file; its branch selector is not a Weight coefficient"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == pallet_deos_actors \
        && ( "$EXTRINSIC_PATTERN" == manual_trigger_future_deadline \
            || "$EXTRINSIC_PATTERN" == scheduler_inner_zero_step_passive_window \
            || "$EXTRINSIC_PATTERN" == scheduler_service_zero_step_user_complete \
            || "$EXTRINSIC_PATTERN" == financial_wait_record_binding \
            || "$EXTRINSIC_PATTERN" == financial_service_repark_unqualified \
            || "$EXTRINSIC_PATTERN" == behavior_policy_installation \
            || "$EXTRINSIC_PATTERN" == behavior_policy_cleanup \
            || "$EXTRINSIC_PATTERN" == close_finalize_policy \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_control \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_source_authentication \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_semantic_publication \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_placement_preparation \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_placement_finish \
            || "$EXTRINSIC_PATTERN" == contract_behavior_context \
            || "$EXTRINSIC_PATTERN" == geometry_behavior_authentication \
            || "$EXTRINSIC_PATTERN" == behavior_closing_reset \
            || "$EXTRINSIC_PATTERN" == close_public_wide_header_prefix \
            || "$EXTRINSIC_PATTERN" == close_public_wide_header_prefix_late_hold \
            || "$EXTRINSIC_PATTERN" == close_public_wide_idle \
            || "$EXTRINSIC_PATTERN" == close_public_wide_idle_late_hold \
            || "$EXTRINSIC_PATTERN" == user_contract_installation \
            || "$EXTRINSIC_PATTERN" == user_creation_admission_payload \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_publication \
            || "$EXTRINSIC_PATTERN" == zero_step_policy_terminal \
            || "$EXTRINSIC_PATTERN" == stop_cycle_policy_terminal \
            || "$EXTRINSIC_PATTERN" == stop_cycle_policy_publication \
            || "$EXTRINSIC_PATTERN" == transfer_policy_terminal \
            || "$EXTRINSIC_PATTERN" == transfer_policy_progression \
            || "$EXTRINSIC_PATTERN" == running_policy_predicate \
            || "$EXTRINSIC_PATTERN" == liquidity_policy_control \
            || "$EXTRINSIC_PATTERN" == liquidity_policy_late_control \
            || "$EXTRINSIC_PATTERN" == opening_policy_pre_effect_control \
            || "$EXTRINSIC_PATTERN" == opening_policy_no_action_control \
            || "$EXTRINSIC_PATTERN" == funding_policy_control \
            || "$EXTRINSIC_PATTERN" == liquidity_failure_policy_control \
            || "$EXTRINSIC_PATTERN" == opening_policy_pre_effect_continue \
            || "$EXTRINSIC_PATTERN" == running_policy_service_fee \
            || "$EXTRINSIC_PATTERN" == running_policy_transfer \
            || "$EXTRINSIC_PATTERN" == running_policy_stop_cycle \
            || "$EXTRINSIC_PATTERN" == service_member_to_deadline_new_key_peers \
            || "$EXTRINSIC_PATTERN" == service_member_to_deadline_existing \
            || "$EXTRINSIC_PATTERN" == pipeline_admission_apoptosis ) \
        && "$INCLUDE_EXTRA_BENCHMARKS" != 1 ]]; then
        log_error "$EXTRINSIC_PATTERN requires --extra --raw-only --json-file"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == pallet_deos_actors && "$RAW_ONLY" != 1 ]] \
        && [[ "$EXTRINSIC_PATTERN" == manual_trigger_future_deadline \
            || "$EXTRINSIC_PATTERN" == scheduler_inner_zero_step_passive_window \
            || "$EXTRINSIC_PATTERN" == scheduler_service_zero_step_user_complete \
            || "$EXTRINSIC_PATTERN" == contract_optional_cleanup \
            || "$EXTRINSIC_PATTERN" == pipeline_admission_apoptosis \
            || "$EXTRINSIC_PATTERN" == financial_wait_record_binding \
            || "$EXTRINSIC_PATTERN" == financial_service_repark_unqualified \
            || "$EXTRINSIC_PATTERN" == behavior_source_snapshot \
        || "$EXTRINSIC_PATTERN" == behavior_service_policy_context \
        || "$EXTRINSIC_PATTERN" == behavior_policy_installation \
        || "$EXTRINSIC_PATTERN" == behavior_policy_cleanup \
        || "$EXTRINSIC_PATTERN" == close_finalize_policy \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_control \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_source_authentication \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_semantic_publication \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_placement_preparation \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_placement_finish \
        || "$EXTRINSIC_PATTERN" == contract_behavior_context \
        || "$EXTRINSIC_PATTERN" == geometry_behavior_authentication \
        || "$EXTRINSIC_PATTERN" == behavior_closing_reset \
        || "$EXTRINSIC_PATTERN" == close_public_wide_header_prefix \
        || "$EXTRINSIC_PATTERN" == close_public_wide_header_prefix_late_hold \
        || "$EXTRINSIC_PATTERN" == close_public_wide_idle \
        || "$EXTRINSIC_PATTERN" == close_public_wide_idle_late_hold \
        || "$EXTRINSIC_PATTERN" == user_contract_installation \
        || "$EXTRINSIC_PATTERN" == user_creation_admission_payload \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_publication \
        || "$EXTRINSIC_PATTERN" == zero_step_policy_terminal \
        || "$EXTRINSIC_PATTERN" == stop_cycle_policy_terminal \
        || "$EXTRINSIC_PATTERN" == stop_cycle_policy_publication \
        || "$EXTRINSIC_PATTERN" == transfer_policy_terminal \
        || "$EXTRINSIC_PATTERN" == transfer_policy_progression \
        || "$EXTRINSIC_PATTERN" == running_policy_predicate \
        || "$EXTRINSIC_PATTERN" == liquidity_policy_control \
        || "$EXTRINSIC_PATTERN" == liquidity_policy_late_control \
        || "$EXTRINSIC_PATTERN" == opening_policy_pre_effect_control \
        || "$EXTRINSIC_PATTERN" == opening_policy_no_action_control \
        || "$EXTRINSIC_PATTERN" == funding_policy_control \
        || "$EXTRINSIC_PATTERN" == liquidity_failure_policy_control \
        || "$EXTRINSIC_PATTERN" == opening_policy_pre_effect_continue \
        || "$EXTRINSIC_PATTERN" == running_policy_service_fee \
        || "$EXTRINSIC_PATTERN" == running_policy_transfer \
        || "$EXTRINSIC_PATTERN" == running_policy_stop_cycle \
        || "$EXTRINSIC_PATTERN" == service_member_to_deadline_new_key_peers \
        || "$EXTRINSIC_PATTERN" == service_member_to_deadline_existing \
        || "$EXTRINSIC_PATTERN" == scheduler_actor_state_probe_wide_header \
            || "$EXTRINSIC_PATTERN" == behavior_candidate_publish \
            || "$EXTRINSIC_PATTERN" == admission_certificate_authority \
            || "$EXTRINSIC_PATTERN" == close_dormant_event_root \
            || "$EXTRINSIC_PATTERN" == cadenced_underfunded_rearm_populated \
            || "$EXTRINSIC_PATTERN" == cadenced_terminal_cleanup_populated \
            || "$EXTRINSIC_PATTERN" == task_add_liquidity_user_refusal \
            || "$EXTRINSIC_PATTERN" == task_add_liquidity_system \
            || "$EXTRINSIC_PATTERN" == task_add_liquidity_system_refusal \
            || "$EXTRINSIC_PATTERN" == task_add_liquidity_system_existing_pool \
            || "$EXTRINSIC_PATTERN" == task_stake_user_refusal \
            || "$EXTRINSIC_PATTERN" == task_unstake_user_refusal \
            || "$EXTRINSIC_PATTERN" == task_remove_liquidity_user_refusal \
            || "$EXTRINSIC_PATTERN" == task_dex_exact_in_user \
            || "$EXTRINSIC_PATTERN" == task_dex_exact_out_user \
            || "$EXTRINSIC_PATTERN" == task_dex_exact_in_user_refusal \
            || "$EXTRINSIC_PATTERN" == task_dex_exact_out_user_refusal \
            || "$EXTRINSIC_PATTERN" == close_load_wide_idle ]]; then
        log_error "$EXTRINSIC_PATTERN is an isolated diagnostic; use --raw-only --json-file, not Weight regression or candidate merge"
        exit 2
    fi
    if [[ -n "$COMPONENT_LOW$COMPONENT_HIGH" \
        && ( -z "$COMPONENT_LOW" || -z "$COMPONENT_HIGH" \
            || ( -z "$OUTPUT_OVERRIDE" && "$RAW_ONLY" != "1" ) \
            || "$EXTRINSIC_PATTERN" == "*" || -n "$ACTION" ) ]]; then
        log_error "Component bounds require --low, --high, --output or --raw-only, and one exact --extrinsic"
        exit 2
    fi
    if [[ -n "$MERGE_INPUT$MERGE_BASE" ]]; then
        if [[ "$ACTION" != merge || -z "$MERGE_INPUT" || -z "$MERGE_BASE" \
            || "$EXTRINSIC_PATTERN" == '*' || -z "$OUTPUT_OVERRIDE" \
            || "$RAW_ONLY" == 1 || -n "$JSON_OUTPUT$COMPONENT_LOW$COMPONENT_HIGH" ]]; then
            log_error "Candidate merge requires two inputs, one exact owner/pallet and isolated --output; measurement modes are excluded"
            exit 2
        fi
        require_commands realpath
        local input
        for input in "$MERGE_INPUT" "$MERGE_BASE"; do
            if [[ "$(realpath -m -- "$input")" == "$(realpath -m -- "$OUTPUT_OVERRIDE")" \
                || "$input" -ef "$OUTPUT_OVERRIDE" ]]; then
                log_error "Candidate output must not replace a merge input"
                exit 2
            fi
        done
    fi
    local artifact
    for artifact in "$OUTPUT_OVERRIDE" "$JSON_OUTPUT"; do
        if [[ -n "$artifact" ]]; then
            validate_artifact_output "$artifact" || exit 2
        fi
    done
}

validate_artifact_output() {
    local artifact="$1" resolved weights_root pallets_root binding
    require_commands realpath
    resolved="$(realpath -m -- "$artifact")" || return 1
    weights_root="$(realpath -m -- "$WEIGHTS_DIR")" || return 1
    pallets_root="$(realpath -m -- "$TEMPLATE_DIR/pallets")" || return 1
    case "$resolved" in
        "$weights_root"|"$weights_root"/*|"$pallets_root"/*/src/weights.rs)
            log_error "Artifact output cannot replace production Weight paths: $artifact"
            return 1
            ;;
    esac
    for binding in "$WEIGHTS_DIR"/*.rs "$TEMPLATE_DIR"/pallets/*/src/weights.rs; do
        if [[ "$artifact" -ef "$binding" ]]; then
            log_error "Artifact output cannot alias an existing Weight binding: $artifact"
            return 1
        fi
    done
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

# Deterministic structural transform; Bash retains argument, output and lifecycle ownership.
transform_weight_candidate() {
    node --input-type=module - "$@" <<'JS'
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const digest = (text) => createHash('sha256').update(text).digest('hex');
const identifier = /^[A-Za-z_][A-Za-z0-9_]*$/;

// Deliberately accepts only the normalized FRAME-generated runtime Weight implementation,
// not arbitrary Rust, package estimates, multiple implementations or associated items.
function parseGenerated(text, pallet) {
  assert(text.endsWith('\n') && !text.includes('\r'), 'Expected LF-terminated generated Rust');
  assert(text.trimStart().startsWith(`//! Autogenerated weights for \`${pallet}\`\n//!\n`), 'Pallet header mismatch');
  const lines = text.split('\n');
  const traitOwner = pallet === 'pallet_xcm' ? `polkadot_sdk::${pallet}` : pallet;
  const header = `impl<T: polkadot_sdk::frame_system::Config> ${traitOwner}::WeightInfo for SubstrateWeight<T> {`;
  const starts = lines.flatMap((line, index) => line === header ? [index] : []);
  assert.equal(starts.length, 1, 'Expected exactly one normalized runtime Weight implementation');
  assert.equal(lines.filter((line) => /^impl\b/.test(line)).length, 1, 'Multiple implementations are unsupported');
  const methods = new Map();
  let cursor = starts[0] + 1;
  while (lines[cursor] !== '}') {
    const start = cursor;
    while (/^\s*(?:\/\/.*)?$/.test(lines[cursor] ?? 'unsupported')) cursor++;
    if (lines[cursor] === '}') break;
    const signature = /^([ \t]+)fn ([A-Za-z_][A-Za-z0-9_]*)\(([^)]*)\) -> Weight \{$/.exec(lines[cursor] ?? '');
    assert(signature, `Unsupported generated method at line ${cursor + 1}`);
    const [, indent, name, parameters] = signature;
    assert(!methods.has(name), `Duplicate method: ${name}`);
    assert(/^\s*(?:[A-Za-z_][A-Za-z0-9_]*\s*:\s*u32(?:\s*,\s*[A-Za-z_][A-Za-z0-9_]*\s*:\s*u32)*\s*,?)?\s*$/.test(parameters), 'Unsupported parameter types');
    const canonicalSignature = lines[cursor].replace(/\s/g, '');
    let expression = '';
    cursor++;
    while (lines[cursor] !== `${indent}}`) {
      assert(cursor < lines.length, 'Unterminated generated method');
      const code = lines[cursor].split('//')[0];
      assert(/^[\sA-Za-z0-9_:().,+]*$/.test(code), `Unsupported method body at line ${cursor + 1}`);
      expression += code;
      cursor++;
    }
    assert(expression.trim().startsWith('Weight::from_parts('), 'Expected generated Weight expression');
    let depth = 0;
    for (const char of expression) {
      if (char === '(') depth++;
      if (char === ')') depth--;
      assert(depth >= 0, 'Unbalanced Weight expression');
    }
    assert.equal(depth, 0, 'Unbalanced Weight expression');
    methods.set(name, { start, end: cursor, signature: canonicalSignature, lines: lines.slice(start, cursor + 1) });
    cursor++;
  }
  assert.equal(lines[cursor], '}', 'Unterminated implementation');
  assert(lines.slice(cursor + 1).every((line) => line.trim() === ''), 'Unexpected trailing Rust items');
  return { lines, methods, end: cursor };
}

function mergeCandidate(base, generated, owner, pallet) {
  assert(identifier.test(owner) && identifier.test(pallet), 'Expected exact owner and pallet identifiers');
  const target = parseGenerated(base, pallet);
  const source = parseGenerated(generated, pallet);
  assert.equal(source.methods.size, 1, 'Generated slice must contain exactly one method');
  const replacement = source.methods.get(owner);
  assert(replacement, 'Selected owner absent from generated slice');
  const previous = target.methods.get(owner);
  if (previous) assert.equal(previous.signature, replacement.signature, 'Selected signature changed');
  const result = [...target.lines];
  result.splice(previous?.start ?? target.end, previous ? previous.end - previous.start + 1 : 0, ...replacement.lines);
  const merged = result.join('\n');
  const checked = parseGenerated(merged, pallet);
  assert.equal(checked.methods.size, target.methods.size + Number(!previous));
  for (const [name, method] of target.methods) {
    if (name !== owner) assert.deepEqual(checked.methods.get(name)?.lines, method.lines, `Unselected owner changed: ${name}`);
  }
  assert.deepEqual(checked.methods.get(owner)?.lines, replacement.lines);
  // Keep the base file's full header truthful about its old owners, and pin the new slice
  // separately. Hashes identify inputs; this does not certify benchmark applicability.
  return merged.replace('\n//!\n', `\n//!\n//! Selected-method CANDIDATE ONLY: ${owner}\n//! Base SHA-256: ${digest(base)}\n//! Generated slice SHA-256: ${digest(generated)}\n//! Original generation header describes retained base owners, not this selected slice.\n//! No numeric, public-dispatch, complete-file or production acceptance is implied.\n//!\n`);
}

function normalizeXcmOverride(text) {
  const parsed = parseGenerated(text, 'pallet_xcm');
  const method = parsed.methods.get('claim_assets');
  if (!method || method.signature !== 'fnclaim_assets()->Weight{') return text;
  const body = method.lines.join('\n');
  assert(body.includes('Storage: `Benchmark::Override` (r:0 w:0)'), 'Missing unsupported benchmark owner');
  assert(body.includes('The range of component `n` is `[1, 20]`.'), 'Missing declared XCM asset component');
  const expression = method.lines.slice(method.lines.findIndex(line => /fn claim_assets/.test(line)) + 1, -1)
    .map(line => line.split('//')[0]).join('').replace(/\s/g, '');
  assert.equal(expression, 'Weight::from_parts(18_446_744_073_709_551_000,0).saturating_add(Weight::from_parts(0,0))', 'Only unchanged unsupported Weight sentinel may lose its component');
  // Override samples lose component data, but the SDK trait still takes n. No price is inferred.
  return text.replace(/fn claim_assets\(\) -> Weight \{/, 'fn claim_assets(_n: u32) -> Weight {');
}

function selfTest() {
  const xcm = '//! Autogenerated weights for `pallet_xcm`\n//!\nimpl<T: polkadot_sdk::frame_system::Config> polkadot_sdk::pallet_xcm::WeightInfo for SubstrateWeight<T> {\n\t/// Storage: `Benchmark::Override` (r:0 w:0)\n\t/// The range of component `n` is `[1, 20]`.\n\tfn claim_assets() -> Weight {\n\t\tWeight::from_parts(18_446_744_073_709_551_000, 0)\n\t\t\t.saturating_add(Weight::from_parts(0, 0))\n\t}\n}\n';
  const normalized = normalizeXcmOverride(xcm);
  assert.equal(normalized.replace('claim_assets(_n: u32)', 'claim_assets()'), xcm);
  assert.equal(normalizeXcmOverride(normalized), normalized);
  assert.throws(() => normalizeXcmOverride(xcm.replace('18_446_744_073_709_551_000', '1000')));
  assert.throws(() => normalizeXcmOverride(xcm.replace('Benchmark::Override', 'PolkadotXcm::Assets')));
  assert.throws(() => normalizeXcmOverride(xcm.replace('[1, 20]', '[1, 21]')));
  assert.equal(normalizeXcmOverride(xcm.replace('claim_assets()', 'claim_assets(n: u32)').replace('18_446_744_073_709_551_000', '1000')), xcm.replace('claim_assets()', 'claim_assets(n: u32)').replace('18_446_744_073_709_551_000', '1000'));
  const pallet = 'pallet_example';
  const file = (body) => `//! Autogenerated weights for \`${pallet}\`\n//!\nimpl<T: polkadot_sdk::frame_system::Config> ${pallet}::WeightInfo for SubstrateWeight<T> {\n${body}}\n`;
  const method = (name, value, parameters = '') => `\t/// Storage: owner { braces are comments }\n\tfn ${name}(${parameters}) -> Weight {\n\t\tWeight::from_parts(${value}, 0)\n\t}\n`;
  const first = method('first', 1);
  const second = method('second', 2, 'n: u32, ');
  const base = file(first + second);
  const added = mergeCandidate(base, file(method('new_owner', 3)), 'new_owner', pallet);
  const replaced = mergeCandidate(base, file(method('second', 4, 'n: u32, ')), 'second', pallet);
  assert(added.includes(first + second));
  assert(mergeCandidate(`\n${base}`, `\n${file(method('new_owner', 3))}`, 'new_owner', pallet).startsWith('\n//!'));
  assert(replaced.includes(first));
  assert(replaced.includes(`Base SHA-256: ${digest(base)}`));
  assert.equal(base, file(first + second), 'Input mutation');
  const rejected = [
    () => mergeCandidate(base, file(first + first), 'first', pallet),
    () => mergeCandidate(base, file(first + second), 'first', pallet),
    () => mergeCandidate(base, file(first), 'missing', pallet),
    () => mergeCandidate(base, file(method('second', 4)), 'second', pallet),
    () => mergeCandidate(base, file(method('second', 4, 's: u32, ')), 'second', pallet),
    () => mergeCandidate(base, file(first), 'first', 'pallet_other'),
    () => mergeCandidate(base + 'impl Extra {}\n', file(first), 'first', pallet),
    () => mergeCandidate(base, file(first).replace('Weight::from_parts(1, 0)', '{ Weight::zero() }'), 'first', pallet),
    () => mergeCandidate(base, file(first).replace('fn first()', 'fn first(x: u64)'), 'first', pallet),
    () => mergeCandidate(base, file(first).replace('\t}\n', ''), 'first', pallet),
    () => mergeCandidate(base, file(first).replace('Weight::from_parts(1, 0)', 'Weight::from_parts(1, 0))'), 'first', pallet),
    () => mergeCandidate(base, file(first).replace('fn first()', 'fn first(n: u32 m: u32)'), 'first', pallet),
    () => mergeCandidate(base.replace('impl<T:', 'impl Extra {}\nimpl<T:'), file(first), 'first', pallet),
  ];
  for (const probe of rejected) assert.throws(probe);
  console.log(`Generated Weight candidate merge self-test passed: addition, replacement, preservation and ${rejected.length} refusal classes`);
}

try {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === '--self-test') {
    selfTest();
  } else if (args.length === 2 && args[0] === '--normalize-xcm-override') {
    process.stdout.write(normalizeXcmOverride(readFileSync(args[1], 'utf8')));
  } else {
    assert.equal(args.length, 4, 'Internal support leaf: use benchmarks.sh --merge-candidate');
    const [base, generated, owner, pallet] = args;
    process.stdout.write(mergeCandidate(readFileSync(base, 'utf8'), readFileSync(generated, 'utf8'), owner, pallet));
  }
} catch (error) {
  console.error(`Weight candidate merge refused: ${error.message}`);
  process.exitCode = 1;
}
JS
}

run_self_test() (
    local fixture value build_status=0
    require_commands mktemp rm grep mkdir ln realpath node
    transform_weight_candidate --self-test
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
    MERGE_INPUT=""
    MERGE_BASE=""

    # Protect full bindings before any build or measurement, including path/inode aliases.
    (
        TEMPLATE_DIR="$fixture/template"
        WEIGHTS_DIR="$TEMPLATE_DIR/runtime/src/weights"
        mkdir -p "$WEIGHTS_DIR" "$TEMPLATE_DIR/pallets/actors/src"
        printf 'runtime binding\n' > "$WEIGHTS_DIR/pallet_deos_actors.rs"
        printf 'package binding\n' > "$TEMPLATE_DIR/pallets/actors/src/weights.rs"
        ln -s "$WEIGHTS_DIR" "$fixture/weight-alias"
        ln "$WEIGHTS_DIR/pallet_deos_actors.rs" "$fixture/weight-hardlink.rs"
        local output status
        for output in \
            "$WEIGHTS_DIR/pallet_deos_actors.rs" \
            "$WEIGHTS_DIR/new_binding.rs" \
            "$WEIGHTS_DIR/../weights/pallet_deos_actors.rs" \
            "$TEMPLATE_DIR/pallets/actors/src/weights.rs" \
            "$fixture/weight-alias/pallet_deos_actors.rs" \
            "$fixture/weight-hardlink.rs"; do
            for mode in generated raw; do
                status=0
                (
                    if [[ "$mode" == generated ]]; then
                        parse_args --extrinsic close_actor --output "$output" pallet_deos_actors
                    else
                        parse_args --extrinsic close_actor --raw-only --json-file "$output" pallet_deos_actors
                    fi
                ) > "$fixture/protection-log" 2>&1 || status=$?
                [[ "$status" == 2 ]] || return 1
                grep -Eq 'cannot (replace|alias)' "$fixture/protection-log" || return 1
            done
        done
        [[ "$(<"$WEIGHTS_DIR/pallet_deos_actors.rs")" == 'runtime binding' ]] || return 1
        [[ "$(<"$TEMPLATE_DIR/pallets/actors/src/weights.rs")" == 'package binding' ]] || return 1
    )

    # A refused structural merge cannot overwrite the previous candidate or leak staging files.
    (
        MERGE_INPUT="$fixture/malformed.rs"
        MERGE_BASE="$fixture/malformed.rs"
        EXTRINSIC_PATTERN="close_actor"
        TARGET_PALLET="pallet_deos_actors"
        OUTPUT_OVERRIDE="$fixture/preserved-candidate.rs"
        printf 'not generated Rust\n' > "$MERGE_INPUT"
        printf 'previous candidate\n' > "$OUTPUT_OVERRIDE"
        local status=0
        merge_weight_candidate > "$fixture/merge-log" 2>&1 || status=$?
        [[ "$status" == 1 && "$(<"$OUTPUT_OVERRIDE")" == 'previous candidate' ]] || return 1
        ! compgen -G "$fixture/.weight-candidate.*" > /dev/null
    )

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
                grep -Fxq -- 'pallet_deos_actors::behavior_source_snapshot' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::behavior_service_policy_context' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::behavior_policy_installation' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::close_finalize_policy' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_source_authentication' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_semantic_publication' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_placement_preparation' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_placement_finish' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::contract_behavior_context' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::geometry_behavior_authentication' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::behavior_closing_reset' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::close_public_wide_header_prefix' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::close_public_wide_header_prefix_late_hold' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::close_public_wide_idle' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::close_public_wide_idle_late_hold' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::user_contract_installation' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::user_creation_admission_payload' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_publication' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::zero_step_policy_terminal' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::stop_cycle_policy_terminal' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::stop_cycle_policy_publication' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::transfer_policy_terminal' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::transfer_policy_progression' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::running_policy_predicate' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::liquidity_policy_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::liquidity_policy_late_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::opening_policy_pre_effect_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::opening_policy_no_action_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::funding_policy_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::liquidity_failure_policy_control' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::opening_policy_pre_effect_continue' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::running_policy_service_fee' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::running_policy_transfer' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::running_policy_stop_cycle' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::service_member_to_deadline_new_key_peers' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::service_member_to_deadline_existing' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::scheduler_actor_state_probe_wide_header' "$fixture/raw-args"
                grep -Fxq -- 'pallet_deos_actors::behavior_candidate_publish' "$fixture/raw-args"
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
    merge_weight_candidate() { printf 'merge\n' >> "$fixture/trace"; }

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
    assert_route 0 merge --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic close_actor --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --merge-candidate "$fixture/slice.rs" --extrinsic close_actor --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic close_actor --output "$fixture/base.rs" pallet_deos_actors
    assert_route 2 '' --extrinsic close_actor pallet_deos_actors
    assert_route 0 benchmark --skip-build --extrinsic close_actor --output "$fixture/focused.rs" pallet_deos_actors
    for value in 'close*' 'close?' 'close,create'; do
        assert_route 2 '' --extrinsic "$value" --output "$fixture/focused.rs" pallet_deos_actors
    done
    assert_route 2 '' --raw-only --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --raw-only --json-file "$fixture/samples.json" --output "$fixture/weights.rs" --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --raw-only --json-file "$fixture/samples.json" pallet_deos_actors
    assert_route 0 benchmark --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic close_detach_user --low 2 --high 2 pallet_deos_actors
    assert_route 2 '' --extrinsic service_pending_enactments_full_topology --output "$fixture/focused.rs" pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic service_pending_enactments_full_topology pallet_governance
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic service_pending_enactments_full_topology --low 0 --high 1 pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic financial_wait_record_binding pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic financial_wait_record_binding --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic financial_wait_record_binding pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic manual_trigger_future_deadline pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic manual_trigger_future_deadline --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic manual_trigger_future_deadline --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic manual_trigger_future_deadline pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic financial_service_repark_unqualified pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic financial_service_repark_unqualified --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic financial_service_repark_unqualified --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic financial_service_repark_unqualified --low 0 --high 1 pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic pipeline_admission_apoptosis pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic pipeline_admission_apoptosis --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic pipeline_admission_apoptosis --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic pipeline_admission_apoptosis pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic scheduler_inner_zero_step_passive_window pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic scheduler_inner_zero_step_passive_window --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic scheduler_inner_zero_step_passive_window pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic scheduler_service_zero_step_user_complete pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic scheduler_service_zero_step_user_complete --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic scheduler_service_zero_step_user_complete --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic scheduler_service_zero_step_user_complete pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_publication pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_publication --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_publication --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_publication pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_terminal pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_terminal --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_terminal --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_terminal pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic stop_cycle_policy_terminal pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic stop_cycle_policy_publication pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic transfer_policy_terminal pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic transfer_policy_progression pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_predicate pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_policy_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_policy_late_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_pre_effect_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_no_action_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic funding_policy_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_failure_policy_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_pre_effect_continue pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_service_fee pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_transfer pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_stop_cycle pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic stop_cycle_policy_terminal --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic stop_cycle_policy_publication --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic transfer_policy_terminal --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic transfer_policy_progression --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic running_policy_predicate --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic liquidity_policy_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic liquidity_policy_late_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic opening_policy_pre_effect_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic opening_policy_no_action_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic funding_policy_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic liquidity_failure_policy_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic opening_policy_pre_effect_continue --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic running_policy_service_fee --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic running_policy_transfer --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic running_policy_stop_cycle --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic stop_cycle_policy_terminal --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic stop_cycle_policy_publication --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic transfer_policy_terminal --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic transfer_policy_progression --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic running_policy_predicate --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic liquidity_policy_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic liquidity_policy_late_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic opening_policy_pre_effect_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic opening_policy_no_action_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic funding_policy_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic liquidity_failure_policy_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic opening_policy_pre_effect_continue --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic running_policy_service_fee --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic running_policy_transfer --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic running_policy_stop_cycle --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic stop_cycle_policy_terminal pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic stop_cycle_policy_publication pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic transfer_policy_terminal pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic transfer_policy_progression pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_predicate pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_policy_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_policy_late_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_pre_effect_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_no_action_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic funding_policy_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic liquidity_failure_policy_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic opening_policy_pre_effect_continue pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_service_fee pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_transfer pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic running_policy_stop_cycle pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic service_member_to_deadline_new_key_peers pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic service_member_to_deadline_new_key_peers --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic service_member_to_deadline_new_key_peers --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic service_member_to_deadline_new_key_peers pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic service_member_to_deadline_existing pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic service_member_to_deadline_existing --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic service_member_to_deadline_existing --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic service_member_to_deadline_existing --low 0 --high 0 pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_control pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_source_authentication pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_semantic_publication pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_placement_preparation pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_placement_finish pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic contract_behavior_context pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic geometry_behavior_authentication pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic geometry_behavior_authentication --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic geometry_behavior_authentication --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic geometry_behavior_authentication pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_closing_reset pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic behavior_closing_reset --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic behavior_closing_reset --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_closing_reset pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic user_contract_installation pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic user_creation_admission_payload pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_control --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_source_authentication --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_semantic_publication --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_placement_preparation --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic zero_step_policy_placement_finish --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic contract_behavior_context --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic user_contract_installation --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic user_creation_admission_payload --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_control --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_source_authentication --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_semantic_publication --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_placement_preparation --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic zero_step_policy_placement_finish --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic contract_behavior_context --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic user_contract_installation --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic user_creation_admission_payload --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_control pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_source_authentication pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_semantic_publication pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_placement_preparation pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic zero_step_policy_placement_finish pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic contract_behavior_context pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic user_contract_installation pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic user_creation_admission_payload pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic close_finalize_policy pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic close_finalize_policy --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic close_finalize_policy --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic close_finalize_policy pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_policy_cleanup pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic behavior_policy_cleanup --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --skip-build --extra --raw-only --extrinsic behavior_policy_cleanup pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic behavior_policy_cleanup --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_policy_cleanup pallet_deos_actors
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_policy_installation pallet_deos_actors
    assert_route 2 '' --skip-build --extra --extrinsic behavior_policy_installation --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic behavior_policy_installation --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic behavior_policy_installation pallet_deos_actors
    for value in contract_optional_cleanup behavior_source_snapshot behavior_service_policy_context scheduler_actor_state_probe_wide_header behavior_candidate_publish admission_certificate_authority close_dormant_event_root close_load_wide_idle cadenced_underfunded_rearm_populated cadenced_terminal_cleanup_populated task_add_liquidity_user_refusal task_add_liquidity_system task_add_liquidity_system_refusal task_add_liquidity_system_existing_pool task_stake_user_refusal task_unstake_user_refusal task_remove_liquidity_user_refusal task_dex_exact_in_user task_dex_exact_out_user task_dex_exact_in_user_refusal task_dex_exact_out_user_refusal; do
        assert_route 2 '' --skip-build --extrinsic "$value" --output "$fixture/weights.rs" pallet_deos_actors
        assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic "$value" --output "$fixture/merged.rs" pallet_deos_actors
        assert_route 0 benchmark --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic "$value" pallet_deos_actors
    done
    for value in close_public_wide_header_prefix close_public_wide_header_prefix_late_hold close_public_wide_idle close_public_wide_idle_late_hold; do
        assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic "$value" pallet_deos_actors
        assert_route 2 '' --skip-build --extra --extrinsic "$value" --output "$fixture/weights.rs" pallet_deos_actors
        assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic "$value" --output "$fixture/merged.rs" pallet_deos_actors
        assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic "$value" pallet_deos_actors
    done
    build_status=42
    assert_route 42 build pallet_deos_actors
    build_status=0
    for value in '' 0 1 false; do
        export SKIP_WASM_BUILD="$value"
        assert_route 1 '' pallet_deos_actors
        assert_route 0 benchmark --skip-build pallet_deos_actors
        assert_route 0 check --check
    done
    log_success "Benchmark output-protection/raw-sample/build/reuse routing self-test passed"
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
    if [[ "$pallet_name" == "pallet_xcm" ]]; then
        local normalized
        normalized="$(mktemp "$(dirname "$file")/.xcm-signature.XXXXXX")"
        if ! transform_weight_candidate --normalize-xcm-override "$file" > "$normalized"; then
            rm -f -- "$normalized"
            return 1
        fi
        mv -- "$normalized" "$file"
    fi
    log_info "  Normalized imports, struct name, local paths, and required proof/signature bridges"
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
        "manual_trigger_future_deadline"
        "pipeline_admission_apoptosis"
        "scheduler_inner_zero_step_passive_window"
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
            "manual_trigger_future_deadline"
            "scheduler_inner_zero_step_user_complete"
            "scheduler_service_zero_step_user_complete"
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
  "scheduler_inner_running_user_transfer_burn_two"
  "scheduler_inner_opening_user_transfer_progress_two"
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
            "contract_optional_cleanup"
            "pipeline_admission_apoptosis"
            "financial_wait_record_binding"
            "financial_service_repark_unqualified"
            "behavior_source_snapshot"
            "behavior_service_policy_context"
            "behavior_policy_installation"
            "behavior_policy_cleanup"
            "close_finalize_policy"
            "zero_step_policy_control"
            "zero_step_policy_source_authentication"
            "zero_step_policy_semantic_publication"
            "zero_step_policy_placement_preparation"
            "zero_step_policy_placement_finish"
            "contract_behavior_context"
            "geometry_behavior_authentication"
            "behavior_closing_reset"
            "close_public_wide_header_prefix"
            "close_public_wide_header_prefix_late_hold"
            "close_public_wide_idle"
            "close_public_wide_idle_late_hold"
            "user_contract_installation"
            "user_creation_admission_payload"
            "zero_step_policy_publication"
            "zero_step_policy_terminal"
            "stop_cycle_policy_terminal"
            "stop_cycle_policy_publication"
            "transfer_policy_terminal"
            "transfer_policy_progression"
            "running_policy_predicate"
            "liquidity_policy_control"
            "liquidity_policy_late_control"
            "opening_policy_pre_effect_control"
            "opening_policy_no_action_control"
            "funding_policy_control"
            "liquidity_failure_policy_control"
            "opening_policy_pre_effect_continue"
            "running_policy_service_fee"
            "running_policy_transfer"
            "running_policy_stop_cycle"
            "service_member_to_deadline_new_key_peers"
            "service_member_to_deadline_existing"
            "scheduler_actor_state_probe_wide_header"
            "behavior_candidate_publish"
            "admission_certificate_authority"
            "close_dormant_event_root"
            "cadenced_underfunded_rearm_populated"
            "cadenced_terminal_cleanup_populated"
            "close_load_wide_idle"
            "task_add_liquidity_user_refusal"
            "task_add_liquidity_system"
            "task_add_liquidity_system_refusal"
            "task_add_liquidity_system_existing_pool"
            "task_stake_user_refusal"
            "task_unstake_user_refusal"
            "task_remove_liquidity_user_refusal"
            "task_dex_exact_in_user"
            "task_dex_exact_out_user"
            "task_dex_exact_in_user_refusal"
            "task_dex_exact_out_user_refusal"
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

    if [[ "$pallet_name" == pallet_governance \
        && "$EXTRINSIC_PATTERN" != service_pending_enactments_full_topology ]]; then
        exclude_args+=(--exclude-extrinsics "pallet_governance::service_pending_enactments_full_topology")
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

merge_weight_candidate() (
    require_commands node mktemp chmod mv rm realpath
    [[ -f "$MERGE_INPUT" && -f "$MERGE_BASE" ]] || {
        log_error "Both merge inputs must be existing generated Weight files"
        return 1
    }
    is_known_pallet "$TARGET_PALLET" || { log_error "Unknown pallet: $TARGET_PALLET"; return 1; }
    validate_artifact_output "$OUTPUT_OVERRIDE" || return 1
    require_directory "$(dirname "$OUTPUT_OVERRIDE")" "Candidate output directory"
    local staged
    staged="$(mktemp "$(dirname "$OUTPUT_OVERRIDE")/.weight-candidate.XXXXXX")"
    trap 'rm -f -- "$staged"' EXIT
    transform_weight_candidate \
        "$MERGE_BASE" "$MERGE_INPUT" "$EXTRINSIC_PATTERN" "$TARGET_PALLET" > "$staged" || return 1
    chmod 0644 "$staged"
    mv -f -- "$staged" "$OUTPUT_OVERRIDE"
    log_success "Isolated candidate -> $OUTPUT_OVERRIDE (no production binding or coefficient acceptance)"
)

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

    if [[ "$ACTION" == merge ]]; then
        merge_weight_candidate
        return
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
