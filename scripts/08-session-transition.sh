#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

PRIMARY_WS_ENDPOINT="${PRIMARY_WS_ENDPOINT:-ws://127.0.0.1:9988}"
SECONDARY_WS_ENDPOINT="${SECONDARY_WS_ENDPOINT:-ws://127.0.0.1:9999}"
SESSION_TIMEOUT_SEC="${SESSION_TIMEOUT_SEC:-28800}"
SESSION_POLL_SEC="${SESSION_POLL_SEC:-6}"
SESSION_STALL_TIMEOUT_SEC="${SESSION_STALL_TIMEOUT_SEC:-120}"

usage() {
    cat <<'EOF'
Usage: 08-session-transition.sh [OPTIONS]

Observe one finalized DEOS session-index transition through both collator RPC
views while requiring continued finality and matching non-empty validators.

Options:
  -h, --help  Show this help message

Environment:
  PRIMARY_WS_ENDPOINT=ws://127.0.0.1:9988
  SECONDARY_WS_ENDPOINT=ws://127.0.0.1:9999
  SESSION_TIMEOUT_SEC=28800
  SESSION_POLL_SEC=6
  SESSION_STALL_TIMEOUT_SEC=120

Inputs:
  Running DEOS collator RPCs and installed web-client dependencies/descriptors.

Outputs:
  One JSON record binding initial/final session indices, finalized blocks, and
  validator count across both RPC views.

Side effects:
  Read-only observation; this command may wait for several hours.
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
    require_directory "$PROJECT_ROOT/web-client/node_modules" "web-client dependencies"
    require_directory "$PROJECT_ROOT/web-client/.papi/descriptors" "PAPI descriptors"
    require_commands node timeout
    local value
    for value in "$SESSION_TIMEOUT_SEC" "$SESSION_POLL_SEC" "$SESSION_STALL_TIMEOUT_SEC"; do
        [[ "$value" =~ ^[1-9][0-9]*$ ]] || { log_error "Session timeouts must be positive integers"; exit 1; }
    done
    (( SESSION_STALL_TIMEOUT_SEC < SESSION_TIMEOUT_SEC )) || { log_error "SESSION_STALL_TIMEOUT_SEC must be shorter than SESSION_TIMEOUT_SEC"; exit 1; }
}

observe_transition() {
    phase_banner "Step 2: Finalized session transition"
    local outer_timeout
    outer_timeout=$((SESSION_TIMEOUT_SEC + 60))
    (
        cd "$PROJECT_ROOT/web-client"
        DEOS_PRIMARY_WS_ENDPOINT="$PRIMARY_WS_ENDPOINT" \
        DEOS_SECONDARY_WS_ENDPOINT="$SECONDARY_WS_ENDPOINT" \
        DEOS_SESSION_TIMEOUT_MS="$((SESSION_TIMEOUT_SEC * 1000))" \
        DEOS_SESSION_POLL_MS="$((SESSION_POLL_SEC * 1000))" \
        DEOS_SESSION_STALL_MS="$((SESSION_STALL_TIMEOUT_SEC * 1000))" \
        timeout --signal=TERM --kill-after=10 "$outer_timeout" \
            node scripts/network-session-transition.mjs
    )
    log_success "Finalized session transition observed through both collator RPC views"
}

main() {
    parse_args "$@"
    phase_banner "DEOS session-transition assurance"
    check_prerequisites
    observe_transition
}

main "$@"
