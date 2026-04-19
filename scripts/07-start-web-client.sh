#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WEB_CLIENT_DIR="${WEB_CLIENT_DIR:-$PROJECT_ROOT/web-client}"
WEB_CLIENT_HOST="${WEB_CLIENT_HOST:-0.0.0.0}"
WEB_CLIENT_PORT="${WEB_CLIENT_PORT:-5173}"

usage() {
    cat <<'EOF'
Usage: 07-start-web-client.sh [OPTIONS]

Starts the local web-client Vite dev server in the foreground.

Options:
  -h, --help        Show this help message

Environment:
  WEB_CLIENT_DIR=<project>/web-client
  WEB_CLIENT_HOST=0.0.0.0
  WEB_CLIENT_PORT=5173
EOF
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

check_prerequisites() {
    phase_banner "Step 1: Web client prerequisites"
    require_directory "$WEB_CLIENT_DIR" "Web client workspace"
    require_commands npm
    if [[ ! -f "$WEB_CLIENT_DIR/package.json" ]]; then
        log_error "web-client/package.json not found: $WEB_CLIENT_DIR/package.json"
        exit 1
    fi
    log_success "Web client prerequisites checked"
}

start_dev_server() {
    phase_banner "Step 2: Start web-client dev server"
    log_info "Starting web client dev server"
    echo "  Workspace: $WEB_CLIENT_DIR"
    echo "  URL: http://127.0.0.1:$WEB_CLIENT_PORT"
    echo "  Host: $WEB_CLIENT_HOST"
    echo ""

    cd "$WEB_CLIENT_DIR"
    exec npm run dev -- --host "$WEB_CLIENT_HOST" --port "$WEB_CLIENT_PORT" --strictPort
}

main() {
    parse_args "$@"
    phase_banner "DEOS web-client dev server"
    check_prerequisites
    start_dev_server
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
