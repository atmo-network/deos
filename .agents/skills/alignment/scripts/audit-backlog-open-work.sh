#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-backlog-open-work.sh [OPTIONS]

Checks that BACKLOG.md remains an open-work surface and does not accumulate
completed checkbox history or command-reference inventory. Completed delivery
belongs in CHANGELOG.md; validation entrypoint maps belong in README/skills.

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
    if [[ ! -f "$PROJECT_ROOT/BACKLOG.md" ]]; then
        log_error "BACKLOG.md not found: $PROJECT_ROOT/BACKLOG.md"
        exit 1
    fi
    require_commands rg
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Backlog open-work shape"
    local matches
    matches="$(rg -n '^\s*[-*+] \[[xX]\]' "$PROJECT_ROOT/BACKLOG.md" || true)"
    if [[ -n "$matches" ]]; then
        log_error "Completed checkbox history found in BACKLOG.md"
        echo "$matches"
        exit 1
    fi

    matches="$(rg -n '^\s*[-*+] `\./' "$PROJECT_ROOT/BACKLOG.md" || true)"
    if [[ -n "$matches" ]]; then
        log_error "Command-reference inventory found in BACKLOG.md"
        echo "$matches"
        exit 1
    fi

    matches="$(rg -n '^## .*validation entrypoints' "$PROJECT_ROOT/BACKLOG.md" || true)"
    if [[ -n "$matches" ]]; then
        log_error "Validation entrypoint reference section found in BACKLOG.md"
        echo "$matches"
        exit 1
    fi
    log_success "Backlog open-work audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
