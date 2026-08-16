#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WIKI_DIR="$PROJECT_ROOT/wiki"

usage() {
    cat <<'EOF'
Usage: audit-native-russian.sh [OPTIONS]

Report reviewed heuristic English borrowings, calques, and noun chains in Russian
Wiki display prose, localized manifest labels, and extracted WikiWidget strings.
This deterministic audit does not claim to prove native fluency; completion still
requires independent bilingual review.

Options:
  --wiki-dir <path>   Override the wiki directory (default: ./wiki)
  --wiki-dir=<path>   Override the wiki directory (default: ./wiki)
  -h, --help          Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --wiki-dir)
                [[ $# -ge 2 ]] || { log_error "Missing value for --wiki-dir"; exit 1; }
                WIKI_DIR="$2"; shift ;;
            --wiki-dir=*) WIKI_DIR="${1#--wiki-dir=}" ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

main() {
    parse_args "$@"
    phase_banner "DEOS native Russian locale audit"
    require_commands node
    [[ -d "$WIKI_DIR" ]] || { log_error "Wiki directory not found: $WIKI_DIR"; exit 1; }
    node "$SCRIPT_DIR/audit-native-russian.mjs" --wiki-dir "$WIKI_DIR"
    log_success "Native Russian locale audit passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
