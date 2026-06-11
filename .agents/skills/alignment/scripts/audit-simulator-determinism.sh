#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-simulator-determinism.sh [OPTIONS]

Checks simulator JavaScript sources for nondeterministic host-time or
random-source usage. Fuzz/stress cases should use explicit seeded PRNGs so
failures can be reproduced from one run to the next.

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
    require_commands rg
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Simulator determinism"
    local pattern='Math\.random|Date\.now|performance\.now'
    local matches
    matches="$(rg -n "$pattern" "$PROJECT_ROOT/simulator" --glob '*.js' || true)"
    if [[ -n "$matches" ]]; then
        log_error "Nondeterministic simulator source detected"
        echo "$matches"
        exit 1
    fi
    log_success "Simulator determinism audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
