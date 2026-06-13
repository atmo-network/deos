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
  - framework artifact names do not regress to pre-DEOS tmctol runtime/pallet names
  - browser wallet helper aliases do not regress to TMCTOL-branded framework names
  - AAA embedding guide remains linked from docs and pallet entrypoints

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
    require_commands rg wc tr
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

check_legacy_framework_artifact_names() {
    local legacy_prefix='tmctol'
    local pallet_prefix="pallet-${legacy_prefix}"
    local pattern="${legacy_prefix}-runtime|${legacy_prefix}_runtime|${pallet_prefix}"
    local matches
    matches="$(rg -n "$pattern" "$PROJECT_ROOT/scripts" "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client/src" || true)"
    if [[ -n "$matches" ]]; then
        record_finding "legacy pre-DEOS framework artifact names detected"
        echo "$matches"
    fi
}

check_legacy_wallet_helper_aliases() {
    local old_standard='Tmctol'
    local old_constant='TMCTOL'
    local pattern="${old_standard}(InjectedSigner|DevSigner|Signer)|${old_constant}_DEV_SIGNER|isValid${old_standard}Address"
    local matches
    matches="$(rg -n "$pattern" "$PROJECT_ROOT/web-client/src" || true)"
    if [[ -n "$matches" ]]; then
        record_finding "legacy TMCTOL-branded wallet helper aliases detected"
        echo "$matches"
    fi
}

check_aaa_embedding_links() {
    local guide="$PROJECT_ROOT/docs/aaa.embedding.en.md"
    if [[ ! -f "$guide" ]]; then
        record_finding "AAA embedding guide is missing"
        return 0
    fi
    local missing=()
    if ! rg -q 'aaa\.embedding\.en\.md' "$PROJECT_ROOT/docs/README.md"; then
        missing+=("docs/README.md")
    fi
    if ! rg -q 'aaa\.embedding\.en\.md' "$PROJECT_ROOT/README.md"; then
        missing+=("README.md")
    fi
    if ! rg -q 'aaa\.embedding\.en\.md' "$TEMPLATE_DIR/pallets/aaa/README.md"; then
        missing+=("template/pallets/aaa/README.md")
    fi
    if [[ ${#missing[@]} -gt 0 ]]; then
        record_finding "AAA embedding guide is not linked from required entrypoints"
        printf '%s\n' "${missing[@]}"
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
    check_legacy_framework_artifact_names
    check_legacy_wallet_helper_aliases
    check_aaa_embedding_links
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
