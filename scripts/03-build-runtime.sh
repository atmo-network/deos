#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: 03-build-runtime.sh [OPTIONS]

Builds the current DEOS reference runtime (`deos-runtime`) WASM artifact in release mode.

Options:
  -h, --help        Show this help message
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
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands rustc cargo rustup du cut sha256sum
    log_success "Runtime build prerequisites checked"
}

setup_wasm_target() {
    phase_banner "Step 2: Configure WASM target"
    log_info "Checking WASM target..."
    if ! (cd "$TEMPLATE_DIR" && rustup target list --installed) | grep -qx "wasm32-unknown-unknown"; then
        log_info "Installing wasm32-unknown-unknown target for the repository-pinned toolchain..."
        (cd "$TEMPLATE_DIR" && rustup target add wasm32-unknown-unknown)
        log_success "WASM target installed"
    else
        log_success "WASM target already installed for the repository-pinned toolchain"
    fi
}

build_runtime() {
    phase_banner "Step 3: Build runtime"
    log_info "Building parachain runtime (this may take several minutes)..."

    # substrate-wasm-builder treats any presence of SKIP_WASM_BUILD, including `0`, as skip.
    unset SKIP_WASM_BUILD
    run_shell_step \
        "Build parachain runtime" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo build --release -p deos-runtime --locked"
}

verify_build() {
    phase_banner "Step 4: Verify output"
    local wasm_path="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"

    if [[ -f "$wasm_path" ]]; then
        local wasm_size
        local wasm_sha256
        wasm_size=$(du -h "$wasm_path" | cut -f1)
        wasm_sha256=$(sha256sum "$wasm_path" | cut -d' ' -f1)
        log_success "Runtime WASM artifact verified"
        echo "  Path:   $wasm_path"
        echo "  Size:   $wasm_size"
        echo "  SHA256: $wasm_sha256"
    else
        log_error "Runtime WASM not found at expected path: $wasm_path"
        exit 1
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Runtime build process completed successfully"
}

main() {
    parse_args "$@"
    phase_banner "DEOS reference runtime build"
    check_prerequisites
    setup_wasm_target
    build_runtime
    verify_build
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
