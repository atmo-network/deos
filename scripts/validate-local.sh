#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

RUN_CI="${RUN_CI:-1}"
RUN_RELEASE="${RUN_RELEASE:-1}"
RUN_E2E="${RUN_E2E:-0}"
PREPARE_E2E="${PREPARE_E2E:-0}"
KEEP_NETWORK="${KEEP_NETWORK:-0}"
RPC_READY_TIMEOUT_SEC="${RPC_READY_TIMEOUT_SEC:-240}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/deos-zombienet.log}"

ZOMBIENET_PID=""

usage() {
    cat <<'EOF2'
Usage: validate-local.sh [OPTIONS]

DEOS local validation orchestrator

Options:
  --all            Run CI + release + E2E
  --ci-only        Run only CI validation
  --release-only   Run only release validation
  --e2e-only       Run only E2E validation
  --with-e2e       Add E2E validation to current plan
  --prepare-e2e    Run 01..04 setup before E2E (implies --with-e2e)
  --no-ci          Disable CI validation
  --no-release     Disable release validation
  --no-e2e         Disable E2E validation
  --keep-network   Keep zombienet running after completion
  -h, --help       Show this help message

Environment flags:
  RUN_CI=0|1
  RUN_RELEASE=0|1
  RUN_E2E=0|1
  PREPARE_E2E=0|1
  KEEP_NETWORK=0|1
  RPC_READY_TIMEOUT_SEC=<seconds>
  ZOMBIENET_LOG=<path>
EOF2
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all)
                RUN_CI=1
                RUN_RELEASE=1
                RUN_E2E=1
                ;;
            --ci-only)
                RUN_CI=1
                RUN_RELEASE=0
                RUN_E2E=0
                ;;
            --release-only)
                RUN_CI=0
                RUN_RELEASE=1
                RUN_E2E=0
                ;;
            --e2e-only)
                RUN_CI=0
                RUN_RELEASE=0
                RUN_E2E=1
                ;;
            --with-e2e)
                RUN_E2E=1
                ;;
            --prepare-e2e)
                RUN_E2E=1
                PREPARE_E2E=1
                ;;
            --no-ci)
                RUN_CI=0
                ;;
            --no-release)
                RUN_RELEASE=0
                ;;
            --no-e2e)
                RUN_E2E=0
                ;;
            --keep-network)
                KEEP_NETWORK=1
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

check_plan() {
    phase_banner "Step 1: Validation plan"
    if (( RUN_CI == 0 && RUN_RELEASE == 0 && RUN_E2E == 0 )); then
        log_error "Nothing to run. Enable at least one validation stage"
        exit 1
    fi
    log_info "Plan: ci=$RUN_CI release=$RUN_RELEASE e2e=$RUN_E2E prepare_e2e=$PREPARE_E2E"
}

run_requested_stages() {
    phase_banner "Step 2: Local validation stages"
    if (( RUN_CI == 1 )); then
        run_script_step "CI local workflow" "ci-local.sh"
    fi
    if (( RUN_RELEASE == 1 )); then
        run_script_step "Release local workflow" "release-local.sh"
    fi
    if (( RUN_E2E == 1 )); then
        run_e2e
    fi
}

run_e2e() {
    phase_banner "Step 3: E2E workflow"
    if (( PREPARE_E2E == 1 )); then
        run_script_step "Download binaries" "01-download-binaries.sh"
        run_script_step "Install tools" "02-install-tools.sh"
        run_script_step "Build runtime" "03-build-runtime.sh"
        run_script_step "Generate chain spec" "04-generate-chain-spec.sh"
    fi

    start_background_script "zombienet" "05-spawn-zombienet.sh" "$ZOMBIENET_LOG" ZOMBIENET_PID
    wait_for_chain_rpc "http://127.0.0.1:9988" "$RPC_READY_TIMEOUT_SEC" "Collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    run_script_step "Zombienet E2E" "06-zombienet-e2e.sh"
}

on_exit() {
    local exit_code=$?
    stop_background_process "$ZOMBIENET_PID" "$KEEP_NETWORK" "$ZOMBIENET_LOG" "zombienet"
    if (( exit_code != 0 )); then
        log_error "Local validation failed"
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Local validation completed successfully"
}

main() {
    parse_args "$@"
    phase_banner "DEOS local validation workflow"
    trap on_exit EXIT
    check_plan
    run_requested_stages
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
