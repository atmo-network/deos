#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WIKI_DIR="$PROJECT_ROOT/wiki"

usage() {
    cat <<'EOF'
Usage: validate-wiki-trust.sh [OPTIONS]

Validate strict OKF v0.2 structure and the trusted wiki-markdown contract used
by the browser-side renderer.

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
                WIKI_DIR="$2"
                shift
                ;;
            --wiki-dir=*) WIKI_DIR="${1#--wiki-dir=}" ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Trusted wiki validation prerequisites"
    hydrate_local_tool_paths
    require_commands node npm rg
    [[ -d "$WIKI_DIR" ]] || { log_error "Wiki directory not found: $WIKI_DIR"; exit 1; }
    if [[ ! -d "$SKILL_DIR/node_modules/yaml" ]]; then
        log_error "Wiki tooling dependencies are missing; run npm ci in $SKILL_DIR or use scripts/setup-environment.sh"
        exit 1
    fi
    log_info "Wiki directory: $WIKI_DIR"
}

run_okf_validation() {
    phase_banner "Step 2: Strict OKF v0.2 bundle"
    node "$SCRIPT_DIR/validate-okf-wiki.mjs" --wiki-dir "$WIKI_DIR" --migration-baseline-ref HEAD
    node "$SCRIPT_DIR/migrate-okf-wiki.mjs" --wiki-dir "$WIKI_DIR"
    npm test --prefix "$SKILL_DIR"
}

check_markdown_trust() {
    phase_banner "Step 3: Markdown trust contract"
    if rg -n '^\s*<([A-Za-z][A-Za-z0-9-]*)(\s|>|/)' "$WIKI_DIR" --glob '*.md'; then
        log_error "Raw HTML tag blocks are not allowed in trusted wiki markdown"
        exit 1
    fi
    if rg -n '<(script|iframe|object|embed|link|style)\b' "$WIKI_DIR" --glob '*.md'; then
        log_error "Executable or embedded HTML tags are not allowed in trusted wiki markdown"
        exit 1
    fi
    if rg -ni 'javascript:|vbscript:|data:text/html' "$WIKI_DIR" --glob '*.md'; then
        log_error "Dangerous URL schemes are not allowed in trusted wiki markdown"
        exit 1
    fi
    if rg -ni '\bon[a-z]+\s*=' "$WIKI_DIR" --glob '*.md'; then
        log_error "Inline DOM event handler attributes are not allowed in trusted wiki markdown"
        exit 1
    fi
}

main() {
    parse_args "$@"
    phase_banner "DEOS trusted wiki markdown validation"
    check_prerequisites
    run_okf_validation
    check_markdown_trust
    log_success "Strict OKF and trusted wiki markdown validation passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
