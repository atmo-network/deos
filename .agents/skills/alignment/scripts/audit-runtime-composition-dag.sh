#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-runtime-composition-dag.sh

Verifies that Governance, Staking, Router, Oracle, and Actors remain reusable
leaf packages without direct dependencies on one another. Concrete cross-system
composition must stay in the DEOS runtime adapters.
EOF
}

main() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        usage
        exit 0
    fi
    [[ "$#" == "0" ]] || { log_error "Unknown argument: $1"; usage; exit 1; }
    phase_banner "Step 1: Prerequisites"
    require_commands rg

    local packages=(governance staking router oracle actors)
    local package
    local manifest
    local forbidden='pallet-deos-(governance|staking|router|oracle|actors)'
    phase_banner "Step 2: Reusable pallet dependency boundary"
    for package in "${packages[@]}"; do
        manifest="$TEMPLATE_DIR/pallets/$package/Cargo.toml"
        if awk '/^\[dependencies\]/{inside=1; next} /^\[/{inside=0} inside' "$manifest" | rg -n "$forbidden"; then
            log_error "Direct cross-subsystem dependency detected in $manifest"
            exit 1
        fi
    done
    log_success "Runtime composition DAG audit passed"
}

main "$@"
