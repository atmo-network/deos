#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-strategic-governance-ingress.sh

Verifies the canonical signed primary-governance ingress and rejects local
strategic-authorization shortcuts in the DEOS reference runtime.
EOF
}

require_anchor() {
    local pattern="$1"
    local path="$2"
    local description="$3"
    if ! rg -q -- "$pattern" "$path"; then
        log_error "Missing strategic-ingress anchor: $description"
        return 1
    fi
}

reject_pattern() {
    local pattern="$1"
    local description="$2"
    shift 2
    local matches
    matches="$(rg -n -- "$pattern" "$@" 2>/dev/null || true)"
    if [[ -n "$matches" ]]; then
        log_error "Forbidden strategic-ingress shortcut: $description"
        printf '%s\n' "$matches"
        return 1
    fi
}

check_canonical_ingress() {
    local governance_config="$TEMPLATE_DIR/runtime/src/configs/governance_config.rs"
    local governance_lib="$TEMPLATE_DIR/pallets/governance/src/lib.rs"
    local governance_admission="$TEMPLATE_DIR/pallets/governance/src/epoch_service.rs"
    require_anchor \
        'ProposalSubmissionAuthority::PrimaryEligibleSigned' \
        "$governance_config" \
        "protocol L1RootAction uses primary-eligible signed submission"
    require_anchor \
        'ordinary_track_base_weight\(domain, account\) > 0' \
        "$governance_config" \
        "eligibility derives from nonzero primary-track governance power"
    require_anchor \
        'ProposalSubmitterNotPrimaryEligible' \
        "$governance_lib" \
        "ineligible signed submission has a typed failure"
    require_anchor \
        'T::ProposalSubmissionEligibilityProvider::has_primary_governance_power' \
        "$governance_admission" \
        "eligibility is enforced by the pallet admission classifier before proposal mutation"
    require_anchor \
        'RuntimeCall::System\(frame_system::Call::authorize_upgrade' \
        "$governance_config" \
        "bounded L1RootAction execution targets System authorize_upgrade"
}

check_shortcut_absence() {
    local runtime_paths=(
        "$TEMPLATE_DIR/runtime/Cargo.toml"
        "$TEMPLATE_DIR/runtime/src/lib.rs"
        "$TEMPLATE_DIR/runtime/src/configs"
        "$TEMPLATE_DIR/runtime/src/chain_specs"
    )
    reject_pattern \
        'pallet[-_]sudo|\bSudo\s*:' \
        "Sudo dependency or runtime pallet" \
        "${runtime_paths[@]}"
    reject_pattern \
        'ParentAsSuperuser|OriginKind::Superuser' \
        "XCM superuser or passthrough conversion" \
        "$TEMPLATE_DIR/runtime/src/configs/xcm_config.rs"
    reject_pattern \
        'RehearsalAuthority|DevelopmentRoot|GenesisProposal|genesis_proposal' \
        "rehearsal-only authority or genesis proposal fixture" \
        "${runtime_paths[@]}"
    reject_pattern \
        'pub type [A-Za-z0-9_]*UpgradeAuthorization[A-Za-z0-9_]*<[^>]*> *= *Storage' \
        "fabricated runtime-upgrade authorization state" \
        "$TEMPLATE_DIR/runtime/src" "$TEMPLATE_DIR/pallets/governance/src"
}

main() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        usage
        exit 0
    fi
    [[ "$#" == "0" ]] || { log_error "Unknown argument: $1"; usage; exit 1; }
    phase_banner "Step 1: Prerequisites"
    require_commands rg
    phase_banner "Step 2: Canonical ingress"
    check_canonical_ingress
    phase_banner "Step 3: Shortcut absence"
    check_shortcut_absence
    log_success "Strategic governance ingress audit passed"
}

main "$@"
