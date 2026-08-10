#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

COLLATOR_RPC_URL="${COLLATOR_RPC_URL:-http://127.0.0.1:9988}"
SECOND_COLLATOR_RPC_URL="${SECOND_COLLATOR_RPC_URL:-http://127.0.0.1:9999}"
RELAY_RPC_URL="${RELAY_RPC_URL:-http://127.0.0.1:9944}"
BLOCK_TARGET="${BLOCK_TARGET:-100}"
BLOCK_TIMEOUT_SEC="${BLOCK_TIMEOUT_SEC:-900}"
BLOCK_STALL_TIMEOUT_SEC="${BLOCK_STALL_TIMEOUT_SEC:-60}"

usage() {
    cat <<'EOF'
Usage: 06-network-smoke.sh [OPTIONS]

Observe finalized relay-chain and parachain progression through live RPC. This
smoke test does not submit extrinsics or satisfy composed E2E acceptance.

Options:
  -h, --help  Show this help message

Environment:
  COLLATOR_RPC_URL=http://127.0.0.1:9988
  SECOND_COLLATOR_RPC_URL=http://127.0.0.1:9999
  RELAY_RPC_URL=http://127.0.0.1:9944
  BLOCK_TARGET=100
  BLOCK_TIMEOUT_SEC=900
  BLOCK_STALL_TIMEOUT_SEC=60

Inputs:
  Reachable local collator JSON-RPC endpoint.

Outputs:
  Pass/fail evidence for bounded finalized relay/parachain progression only.

Side effects:
  Waits for the configured live block target; chain state remains unchanged.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands curl jq
    [[ "$BLOCK_TARGET" =~ ^[1-9][0-9]*$ ]] || { log_error "BLOCK_TARGET must be a positive integer"; exit 1; }
    [[ "$BLOCK_TIMEOUT_SEC" =~ ^[1-9][0-9]*$ ]] || { log_error "BLOCK_TIMEOUT_SEC must be a positive integer"; exit 1; }
    [[ "$BLOCK_STALL_TIMEOUT_SEC" =~ ^[1-9][0-9]*$ ]] || { log_error "BLOCK_STALL_TIMEOUT_SEC must be a positive integer"; exit 1; }
}

rpc_call() {
    local rpc_url="$1"
    local payload="$2"
    curl -fsS -H 'Content-Type: application/json' -d "$payload" "$rpc_url"
}

rpc_finalized_block_number() {
    local rpc_url="$1"
    local finalized_hash response
    finalized_hash="$(rpc_call "$rpc_url" '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' | jq -er '.result | select(type == "string" and test("^0x[0-9a-fA-F]{64}$"))')"
    response="$(rpc_call "$rpc_url" "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$finalized_hash\"]}")"
    jq -er '.result.number | select(type == "string" and test("^0x[0-9a-fA-F]+$"))' <<< "$response"
}

observe_block_production() {
    local rpc_url="$1"
    local collator_label="$2"
    phase_banner "Step 2: $collator_label finalized-chain view"
    local start_ts last_advance_ts last=0
    start_ts="$(date +%s)"
    last_advance_ts="$start_ts"
    while true; do
        local now block_hex block
        now="$(date +%s)"
        (( now - start_ts <= BLOCK_TIMEOUT_SEC )) || { log_error "Timed out before block $BLOCK_TARGET"; exit 1; }
        block_hex="$(rpc_finalized_block_number "$rpc_url")" || { log_error "Invalid finalized-chain response from $rpc_url"; exit 1; }
        block=$((16#${block_hex#0x}))
        if (( block >= BLOCK_TARGET )); then
            log_success "Observed finalized block $block"
            return
        fi
        if (( block > last )); then
            last="$block"
            last_advance_ts="$now"
        elif (( block > 0 && now - last_advance_ts > BLOCK_STALL_TIMEOUT_SEC )); then
            log_error "Finalized progress stalled at $block"
            exit 1
        fi
        sleep 2
    done
}

main() {
    parse_args "$@"
    phase_banner "DEOS local network smoke"
    check_prerequisites
    observe_block_production "$RELAY_RPC_URL" "Relay chain"
    observe_block_production "$COLLATOR_RPC_URL" "Primary collator"
    observe_block_production "$SECOND_COLLATOR_RPC_URL" "Secondary collator"
    log_success "Relay and both collator RPC views passed finalized-progress smoke; author participation and composed E2E acceptance remain separate"
}

main "$@"
