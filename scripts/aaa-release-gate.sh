#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CARGO_PROFILE="${CARGO_PROFILE:-release}"
INCLUDE_OCCUPANCY_PROFILE="${INCLUDE_OCCUPANCY_PROFILE:-1}"
QUICK_MODE="${QUICK_MODE:-0}"

REQUIRED_HEAVY_PROFILES=(
    "scheduler_stress_fifo_over_capacity_fairness_matrix"
    "scheduler_stress_fifo_dense_vs_sparse_topology_matrix"
    "scheduler_stress_fifo_sparse_topology_long_run_liveness"
    "stress_10k_actors_queue_scheduler"
    "checkpoint_a_s6_dense_10k_wakeups_converge_without_drops"
)
OCCUPANCY_HEAVY_PROFILE="profile_scheduler_queue_wakeup_occupancy_10k"
DIAGNOSTIC_HEAVY_PROFILES=("profile_scheduler_wallclock_matrix")

usage() {
    cat <<'EOF'
Usage: aaa-release-gate.sh [OPTIONS]

Runs the AAA release gate across the package archive, external-consumer fixture, and deos-runtime.

Options:
  --skip-occupancy-profile   Skip the gating 10k occupancy profile
  --quick                    Run only fast checks (Clippy + light tests)

The wall-clock matrix is diagnostic/non-gating and is not executed by this gate.
  -h, --help                 Show this help message

Environment:
  CARGO_PROFILE=release|dev
  INCLUDE_OCCUPANCY_PROFILE=0|1
  QUICK_MODE=0|1
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-occupancy-profile)
                INCLUDE_OCCUPANCY_PROFILE=0
                ;;
            --quick)
                QUICK_MODE=1
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands cargo npm
    log_success "Release gate prerequisites checked"
}

required_heavy_profiles() {
    printf '%s\n' "${REQUIRED_HEAVY_PROFILES[@]}"
    if [[ "$INCLUDE_OCCUPANCY_PROFILE" == "1" ]]; then
        printf '%s\n' "$OCCUPANCY_HEAVY_PROFILE"
    fi
}

# Lists every test in the deos-runtime test harness and fails unless each
# required heavy profile resolves to exactly one test. The same inventory owns
# execution below, so a renamed/deleted/duplicated profile cannot turn green.
verify_heavy_profiles_resolve_exactly_once() {
    phase_banner "Step 1b: Exact heavy-profile resolution"
    local profile
    local required_profiles=()
    mapfile -t required_profiles < <(required_heavy_profiles)
    local listing
    listing="$(cd "$TEMPLATE_DIR" && cargo test --$CARGO_PROFILE -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "${required_profiles[@]}"; do
        local matches
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Heavy profile '${profile}' resolved to ${matches} test(s); expected exactly 1. Zero-match success is impossible by design."
            return 1
        fi
        log_info "  exact profile: ${profile} (1 test)"
    done
    log_success "All required heavy profiles resolve to exactly one test"
}

run_gate() {
    run_shell_step "AAA gate: semantic manifest freshness" "" "cd \"$TEMPLATE_DIR\" && cargo run -q -p pallet-deos-aaa --example semantic_manifest -- --check ../web-client/src/lib/automation/aaa-semantic-manifest.json"
    run_shell_step "AAA gate: fee-envelope vector freshness" "" "cd \"$TEMPLATE_DIR\" && cargo run -q -p pallet-deos-aaa --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/aaa-fee-envelope-vectors.json"
    run_shell_step "AAA gate: ABI manifest drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:aaa-abi"
    run_shell_step "AAA gate: normative surface drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:aaa-normative-drift"
    run_shell_step "AAA gate: funding terminology drift" "" "\"$PROJECT_ROOT/.agents/skills/alignment/scripts/audit-aaa-funding-terminology.sh\""
    run_shell_step "AAA gate: observation runtime evidence drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:observation-evidence"
    run_shell_step "AAA gate: cross-language semantic contract" "" "cd \"$PROJECT_ROOT/web-client\" && npm run test:automation"
    run_shell_step "AAA gate: reactive operations corpus contract" "" "\"$PROJECT_ROOT/scripts/reactive-operations-corpus.sh\""

    if [[ "$QUICK_MODE" == "1" ]]; then
        run_shell_step "AAA quick gate: Clippy" "" "cd \"$TEMPLATE_DIR\" && cargo clippy -p pallet-deos-aaa -p deos-runtime -p pallet-aaa-embedding-fixture --all-targets -- -D warnings"
        run_shell_step "AAA quick gate: basic tests" "" "cd \"$TEMPLATE_DIR\" && cargo test -q -p pallet-deos-aaa --lib && cargo test -q -p pallet-aaa-embedding-fixture --lib"
        run_shell_step "AAA quick gate: package archive surface" "" "cd \"$TEMPLATE_DIR\" && cargo package -p pallet-deos-aaa --allow-dirty --locked --list"
        return
    fi

    if [[ "$CARGO_PROFILE" == "release" ]]; then
        run_shell_step "AAA gate: executable reactive operations corpus" "" "\"$PROJECT_ROOT/scripts/reactive-operations-corpus.sh\" --execute --release"
    else
        run_shell_step "AAA gate: executable reactive operations corpus" "" "\"$PROJECT_ROOT/scripts/reactive-operations-corpus.sh\" --execute"
    fi

    run_shell_step \
        "AAA gate: pallet package archive" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo package -p pallet-deos-aaa --allow-dirty --locked"

    run_shell_step \
        "AAA gate: independent embedding default profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-aaa-embedding-fixture --locked --lib"

    run_shell_step \
        "AAA gate: independent embedding DEX profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-aaa-embedding-fixture --locked --lib --features dex-fixture"

    run_shell_step \
        "AAA gate: independent embedding try-runtime profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-aaa-embedding-fixture --locked --lib --features try-runtime"

    run_shell_step \
        "AAA gate: independent embedding no-std contract" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo check --$CARGO_PROFILE -p pallet-aaa-embedding-fixture --locked --no-default-features"

    if [[ "$QUICK_MODE" != "1" ]]; then
        verify_heavy_profiles_resolve_exactly_once
    fi

    local profile
    local required_profiles=()
    mapfile -t required_profiles < <(required_heavy_profiles)
    for profile in "${required_profiles[@]}"; do
        run_shell_step \
            "AAA gate: exact heavy profile ${profile}" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked ${profile} -- --ignored --nocapture"
    done
    if [[ "$INCLUDE_OCCUPANCY_PROFILE" != "1" ]]; then
        log_warning "Skipping occupancy profile"
    fi
    log_info "Non-gating diagnostics (not resolved or executed by this gate): ${DIAGNOSTIC_HEAVY_PROFILES[*]}"
}

main() {
    parse_args "$@"
    phase_banner "DEOS AAA release gate"
    check_prerequisites
    log_info "Profile: $CARGO_PROFILE | quick: $QUICK_MODE | occupancy: $INCLUDE_OCCUPANCY_PROFILE"
    run_gate
    phase_banner "Summary"
    log_success "AAA scheduler release gate completed successfully"
}

run_entrypoint() {
    if [[ "${1:-}" == "--internal" ]]; then
        shift
        main "$@"
        return
    fi
    local arg
    for arg in "$@"; do
        if [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
            main "$@"
            return
        fi
    done
    local script_path
    script_path="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
    run_command_step "DEOS AAA release gate" "" "$script_path" --internal "$@"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_entrypoint "$@"
fi
