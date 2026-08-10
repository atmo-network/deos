#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: 07-network-e2e.sh [OPTIONS]

Submit a signed native transfer to a running DEOS node and verify finalized
success, the live Balances.Transfer event, and finalized recipient storage.

Options:
  -h, --help  Show this help message

Environment:
  DEOS_WS_ENDPOINT=ws://127.0.0.1:9988
  DEOS_E2E_TRANSFER_AMOUNT=1000000000
  DEOS_E2E_TIMEOUT_MS=120000

Inputs:
  Running DEOS websocket RPC, funded Alice dev account, generated descriptors,
  and a clean web-client dependency install from setup-environment.sh client.

Outputs:
  JSON containing transaction hash, finalized block, and recipient delta.

Side effects:
  Signs and submits one Alice-to-Bob native transfer to the configured chain.
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
    require_commands node
    [[ -d "$PROJECT_ROOT/web-client/node_modules" ]] || { log_error "Run ./scripts/setup-environment.sh client first"; exit 1; }
    [[ -f "$PROJECT_ROOT/web-client/.papi/metadata/deos.scale" ]] || { log_error "Generated runtime metadata not found"; exit 1; }
}

main() {
    parse_args "$@"
    check_prerequisites
    phase_banner "Step 2: Finalized signed transfer"
    (cd "$PROJECT_ROOT/web-client" && node scripts/network-e2e.mjs)
    log_success "Live finalized transfer E2E passed"
}

main "$@"
