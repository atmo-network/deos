#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

PREPARE_NETWORK="${PREPARE_NETWORK:-0}"
START_NETWORK="${START_NETWORK:-1}"
KEEP_NETWORK="${KEEP_NETWORK:-0}"
RUN_ON_RUNTIME_UPGRADE="${RUN_ON_RUNTIME_UPGRADE:-1}"
RUN_EXECUTE_BLOCK="${RUN_EXECUTE_BLOCK:-1}"
DISABLE_SPEC_VERSION_CHECK="${DISABLE_SPEC_VERSION_CHECK:-1}"
WS_URI="${WS_URI:-ws://127.0.0.1:9988}"
HTTP_URI="${HTTP_URI:-http://127.0.0.1:9988}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/tmctol-try-runtime-zombienet.log}"
CHAIN_TYPE="${CHAIN_TYPE:-Development}"
BLOCK_TIME_MILLIS="${BLOCK_TIME_MILLIS:-6000}"
FINALIZED_HEAD_TIMEOUT_SEC="${FINALIZED_HEAD_TIMEOUT_SEC:-120}"
TRY_RUNTIME_CASE_TIMEOUT_SEC="${TRY_RUNTIME_CASE_TIMEOUT_SEC:-300}"
ON_RUNTIME_UPGRADE_TIMEOUT_SEC="${ON_RUNTIME_UPGRADE_TIMEOUT_SEC:-$TRY_RUNTIME_CASE_TIMEOUT_SEC}"
EXECUTE_BLOCK_TIMEOUT_SEC="${EXECUTE_BLOCK_TIMEOUT_SEC:-$TRY_RUNTIME_CASE_TIMEOUT_SEC}"
WASM_PATH_RELATIVE="target/release/wbuild/tmctol-runtime/tmctol_runtime.compact.compressed.wasm"
WASM_PATH="$TEMPLATE_DIR/$WASM_PATH_RELATIVE"

ZOMBIENET_PID=""

usage() {
    cat <<'EOF'
Usage: try-runtime-local.sh [OPTIONS]

Builds the runtime with `try-runtime`, optionally prepares/spawns the local Zombienet dev chain,
and runs the canonical local dry-runs against the parachain RPC.

Options:
  --prepare         Run scripts 01..04 before spawning the network
  --no-network      Do not start Zombienet; use the configured WS_URI/HTTP_URI as-is
  --keep-network    Leave Zombienet running after completion
  --no-upgrade      Skip `try-runtime ... on-runtime-upgrade live`
  --no-execute      Skip `try-runtime ... execute-block live`
  -h, --help        Show this help message

Environment:
  PREPARE_NETWORK=0|1
  START_NETWORK=0|1
  KEEP_NETWORK=0|1
  RUN_ON_RUNTIME_UPGRADE=0|1
  RUN_EXECUTE_BLOCK=0|1
  DISABLE_SPEC_VERSION_CHECK=0|1
  CHAIN_TYPE=Development|Local|Live
  BLOCK_TIME_MILLIS=6000
  FINALIZED_HEAD_TIMEOUT_SEC=120
  TRY_RUNTIME_CASE_TIMEOUT_SEC=300
  ON_RUNTIME_UPGRADE_TIMEOUT_SEC=300
  EXECUTE_BLOCK_TIMEOUT_SEC=300
  WS_URI=ws://127.0.0.1:9988
  HTTP_URI=http://127.0.0.1:9988
  ZOMBIENET_LOG=/tmp/tmctol-try-runtime-zombienet.log
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --prepare)
                PREPARE_NETWORK=1
                ;;
            --no-network)
                START_NETWORK=0
                ;;
            --keep-network)
                KEEP_NETWORK=1
                ;;
            --no-upgrade)
                RUN_ON_RUNTIME_UPGRADE=0
                ;;
            --no-execute)
                RUN_EXECUTE_BLOCK=0
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

prepare_prerequisites() {
    phase_banner "Step 1: Prepare local artifacts"
    export CHAIN_TYPE
    run_script_step "Download Polkadot binaries" "01-download-binaries.sh"
    run_script_step "Install cargo tools" "02-install-tools.sh"
    run_script_step "Build runtime" "03-build-runtime.sh"
    run_script_step "Generate chain spec" "04-generate-chain-spec.sh"
}

check_prerequisites() {
    phase_banner "Step 2: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands cargo curl timeout
    if [[ "$RUN_ON_RUNTIME_UPGRADE" == "1" || "$RUN_EXECUTE_BLOCK" == "1" ]]; then
        require_commands try-runtime
    fi
    if [[ "$RUN_EXECUTE_BLOCK" == "1" ]]; then
        require_commands jq
    fi
    if [[ "$START_NETWORK" == "1" ]]; then
        require_commands polkadot polkadot-omni-node zombienet chain-spec-builder
        if [[ ! -f "$TEMPLATE_DIR/chain_spec.json" ]]; then
            log_error "Missing chain spec: $TEMPLATE_DIR/chain_spec.json"
            echo "  Hint: run ./scripts/04-generate-chain-spec.sh or rerun with --prepare"
            exit 1
        fi
    fi
}

build_try_runtime_wasm() {
    phase_banner "Step 3: Build try-runtime WASM"
    log_info "Building tmctol-runtime with try-runtime feature"
    (
        cd "$TEMPLATE_DIR"
        cargo build --release -p tmctol-runtime --features try-runtime
    )

    if [[ ! -f "$WASM_PATH" ]]; then
        log_error "try-runtime WASM artifact not found: $WASM_PATH"
        exit 1
    fi

    log_success "try-runtime WASM ready: $WASM_PATH"
}

run_try_runtime_case() {
    local label="$1"
    local timeout_sec="$2"
    shift 2

    log_info "Running try-runtime: $label"
    if [[ "$timeout_sec" =~ ^[0-9]+$ ]] && (( timeout_sec > 0 )); then
        log_info "Case timeout: ${timeout_sec}s"
        (
            cd "$TEMPLATE_DIR"
            timeout --foreground --signal=TERM "${timeout_sec}s" "$@"
        )
    else
        (
            cd "$TEMPLATE_DIR"
            "$@"
        )
    fi
    log_success "try-runtime case passed: $label"
}

latest_finalized_head() {
    local finalized_head
    finalized_head=$(
        curl -sS \
            -H 'Content-Type: application/json' \
            -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' \
            "$HTTP_URI" | jq -r '.result // empty'
    )
    if [[ -z "$finalized_head" || "$finalized_head" == "null" ]]; then
        log_error "Failed to resolve latest finalized head from $HTTP_URI"
        exit 1
    fi
    echo "$finalized_head"
}

wait_for_non_genesis_finalized_head() {
    local deadline=$((SECONDS + FINALIZED_HEAD_TIMEOUT_SEC))
    local finalized_head
    local finalized_number
    while (( SECONDS < deadline )); do
        finalized_head="$(latest_finalized_head)"
        finalized_number=$(
            curl -sS \
                -H 'Content-Type: application/json' \
                --data "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$finalized_head\"]}" \
                "$HTTP_URI" | jq -r '.result.number // empty'
        )
        if [[ -n "$finalized_number" && "$finalized_number" != "0x0" ]]; then
            echo "$finalized_head"
            return 0
        fi
        sleep 2
    done
    log_error "Timed out waiting for a non-genesis finalized head at $HTTP_URI (timeout=${FINALIZED_HEAD_TIMEOUT_SEC}s)"
    exit 1
}

on_exit() {
    local exit_code=$?
    stop_background_process "$ZOMBIENET_PID" "$KEEP_NETWORK" "$ZOMBIENET_LOG" "zombienet"
    if (( exit_code != 0 )); then
        log_error "Local try-runtime workflow failed"
    fi
}

main() {
    local execute_block_at=""
    local -a spec_version_check_flag=()

    parse_args "$@"
    phase_banner "DEOS local try-runtime workflow"
    trap on_exit EXIT

    if [[ "$PREPARE_NETWORK" == "1" ]]; then
        prepare_prerequisites
    fi

    check_prerequisites
    build_try_runtime_wasm

    if [[ "$DISABLE_SPEC_VERSION_CHECK" == "1" ]]; then
        spec_version_check_flag=(--disable-spec-version-check)
    fi

    if [[ "$START_NETWORK" == "1" ]]; then
        phase_banner "Step 4: Start local network"
        start_background_script "Zombienet" "05-spawn-zombienet.sh" "$ZOMBIENET_LOG" ZOMBIENET_PID
        wait_for_chain_rpc "$HTTP_URI" 240 "Parachain RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    fi

    if [[ "$RUN_ON_RUNTIME_UPGRADE" == "1" ]]; then
        phase_banner "Step 5: Run try-runtime cases"
        run_try_runtime_case \
            "on-runtime-upgrade live" \
            "$ON_RUNTIME_UPGRADE_TIMEOUT_SEC" \
            try-runtime --runtime "$WASM_PATH_RELATIVE" on-runtime-upgrade --blocktime "$BLOCK_TIME_MILLIS" "${spec_version_check_flag[@]}" live --uri "$WS_URI"
    fi

    if [[ "$RUN_EXECUTE_BLOCK" == "1" ]]; then
        if [[ "$RUN_ON_RUNTIME_UPGRADE" != "1" ]]; then
            phase_banner "Step 5: Run try-runtime cases"
        fi
        execute_block_at="$(wait_for_non_genesis_finalized_head)"
        log_info "Resolved non-genesis finalized head for execute-block: $execute_block_at"
        run_try_runtime_case \
            "execute-block live" \
            "$EXECUTE_BLOCK_TIMEOUT_SEC" \
            try-runtime --runtime "$WASM_PATH_RELATIVE" execute-block live --at "$execute_block_at" --uri "$WS_URI"
    fi

    phase_banner "Summary"
    log_success "Local try-runtime workflow completed successfully"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
