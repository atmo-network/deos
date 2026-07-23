#!/usr/bin/env bash

set -euo pipefail
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_ROOT="$(cd "$SKILL_DIR/../../.." && pwd)"
DEOS_PROJECT_ROOT="$PROJECT_ROOT"
source "$PROJECT_ROOT/scripts/_common.sh"

CARGO_PROFILE="${CARGO_PROFILE:-release}"
INCLUDE_OCCUPANCY_PROFILE="${INCLUDE_OCCUPANCY_PROFILE:-1}"
QUICK_MODE="${QUICK_MODE:-0}"

usage() {
    cat <<'EOF'
Usage: release-gate.sh [OPTIONS]

Runs the AAA scheduler release gate against deos-runtime.

Options:
  --skip-occupancy-profile   Skip the 10k occupancy diagnostics profile
  --quick                    Run only fast checks (Clippy + light tests)
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
    require_commands cargo
    log_success "Release gate prerequisites checked"
}

run_gate() {
    if [[ "$QUICK_MODE" == "1" ]]; then
        run_shell_step "AAA quick gate: Clippy" "" "cd \"$TEMPLATE_DIR\" && cargo clippy -p pallet-aaa -p deos-runtime --all-targets -- -D warnings"
        run_shell_step "AAA quick gate: basic tests" "" "cd \"$TEMPLATE_DIR\" && cargo test -q -p pallet-aaa --lib"
        return
    fi

    run_shell_step \
        "AAA gate: over-capacity fairness matrix" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked scheduler_stress_lane_over_capacity_fairness_matrix -- --ignored --nocapture"

    run_shell_step \
        "AAA gate: dense vs sparse topology matrix" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked scheduler_stress_lane_dense_vs_sparse_topology_matrix -- --ignored --nocapture"

    run_shell_step \
        "AAA gate: sparse topology long-run liveness" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked scheduler_stress_lane_sparse_topology_long_run_liveness -- --ignored --nocapture"

    run_shell_step \
        "AAA gate: 10k queue scheduler stress" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked stress_10k_actors_queue_scheduler -- --ignored --nocapture"

    if [[ "$INCLUDE_OCCUPANCY_PROFILE" == "1" ]]; then
        run_shell_step \
            "AAA gate: 10k queue/wakeup occupancy profile" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked profile_scheduler_queue_wakeup_occupancy_10k -- --ignored --nocapture"
    else
        log_warning "Skipping occupancy profile"
    fi
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
