#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WS_ENDPOINT="${WS_ENDPOINT:-ws://127.0.0.1:9988}"
FOREIGN_ID="${FOREIGN_ID:-4026531841}"
COMPOSED_SWAP_AMOUNT="${COMPOSED_SWAP_AMOUNT:-250000000000000}"
COMPOSED_TIMEOUT_SEC="${COMPOSED_TIMEOUT_SEC:-180}"
COMPOSED_SEED_AMOUNT="${COMPOSED_SEED_AMOUNT:-500000000000000}"

usage() {
    cat <<'EOF'
Usage: 09-composed-economic-path.sh [OPTIONS]

Prepare the existing local foreign/Native pool through the canonical live-state
seeder, execute one finalized Native-to-foreign Router swap, and reconcile the
Router -> Oracle -> Burn Actor path against finalized events and storage.

Options:
  -h, --help  Show this help message

Environment:
  WS_ENDPOINT=ws://127.0.0.1:9988
  FOREIGN_ID=4026531841
  COMPOSED_SWAP_AMOUNT=250000000000000
  COMPOSED_TIMEOUT_SEC=180
  COMPOSED_SEED_AMOUNT=500000000000000

Inputs:
  Running DEOS collator RPC, fresh canonical local genesis state with an
  uninitialized exact Native/foreign Oracle feed and exactly one free native ED
  on the Burn Actor, Alice development signer, installed web-client
  dependencies, and current PAPI descriptors.

Outputs:
  One JSON record binding the finalized Router transaction, Oracle revision,
  Burn Actor cycle nonce, and native issuance delta.

Side effects:
  Mints local-development foreign assets when needed, creates/seeds local pools
  through seed-web-client-state.sh, submits one Alice Router transaction, and
  allows the genesis Burn Actor to execute its routed Native fee.
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
    activate_pinned_node
    require_local_script "seed-web-client-state.sh"
    require_directory "$PROJECT_ROOT/web-client/node_modules" "web-client dependencies"
    require_directory "$PROJECT_ROOT/web-client/.papi/descriptors" "PAPI descriptors"
    require_commands node timeout
    local value
    for value in "$FOREIGN_ID" "$COMPOSED_SWAP_AMOUNT" "$COMPOSED_TIMEOUT_SEC" "$COMPOSED_SEED_AMOUNT"; do
        [[ "$value" =~ ^[1-9][0-9]*$ ]] || { log_error "Composed-path numeric inputs must be positive integers"; exit 1; }
    done
    (( COMPOSED_TIMEOUT_SEC <= 86400 )) || { log_error "COMPOSED_TIMEOUT_SEC must not exceed 86400"; exit 1; }
    node -e 'if (BigInt(process.argv[1]) < BigInt(process.argv[2]) * 2n) process.exit(1)' \
        "$COMPOSED_SEED_AMOUNT" "$COMPOSED_SWAP_AMOUNT" || { log_error "COMPOSED_SEED_AMOUNT must be at least twice COMPOSED_SWAP_AMOUNT"; exit 1; }
}

prepare_state() {
    phase_banner "Step 2: Minimal economic state"
    WS_ENDPOINT="$WS_ENDPOINT" \
    FOREIGN_ID="$FOREIGN_ID" \
    MINT_AMOUNT="$COMPOSED_SEED_AMOUNT" \
    LIQUIDITY_NATIVE="$COMPOSED_SEED_AMOUNT" \
    LIQUIDITY_FOREIGN="$COMPOSED_SEED_AMOUNT" \
        "$SCRIPT_DIR/seed-web-client-state.sh"
}

execute_path() {
    phase_banner "Step 3: Finalized Router -> Oracle -> Burn Actor path"
    local outer_timeout
    outer_timeout=$((COMPOSED_TIMEOUT_SEC + 30))
    (
        cd "$PROJECT_ROOT/web-client"
        DEOS_WS_ENDPOINT="$WS_ENDPOINT" \
        DEOS_COMPOSED_FOREIGN_ID="$FOREIGN_ID" \
        DEOS_COMPOSED_SWAP_AMOUNT="$COMPOSED_SWAP_AMOUNT" \
        DEOS_COMPOSED_TIMEOUT_MS="$((COMPOSED_TIMEOUT_SEC * 1000))" \
        timeout --signal=TERM --kill-after=10 "$outer_timeout" \
            node scripts/network-composed-path.mjs
    )
    log_success "Finalized composed economic path passed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS composed economic-path assurance"
    check_prerequisites
    prepare_state
    execute_path
}

main "$@"
