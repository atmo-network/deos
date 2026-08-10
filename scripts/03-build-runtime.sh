#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CANONICAL_BUILD_ROOT="/tmp/deos-runtime-production-source"
BUILD_PROJECT_ROOT=""
BUILD_TEMPLATE_DIR=""
BUILD_STAGE_OWNED=0
OUTPUT_WASM_PATH="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"

usage() {
    cat <<'EOF'
Usage: 03-build-runtime.sh [OPTIONS]

Builds the current DEOS reference runtime (`deos-runtime`) Wasm artifact with substrate-wasm-builder's production profile.

Options:
  -h, --help        Show this help message

Inputs:
  Locked template Cargo workspace and repository-pinned Rust toolchain.

Outputs:
  target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm.

Side effects:
  Installs the pinned Wasm target when absent, builds in a fixed temporary
  source root, and atomically replaces the selected Wasm output after success.

Notes:
  The production Wasm profile fixes fat LTO and one codegen unit. One physical
  build root stabilizes Cargo crate identity; canonical virtual source,
  Cargo-home, and Rustup-home prefixes remove machine paths from Wasm bytes.
  A concurrent or stale canonical build root fails closed rather than sharing
  mutable build state.
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
    require_commands rustc cargo rustup du cut sha256sum tar mkdir rm cp mv dirname mktemp
    log_success "Runtime build prerequisites checked"
}

cleanup_build_stage() {
    if (( BUILD_STAGE_OWNED == 1 )); then
        rm -rf "$CANONICAL_BUILD_ROOT"
        BUILD_STAGE_OWNED=0
    fi
}

stage_runtime_source() {
    phase_banner "Step 2: Stage deterministic source"
    if ! mkdir "$CANONICAL_BUILD_ROOT"; then
        log_error "Canonical runtime build root is already present: $CANONICAL_BUILD_ROOT"
        echo "  Refusing to share or remove a build root not created by this process."
        exit 1
    fi
    BUILD_STAGE_OWNED=1
    trap cleanup_build_stage EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    if ! (cd "$PROJECT_ROOT" && tar --exclude='template/target' --exclude='template/chain_spec.json' -cf - template) \
        | (cd "$CANONICAL_BUILD_ROOT" && tar -xf -); then
        log_error "Unable to stage the template workspace at the canonical build root"
        exit 1
    fi
    BUILD_PROJECT_ROOT="$CANONICAL_BUILD_ROOT"
    BUILD_TEMPLATE_DIR="$CANONICAL_BUILD_ROOT/template"
    require_directory "$BUILD_TEMPLATE_DIR" "Staged template directory"
    log_success "Runtime source staged at the canonical physical build root"
}

setup_wasm_target() {
    phase_banner "Step 3: Configure Wasm target"
    log_info "Checking Wasm target..."
    if ! (cd "$BUILD_TEMPLATE_DIR" && rustup target list --installed) | grep -qx "wasm32-unknown-unknown"; then
        log_info "Installing wasm32-unknown-unknown target for the repository-pinned toolchain..."
        (cd "$BUILD_TEMPLATE_DIR" && rustup target add wasm32-unknown-unknown)
        log_success "Wasm target installed"
    else
        log_success "Wasm target already installed for the repository-pinned toolchain"
    fi
}

configure_reproducible_wasm_paths() {
    local cargo_home rustup_home path
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
    for path in "$BUILD_PROJECT_ROOT" "$cargo_home" "$rustup_home"; do
        [[ "$path" == /* ]] || { log_error "Reproducible Wasm path roots must be absolute: $path"; exit 1; }
        [[ ! "$path" =~ [[:space:]] ]] || { log_error "Reproducible Wasm path roots must not contain whitespace: $path"; exit 1; }
    done
    export WASM_BUILD_TYPE=production
    export WASM_BUILD_RUSTFLAGS="--remap-path-prefix=$BUILD_PROJECT_ROOT=/deos/source --remap-path-prefix=$cargo_home=/deos/cargo --remap-path-prefix=$rustup_home=/deos/rustup"
    export CARGO_INCREMENTAL=0
    log_success "Runtime Wasm uses the production profile and canonical build identity"
}

build_runtime() {
    phase_banner "Step 4: Build runtime"
    local built_wasm output_dir temporary_output
    built_wasm="$BUILD_TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
    output_dir="$(dirname "$OUTPUT_WASM_PATH")"
    log_info "Building parachain runtime (this may take several minutes)..."

    # substrate-wasm-builder treats any presence of SKIP_WASM_BUILD, including `0`, as skip.
    unset SKIP_WASM_BUILD
    configure_reproducible_wasm_paths
    run_shell_step \
        "Build parachain runtime" \
        "" \
        "cd '$BUILD_TEMPLATE_DIR' && cargo build --release -p deos-runtime --locked"
    [[ -f "$built_wasm" ]] || { log_error "Staged runtime Wasm is unavailable: $built_wasm"; exit 1; }

    mkdir -p "$output_dir"
    temporary_output="$(mktemp "$output_dir/.deos_runtime.compact.compressed.wasm.XXXXXX")"
    if ! cp "$built_wasm" "$temporary_output"; then
        rm -f "$temporary_output"
        log_error "Unable to stage the successful runtime Wasm for publication"
        exit 1
    fi
    mv "$temporary_output" "$OUTPUT_WASM_PATH"
    log_success "Runtime Wasm atomically published from the canonical build root"
}

verify_build() {
    phase_banner "Step 5: Verify output"
    local wasm_path="$OUTPUT_WASM_PATH"

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
    stage_runtime_source
    setup_wasm_target
    build_runtime
    verify_build
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
