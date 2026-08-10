#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-router-identity.sh

Rejects the retired pre-DEOS Router identity from active paths and content.
Historical CHANGELOG entries remain explicitly excluded.
EOF
}

check_canonical_anchors() {
    rg -q 'name = "pallet_deos_router"' "$TEMPLATE_DIR/pallets/router/Cargo.toml" || {
        log_error "Canonical Router Rust crate identity is missing"
        return 1
    }
    rg -q 'pub type DeosRouter = pallet_deos_router;' "$TEMPLATE_DIR/runtime/src/lib.rs" || {
        log_error "Canonical runtime Router identity is missing"
        return 1
    }
    rg -q 'ROUTER_PALLET_ID: &\[u8; 8\] = b"router00"' \
        "$TEMPLATE_DIR/primitives/src/ecosystem.rs" || {
        log_error "Canonical Router account seed is missing"
        return 1
    }
}

check_retired_identity() {
    local retired_title retired_lower retired_upper pattern content_matches path_matches
    retired_title="A""xial"
    retired_lower="a""xial"
    retired_upper="A""XIAL"
    pattern="${retired_title}|${retired_lower}|${retired_upper}"
    content_matches="$(rg -n "$pattern" "$PROJECT_ROOT" \
        --hidden \
        -g '!.git/**' \
        -g '!target/**' \
        -g '!node_modules/**' \
        -g '!CHANGELOG.md' 2>/dev/null || true)"
    path_matches="$(git -C "$PROJECT_ROOT" ls-files -co --exclude-standard | rg -i "$retired_lower" || true)"
    if [[ -n "$content_matches" || -n "$path_matches" ]]; then
        log_error "Retired Router identity remains"
        [[ -z "$path_matches" ]] || printf '%s\n' "$path_matches"
        [[ -z "$content_matches" ]] || printf '%s\n' "$content_matches"
        return 1
    fi
}

main() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        usage
        exit 0
    fi
    [[ "$#" == "0" ]] || { log_error "Unknown argument: $1"; usage; exit 1; }
    phase_banner "Step 1: Prerequisites"
    require_commands git rg
    phase_banner "Step 2: Canonical identity"
    check_canonical_anchors
    phase_banner "Step 3: Retired identity"
    check_retired_identity
    log_success "DEOS Router identity audit passed"
}

main "$@"
