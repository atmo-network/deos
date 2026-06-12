#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-repo-portability.sh [OPTIONS]

Checks committed project surfaces for operator-local path and global agent-skill
coupling. Repository validation should run from the checkout itself rather than
from one maintainer's home directory or globally installed agent skills.

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
    require_commands git rg
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Repository portability"
    local home_path="/home/""llb"
    local agent_home="~/"".pi"
    local agent_dir="\\.""pi/""agent"
    local default_pi_phrase="current user.*default p""i"
    local skill_dir_home="SKILL_DIR=""~"
    local pattern="$home_path|$agent_home|$agent_dir|$default_pi_phrase|$skill_dir_home"
    local matches
    matches="$({
        git -C "$PROJECT_ROOT" ls-files -z \
            ':!:web-client/package-lock.json' \
            ':!:template/Cargo.lock' \
            ':!:**/target/**' \
            ':!:**/node_modules/**' \
            ':!:**/build/**' \
            ':!:**/.svelte-kit/**' \
          | xargs -0 rg -n "$pattern" -- || true
    } || true)"
    if [[ -n "$matches" ]]; then
        log_error "Operator-local path or global agent dependency detected"
        echo "$matches"
        exit 1
    fi
    log_success "Repository portability audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
