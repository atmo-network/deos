#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CARGO_PROFILE="${CARGO_PROFILE:-release}"
INCLUDE_OCCUPANCY_PROFILE="${INCLUDE_OCCUPANCY_PROFILE:-1}"
QUICK_MODE="${QUICK_MODE:-0}"

usage() {
    cat <<'EOF'
Usage: aaa-release-gate.sh [OPTIONS]

Runs the documented AAA scheduler release gate against deos-runtime.

Options:
  --skip-occupancy-profile   Skip the 10k occupancy diagnostics profile
  --quick                    Run only fast checks (clippy + light tests)
  -h, --help                 Show this help message

Environment:
  CARGO_PROFILE=release|dev
  INCLUDE_OCCUPANCY_PROFILE=0|1
  QUICK_MODE=0|1
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

main() {
    parse_args "$@"
    phase_banner "DEOS AAA release gate"
    check_prerequisites

    log_info "Starting AAA scheduler release gate"
    log_info "Cargo profile: $CARGO_PROFILE"
    log_info "Quick mode: $QUICK_MODE"

    if [[ "$QUICK_MODE" == "1" ]]; then
        log_info "Quick mode enabled - running only fast checks"
        run_shell_step "AAA quick gate: clippy" "" "cd \"$TEMPLATE_DIR\" && cargo clippy -p pallet-aaa -p deos-runtime --all-targets -- -D warnings"
        run_shell_step "AAA quick gate: basic tests" "" "cd \"$TEMPLATE_DIR\" && cargo test -q -p pallet-aaa --lib"
        log_success "AAA quick gate completed successfully"
        exit 0
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

    log_success "AAA scheduler release gate completed successfully"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
