#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WIKI_DIR="$PROJECT_ROOT/wiki"
MIN_BODY_LINES="${MIN_BODY_LINES:-18}"

usage() {
    cat <<'EOF'
Usage: audit-wiki-consolidation.sh [OPTIONS]

Audit canonical OKF wiki metadata, source provenance, locale mirrors, and
navigation/graph reachability. Short pages and graph leaves remain non-blocking
consolidation candidates.

Options:
  --wiki-dir <path>              Override the wiki directory (default: ./wiki)
  --wiki-dir=<path>              Override the wiki directory (default: ./wiki)
  --min-body-lines <n> Short-page threshold (default: 18)
  -h, --help           Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --wiki-dir)
                [[ $# -ge 2 ]] || { log_error "Missing value for --wiki-dir"; exit 1; }
                WIKI_DIR="$2"; shift ;;
            --wiki-dir=*) WIKI_DIR="${1#--wiki-dir=}" ;;
            --min-body-lines)
                [[ $# -ge 2 ]] || { log_error "Missing value for --min-body-lines"; exit 1; }
                MIN_BODY_LINES="$2"; shift ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

main() {
    parse_args "$@"
    phase_banner "DEOS wiki consolidation audit"
    hydrate_local_tool_paths
    require_commands node
    [[ -d "$WIKI_DIR" ]] || { log_error "Wiki directory not found: $WIKI_DIR"; exit 1; }
    node "$SCRIPT_DIR/audit-wiki-consolidation.mjs" "$WIKI_DIR" "$MIN_BODY_LINES"
    log_success "Wiki consolidation audit passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
