#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

POLKADOT_VERSION="${POLKADOT_VERSION:-polkadot-stable2606}"
RELEASE_URL="https://github.com/paritytech/polkadot-sdk/releases/download/${POLKADOT_VERSION}"

BINARIES=(
    "polkadot"
    "polkadot-execute-worker"
    "polkadot-prepare-worker"
    "polkadot-omni-node"
)

usage() {
    cat <<'EOF'
Usage: 01-download-binaries.sh [OPTIONS]

Downloads local Polkadot SDK binaries into ./bin.

Options:
  -h, --help        Show this help message

Environment:
  POLKADOT_VERSION=polkadot-stable2606
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
    require_commands curl mkdir chmod
    log_success "Binary download prerequisites checked"
}

download_binaries() {
    phase_banner "Step 2: Download binaries"
    echo "  Version: $POLKADOT_VERSION"
    echo "  Target dir: $BIN_DIR"
    echo ""
    mkdir -p "$BIN_DIR"

    for binary in "${BINARIES[@]}"; do
        local binary_path="$BIN_DIR/$binary"
        if [[ -x "$binary_path" ]]; then
            log_warning "$binary already exists, skipping download"
            continue
        fi

        log_info "Downloading $binary..."
        if curl -fsSL "${RELEASE_URL}/${binary}" -o "$binary_path"; then
            chmod +x "$binary_path"
            log_success "$binary downloaded and made executable"
        else
            log_error "Failed to download $binary"
            exit 1
        fi
    done
}

print_summary() {
    phase_banner "Summary"
    log_success "All binaries downloaded successfully"
    log_info "To add binaries to PATH:"
    echo "  export PATH=\"\$PATH:$BIN_DIR\""
}

main() {
    parse_args "$@"
    phase_banner "DEOS binary download"
    check_prerequisites
    download_binaries
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
