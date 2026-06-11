#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-numeric-parsing.sh [OPTIONS]

Checks JavaScript/TypeScript surfaces for prefix-style numeric parsing patterns
that can silently accept malformed literals. Numeric domain inputs should validate
complete literals before conversion and route direct trimmed-string conversion
through the shared parser boundary.

Scope:
  web-client/src
  web-client/scripts
  scripts

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
    phase_banner "Step 2: Numeric parsing"
    local prefix_pattern='Number\.parse(Int|Float)|\bparse(Int|Float)\(|Number\(process\.env|BigInt\(process\.env'
    local direct_trimmed_pattern='Number\(trimmed\)|BigInt\(trimmed\)'
    local decimal_regex_pattern
    decimal_regex_pattern="/\\^\\\\d\\+\\$|/\\^\\\\d\\+\\(\\?:\\\\.\\\\d\\+\\)\\?\\$"
    local matches
    matches="$({
        rg -n "$prefix_pattern" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/web-client/scripts" "$PROJECT_ROOT/scripts" || true
        rg -n "$direct_trimmed_pattern" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/web-client/scripts" "$PROJECT_ROOT/scripts" \
            | grep -v 'web-client/src/lib/format/numeric.ts' || true
        rg -n "$decimal_regex_pattern" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/web-client/scripts" \
            | grep -v 'web-client/src/lib/format/numeric.ts' || true
    } || true)"
    if [[ -n "$matches" ]]; then
        log_error "Prefix/coercive numeric parsing patterns detected"
        echo "$matches"
        exit 1
    fi
    log_success "Numeric parsing audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
