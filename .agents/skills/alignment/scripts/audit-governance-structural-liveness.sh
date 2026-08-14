#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-governance-structural-liveness.sh

Rejects governance documentation that presents proposal fees as a structural
liveness or anti-DoS control instead of economic friction above bounded caps.
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

    local specification="$TEMPLATE_DIR/pallets/governance/docs/specification.en.md"
    local architecture="$TEMPLATE_DIR/pallets/governance/docs/architecture.en.md"
    phase_banner "Step 2: Structural liveness ownership"
    rg -q 'economic friction only' "$specification" "$architecture" || {
        log_error "Governance fee must be classified as economic friction only"
        exit 1
    }
    rg -q 'structurally bounded by domain capacity' "$specification" || {
        log_error "Governance specification must assign spam bounds to structural capacity"
        exit 1
    }
    if rg -n -i '(anti-spam|anti-dos|liveness).{0,80}(opening fee|proposal fee)|(?:opening fee|proposal fee).{0,80}(guarantees?|ensures?|provides?).{0,30}(liveness|anti-dos)' \
        "$specification" "$architecture"; then
        log_error "Governance fee is presented as a structural liveness or anti-DoS control"
        exit 1
    fi
    log_success "Governance structural liveness audit passed"
}

main "$@"
