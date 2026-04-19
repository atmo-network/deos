#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CLEAN_BUILD="${CLEAN_BUILD:-0}"
CLEAN_BINARIES="${CLEAN_BINARIES:-0}"

usage() {
    cat <<'EOF'
Usage: clean-local-artifacts.sh [OPTIONS]

Removes generated local artifacts without touching running processes.

Options:
  --with-build      Also remove template/target
  --with-binaries   Also remove downloaded binaries under ./bin
  -h, --help        Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --with-build)
                CLEAN_BUILD=1
                ;;
            --with-binaries)
                CLEAN_BINARIES=1
                ;;
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
    phase_banner "Step 1: Cleanup plan"
    log_info "Plan: clean_build=$CLEAN_BUILD clean_binaries=$CLEAN_BINARIES"
}

clean_chain_spec() {
    if [[ -f "$TEMPLATE_DIR/chain_spec.json" ]]; then
        log_info "Removing generated chain spec"
        rm -f "$TEMPLATE_DIR/chain_spec.json"
        log_success "Chain spec removed"
    else
        log_info "No generated chain spec found"
    fi
}

clean_build_artifacts() {
    if [[ "$CLEAN_BUILD" != "1" ]]; then
        return 0
    fi

    log_info "Removing build artifacts"
    if [[ -d "$TEMPLATE_DIR/target" ]]; then
        rm -rf "$TEMPLATE_DIR/target"
        log_success "Build artifacts removed"
    else
        log_info "No build artifacts found"
    fi
}

clean_binaries() {
    if [[ "$CLEAN_BINARIES" != "1" ]]; then
        return 0
    fi

    log_info "Removing downloaded binaries"
    if [[ -d "$BIN_DIR" ]]; then
        rm -rf "$BIN_DIR"
        log_success "Downloaded binaries removed"
    else
        log_info "No downloaded binaries found"
    fi
}

main() {
    parse_args "$@"
    phase_banner "DEOS local artifact cleanup"
    check_prerequisites
    phase_banner "Step 2: Remove local artifacts"
    clean_chain_spec
    clean_build_artifacts
    clean_binaries
    log_success "Local artifact cleanup completed successfully"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
