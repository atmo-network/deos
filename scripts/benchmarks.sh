#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WEIGHTS_DIR="$TEMPLATE_DIR/runtime/src/weights"
BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks"
AUTHORITY_PROFILE="fixtures"
BENCHMARK_FEATURES="runtime-benchmarks,benchmark-authority-scaffolding"
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
RUNTIME_WASM_OVERRIDE=""
MERGE_INPUT=""
MERGE_BASE=""

usage() {
    cat <<EOF2
Usage: $(basename "$0") [OPTIONS] [PALLET_NAME]

Prepare benchmark Wasm or run benchmarks and generate weights for the current DEOS reference runtime.

Options:
  --steps N       Number of steps per benchmark (default: $STEPS)
  --repeat N      Number of repetitions per benchmark (default: $REPEAT)
  --min-duration N
                  Minimum seconds per benchmark (bencher default: 10; 0 uses --repeat)
  --all           Benchmark all custom pallets (currently refused: Actors P/idle/finalization are unqualified)
  --list          List available pallets
  --check         Run self-tests, verify benchmark compilation, generated storage names and the Actors Weight registry
  --prepare       Build isolated benchmark Wasm and print requested profile/features/path/hash;
                  use alone, except --authority-profile. No bencher, samples, weights or production replacement
  --authority-profile fixtures|ordinary
                  Default fixtures preserves optional benchmark authority scaffolding
                  Ordinary admits standalone --check/--prepare, or the exact frozen-c0 route:
                  --extra --raw-only --json-file NEW_PATH --extrinsic mandatory_prepass_control
                  --low 0 --high 0 --steps 2 --repeat 3 --min-duration 0 pallet_deos_actors
                  Diagnostic requires Linux x86_64, GNU timeout, offline cache and no Cargo configs;
                  output parent must exist, outside the repo or directly in ignored template/target
                  No ordinary reuse, generation, merge or Cargo fallback
                  Preparation identity is not freshness or executed-authority attestation
  --self-test     Test output/raw/build/reuse routing and lossless diagnostic JSON in isolated fixtures
  --extra         Include diagnostic benchmarks excluded from production weights
                  mandatory_prepass_control is exact/raw-only, c=0 frozen/1 ordinary/2 duplicate
  mandatory_prepass_entry_control is exact/raw-only, c=0 fresh/1 duplicate entry
  mandatory_prepass_deadline_control is exact/raw-only, c=0 populated/1 deep retry
  mandatory_prepass_dependency_control is exact/raw-only, c=0 empty/1 member/2 completion
  scheduler_resource_pass_control is exact/raw-only, c=0 no-Action/1 zero/2 RefTime/3 ProofSize
                  runtime_authorship is exact/raw-only; c,d are categorical
                  (pin --low C,D --high C,D for one corner); see --list
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
  --runtime FILE  Read an explicit benchmark Wasm for archived A/B diagnostics; requires
                  --skip-build --raw-only, exact --extrinsic and --json-file; no generation
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
  $(basename "$0") --prepare                  # Build benchmark Wasm without collection
  $(basename "$0") --all                      # Refused until Actors P coverage is qualified
  $(basename "$0") pallet_deos_router        # Benchmark one pallet
  $(basename "$0") --check                    # Compile and audit generated storage names and Weight registry
  $(basename "$0") --extra pallet_deos_actors         # Include Actors diagnostics
  $(basename "$0") --extrinsic service_member_to_deadline_new_key --output /tmp/deadline.rs pallet_deos_actors
  $(basename "$0") --check                    # Source/build validation without generation
  $(basename "$0") --merge-candidate /tmp/setter.rs --base template/runtime/src/weights/pallet_deos_actors.rs --extrinsic set_user_creation_enabled --output /tmp/actors-candidate.rs pallet_deos_actors

Environment:
  DEOS_BINARY_DIR             Tool bundle directory (default: project/bin); select and
                              verify the intended SDK bundle before measured collection
  SKIP_WASM_BUILD              Must be unset for fresh Wasm builds (even 0/empty skips upstream);
                              allowed for --check and explicit --skip-build reuse
  PROTOC, PERL5LIB,
  LD_LIBRARY_PATH             Optional declared ordinary-diagnostic builder inputs only;
                              absolute executable/directory paths, bounded native preflight
                              before allocation. Never passed to collector or its version calls
  INCLUDE_EXTRA_BENCHMARKS=0|1
  DEOS_VERBOSE=0|1              Stream full command and benchmark output (default: 0)
  DEOS_FAILURE_TAIL_LINES=N     Failure excerpt length in compact mode (default: 80)
EOF2
    exit 0
}

parse_args() {
    ACTION=""
    TARGET_PALLET=""

    AUTHORITY_PROFILE="fixtures"
    local profile_seen=0
    local -a original_options=("$@") options=()
    while [[ $# -gt 0 ]]; do
        if [[ "$1" == --authority-profile ]]; then
            if (( profile_seen == 1 )) || [[ $# -lt 2 || ( "$2" != fixtures && "$2" != ordinary ) ]]; then
                log_error "--authority-profile requires fixtures or ordinary and cannot repeat"
                exit 2
            fi
            profile_seen=1
            AUTHORITY_PROFILE="$2"
            shift 2
        else
            options+=("$1")
            shift
        fi
    done
    BENCHMARK_FEATURES="runtime-benchmarks,benchmark-authority-scaffolding"
    BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks"
    if [[ "$AUTHORITY_PROFILE" == ordinary ]]; then
        BENCHMARK_FEATURES="runtime-benchmarks"
        BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks-ordinary-authority"
        if (( ${#options[@]} != 1 )) || [[ "${options[0]:-}" != --check && "${options[0]:-}" != --prepare ]]; then
            # Preserve option/value adjacency; profile extraction must not repair input.
            parse_ordinary_diagnostic "${original_options[@]}" || exit 2
            return 0
        fi
    fi
    set -- "${options[@]}"

    local argument
    for argument in "$@"; do
        if [[ "$argument" == --prepare ]]; then
            if [[ $# != 1 ]]; then
                log_error "--prepare must be used alone except --authority-profile; no measurement, reuse, merge or other action options"
                exit 2
            fi
            ACTION="prepare"
            return 0
        fi
    done

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
            --runtime)
                if [[ $# -lt 2 || -z "$2" || "$2" == --* ]]; then
                    log_error "--runtime requires a benchmark Wasm input file"
                    exit 2
                fi
                RUNTIME_WASM_OVERRIDE="$2"
                shift 2
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
    if [[ -n "$RUNTIME_WASM_OVERRIDE" \
        && ( "$SKIP_BUILD" != 1 || "$RAW_ONLY" != 1 || "$EXTRINSIC_PATTERN" == '*' || -n "$ACTION" ) ]]; then
        log_error "--runtime requires exact --skip-build --raw-only --extrinsic --json-file selection, not another action"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == runtime_authorship ]] \
        && [[ ( "$EXTRINSIC_PATTERN" != authorship_complete && "$EXTRINSIC_PATTERN" != authorship_with_initialize \
            && "$EXTRINSIC_PATTERN" != authorship_initialize_root ) \
            || "$INCLUDE_EXTRA_BENCHMARKS" != 1 \
            || "$RAW_ONLY" != 1 || -z "$JSON_OUTPUT" || -n "$MERGE_INPUT" || -n "$OUTPUT_OVERRIDE" ]]; then
        log_error "runtime_authorship requires --extra --raw-only --json-file and exact --extrinsic authorship_complete, authorship_with_initialize or authorship_initialize_root; no generation or merge"
        exit 2
    fi
    if [[ ( "$ACTION" == all && -z "$TARGET_PALLET" ) \
        || ( "$TARGET_PALLET" == pallet_deos_actors \
            && ( "$EXTRINSIC_PATTERN" == all \
                || "$EXTRINSIC_PATTERN" == scheduler_on_initialize_cutoff \
                || "$EXTRINSIC_PATTERN" == scheduler_on_idle_base \
                || "$EXTRINSIC_PATTERN" == block_resource_finalize \
                || ( "$EXTRINSIC_PATTERN" == '*' && -z "$ACTION" ) ) ) ]]; then
        log_error "Actors scheduler_on_initialize_cutoff, scheduler_on_idle_base and block_resource_finalize are source-unqualified: bulk generation and exact collection/merge are refused; source controls remain diagnostic-only"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == pallet_governance \
        && ( "$EXTRINSIC_PATTERN" == service_pending_enactments_full_topology \
            || "$EXTRINSIC_PATTERN" == service_finalized_proposal_outcomes_diverse \
            || "$EXTRINSIC_PATTERN" == on_initialize_finalized_cleanup \
            || "$EXTRINSIC_PATTERN" == on_initialize_finalized_cleanup_diverse \
            || "$EXTRINSIC_PATTERN" == on_initialize_full_index_value_classes ) \
        && ( "$RAW_ONLY" != 1 || "$INCLUDE_EXTRA_BENCHMARKS" != 1 ) ]]; then
        log_error "$EXTRINSIC_PATTERN requires --extra --raw-only --json-file; unqualified topology is not a production Weight binding"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == pallet_deos_actors \
        && ( "$EXTRINSIC_PATTERN" == manual_trigger_future_deadline \
            || "$EXTRINSIC_PATTERN" == scheduler_inner_zero_step_passive_window \
            || "$EXTRINSIC_PATTERN" == scheduler_service_zero_step_* \
            || "$EXTRINSIC_PATTERN" == financial_wait_record_binding \
            || "$EXTRINSIC_PATTERN" == financial_service_repark_unqualified \
            || "$EXTRINSIC_PATTERN" == behavior_policy_installation \
            || "$EXTRINSIC_PATTERN" == behavior_policy_cleanup \
            || "$EXTRINSIC_PATTERN" == task_transfer_minimum \
            || "$EXTRINSIC_PATTERN" == service_head_allowance \
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
            || "$EXTRINSIC_PATTERN" == pipeline_admission_apoptosis \
            || "$EXTRINSIC_PATTERN" == scheduler_user_pipeline_capacity \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_inspection_refusal \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_shared_refusal \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_opening_no_action \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_control \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_entry_control \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_deadline_control \
        || "$EXTRINSIC_PATTERN" == mandatory_prepass_dependency_control \
            || "$EXTRINSIC_PATTERN" == scheduler_resource_pass_control ) \
        && "$INCLUDE_EXTRA_BENCHMARKS" != 1 ]]; then
        log_error "$EXTRINSIC_PATTERN requires --extra --raw-only --json-file"
        exit 2
    fi
    if [[ "$TARGET_PALLET" == pallet_deos_actors && "$RAW_ONLY" != 1 ]] \
        && [[ "$EXTRINSIC_PATTERN" == manual_trigger_future_deadline \
            || "$EXTRINSIC_PATTERN" == scheduler_inner_zero_step_passive_window \
            || "$EXTRINSIC_PATTERN" == scheduler_service_zero_step_* \
            || "$EXTRINSIC_PATTERN" == contract_optional_cleanup \
            || "$EXTRINSIC_PATTERN" == task_transfer_minimum \
            || "$EXTRINSIC_PATTERN" == service_head_allowance \
            || "$EXTRINSIC_PATTERN" == pipeline_admission_apoptosis \
            || "$EXTRINSIC_PATTERN" == scheduler_user_pipeline_capacity \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_inspection_refusal \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_shared_refusal \
            || "$EXTRINSIC_PATTERN" == scheduler_service_user_opening_no_action \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_control \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_entry_control \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_deadline_control \
            || "$EXTRINSIC_PATTERN" == mandatory_prepass_dependency_control \
            || "$EXTRINSIC_PATTERN" == scheduler_resource_pass_control \
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

# Presence-aware scanner is deliberately separate from the permissive fixture grammar.
parse_ordinary_diagnostic() {
    local key value
    local -A seen=()
    while (( $# )); do
        key="$1"
        [[ -n "$key" ]] || { log_error "Empty ordinary argument"; return 2; }
        [[ ! ${seen[$key]+yes} ]] || { log_error "Repeated ordinary option: $key"; return 2; }
        seen[$key]=1
        case "$key" in
            --extra|--raw-only|pallet_deos_actors) shift ;;
            --authority-profile|--json-file|--extrinsic|--low|--high|--steps|--repeat|--min-duration)
                [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || { log_error "Missing ordinary value: $key"; return 2; }
                value="$2"
                case "$key:$value" in
                    --json-file:*) JSON_OUTPUT="$value" ;;
                    --authority-profile:ordinary|--extrinsic:mandatory_prepass_control|--low:0|--high:0|--steps:2|--repeat:3|--min-duration:0) ;;
                    *) log_error "Not the frozen ordinary coordinate: $key $value"; return 2 ;;
                esac
                shift 2 ;;
            *) log_error "Unsupported ordinary option: $key"; return 2 ;;
        esac
    done
    for key in --authority-profile --extra --raw-only --json-file --extrinsic --low --high --steps --repeat --min-duration pallet_deos_actors; do
        [[ ${seen[$key]+yes} ]] || { log_error "Required ordinary option: $key"; return 2; }
    done
    ACTION=ordinary-diagnostic
    TARGET_PALLET=pallet_deos_actors
}

ordinary_output_path() {
    node --input-type=module - "$JSON_OUTPUT" "$PROJECT_ROOT" "$TEMPLATE_DIR/target" <<'JS'
import assert from 'node:assert/strict';
import { lstatSync, realpathSync } from 'node:fs';
import { resolve, dirname, basename, parse, sep } from 'node:path';
const [input, root, target] = process.argv.slice(2);
const path = resolve(input);
// Refuse symlink ancestors, including dangling destinations, rather than chasing aliases.
for (let at = parse(path).root, parts = path.slice(at.length).split(sep), i = 0; i < parts.length; i++) {
  at = resolve(at, parts[i]);
  try { const s = lstatSync(at); assert(!s.isSymbolicLink(), 'Output symlink/alias refused');
    if (i === parts.length - 1) throw new Error('Output destination already exists');
    assert(s.isDirectory(), 'Output ancestor must be a directory');
  } catch (error) { if (error.code !== 'ENOENT' || i !== parts.length - 1) throw error; }
}
const parent = realpathSync(dirname(path));
const repo = realpathSync(root), allowed = resolve(target);
assert(!(path === repo || path.startsWith(repo + sep)) || parent === allowed,
  'In-repository output is restricted to direct children of ignored template/target');
assert(basename(path) !== '.' && basename(path) !== '..');
console.log(path);
JS
}

ordinary_environment() {
    [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || { log_error "Ordinary diagnostic supports Linux x86_64 only"; return 1; }
    timeout --version | grep -Fq 'GNU coreutils' || { log_error "GNU timeout is required"; return 1; }
    local name dir config
    # First supported scope: no Cargo configs anywhere in the lookup chain, even empty ones.
    for dir in "$PWD" "$TEMPLATE_DIR" "$HOME" "${CARGO_HOME:-$HOME/.cargo}"; do
        dir="$(realpath -e -- "$dir")" || return 1
        while :; do
            for config in "$dir/.cargo/config" "$dir/.cargo/config.toml"; do
                [[ ! -e "$config" && ! -L "$config" ]] || { log_error "Cargo config refused: $config"; return 1; }
            done
            [[ "$dir" != / ]] || break
            dir="$(dirname -- "$dir")"
        done
    done
    for config in "${CARGO_HOME:-$HOME/.cargo}/config" "${CARGO_HOME:-$HOME/.cargo}/config.toml"; do
        [[ ! -e "$config" && ! -L "$config" ]] || { log_error "Cargo home config refused: $config"; return 1; }
    done
    # A clean child environment admits only cache/job/native prerequisite inputs. Reject
    # execution-changing inherited controls by presence, not truthiness (including empty).
    ORDINARY_ENV=(-i "PATH=$PATH" "HOME=$HOME" LC_ALL=C)
    while IFS= read -r name; do
        case "$name" in
            CARGO_HOME|CARGO_BUILD_JOBS|RUSTUP_HOME|CC|CXX|AR|LIBCLANG_PATH|CLANG_PATH|PKG_CONFIG_PATH|PKG_CONFIG_LIBDIR)
                ORDINARY_ENV+=("$name=${!name}")
                printf 'Allowed producer input %s=%s\n' "$name" "${!name}" ;;
            CARGO*|RUST*|WASM*|SKIP_*WASM_BUILD|FORCE_WASM_BUILD|SUBSTRATE*|DOCS_RS|RUNTIME_LOG|RUST_LOG|DEOS_ACTOR*|DEOS_PROJECT_ROOT|INCLUDE_EXTRA_BENCHMARKS|LD_PRELOAD|DYLD_*|SCCACHE*|CCACHE*|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|BINDGEN_EXTRA_CLANG_ARGS*|*_OBSERVER*|*_DIAGNOSTIC*)
                log_error "Inherited producer/observer override refused: $name"; return 1 ;;
        esac
    done < <(compgen -e)
    ORDINARY_BUILDER_ENV=("${ORDINARY_ENV[@]}")
    local value component rest selected
    for name in PROTOC PERL5LIB LD_LIBRARY_PATH; do
        [[ ${!name+x} ]] || continue
        value="${!name}"
        [[ -n "$value" ]] || { log_error "Empty builder input: $name"; return 1; }
        printf 'Declared builder input %s=%s\n' "$name" "$value"
        if [[ "$name" == PROTOC ]]; then
            [[ "$value" == /* && -f "$value" && -x "$value" ]] || { log_error "Invalid executable PROTOC: $value"; return 1; }
            selected="$(realpath -e -- "$value")" || return 1
            printf 'Resolved PROTOC: %s\n' "$selected"
        else
            rest="$value"
            while :; do
                component="${rest%%:*}"
                [[ "$component" == /* && -d "$component" ]] || { log_error "Invalid $name directory component: $component"; return 1; }
                selected="$(realpath -e -- "$component")" || return 1
                printf 'Resolved %s directory: %s\n' "$name" "$selected"
                [[ "$rest" == *:* ]] || break
                rest="${rest#*:}"
            done
        fi
        ORDINARY_BUILDER_ENV+=("$name=$value")
    done
    # Resolve only from the declared PATH; explicitly invalid inputs never fall back.
    selected="${PROTOC-}"
    if [[ ! ${PROTOC+x} ]]; then
        selected="$(env "${ORDINARY_BUILDER_ENV[@]}" bash -c 'command -v protoc')" || { log_error "No selected protoc in PATH"; return 1; }
    fi
    [[ "$selected" == /* && -f "$selected" && -x "$selected" ]] || { log_error "Invalid selected protoc: $selected"; return 1; }
    ORDINARY_PROTOC="$(realpath -e -- "$selected")" || return 1
    selected="$(env "${ORDINARY_BUILDER_ENV[@]}" bash -c 'command -v perl')" || { log_error "No selected Perl in PATH"; return 1; }
    [[ "$selected" == /* && -f "$selected" && -x "$selected" ]] || { log_error "Invalid selected Perl: $selected"; return 1; }
    ORDINARY_PERL="$(realpath -e -- "$selected")" || return 1
    printf 'Selected protoc: %s\nSelected Perl: %s\n' "$ORDINARY_PROTOC" "$ORDINARY_PERL"
}

ordinary_native_preflight() {
    local log status=0 digest
    log="$(mktemp "${TMPDIR:-/tmp}/ordinary-native.XXXXXXXX.log")" || return 1
    digest="$(sha256sum -- "$ORDINARY_PROTOC")" || return 1
    printf 'Selected protoc SHA-256: %s\n' "$digest"
    ordinary_command "Check ordinary native prerequisites" 3m "$log" \
        env "${ORDINARY_BUILDER_ENV[@]}" bash -c '
            "$1" --version || exit $?
            exec "$2" -MFindBin -MFile::Copy -MIPC::Cmd -e '\''
                require Cwd;
                for my $module (qw(FindBin.pm File/Copy.pm IPC/Cmd.pm version.pm)) {
                    next if !exists $INC{$module};
                    my $path = Cwd::abs_path($INC{$module}) or die "Unresolved module $module";
                    print "$module=$path\n";
                }
            '\''
        ' _ "$ORDINARY_PROTOC" "$ORDINARY_PERL" || status=$?
    cat -- "$log" || return 1
    return "$status"
}

# Keep every attempted operation's complete log, even under verbose harness mode.
# Positional argv, not interpolated shell code, carries paths and command arguments.
ordinary_command() {
    local label="$1" deadline="$2" log="$3"
    shift 3
    printf 'Retained log: %s\n' "$log"
    run_command_step "$label" "" bash -c \
        'cd "$1" || exit; log=$2; shift 2; exec "$@" >"$log" 2>&1' \
        _ "$TEMPLATE_DIR" "$log" timeout --kill-after=30s "$deadline" "$@"
}

ordinary_file_identity() {
    local path="$1"
    if [[ ! -e "$path" && ! -L "$path" ]]; then printf 'absent\n'; return; fi
    [[ -f "$path" && ! -L "$path" && -s "$path" ]] || { log_error "Nonregular/empty identity input: $path" >&2; return 1; }
    sha256sum -- "$path" | cut -d ' ' -f 1
}

ordinary_recheck() {
    local current_source current_production current_collector current_wasm
    current_source="$(worktree_evidence_identity)" || return 1
    current_production="$(ordinary_file_identity "$PRODUCTION_RUNTIME_WASM")" || return 1
    current_collector="$(ordinary_file_identity "$collector")" || return 1
    [[ "$current_source" == "$source_identity" \
        && "$current_production" == "$production_identity" \
        && "$current_collector" == "$collector_identity" ]] || {
        log_error "Source, production Wasm or collector drift; incomplete diagnostic retained"; return 1;
    }
    if [[ -n "${wasm_identity:-}" ]]; then
        current_wasm="$(ordinary_file_identity "$wasm")" || return 1
        [[ "$current_wasm" == "$wasm_identity" ]] || { log_error "Selected Wasm drift"; return 1; }
    fi
}

ordinary_validate_metadata() {
    node --input-type=module - "$SCRIPT_DIR/benchmark-diagnostic.mjs" "$1" "$2" "$3" <<'JS'
import { readFileSync, realpathSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
const [modulePath, metadata, manifest, runtime] = process.argv.slice(2);
const { validateOrdinaryWbuildMetadata } = await import(pathToFileURL(modulePath));
const declared = validateOrdinaryWbuildMetadata(readFileSync(metadata, 'utf8'),
  { manifestPath: realpathSync(manifest), runtimePath: realpathSync(runtime) });
if (realpathSync(declared.runtimePath) !== realpathSync(runtime)) throw new Error('Runtime path alias');
console.log('Guest declared inputs validated; not resolved/executed provider attestation');
JS
}

ordinary_report_samples() {
    node --input-type=module - "$SCRIPT_DIR/benchmark-diagnostic.mjs" "$1" <<'JS'
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
const { validateFrozenPrepassSamples } = await import(pathToFileURL(process.argv[2]));
const { observations: o } = validateFrozenPrepassSamples(readFileSync(process.argv[3], 'utf8'));
const exact = x => JSON.stringify(x, (_k, v) => typeof v === 'bigint' ? v.toString() : v);
console.log('DB rows=2; time rows=6; declaration_ps=' + o.declaration);
console.log('DB counters/proof increment: ' + exact(o.database));
o.samples.forEach((sample, i) => console.log('Sample ' + (i + 1) + ': ' + exact(sample)));
console.log('Exceedance count: ' + o.exceedanceCount);
console.log(o.exceedanceCount > 0n ? 'Conditional selected-host/model counterexample.' : 'No counterexample in this finite sample; not soundness.');
console.log('Body ns, separate root ns, proof increments and DB/repeat counters are distinct. No root-time/repeat charge, fit, margin, price, fee, reclaim or retirement credit.');
console.log('No maximum, executed-provider or physical qualification claim. Proof <=1560 does not qualify complete witness.');
JS
}

ordinary_publish() {
    # link(2) is atomic no-clobber (including dangling symlinks and destination races).
    # Keep the staging link as original-byte evidence on success and failure alike.
    node --input-type=module - "$1" "$JSON_OUTPUT" <<'JS'
import { linkSync } from 'node:fs';
linkSync(process.argv[2], process.argv[3]);
JS
}

run_ordinary_diagnostic() (
    require_commands node cargo rustc rustup timeout realpath sha256sum git mktemp uname grep cut dirname env bash || return $?
    JSON_OUTPUT="$(ordinary_output_path)" || return 2
    ordinary_environment || return 2
    ordinary_native_preflight || return $?
    # Verify only the selected bundle through its public offline check; never PATH fallback.
    local collector collector_identity collector_version source_identity production_identity wasm wasm_identity="" manifest metadata staged raw_identity
    collector="$(realpath -e -- "$BIN_DIR/frame-omni-bencher")" || return 1
    [[ "$collector" == "$(realpath -e -- "$BIN_DIR")/frame-omni-bencher" && -x "$collector" && ! -L "$BIN_DIR/frame-omni-bencher" ]] || return 1
    collector_identity="$(ordinary_file_identity "$collector")" || return 1
    # The public check also executes the selected collector's version query.
    env -u PROTOC -u PERL5LIB -u LD_LIBRARY_PATH \
        "$SCRIPT_DIR/01-download-binaries.sh" --check --only frame-omni-bencher || return $?
    [[ "$(ordinary_file_identity "$collector")" == "$collector_identity" ]] || return 1
    source_identity="$(worktree_evidence_identity)" || return 1
    production_identity="$(ordinary_file_identity "$PRODUCTION_RUNTIME_WASM")" || return 1
    printf 'Source identity: %s\nProduction Wasm identity: %s\nCollector: %s\nCollector SHA-256: %s\n' \
        "$source_identity" "$production_identity" "$collector" "$collector_identity"
    collector_version="$(env "${ORDINARY_ENV[@]}" "$collector" --version)" || return 1
    printf 'Collector version: %s\n' "$collector_version"
    uname -a
    grep -m1 'model name' /proc/cpuinfo || return 1
    (cd "$TEMPLATE_DIR" && env "${ORDINARY_BUILDER_ENV[@]}" rustup show active-toolchain \
        && env "${ORDINARY_BUILDER_ENV[@]}" rustc -vV && env "${ORDINARY_BUILDER_ENV[@]}" cargo --version) || return 1
    # Validate ignored allocation before creating anything; existing parent is mandatory.
    [[ -d "$TEMPLATE_DIR/target" && ! -L "$TEMPLATE_DIR/target" ]] || return 1
    git -C "$PROJECT_ROOT" check-ignore -q -- template/target/ordinary-frozen-c0.probe || return 1
    ordinary_recheck || return 1
    BENCHMARK_TARGET_DIR="$(mktemp -d "$TEMPLATE_DIR/target/ordinary-frozen-c0.XXXXXXXX")" || return 1
    printf 'Fresh target retained: %s\n' "$BENCHMARK_TARGET_DIR"
    build_benchmarks || return $?
    ordinary_recheck || return 1
    wasm="$BENCHMARK_TARGET_DIR/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
    wasm_identity="$(ordinary_file_identity "$wasm")" || return 1
    [[ "$wasm_identity" != absent && "$(realpath -e -- "$wasm")" == "$wasm" ]] || { log_error "Missing or aliased fresh Wasm"; return 1; }
    manifest="$BENCHMARK_TARGET_DIR/release/wbuild/deos-runtime/Cargo.toml"
    [[ -f "$manifest" && ! -L "$manifest" && "$(realpath -e -- "$manifest")" == "$manifest" ]] || return 1
    metadata="$BENCHMARK_TARGET_DIR/guest-metadata.json"
    ordinary_command "Inspect guest declared inputs" 3m "$BENCHMARK_TARGET_DIR/metadata.log" \
        bash -c 'out=$1; shift; exec "$@" >"$out"' _ "$metadata" \
        env "${ORDINARY_BUILDER_ENV[@]}" CARGO_TARGET_DIR="$BENCHMARK_TARGET_DIR" CARGO_NET_OFFLINE=true \
        cargo metadata --manifest-path "$manifest" --no-deps --format-version 1 --locked --offline || return $?
    ordinary_validate_metadata "$metadata" "$manifest" "$TEMPLATE_DIR/runtime" || return 1
    ordinary_recheck || return 1
    # Repeat environment/config and destination checks immediately before collection.
    ordinary_environment || return 1
    JSON_OUTPUT="$(ordinary_output_path)" || return 1
    staged="$(mktemp "$(dirname -- "$JSON_OUTPUT")/.ordinary-frozen-c0.XXXXXXXX.json")" || return 1
    printf 'Incomplete/raw staging retained: %s\nSelected Wasm: %s\nSHA-256: %s\n' "$staged" "$wasm" "$wasm_identity"
    ordinary_command "Collect ordinary frozen-c0" 3m "$BENCHMARK_TARGET_DIR/collector.log" \
        env "${ORDINARY_ENV[@]}" "$collector" v1 benchmark pallet --runtime "$wasm" \
        --genesis-builder runtime --genesis-builder-preset development \
        --pallet pallet_deos_actors --extrinsic mandatory_prepass_control \
        --steps 2 --repeat 3 --min-duration 0 --heap-pages 8192 --extra --low 0 --high 0 \
        --json-file "$staged" --no-median-slopes --no-min-squares || return $?
    ordinary_recheck || return 1
    [[ "$(env "${ORDINARY_ENV[@]}" "$collector" --version)" == "$collector_version" ]] || { log_error "Collector version drift"; return 1; }
    [[ -s "$staged" && ! -L "$staged" ]] || return 1
    raw_identity="$(ordinary_file_identity "$staged")" || return 1
    ordinary_report_samples "$staged" || return 1
    ordinary_recheck || return 1
    JSON_OUTPUT="$(ordinary_output_path)" || return 1
    [[ "$(ordinary_file_identity "$staged")" == "$raw_identity" ]] || { log_error "Staged JSON drift"; return 1; }
    ordinary_publish "$staged" || return 1
    printf 'Published original JSON bytes (diagnostic only): %s\n' "$JSON_OUTPUT"
)

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
        log_info "Falling back to cargo test with requested features: $BENCHMARK_FEATURES"
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
    if [[ "$ACTION" == ordinary-diagnostic ]]; then
        ordinary_command "Build ordinary frozen-c0 runtime" 30m "$BENCHMARK_TARGET_DIR/build.log" \
            env "${ORDINARY_BUILDER_ENV[@]}" CARGO_TARGET_DIR="$BENCHMARK_TARGET_DIR" CARGO_NET_OFFLINE=true \
            cargo build --release --locked --offline --features runtime-benchmarks -p deos-runtime || return $?
    else
        run_shell_step \
            "Build deos-runtime with runtime-benchmarks" \
            "" \
            "cd \"$TEMPLATE_DIR\" && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo build --release --locked --features '$BENCHMARK_FEATURES' -p deos-runtime" || return $?
    fi
    if [[ -n "$production_identity" \
        && "$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)" != "$production_identity" ]]; then
        log_error "Benchmark build mutated the canonical production runtime Wasm"
        return 1
    fi
}

prepare_benchmark_runtime() {
    phase_banner "Step 1: Benchmark build-only prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory" || return $?
    hydrate_local_tool_paths
    require_commands cargo sha256sum cut || return $?
    build_benchmarks || return $?
    local runtime_wasm runtime_identity
    runtime_wasm="$(resolve_runtime_wasm_path)" || return $?
    [[ -s "$runtime_wasm" ]] || { log_error "Prepared benchmark Wasm is empty: $runtime_wasm"; return 1; }
    runtime_identity="$(sha256sum "$runtime_wasm" | cut -d ' ' -f 1)" || return $?
    log_success "Prepared isolated benchmark Wasm; no bencher, samples or weights"
    printf 'Requested authority profile: %s\nRequested Cargo features: %s\nBenchmark Wasm: %s\nSHA-256: %s\n' \
        "$AUTHORITY_PROFILE" "$BENCHMARK_FEATURES" "$runtime_wasm" "$runtime_identity"
    log_info "Preparation identity only; verify source freshness and actual Wasm feature/authority closure before collection"
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

run_ordinary_self_test() (
    local fixture="$1/ordinary lane 'quoted'" original_script_dir="$SCRIPT_DIR" original_path="$PATH" name status scenario
    mkdir -p "$fixture/tools" "$fixture/ambient" "$fixture/repo/template/target" "$fixture/repo/template/runtime" \
        "$fixture/repo/scripts" "$fixture/home/.cargo" "$fixture/out" || return 1
    # No ambient build controls or host Cargo configuration participate in these fixtures.
    while IFS= read -r name; do
        case "$name" in CARGO*|RUST*|WASM*|SKIP_*WASM_BUILD|FORCE_WASM_BUILD|SUBSTRATE*|DOCS_RS|RUNTIME_LOG|DEOS_ACTOR*|DEOS_PROJECT_ROOT|INCLUDE_EXTRA_BENCHMARKS|LD_*|SCCACHE*|CCACHE*|*_OBSERVER*|*_DIAGNOSTIC*) unset "$name" ;; esac
    done < <(compgen -e)
    unset PROTOC PERL5LIB LD_LIBRARY_PATH
    mkdir -p "$fixture/native-logs" "$fixture/native one 'quoted'" "$fixture/native two" || return 1
    export TMPDIR="$fixture/native-logs"
    export HOME="$fixture/home" CARGO_HOME="$fixture/home/.cargo"
    PROJECT_ROOT="$fixture/repo"
    TEMPLATE_DIR="$PROJECT_ROOT/template"
    PRODUCTION_RUNTIME_WASM="$fixture/production.wasm"
    SCRIPT_DIR="$fixture/tools"
    BIN_DIR="$fixture/tools"
    export PATH="$fixture/ambient:$fixture/tools:$original_path"
    cat > "$fixture/ambient/frame-omni-bencher" <<'SH' || return 1
#!/usr/bin/env bash
printf 'ambient fallback must never execute\n' >&2
exit 97
SH
    chmod +x "$fixture/ambient/frame-omni-bencher" || return 1
    mkdir -p "$TEMPLATE_DIR/target/benchmarks/release/wbuild/deos-runtime" || return 1
    printf 'stale fallback forbidden\n' > "$TEMPLATE_DIR/target/benchmarks/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" || return 1
    printf 'source\n' > "$PROJECT_ROOT/scripts/source" || return 1
    printf 'production\n' > "$PRODUCTION_RUNTIME_WASM" || return 1
    cp "$original_script_dir/benchmark-diagnostic.mjs" "$SCRIPT_DIR/benchmark-diagnostic.mjs" || return 1
    # The fake executables exercise real handler/harness/argv/validation/publication wiring.
    cat > "$fixture/tools/fake" <<'SH' || return 1
#!/bin/bash
set -eu
base="$(dirname -- "$0")"
scenario="$(<"$base/scenario")"
name="${0##*/}"
printf '%s\n' "$name $*" >> "$base/calls"
case "$name" in
    cargo|rustup|rustc|protoc|perl)
        for variable in PROTOC PERL5LIB LD_LIBRARY_PATH; do
            if [[ -e "$base/expected-$variable" ]]; then
                [[ ${!variable+x} && "${!variable}" == "$(<"$base/expected-$variable")" ]] || exit 81
            else
                [[ ! ${!variable+x} ]] || exit 82
            fi
        done ;;
    frame-omni-bencher)
        # Check before the early --version branch as well as the collector body.
        [[ ! ${PROTOC+x} && ! ${PERL5LIB+x} && ! ${LD_LIBRARY_PATH+x} ]] || {
            echo 'Builder-only inputs reached collector execution' >&2; exit 83;
        } ;;
esac
case "$name" in
    bash)
        if [[ "${2:-}" == 'command -v protoc' && "$scenario" == nonexecutable-protoc \
            || "${2:-}" == 'command -v perl' && "$scenario" == nonexecutable-perl ]]; then
            printf '%s\n' "$base/scenario"; exit 0
        fi
        if [[ "${2:-}" == 'command -v protoc' && "$scenario" == missing-protoc \
            || "${2:-}" == 'command -v perl' && "$scenario" == missing-perl ]]; then exit 1; fi
        exec /bin/bash "$@" ;;
    protoc)
        [[ "$*" == --version ]] || exit 84
        [[ "$scenario" != protoc-failure ]] || exit 18
        printf 'libprotoc fixture\n' ;;
    perl)
        [[ "$1" == -MFindBin && "$2" == -MFile::Copy && "$3" == -MIPC::Cmd && "$4" == -e ]] || exit 85
        [[ "$5" == *'qw(FindBin.pm File/Copy.pm IPC/Cmd.pm version.pm)'* \
            && "$5" == *'print "$module=$path\n";'* ]] || exit 86
        [[ "$scenario" != perl-failure ]] || { echo 'Missing version.pm' >&2; exit 19; }
        printf 'FindBin.pm=/fixture/FindBin.pm\nFile/Copy.pm=/fixture/File/Copy.pm\nIPC/Cmd.pm=/fixture/IPC/Cmd.pm\nversion.pm=/fixture/version.pm\n' ;;
    git)
        if [[ "$*" == *check-ignore* ]]; then exit 0; fi
        printf 'scripts/source\0' ;;
    rustup|rustc) printf 'fixture toolchain\n' ;;
    01-download-binaries.sh)
        [[ "$*" == '--check --only frame-omni-bencher' ]] || exit 88
        [[ "$scenario" != pinned-refusal ]] || exit 17
        "$base/frame-omni-bencher" --version || exit $? ;;
    timeout)
        if [[ "${1:-}" == --version ]]; then echo 'timeout (GNU coreutils) fixture'; exit; fi
        [[ "$1" == --kill-after=30s && ( "$2" == 30m || "$2" == 3m ) ]] || exit 89
        printf '%s\0' "$@" >> "$base/deadlines"
        shift 2
        if [[ "$*" == *-MFindBin* ]]; then
            case "$scenario" in native-timeout) exit 124 ;; native-killed) exit 137 ;; esac
        fi
        exec "$@" ;;
    cargo)
        if [[ "$1" == --version ]]; then echo 'cargo fixture'; exit; fi
        printf '%s\0' "$@" > "$base/cargo-$1.argv"
        if [[ "$1" == build ]]; then
            [[ "$*" == 'build --release --locked --offline --features runtime-benchmarks -p deos-runtime' ]] || exit 90
            [[ -d "$CARGO_TARGET_DIR" && -z "$(ls -A "$CARGO_TARGET_DIR")" ]] || {
                # build.log is opened before Cargo; no artifact/cache reuse is permitted.
                [[ "$(ls -A "$CARGO_TARGET_DIR")" == build.log ]] || exit 91
            }
            printf '%s\n' "$CARGO_TARGET_DIR" >> "$base/targets"
            echo 'complete build log'
            [[ "$scenario" != build-failure ]] || exit 17
            guest="$CARGO_TARGET_DIR/release/wbuild/deos-runtime"
            mkdir -p "$guest"
            printf 'manifest fixture\n' > "$guest/Cargo.toml"
            if [[ "$scenario" != missing-artifact ]]; then printf 'fresh wasm\n' > "$guest/deos_runtime.compact.compressed.wasm"; fi
            case "$scenario" in
                source-build-drift) printf 'drift\n' >> "$base/../repo/scripts/source" ;;
                production-build-drift) printf 'drift\n' >> "$base/../production.wasm" ;;
            esac
        else
            [[ "$1" == metadata ]] || exit 92
            manifest="$3"
            if [[ "$scenario" == native-path-drift ]]; then chmod -x "$base/protoc"; fi
            node --input-type=module - "$manifest" "$base/../repo/template/runtime" "$scenario" <<'JS'
import { realpathSync } from 'node:fs';
const [manifest, runtime, scenario] = process.argv.slice(2);
console.log(JSON.stringify({version:1, packages:[{manifest_path:manifest, dependencies:[{
  name:'deos-runtime', rename:'wasm-project', source:null, path:realpathSync(runtime),
  uses_default_features:scenario === 'bad-metadata', features:['hex-literal','runtime-benchmarks']
}]}]}));
JS
        fi ;;
    frame-omni-bencher)
        if [[ "$1" == --version ]]; then echo 'frame-omni-bencher fixture-pinned'; exit; fi
        printf '%s\0' "$@" > "$base/collector.argv"
        [[ $# -ge 3 && "$1" == v1 && "$2" == benchmark && "$3" == pallet ]] || exit 93
        shift 3
        wasm='' json=''
        while (( $# )); do
            case "$1" in --runtime) wasm="$2"; shift ;; --json-file) json="$2"; shift ;; esac
            shift
        done
        [[ -n "$wasm" && -n "$json" ]] || exit 93
        printf '%s\n' "$json" > "$base/staged"
        echo 'complete collector log'
        if [[ "$scenario" == timeout ]]; then printf '[partial' > "$json"; exit 124; fi
        cp "$base/sample.json" "$json"
        case "$scenario" in
            invalid-json) printf '[partial' > "$json" ;;
            source-drift) printf 'drift\n' >> "$base/../repo/scripts/source" ;;
            wasm-drift) printf 'drift\n' >> "$wasm" ;;
            collector-drift) printf '\n# drift\n' >> "$0" ;;
            production-drift) printf 'drift\n' >> "$base/../production.wasm" ;;
            race) printf 'concurrent winner\n' > "$base/../out/result.json" ;;
        esac ;;
    *) exit 94 ;;
esac
SH
    chmod +x "$fixture/tools/fake" || return 1
    for name in bash protoc perl cargo rustup rustc git timeout frame-omni-bencher 01-download-binaries.sh; do
        cp "$fixture/tools/fake" "$fixture/tools/$name" || return 1
        [[ "$(command -v "$name")" == "$fixture/tools/$name" || "$name" == frame-omni-bencher ]] || return 1
    done
    node --input-type=module - "$fixture/tools/sample.json" <<'JS' || return 1
import { writeFileSync } from 'node:fs';
const row = {components:[['c',0]], extrinsic_time:1, storage_root_time:2,
  reads:1, repeat_reads:3, writes:1, repeat_writes:4, proof_size:0};
let text = JSON.stringify([{pallet:'pallet_deos_actors',instance:'Actors',benchmark:'mandatory_prepass_control',
  time_results:Array.from({length:6},()=>({...row})),db_results:[row,row]}],null,2);
text = text.replace('"extrinsic_time": 1', '"extrinsic_time": 340282366920938463463374607431768211455');
writeFileSync(process.argv[2],text + '\n');
JS
    local -a args=(--authority-profile ordinary --extra --raw-only --json-file "$fixture/out/result.json"
        --extrinsic mandatory_prepass_control --low 0 --high 0 --steps 2 --repeat 3 --min-duration 0 pallet_deos_actors)
    printf 'success\n' > "$fixture/tools/scenario" || return 1
    ordinary_assert_refusal() {
        local status=0 before after before_targets after_targets
        before_targets="$(compgen -G "$TEMPLATE_DIR/target/ordinary-frozen-c0.*" || :)"
        before="$(grep -c '^frame-omni-bencher ' "$fixture/tools/calls" 2>/dev/null || :)"
        (main "$@") > "$fixture/refusal.log" 2>&1 || status=$?
        after="$(grep -c '^frame-omni-bencher ' "$fixture/tools/calls" 2>/dev/null || :)"
        after_targets="$(compgen -G "$TEMPLATE_DIR/target/ordinary-frozen-c0.*" || :)"
        [[ "$status" != 0 && "$before_targets" == "$after_targets" && "${before:-0}" == "${after:-0}" && ! -e "$fixture/tools/targets" && ! -e "$fixture/tools/collector.argv" ]] || {
            log_error "Ordinary refusal had effects or succeeded: $* ($status)"; return 1;
        }
    }
    ordinary_assert_original_refusal() {
        local status=0
        # A malformed original stream must exit 2, never even enter prerequisites.
        (run_ordinary_diagnostic() { return 95; }; main "$@") > "$fixture/original-argv.log" 2>&1 || status=$?
        [[ "$status" == 2 ]] || {
            log_error "Malformed original ordinary argv reached dispatch or had wrong status: $status"; return 1;
        }
    }
    # Exercise native admission through the real handler, never ambient native tools.
    local variable value
    for variable in PROTOC PERL5LIB LD_LIBRARY_PATH; do
        local -a invalid=('' relative "$fixture/missing" "$fixture/tools/scenario")
        if [[ "$variable" == PROTOC ]]; then
            invalid+=("$fixture/tools")
        else
            invalid+=(":$fixture/tools" "$fixture/tools:" "$fixture/tools::$fixture/home" "$fixture/tools:relative")
        fi
        for value in "${invalid[@]}"; do
            (export "$variable=$value"; ordinary_assert_refusal "${args[@]}") || return 1
        done
    done
    for scenario in missing-protoc missing-perl nonexecutable-protoc nonexecutable-perl protoc-failure perl-failure native-timeout native-killed; do
        printf '%s\n' "$scenario" > "$fixture/tools/scenario" || return 1
        status=0
        (main "${args[@]}") > "$fixture/$scenario.log" 2>&1 || status=$?
        [[ "$status" != 0 && ! -e "$fixture/tools/targets" && ! -e "$fixture/tools/collector.argv" ]] || return 1
        ! compgen -G "$TEMPLATE_DIR/target/ordinary-frozen-c0.*" > /dev/null || return 1
        ! grep -Eq '^frame-omni-bencher |^cargo ' "$fixture/tools/calls" || return 1
        case "$scenario" in
            native-timeout) [[ "$status" == 124 ]] || return 1 ;;
            native-killed) [[ "$status" == 137 ]] || return 1 ;;
        esac
    done
    printf 'success\n' > "$fixture/tools/scenario" || return 1
    # Unset inputs use selected PATH tools, with the same functional preflight.
    local native_output="$fixture/out/native-default.json"
    (main "${args[@]:0:5}" "$native_output" "${args[@]:6}") > "$fixture/native-default.log" 2>&1 || {
        report_step_failure "$fixture/native-default.log"; return 1;
    }
    grep -Fq 'version.pm=/fixture/version.pm' "$fixture/native-default.log" || return 1
    [[ "$(grep -c '^protoc --version$' "$fixture/tools/calls")" == 3 ]] || return 1
    # Remove only fixture bookkeeping so subsequent no-effect refusal assertions stay strict.
    rm -f -- "$fixture/tools/targets" "$fixture/tools/collector.argv" "$fixture/tools/calls" || return 1
    # Preserve fake dispatch identity while exercising quoted/space executable paths via a symlink.
    ln -s "$fixture/tools/protoc" "$fixture/tools/protoc 'selected'" || return 1
    export PROTOC="$fixture/tools/protoc 'selected'"
    export PERL5LIB="$fixture/native one 'quoted':$fixture/native two"
    export LD_LIBRARY_PATH="$fixture/native two:$fixture/native one 'quoted'"
    for variable in PROTOC PERL5LIB LD_LIBRARY_PATH; do
        printf '%s' "${!variable}" > "$fixture/tools/expected-$variable" || return 1
    done
    # The public check delegates a version query; exercise that complete process edge.
    (main "${args[@]:0:5}" "$fixture/out/native-checker-isolation.json" "${args[@]:6}") > "$fixture/native-checker-isolation.log" 2>&1 || {
        report_step_failure "$fixture/native-checker-isolation.log"; return 1;
    }
    [[ "$(grep -c '^frame-omni-bencher --version$' "$fixture/tools/calls")" == 3 \
        && "$(grep -c '^frame-omni-bencher v1 benchmark pallet ' "$fixture/tools/calls")" == 1 ]] || return 1
    rm -f -- "$fixture/tools/targets" "$fixture/tools/collector.argv" "$fixture/tools/calls" "$fixture/tools/staged" || return 1
    for scenario in protoc-failure perl-failure native-timeout native-killed; do
        printf '%s\n' "$scenario" > "$fixture/tools/scenario" || return 1
        ordinary_assert_refusal "${args[@]}" || return 1
    done
    printf 'success\n' > "$fixture/tools/scenario" || return 1
    # Exact allowlist accepts profile/flags in either order without entering generic machinery.
    (parse_args "${args[@]}"; [[ "$ACTION" == ordinary-diagnostic ]]) || return 1
    (parse_args pallet_deos_actors --min-duration 0 --repeat 3 --steps 2 --high 0 --low 0 \
        --extrinsic mandatory_prepass_control --json-file "$fixture/out/result.json" --raw-only --extra \
        --authority-profile ordinary; [[ "$ACTION" == ordinary-diagnostic ]]) || return 1
    # Each option missing, each option duplicated, unknown/conflicting/inert values, both orders.
    local i width bad
    for (( i=2; i<${#args[@]}; )); do
        width=1
        case "${args[i]}" in --json-file|--extrinsic|--low|--high|--steps|--repeat|--min-duration) width=2 ;; esac
        ordinary_assert_refusal "${args[@]:0:i}" "${args[@]:i+width}" || return 1
        ordinary_assert_refusal "${args[@]}" "${args[@]:i:width}" || return 1
        ordinary_assert_refusal "${args[@]:i:width}" "${args[@]}" || return 1
        if (( width == 2 )); then
            ordinary_assert_original_refusal "${args[@]:2:i-1}" --authority-profile ordinary "${args[@]:i+1}" || return 1
            ordinary_assert_refusal "${args[@]:0:i+1}" || return 1
            if [[ "${args[i]}" != --json-file ]]; then
                ordinary_assert_refusal "${args[@]:0:i+1}" wrong "${args[@]:i+2}" || return 1
            fi
        fi
        (( i+=width ))
    done
    for bad in --runtime --skip-build --output --regression --merge-candidate --list --all --unknown --prepare --check pallet_other; do
        ordinary_assert_refusal "$bad" "${args[@]}" || return 1
        ordinary_assert_refusal "${args[@]}" "$bad" || return 1
    done
    for name in CARGO_NET_OFFLINE SKIP_WASM_BUILD CARGO_TARGET_DIR CARGO_BUILD_TARGET CARGO_PROFILE_RELEASE_OPT_LEVEL CARGO_FEATURE_X RUSTFLAGS RUSTUP_TOOLCHAIN WASM_BUILD_CARGO_ARGS WASM_BUILD_TOOLCHAIN WASM_TARGET_DIRECTORY SUBSTRATE_RUNTIME_TARGET RUST_LOG RUNTIME_LOG DEOS_ACTOR_CONTROL_WASM; do
        (export "$name="; ordinary_assert_refusal "${args[@]}") || return 1
    done
    for bad in "$fixture/home/.cargo/config" "$fixture/repo/.cargo/config.toml"; do
        mkdir -p "$(dirname -- "$bad")"
        printf 'config\n' > "$bad"
        ordinary_assert_refusal "${args[@]}" || return 1
        rm -- "$bad"
    done
    local output
    for output in "$fixture/repo/source.json" "$TEMPLATE_DIR/runtime/output.json" "$TEMPLATE_DIR/target/release/out.json" "$fixture/missing/output.json"; do
        ordinary_assert_refusal "${args[@]:0:5}" "$output" "${args[@]:6}" || return 1
    done
    printf 'old evidence\n' > "$fixture/out/existing.json"
    ln -s "$fixture/out/no-file" "$fixture/out/dangling.json"
    ln -s "$fixture/out" "$fixture/out-alias"
    ln "$fixture/out/existing.json" "$fixture/out/hardlink.json"
    for output in "$fixture/out/existing.json" "$fixture/out/dangling.json" "$fixture/out/hardlink.json" "$fixture/out-alias/result.json"; do
        ordinary_assert_refusal "${args[@]:0:5}" "$output" "${args[@]:6}" || return 1
    done
    printf 'pinned-refusal\n' > "$fixture/tools/scenario"
    ordinary_assert_refusal "${args[@]}" || return 1
    mv -- "$fixture/tools/frame-omni-bencher" "$fixture/tools/selected-backup"
    ordinary_assert_refusal "${args[@]}" || return 1
    mv -- "$fixture/tools/selected-backup" "$fixture/tools/frame-omni-bencher"
    for scenario in build-failure missing-artifact bad-metadata native-path-drift source-build-drift production-build-drift invalid-json source-drift wasm-drift collector-drift production-drift timeout race success; do
        printf '%s\n' "$scenario" > "$fixture/tools/scenario" || return 1
        chmod +x "$fixture/tools/protoc" || return 1
        rm -f -- "$fixture/tools/calls" || return 1
        rm -f -- "$fixture/tools/collector.argv" "$fixture/tools/staged" "$fixture/out/result.json"
        status=0
        (main "${args[@]}") > "$fixture/$scenario.log" 2>&1 || status=$?
        if [[ "$scenario" == success ]]; then
            [[ "$status" == 0 ]] || { report_step_failure "$fixture/$scenario.log"; return 1; }
            cmp "$fixture/tools/sample.json" "$fixture/out/result.json" || return 1
            grep -Fq '340282366920938463463374607431768211455' "$fixture/success.log" || return 1
            grep -Fq 'Exceedance count: 1' "$fixture/success.log" || return 1
        else
            [[ "$status" != 0 ]] || { log_error "Unexpected ordinary success: $scenario"; return 1; }
            if [[ "$scenario" == race ]]; then
                [[ "$(<"$fixture/out/result.json")" == 'concurrent winner' ]] || return 1
            else [[ ! -e "$fixture/out/result.json" ]] || return 1
            fi
            if [[ "$scenario" == timeout ]]; then
                [[ "$status" == 124 && "$(<"$(<"$fixture/tools/staged")")" == '[partial' ]] || { report_step_failure "$fixture/$scenario.log"; return 1; }
            fi
            case "$scenario" in
                build-failure) [[ "$status" == 17 ]] || return 1 ;;
                missing-artifact) grep -Fq 'Missing or aliased fresh Wasm' "$fixture/$scenario.log" || return 1 ;;
                bad-metadata) grep -Fq 'default features must be disabled' "$fixture/$scenario.log" || return 1 ;;
                native-path-drift)
                    [[ ! -e "$fixture/tools/collector.argv" ]] || return 1
                    grep -Fq 'Invalid executable PROTOC' "$fixture/$scenario.log" || return 1 ;;
                production-build-drift) grep -Fq 'Benchmark build mutated the canonical production runtime Wasm' "$fixture/$scenario.log" || return 1 ;;
                source-build-drift|source-drift|collector-drift|production-drift)
                    grep -Fq 'Source, production Wasm or collector drift' "$fixture/$scenario.log" || return 1 ;;
                wasm-drift) grep -Fq 'Selected Wasm drift' "$fixture/$scenario.log" || return 1 ;;
                invalid-json) grep -Fq 'SyntaxError' "$fixture/$scenario.log" || return 1 ;;
            esac
        fi
        [[ "$(grep -c '^protoc --version$' "$fixture/tools/calls")" == 1 \
            && "$(grep -c '^perl -MFindBin ' "$fixture/tools/calls")" == 1 ]] || return 1
        local target
        target="$(tail -1 "$fixture/tools/targets")"
        [[ -s "$target/build.log" ]] || return 1
        if [[ -e "$fixture/tools/collector.argv" ]]; then [[ -s "$target/collector.log" && -s "$(<"$fixture/tools/staged")" ]] || return 1; fi
    done
    # Exact argv/deadlines and distinct empty-target allocations, with quotes/spaces intact.
    node --input-type=module - "$fixture" <<'JS' || return 1
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const f=process.argv[2], args=readFileSync(f+'/tools/collector.argv','utf8').split('\0').slice(0,-1);
const target=readFileSync(f+'/tools/targets','utf8').trimEnd().split('\n');
assert.equal(new Set(target).size,target.length);
assert.deepEqual(args,['v1','benchmark','pallet','--runtime',target.at(-1)+'/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm',
  '--genesis-builder','runtime','--genesis-builder-preset','development','--pallet','pallet_deos_actors',
  '--extrinsic','mandatory_prepass_control','--steps','2','--repeat','3','--min-duration','0','--heap-pages','8192',
  '--extra','--low','0','--high','0','--json-file',readFileSync(f+'/tools/staged','utf8').trimEnd(),
  '--no-median-slopes','--no-min-squares']);
const deadlines=readFileSync(f+'/tools/deadlines','utf8').split('\0');
assert(deadlines.includes('30m') && deadlines.includes('3m') && deadlines.includes('--kill-after=30s'));
assert.deepEqual(readFileSync(f+'/tools/cargo-metadata.argv','utf8').split('\0').slice(0,-1),
 ['metadata','--manifest-path',target.at(-1)+'/release/wbuild/deos-runtime/Cargo.toml','--no-deps','--format-version','1','--locked','--offline']);
JS
    # A concurrent winner inserted after the last path check still cannot be overwritten.
    JSON_OUTPUT="$fixture/out/publication-race.json"
    ordinary_output_path > /dev/null || return 1
    printf 'old evidence\n' > "$JSON_OUTPUT"
    if ordinary_publish "$fixture/tools/sample.json" > "$fixture/link.log" 2>&1; then return 1; fi
    [[ "$(<"$JSON_OUTPUT")" == 'old evidence' ]] || return 1
    log_success "Ordinary frozen-c0 guarded lane self-test passed (fake build/collector only)"
)

run_self_test() (
    local fixture value build_status=0 extrinsic
    require_commands mktemp rm grep mkdir ln realpath node
    transform_weight_candidate --self-test
    node --input-type=module - "$PROJECT_ROOT/scripts/benchmark-diagnostic.mjs" <<'JS'
import { pathToFileURL } from 'node:url';
const { selfTest } = await import(pathToFileURL(process.argv[2]).href);
selfTest();
console.log('Frozen Prepass diagnostic JSON self-test passed: exact integers, schema, model and guest metadata');
JS
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/deos-benchmark-routing.XXXXXX")"
    trap 'rm -rf -- "$fixture"' EXIT
    run_ordinary_self_test "$fixture" || return 1
    PRODUCTION_RUNTIME_WASM="$fixture/absent-production.wasm"
    SKIP_BUILD=0
    RUNTIME_WASM_OVERRIDE=""
    EXTRINSIC_PATTERN="*"
    OUTPUT_OVERRIDE=""
    JSON_OUTPUT=""
    RAW_ONLY=0
    COMPONENT_LOW=""
    COMPONENT_HIGH=""
    MERGE_INPUT=""
    MERGE_BASE=""

    # Build-only is a standalone action, not an alias for a measuring or reuse route.
    (
        local option status
        for option in --all --list --check --self-test --extra --skip-build --raw-only \
            --steps --repeat --min-duration --extrinsic --output --json-file --low --high \
            --runtime --merge-candidate --base --prepare pallet_deos_actors; do
            for order in before after; do
                status=0
                (
                    if [[ "$order" == before ]]; then parse_args "$option" --prepare
                    else parse_args --prepare "$option"; fi
                ) > "$fixture/prepare-guard-log" 2>&1 || status=$?
                [[ "$status" == 2 ]] || return 1
            done
        done
        TEMPLATE_DIR="$fixture/prepare-template"
        BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks"
        PRODUCTION_RUNTIME_WASM="$fixture/prepare-production.wasm"
        local prepared profile expected_features
        local -a prepare_args=()
        mkdir -p "$TEMPLATE_DIR"
        printf 'protected production\n' > "$PRODUCTION_RUNTIME_WASM"
        local production_identity
        production_identity="$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)"
        require_commands() { :; }
        hydrate_local_tool_paths() { :; }
        check_prerequisites() { return 91; }
        run_pallet_benchmark() { return 92; }
        run_all_benchmarks() { return 93; }
        merge_weight_candidate() { return 94; }
        normalize_weight_file() { return 95; }
        for profile in fixtures ordinary; do
            prepare_args=(--prepare)
            expected_features="runtime-benchmarks,benchmark-authority-scaffolding"
            BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks"
            if [[ "$profile" == ordinary ]]; then
                prepare_args+=(--authority-profile ordinary)
                expected_features="runtime-benchmarks"
                BENCHMARK_TARGET_DIR="$TEMPLATE_DIR/target/benchmarks-ordinary-authority"
            fi
            prepared="$BENCHMARK_TARGET_DIR/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
            mkdir -p "$(dirname "$prepared")"
            run_shell_step() {
                [[ "$3" == *"CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo build --release --locked --features '$expected_features' -p deos-runtime" ]] || return 96
                printf 'compiled fixture\n' > "$prepared"
            }
            unset SKIP_WASM_BUILD
            main "${prepare_args[@]}" > "$fixture/prepare-log" 2>&1 || return 1
            grep -Fxq "Requested authority profile: $profile" "$fixture/prepare-log" || return 1
            grep -Fxq "Requested Cargo features: $expected_features" "$fixture/prepare-log" || return 1
            grep -Fq "Benchmark Wasm: $prepared" "$fixture/prepare-log" || return 1
            grep -Fq "SHA-256: $(sha256sum "$prepared" | cut -d ' ' -f 1)" "$fixture/prepare-log" || return 1
            [[ "$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)" == "$production_identity" ]] || return 1
            for skip_value in 1 0 ''; do
                status=0
                (export SKIP_WASM_BUILD="$skip_value"; main "${prepare_args[@]}") > "$fixture/prepare-skip-log" 2>&1 || status=$?
                [[ "$status" != 0 ]] || return 1
            done
            # A failed build cannot publish the preceding artifact's identity as preparation success.
            run_shell_step() { return 17; }
            status=0
            main "${prepare_args[@]}" > "$fixture/prepare-failure-log" 2>&1 || status=$?
            [[ "$status" == 17 ]] || return 1
            ! grep -Fq 'Prepared isolated benchmark Wasm' "$fixture/prepare-failure-log" || return 1
        done
    ) || return 1

    # The ordinary lane admits only its two standalone actions, regardless of option order/value.
    (
        expect_profile_refusal() {
            local status=0
            (main "$@") > "$fixture/profile-guard-log" 2>&1 || status=$?
            [[ "$status" == 2 ]] || { log_error "Expected profile refusal: $* (status=$status)"; return 1; }
        }
        (
            check_prerequisites() { return 91; }
            check_only() { return 92; }
            prepare_benchmark_runtime() { return 93; }
            local action option
            for action in --check --prepare; do
                for option in --all --list --check --prepare --self-test --extra --skip-build --raw-only \
                    --steps --repeat --min-duration --extrinsic --output --json-file --low --high \
                    --runtime --merge-candidate --base pallet_deos_actors; do
                    expect_profile_refusal --authority-profile ordinary "$option" "$action" || return 1
                    expect_profile_refusal "$action" "$option" --authority-profile ordinary || return 1
                done
                expect_profile_refusal --authority-profile ordinary "$action" --steps 50 || return 1
                expect_profile_refusal --repeat 20 "$action" --authority-profile ordinary || return 1
                expect_profile_refusal --runtime "$fixture/old.wasm" --skip-build --raw-only \
                    --json-file "$fixture/old.json" --extrinsic mandatory_prepass_control \
                    pallet_deos_actors "$action" --authority-profile ordinary || return 1
            done
            expect_profile_refusal --authority-profile ordinary || return 1
            expect_profile_refusal --check --authority-profile || return 1
            expect_profile_refusal --prepare --authority-profile other || return 1
            for profile in fixtures ordinary; do
                expect_profile_refusal --authority-profile "$profile" --check --authority-profile "$profile" || return 1
            done
        ) || return 1
        TEMPLATE_DIR="$fixture/profile-template"
        PRODUCTION_RUNTIME_WASM="$fixture/profile-production.wasm"
        local default_wasm="$TEMPLATE_DIR/target/benchmarks/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
        mkdir -p "$(dirname "$default_wasm")"
        printf 'fixture only\n' > "$default_wasm"
        printf 'production only\n' > "$PRODUCTION_RUNTIME_WASM"
        parse_args --authority-profile ordinary --prepare
        if resolve_runtime_wasm_path > "$fixture/profile-resolution-log" 2>&1; then return 1; fi
        [[ "$BENCHMARK_TARGET_DIR" == "$TEMPLATE_DIR/target/benchmarks-ordinary-authority" ]] || return 1

        run_self_test() { :; }
        verify_actors_required_benchmark_source() { :; }
        audit_generated_weight_storage_names() { :; }
        audit_weight_registry() { :; }
        run_shell_step() { printf '%s\n' "$3" > "$fixture/profile-command"; }
        local profile target features
        for profile in fixtures ordinary; do
            target="$TEMPLATE_DIR/target/benchmarks"
            features="runtime-benchmarks,benchmark-authority-scaffolding"
            if [[ "$profile" == ordinary ]]; then
                target="$TEMPLATE_DIR/target/benchmarks-ordinary-authority"
                features="runtime-benchmarks"
            fi
            (main --authority-profile "$profile" --check) > "$fixture/profile-check-log" 2>&1 || return 1
            grep -Fxq "cd \"$TEMPLATE_DIR\" && CARGO_TARGET_DIR='$target' cargo check --locked --features '$features'" "$fixture/profile-command" || return 1
        done
        parse_args pallet_deos_router
        BENCHER_MODE="cargo"
        run_pallet_benchmark pallet_deos_router > "$fixture/profile-fallback-log" 2>&1 || return 1
        grep -Fxq "cd '$TEMPLATE_DIR' && CARGO_TARGET_DIR='$TEMPLATE_DIR/target/benchmarks' cargo test --release --locked --features 'runtime-benchmarks,benchmark-authority-scaffolding' -p deos-runtime -- benchmark --nocapture" "$fixture/profile-command" || return 1
    ) || return 1

    # Runtime hook diagnostics are exact/raw-only and never join bulk weight generation.
    for extrinsic in authorship_complete authorship_with_initialize authorship_initialize_root; do
        (
            local mode status
            for mode in generated missing-extra wrong-name merge; do
                status=0
                (
                    case "$mode" in
                        generated) parse_args --extra --extrinsic "$extrinsic" --output "$fixture/hook.rs" runtime_authorship ;;
                        missing-extra) parse_args --raw-only --extrinsic "$extrinsic" --json-file "$fixture/hook.json" runtime_authorship ;;
                        wrong-name) parse_args --extra --raw-only --extrinsic other --json-file "$fixture/hook.json" runtime_authorship ;;
                        merge) parse_args --merge-candidate "$fixture/method.rs" --base "$fixture/base.rs" --extrinsic "$extrinsic" --output "$fixture/hook.rs" runtime_authorship ;;
                    esac
                ) > "$fixture/authorship-guard-log" 2>&1 || status=$?
                [[ "$status" == 2 ]] || return 1
            done
            parse_args --extra --raw-only --extrinsic "$extrinsic" --json-file "$fixture/hook.json" --low 0,2 --high 0,2 runtime_authorship
            is_known_pallet "$TARGET_PALLET" || return 1
            [[ " ${PALLETS[*]} " != *" runtime_authorship "* ]] || return 1
            BENCHER_MODE="omni"
            printf 'fixture runtime\n' > "$fixture/runtime.wasm"
            resolve_runtime_wasm_path() { printf '%s\n' "$fixture/runtime.wasm"; }
            normalize_weight_file() { return 99; }
            run_command_step() {
                shift 2
                printf '%s\n' "$@" > "$fixture/authorship-args"
                local json=""
                while [[ $# -gt 0 ]]; do
                    [[ "$1" != --output ]] || return 98
                    if [[ "$1" == --json-file ]]; then json="$2"; shift; fi
                    shift
                done
                [[ -n "$json" ]] || return 97
                printf '[{"fixture":true}]\n' > "$json"
            }
            run_pallet_benchmark runtime_authorship > "$fixture/authorship-route-log" 2>&1 || return 1
            grep -Fxq -- runtime_authorship "$fixture/authorship-args" || return 1
            grep -Fxq -- "$extrinsic" "$fixture/authorship-args" || return 1
            grep -Fxq -- --extra "$fixture/authorship-args" || return 1
            grep -Fxq -- --no-median-slopes "$fixture/authorship-args" || return 1
            grep -Fxq -- --low "$fixture/authorship-args" || return 1
            grep -Fxq -- --high "$fixture/authorship-args" || return 1
            [[ "$(grep -Fxc -- '0,2' "$fixture/authorship-args")" == 2 ]] || return 1
            [[ -s "$fixture/hook.json" ]] || return 1
        )
    done

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

    # Explicit archived inputs resolve only readable nonempty files, never directories.
    (
        RUNTIME_WASM_OVERRIDE="$fixture/missing.wasm"
        ! resolve_runtime_wasm_path > "$fixture/resolve-log" 2>&1 || return 1
        : > "$fixture/empty.wasm"
        RUNTIME_WASM_OVERRIDE="$fixture/empty.wasm"
        ! resolve_runtime_wasm_path > "$fixture/resolve-log" 2>&1 || return 1
        RUNTIME_WASM_OVERRIDE="$fixture"
        ! resolve_runtime_wasm_path > "$fixture/resolve-log" 2>&1 || return 1
        printf 'archived runtime\n' > "$fixture/runtime.wasm"
        ln -s "$fixture/runtime.wasm" "$fixture/runtime-link.wasm"
        RUNTIME_WASM_OVERRIDE="$fixture/runtime-link.wasm"
        [[ "$(resolve_runtime_wasm_path)" == "$fixture/runtime.wasm" ]] || return 1
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
            if [[ "$raw_behavior" == changed ]]; then printf 'mutated runtime\n' > "$fixture/runtime.wasm"; fi
            if [[ "$raw_behavior" == success || "$raw_behavior" == changed ]]; then printf '[{"samples":1}]\n' > "$json"; fi
        }
        parse_args --raw-only --json-file "$fixture/samples.json" \
            --extrinsic close_detach_user --low 2 --high 2 pallet_deos_actors
        local raw_behavior status
        for raw_behavior in failure empty changed success; do
            printf 'archived runtime\n' > "$fixture/runtime.wasm"
            status=0
            run_pallet_benchmark pallet_deos_actors > "$fixture/raw-log" 2>&1 || status=$?
            if [[ "$raw_behavior" == success ]]; then
                [[ "$status" == 0 && -s "$fixture/samples.json" ]] || return 1
                [[ "$(grep -A1 -Fx -- '--genesis-builder' "$fixture/raw-args")" == $'--genesis-builder\nruntime' ]] || return 1
                [[ "$(grep -A1 -Fx -- '--genesis-builder-preset' "$fixture/raw-args")" == $'--genesis-builder-preset\ndevelopment' ]] || return 1
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
            grep -Fxq -- 'pallet_deos_actors::scheduler_user_pipeline_capacity' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_service_user_inspection_refusal' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_service_user_shared_refusal' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_service_user_opening_no_action' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_entry_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_deadline_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_dependency_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_resource_pass_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_on_initialize_cutoff' "$fixture/raw-args"
            grep -Fxq -- 'pallet_deos_actors::service_head_allowance' "$fixture/raw-args"
                grep -Fxq -- 'pallet_deos_actors::behavior_candidate_publish' "$fixture/raw-args"
            else
                [[ "$status" != 0 && ! -e "$fixture/samples.json" ]] || return 1
                ! compgen -G "$fixture/.pallet_deos_actors.samples.*" > /dev/null || return 1
                if [[ "$raw_behavior" == changed ]]; then
                    grep -Fq 'changed during benchmark execution' "$fixture/raw-log" || return 1
                fi
            fi
        done
        parse_args --skip-build --extra --raw-only --json-file "$fixture/prepass.json" \
            --extrinsic mandatory_prepass_control --low 2 --high 2 pallet_deos_actors
        run_pallet_benchmark pallet_deos_actors > "$fixture/prepass-route-log" 2>&1 || return 1
        grep -Fxq -- 'mandatory_prepass_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::scheduler_on_initialize_cutoff' "$fixture/raw-args"
        ! grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_control' "$fixture/raw-args"
        parse_args --skip-build --extra --raw-only --json-file "$fixture/entry.json" \
            --extrinsic mandatory_prepass_entry_control --low 0 --high 0 pallet_deos_actors
        run_pallet_benchmark pallet_deos_actors > "$fixture/entry-route-log" 2>&1 || return 1
        grep -Fxq -- 'mandatory_prepass_entry_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_control' "$fixture/raw-args"
        ! grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_entry_control' "$fixture/raw-args"
        parse_args --skip-build --extra --raw-only --json-file "$fixture/deadline.json" \
            --extrinsic mandatory_prepass_deadline_control --low 1 --high 1 pallet_deos_actors
        run_pallet_benchmark pallet_deos_actors > "$fixture/deadline-route-log" 2>&1 || return 1
        grep -Fxq -- 'mandatory_prepass_deadline_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_entry_control' "$fixture/raw-args"
        ! grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_deadline_control' "$fixture/raw-args"
        parse_args --skip-build --extra --raw-only --json-file "$fixture/dependency.json" \
            --extrinsic mandatory_prepass_dependency_control --low 2 --high 2 pallet_deos_actors
        run_pallet_benchmark pallet_deos_actors > "$fixture/dependency-route-log" 2>&1 || return 1
        grep -Fxq -- 'mandatory_prepass_dependency_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_deadline_control' "$fixture/raw-args"
        ! grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_dependency_control' "$fixture/raw-args"
        parse_args --skip-build --extra --raw-only --json-file "$fixture/pass.json" \
            --extrinsic scheduler_resource_pass_control --low 3 --high 3 pallet_deos_actors
        run_pallet_benchmark pallet_deos_actors > "$fixture/pass-route-log" 2>&1 || return 1
        grep -Fxq -- 'scheduler_resource_pass_control' "$fixture/raw-args"
        grep -Fxq -- 'pallet_deos_actors::mandatory_prepass_dependency_control' "$fixture/raw-args"
        ! grep -Fxq -- 'pallet_deos_actors::scheduler_resource_pass_control' "$fixture/raw-args"
        parse_args --skip-build --extra --raw-only --json-file "$fixture/governance.json" \
            --extrinsic service_finalized_proposal_outcomes pallet_governance
        run_pallet_benchmark pallet_governance > "$fixture/governance-route-log" 2>&1 || return 1
        grep -Fxq -- 'pallet_governance::service_finalized_proposal_outcomes_diverse' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup_diverse' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_full_index_value_classes' "$fixture/raw-args" || return 1
        parse_args --skip-build --extra --raw-only --json-file "$fixture/governance.json" \
            --extrinsic service_finalized_proposal_outcomes_diverse pallet_governance
        run_pallet_benchmark pallet_governance > "$fixture/governance-route-log" 2>&1 || return 1
        ! grep -Fxq -- 'pallet_governance::service_finalized_proposal_outcomes_diverse' "$fixture/raw-args" || return 1
        parse_args --skip-build --extra --raw-only --json-file "$fixture/governance.json" \
            --extrinsic on_initialize_finalized_cleanup pallet_governance
        run_pallet_benchmark pallet_governance > "$fixture/governance-route-log" 2>&1 || return 1
        ! grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup_diverse' "$fixture/raw-args" || return 1
        parse_args --skip-build --extra --raw-only --json-file "$fixture/governance.json" \
            --extrinsic on_initialize_finalized_cleanup_diverse pallet_governance
        run_pallet_benchmark pallet_governance > "$fixture/governance-route-log" 2>&1 || return 1
        ! grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup_diverse' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_full_index_value_classes' "$fixture/raw-args" || return 1
        parse_args --skip-build --extra --raw-only --json-file "$fixture/governance.json" \
            --extrinsic on_initialize_full_index_value_classes pallet_governance
        run_pallet_benchmark pallet_governance > "$fixture/governance-route-log" 2>&1 || return 1
        ! grep -Fxq -- 'pallet_governance::on_initialize_full_index_value_classes' "$fixture/raw-args" || return 1
        grep -Fxq -- 'pallet_governance::on_initialize_finalized_cleanup_diverse' "$fixture/raw-args" || return 1
    )

    # Output path, symlink and hardlink aliases cannot overwrite either runtime input.
    (
        BENCHER_MODE="omni"
        printf 'archived runtime\n' > "$fixture/runtime.wasm"
        ln "$fixture/runtime.wasm" "$fixture/runtime-hardlink.wasm"
        for value in "$fixture/runtime.wasm" "$fixture/runtime-link.wasm" "$fixture/runtime-hardlink.wasm"; do
            local status=0
            parse_args --runtime "$fixture/runtime.wasm" --skip-build --raw-only \
                --json-file "$value" --extrinsic close_detach_user pallet_deos_actors
            run_pallet_benchmark pallet_deos_actors > "$fixture/runtime-alias-log" 2>&1 || status=$?
            [[ "$status" != 0 ]] || return 1
            grep -Fq 'cannot replace or alias' "$fixture/runtime-alias-log" || return 1
            [[ "$(<"$fixture/runtime.wasm")" == 'archived runtime' ]] || return 1
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
    assert_route 0 $'build\nbenchmark' pallet_deos_router
    assert_route 0 benchmark --skip-build pallet_deos_router
    assert_route 2 '' pallet_deos_actors
    assert_route 2 '' --skip-build pallet_deos_actors
    assert_route 2 '' --all
    local unqualified selector
    for unqualified in scheduler_on_initialize_cutoff scheduler_on_idle_base block_resource_finalize; do
        assert_route 2 '' --extrinsic "$unqualified" --output "$fixture/focused.rs" pallet_deos_actors || return 1
        assert_route 2 '' --extra --raw-only --json-file "$fixture/samples.json" --extrinsic "$unqualified" pallet_deos_actors || return 1
        assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" \
            --extrinsic "$unqualified" --output "$fixture/merged.rs" pallet_deos_actors || return 1
        for selector in "${unqualified%?}[${unqualified: -1}]" "$unqualified.*" "$unqualified|create_actor"; do
            assert_route 2 '' --extrinsic "$selector" --output "$fixture/focused.rs" pallet_deos_actors || return 1
            assert_route 2 '' --extra --raw-only --json-file "$fixture/samples.json" \
                --extrinsic "$selector" pallet_deos_actors || return 1
            assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" \
                --extrinsic "$selector" --output "$fixture/merged.rs" pallet_deos_actors || return 1
        done
    done
    assert_route 2 '' --extrinsic all --output "$fixture/focused.rs" pallet_deos_actors || return 1
    assert_route 2 '' --extra --raw-only --json-file "$fixture/samples.json" --extrinsic all pallet_deos_actors || return 1
    assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" \
        --extrinsic all --output "$fixture/merged.rs" pallet_deos_actors || return 1
    assert_route 2 '' --extrinsic 'create_.*' --output "$fixture/focused.rs" pallet_deos_actors || return 1
    assert_route 2 '' --extrinsic 'create_actor,block_resource_finalize' --output "$fixture/focused.rs" pallet_deos_actors || return 1
    assert_route 0 check --check
    assert_route 0 benchmark --runtime "$fixture/runtime.wasm" --skip-build --raw-only \
        --json-file "$fixture/samples.json" --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --runtime
    assert_route 2 '' --runtime "$fixture/runtime.wasm" --raw-only --json-file "$fixture/samples.json" \
        --extrinsic close_detach_user pallet_deos_actors
    assert_route 2 '' --runtime "$fixture/runtime.wasm" --skip-build --extrinsic close_detach_user \
        --output "$fixture/focused.rs" pallet_deos_actors
    assert_route 2 '' --runtime "$fixture/runtime.wasm" --skip-build --raw-only \
        --json-file "$fixture/samples.json" pallet_deos_actors
    for value in --all --check --list --self-test; do
        assert_route 2 '' --runtime "$fixture/runtime.wasm" --skip-build --raw-only \
            --json-file "$fixture/samples.json" --extrinsic close_detach_user "$value" pallet_deos_actors
    done
    assert_route 2 '' --runtime "$fixture/runtime.wasm" --skip-build --raw-only --json-file "$fixture/samples.json" \
        --extrinsic close_detach_user --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" \
        --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 0 merge --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic close_actor --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --merge-candidate "$fixture/slice.rs" --extrinsic close_actor --output "$fixture/merged.rs" pallet_deos_actors
    assert_route 2 '' --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic close_actor --output "$fixture/base.rs" pallet_deos_actors
    assert_route 2 '' --extrinsic close_actor pallet_deos_actors
    assert_route 0 benchmark --skip-build --extrinsic close_actor --output "$fixture/focused.rs" pallet_deos_actors
    for value in scheduler_inner_suspended_user_tail_retry scheduler_inner_suspended_user_tail_progress; do
        assert_route 0 benchmark --skip-build --extrinsic "$value" --output "$fixture/focused.rs" pallet_deos_actors
        assert_route 0 merge --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic "$value" --output "$fixture/merged.rs" pallet_deos_actors
    done
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
    assert_route 2 '' --extrinsic service_finalized_proposal_outcomes_diverse --output "$fixture/focused.rs" pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic service_finalized_proposal_outcomes_diverse pallet_governance
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic service_finalized_proposal_outcomes_diverse --output "$fixture/merged.rs" pallet_governance
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic service_finalized_proposal_outcomes_diverse --low 3 --high 3 pallet_governance
    assert_route 2 '' --extrinsic on_initialize_finalized_cleanup --output "$fixture/focused.rs" pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_finalized_cleanup pallet_governance
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic on_initialize_finalized_cleanup --output "$fixture/merged.rs" pallet_governance
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_finalized_cleanup --low 1,0 --high 1,1 pallet_governance
    assert_route 2 '' --extrinsic on_initialize_finalized_cleanup_diverse --output "$fixture/focused.rs" pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_finalized_cleanup_diverse pallet_governance
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic on_initialize_finalized_cleanup_diverse --output "$fixture/merged.rs" pallet_governance
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_finalized_cleanup_diverse --low 3,0 --high 3,1 pallet_governance
    assert_route 2 '' --extrinsic on_initialize_full_index_value_classes --output "$fixture/focused.rs" pallet_governance
    assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_full_index_value_classes pallet_governance
    assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic on_initialize_full_index_value_classes --output "$fixture/merged.rs" pallet_governance
    assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic on_initialize_full_index_value_classes --low 0 --high 1 pallet_governance
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
    local service_diagnostic
    for service_diagnostic in scheduler_service_zero_step_user_complete scheduler_service_zero_step_system_complete scheduler_service_zero_step_user_address_event scheduler_service_zero_step_system_address_event scheduler_service_zero_step_admission_refusal scheduler_user_pipeline_capacity scheduler_service_user_inspection_refusal scheduler_service_user_shared_refusal scheduler_service_user_opening_no_action mandatory_prepass_control mandatory_prepass_entry_control mandatory_prepass_deadline_control mandatory_prepass_dependency_control scheduler_resource_pass_control; do
        assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic "$service_diagnostic" pallet_deos_actors
        assert_route 2 '' --skip-build --extra --extrinsic "$service_diagnostic" --output "$fixture/weights.rs" pallet_deos_actors
        assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic "$service_diagnostic" --output "$fixture/merged.rs" pallet_deos_actors
        assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic "$service_diagnostic" --low 0 --high 1 --steps 2 pallet_deos_actors
    done
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
    for value in close_public_wide_header_prefix close_public_wide_header_prefix_late_hold close_public_wide_idle close_public_wide_idle_late_hold task_transfer_minimum service_head_allowance; do
        assert_route 2 '' --skip-build --raw-only --json-file "$fixture/samples.json" --extrinsic "$value" pallet_deos_actors
        assert_route 2 '' --skip-build --extra --extrinsic "$value" --output "$fixture/weights.rs" pallet_deos_actors
        assert_route 2 '' --extra --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic "$value" --output "$fixture/merged.rs" pallet_deos_actors
        assert_route 0 benchmark --skip-build --extra --raw-only --json-file "$fixture/samples.json" --extrinsic "$value" pallet_deos_actors
    done
    assert_route 0 benchmark --skip-build --extra --extrinsic scheduler_inner_opening_user_progress_min --output "$fixture/weights.rs" pallet_deos_actors
    assert_route 0 merge --merge-candidate "$fixture/slice.rs" --base "$fixture/base.rs" --extrinsic scheduler_inner_opening_user_progress_min --output "$fixture/merged.rs" pallet_deos_actors
    build_status=42
    assert_route 42 build pallet_deos_router
    build_status=0
    for value in '' 0 1 false; do
        export SKIP_WASM_BUILD="$value"
        assert_route 1 '' pallet_deos_router
        assert_route 0 benchmark --skip-build pallet_deos_router
        assert_route 0 check --check
    done
    printf 'pub trait WeightInfo {\n  fn a() -> Weight;\n  fn b() -> Weight { Weight::zero() }\n}\n' > "$fixture/registry-trait.rs"
    printf 'impl WeightInfo for X {\n  fn a() -> Weight { w }\n}\n' > "$fixture/registry-ok.rs"
    printf 'impl WeightInfo for X {\n  fn b() -> Weight { w }\n}\n' > "$fixture/registry-undocumented.rs"
    printf 'impl WeightInfo for X {\n  fn a() -> Weight { w }\n  fn c() -> Weight { w }\n}\n' > "$fixture/registry-orphan.rs"
    audit_weight_registry "$fixture/registry-trait.rs" "$fixture/registry-ok.rs" > /dev/null
    for value in undocumented orphan; do
        if audit_weight_registry "$fixture/registry-trait.rs" "$fixture/registry-$value.rs" > /dev/null 2>&1; then
            log_error "Weight registry audit accepted a $value fixture"
            return 1
        fi
    done
    log_success "Benchmark output-protection/raw-sample/build/reuse routing self-test passed"
)

# Registry contract: every trait method is generated or carries a documented default body, and
# every generated owner is a trait method. Counts are populations, not coverage or cost claims.
audit_weight_registry() {
    local trait_file="$1" generated_file="$2"
    require_commands node
    node --input-type=module - "$trait_file" "$generated_file" <<'NODE'
import fs from 'node:fs';

const [, , traitPath, generatedPath] = process.argv;
const source = fs.readFileSync(traitPath, 'utf8');
const start = source.search(/pub trait WeightInfo\b/);
if (start < 0) {
  console.error(`No WeightInfo trait in ${traitPath}`);
  process.exit(1);
}
let depth = 0;
let end = -1;
for (let index = source.indexOf('{', start); index < source.length; index += 1) {
  if (source[index] === '{') depth += 1;
  if (source[index] === '}' && (depth -= 1) === 0) {
    end = index;
    break;
  }
}
if (end < 0) {
  console.error(`Unterminated WeightInfo trait in ${traitPath}`);
  process.exit(1);
}
const trait = new Map();
for (const match of source.slice(start, end).matchAll(/\bfn (\w+)\([^)]*\)\s*->\s*Weight\s*([{;])/g)) {
  trait.set(match[1], match[2] === '{');
}
const generated = new Set([...fs.readFileSync(generatedPath, 'utf8').matchAll(/\bfn (\w+)\(/g)].map((match) => match[1]));
const undocumented = [...trait].filter(([name, hasDefault]) => !generated.has(name) && !hasDefault).map(([name]) => name);
const orphans = [...generated].filter((name) => !trait.has(name));
if (trait.size === 0 || generated.size === 0 || undocumented.length > 0 || orphans.length > 0) {
  console.error(`Weight registry drift: trait=${trait.size} generated=${generated.size}`);
  if (undocumented.length > 0) console.error(`  trait methods neither generated nor defaulted: ${undocumented.join(', ')}`);
  if (orphans.length > 0) console.error(`  generated owners absent from the trait: ${orphans.join(', ')}`);
  process.exit(1);
}
const defaults = [...trait].filter(([name, hasDefault]) => hasDefault && !generated.has(name)).map(([name]) => name);
console.log(`Weight registry: trait=${trait.size} generated=${generated.size} defaulted-not-generated=${defaults.length}`);
NODE
}

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
        "cd \"$TEMPLATE_DIR\" && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo check --locked --features '$BENCHMARK_FEATURES'"
    if [[ -n "$production_identity" \
        && "$(sha256sum "$PRODUCTION_RUNTIME_WASM" | cut -d ' ' -f 1)" != "$production_identity" ]]; then
        log_error "Benchmark compilation mutated the canonical production runtime Wasm"
        return 1
    fi
    audit_generated_weight_storage_names
    audit_weight_registry "$TEMPLATE_DIR/pallets/actors/src/weights.rs" "$WEIGHTS_DIR/pallet_deos_actors.rs"
    log_success "Benchmark compilation, generated storage-name and Weight registry audit passed"
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
    if [[ -n "$RUNTIME_WASM_OVERRIDE" ]]; then
        require_commands realpath
        if [[ ! -f "$RUNTIME_WASM_OVERRIDE" || ! -r "$RUNTIME_WASM_OVERRIDE" || ! -s "$RUNTIME_WASM_OVERRIDE" ]]; then
            log_error "Explicit benchmark runtime must be a readable nonempty file: $RUNTIME_WASM_OVERRIDE"
            return 1
        fi
        realpath -- "$RUNTIME_WASM_OVERRIDE"
        return
    fi
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
            "scheduler_on_initialize_cutoff"
            "mandatory_prepass_control"
            "mandatory_prepass_entry_control"
            "mandatory_prepass_deadline_control"
        "mandatory_prepass_dependency_control"
        "scheduler_resource_pass_control"
            "manual_trigger_future_deadline"
            "scheduler_inner_zero_step_user_complete"
            "scheduler_service_zero_step_user_complete"
            "scheduler_service_zero_step_system_complete"
            "scheduler_service_zero_step_user_address_event"
            "scheduler_service_zero_step_system_address_event"
            "scheduler_service_zero_step_admission_refusal"
            "scheduler_user_pipeline_capacity"
            "scheduler_service_user_inspection_refusal"
            "scheduler_service_user_shared_refusal"
            "scheduler_service_user_opening_no_action"
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
            "task_transfer_minimum"
            "service_head_allowance"
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

    if [[ "$pallet_name" == pallet_governance ]]; then
        local governance_diagnostic
        for governance_diagnostic in service_pending_enactments_full_topology service_finalized_proposal_outcomes_diverse on_initialize_finalized_cleanup on_initialize_finalized_cleanup_diverse on_initialize_full_index_value_classes; do
            if [[ "$EXTRINSIC_PATTERN" != "$governance_diagnostic" ]]; then
                exclude_args+=(--exclude-extrinsics "pallet_governance::${governance_diagnostic}")
            fi
        done
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
            "cd '$TEMPLATE_DIR' && CARGO_TARGET_DIR='$BENCHMARK_TARGET_DIR' cargo test --release --locked --features '$BENCHMARK_FEATURES' -p deos-runtime -- benchmark --nocapture" || true
        log_warning "Weight files NOT updated (frame-omni-bencher required for weight generation)"
        return 0
    fi

    local template_file="$TEMPLATE_DIR/.maintain/frame-weight-template.hbs"
    local runtime_wasm runtime_identity destination output_dir staged_output="" staged_json=""
    local step_label="Generate $pallet_name weights"
    runtime_wasm="$(resolve_runtime_wasm_path)" || return 1
    require_commands realpath sha256sum
    for destination in "$output_file" "$JSON_OUTPUT"; do
        [[ -n "$destination" ]] || continue
        if [[ "$(realpath -m -- "$destination")" == "$(realpath -- "$runtime_wasm")" \
            || "$destination" -ef "$runtime_wasm" ]]; then
            log_error "Benchmark output cannot replace or alias the selected runtime Wasm"
            return 1
        fi
    done
    runtime_identity="$(sha256sum -- "$runtime_wasm" | cut -d ' ' -f 1)"
    log_info "Benchmark Wasm: $runtime_wasm | SHA-256: $runtime_identity"
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
        --genesis-builder runtime
        --genesis-builder-preset development
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
    if [[ "$(sha256sum -- "$runtime_wasm" | cut -d ' ' -f 1)" != "$runtime_identity" ]]; then
        log_error "Selected runtime Wasm changed during benchmark execution; refusing publication"
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
    echo "  - runtime_authorship (diagnostic only: --extra --raw-only --extrinsic authorship_complete|authorship_with_initialize|authorship_initialize_root --json-file FILE)"
}

is_known_pallet() {
    local candidate="$1"
    local pallet

    if [[ "$candidate" == runtime_authorship && "$RAW_ONLY" == 1 ]]; then return 0; fi
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

    if [[ "$ACTION" == prepare ]]; then
        prepare_benchmark_runtime
        return
    fi

    if [[ "$ACTION" == ordinary-diagnostic ]]; then
        run_ordinary_diagnostic
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
