#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-code-suppressions.sh [OPTIONS]

Checks active JavaScript/TypeScript/Svelte and helper-script surfaces for broad
lint/type suppressions. Suppressions should stay exceptional and explicit rather
than spreading through product or validation code.

Options:
  -h, --help  Show this help message
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
    phase_banner "Step 1: Prerequisites"
    require_commands rg grep
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Code suppressions"
    local ts_marker='@''ts'
    local eslint_marker='eslint''-disable'
    local any_cast='as[[:space:]]+any'
    local pattern="${ts_marker}-ignore|${ts_marker}-expect-error|${eslint_marker}|${any_cast}|${ts_marker}-nocheck"
    local matches
    matches="$({
        rg -n "$pattern" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/web-client/scripts" "$PROJECT_ROOT/scripts" || true
        rg -n "$pattern" "$PROJECT_ROOT/simulator" --glob '*.js' || true
    } || true)"
    if [[ -n "$matches" ]]; then
        log_error "Broad code suppression detected"
        echo "$matches"
        exit 1
    fi
    log_success "Code suppression audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
