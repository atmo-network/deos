#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

COLLATOR_RPC_URL="${COLLATOR_RPC_URL:-http://127.0.0.1:9988}"
BLOCK_TARGET="${BLOCK_TARGET:-100}"
BLOCK_TIMEOUT_SEC="${BLOCK_TIMEOUT_SEC:-900}"
BLOCK_STALL_TIMEOUT_SEC="${BLOCK_STALL_TIMEOUT_SEC:-60}"

usage() {
    cat <<'EOF2'
Usage: 06-zombienet-e2e.sh [OPTIONS]

Runs the runtime-facing E2E scenario set against a live local network.

Options:
  -h, --help        Show this help message

Environment:
  COLLATOR_RPC_URL=http://127.0.0.1:9988
  BLOCK_TARGET=100
  BLOCK_TIMEOUT_SEC=900
  BLOCK_STALL_TIMEOUT_SEC=60

Inputs:
  Reachable local collator RPC and the locked template test workspace.

Outputs:
  Pass/fail evidence for block stability and runtime-facing E2E scenarios.

Side effects:
  Waits for live blocks and may submit test-only state transitions to the local chain.
EOF2
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
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

rpc_block_number() {
    curl -sS -H 'Content-Type: application/json' \
        -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
        "$COLLATOR_RPC_URL" | jq -r '.result.number // "0x0"'
}

hex_to_dec() {
    local hex="${1#0x}"
    printf "%d" "0x${hex}"
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands curl jq cargo
    log_success "Zombienet E2E prerequisites checked"
}

scenario_block_stability() {
    phase_banner "Step 2: Block production stability"
    log_info "Scenario 1: block production stability (${BLOCK_TARGET} blocks, no stalls)"

    local start_ts
    local last=0
    local last_advance_ts

    start_ts=$(date +%s)
    last_advance_ts=$start_ts
    while true; do
        local now
        local block_hex
        local block

        now=$(date +%s)
        if (( now - start_ts > BLOCK_TIMEOUT_SEC )); then
            log_error "Timeout while waiting for block ${BLOCK_TARGET} on ${COLLATOR_RPC_URL}"
            exit 1
        fi

        block_hex=$(rpc_block_number)
        block=$(hex_to_dec "$block_hex")
        if (( block >= BLOCK_TARGET )); then
            log_success "Reached block ${block}"
            break
        fi

        if (( block > last )); then
            last=$block
            last_advance_ts=$now
        elif (( block > 0 && now - last_advance_ts > BLOCK_STALL_TIMEOUT_SEC )); then
            log_error "Block production appears stalled at ${block}"
            exit 1
        fi

        sleep 2
    done
}

scenario_xcm_foreign_registration() {
    phase_banner "Step 3: Runtime scenario suite"
    log_info "Scenario 2: XCM reserve transfer -> foreign asset registration"
    log_info "Using runtime integration proxy (asset-registry flow)"
    run_shell_step \
        "XCM foreign-registration runtime proxy" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo test -p deos-runtime tests::asset_registry_integration_tests::register_foreign_asset_creates_mapping_and_metadata"
    log_success "Scenario 2 passed"
}

scenario_full_economic_cycle() {
    log_info "Scenario 3: full economic cycle (TMC -> Router -> Burn -> TOL)"
    run_shell_step \
        "TMCTOL full economic cycle" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo test -p deos-runtime tests::tmctol_integration_tests::bldr_full_e2e_router_tmc_splitter_zm_bucket"
    log_success "Scenario 3 passed"
}

scenario_aaa_lifecycle() {
    log_info "Scenario 4: AAA lifecycle (create -> execute cycles -> close)"
    run_shell_step \
        "AAA lifecycle runtime proxy" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo test -p deos-runtime tests::aaa_integration_tests::user_dca_e2e_lifecycle_with_natural_close"
    log_success "Scenario 4 passed"
}

print_summary() {
    phase_banner "Summary"
    log_success "All E2E scenarios passed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS Zombienet E2E"
    check_prerequisites
    scenario_block_stability
    scenario_xcm_foreign_registration
    scenario_full_economic_cycle
    scenario_aaa_lifecycle
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
