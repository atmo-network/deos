#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

usage() {
    cat <<'USAGE'
Usage: audit-template-readiness.sh [--help] [--warn]

Audit lightweight template readiness smells without building the workspace.

Checks:
  - production XCM config does not use pallet_xcm::TestWeightInfo
  - runtime production configs do not carry unclassified WeightInfo = ()
  - template workspace docs do not present native binding as live staking truth
  - runtime asset-conversion integration test module/file spelling is canonical

Options:
  --warn    Print findings but exit successfully
  --help    Show this help
USAGE
}

WARN_ONLY=0

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --warn)
                WARN_ONLY=1
                shift
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
        esac
    done
}

record_finding() {
    local message="$1"
    FINDINGS+=("$message")
    log_error "$message"
}

check_prerequisites() {
    require_directory "$TEMPLATE_DIR" "template workspace"
    require_commands rg
}

check_xcm_weights() {
    local file="$TEMPLATE_DIR/runtime/src/configs/xcm_config.rs"
    if rg -n "type WeightInfo = pallet_xcm::TestWeightInfo" "$file" >/dev/null; then
        record_finding "XCM config still uses pallet_xcm::TestWeightInfo"
    fi
}

check_placeholder_weights() {
    local matches
    matches="$(rg -n "type WeightInfo = \\(\\);" "$TEMPLATE_DIR/runtime/src/configs" || true)"
    if [[ -z "$matches" ]]; then
        return 0
    fi
    if [[ "$(echo "$matches" | wc -l | tr -d ' ')" == "1" ]] && rg -n "same measured constant weight" "$TEMPLATE_DIR/runtime/src/configs/mod.rs" >/dev/null; then
        log_info "Only classified weight-reclaim placeholder remains"
        return 0
    fi
    record_finding "runtime configs still contain unclassified WeightInfo = ()"
    echo "$matches"
}

check_staking_aliases() {
    local matches
    matches="$(rg -n "native binding" "$TEMPLATE_DIR/README.md" "$TEMPLATE_DIR/pallets/README.md" || true)"
    if [[ -n "$matches" ]]; then
        record_finding "template docs still present native binding wording"
        echo "$matches"
    fi
}

check_asset_conversion_name() {
    if [[ -e "$TEMPLATE_DIR/runtime/src/tests/asset_convertion_integration_tests.rs" ]]; then
        record_finding "legacy asset_convertion integration test file still exists"
    fi
    if rg -n "asset_convertion_integration_tests" "$TEMPLATE_DIR/runtime/src/tests" >/dev/null; then
        record_finding "legacy asset_convertion module reference still exists"
    fi
}

main() {
    parse_args "$@"
    phase_banner "Template readiness audit"
    check_prerequisites
    FINDINGS=()
    check_xcm_weights
    check_placeholder_weights
    check_staking_aliases
    check_asset_conversion_name
    if [[ ${#FINDINGS[@]} -eq 0 ]]; then
        log_success "Template readiness audit passed"
        return 0
    fi
    log_warning "Template readiness audit found ${#FINDINGS[@]} issue(s)"
    if [[ $WARN_ONLY -eq 1 ]]; then
        return 0
    fi
    return 1
}

main "$@"
