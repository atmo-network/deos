#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

RELEASE_DIR="${RELEASE_DIR:-./target/production/wbuild/deos-runtime}"
WASM_FILE="${WASM_FILE:-deos_runtime.compact.compressed.wasm}"
OUTPUT_DIR="${OUTPUT_DIR:-./release-test-output}"
BUILD_DURATION=0
WASM_SIZE=0
WASM_SIZE_MB="0"

usage() {
    cat <<'EOF'
Usage: release-local.sh [OPTIONS]

Runs the local release workflow: production build, WASM verification, artifact copy, and release info generation.

Options:
  -h, --help        Show this help message

Environment:
  RELEASE_DIR=./target/production/wbuild/deos-runtime
  WASM_FILE=deos_runtime.compact.compressed.wasm
  OUTPUT_DIR=./release-test-output
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
    require_commands cargo rustup protoc stat cp file bc grep
    cd "$TEMPLATE_DIR"
    log_info "Working from: $(pwd)"
    log_success "protobuf-compiler found"

    if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
        log_info "Adding wasm32-unknown-unknown target"
        rustup target add wasm32-unknown-unknown
    else
        log_success "wasm32-unknown-unknown target already installed"
    fi

    if ! rustup component list --installed | grep -q "rust-src"; then
        log_info "Adding rust-src component"
        rustup component add rust-src
    else
        log_success "rust-src component already installed"
    fi
}

build_release_runtime() {
    phase_banner "Step 2: Build release runtime"
    local start_time
    start_time=$(date +%s)
    cargo build --workspace --locked --profile production
    local end_time
    end_time=$(date +%s)
    BUILD_DURATION=$((end_time - start_time))
    log_success "Runtime build completed in ${BUILD_DURATION} seconds"
}

verify_wasm_artifact() {
    phase_banner "Step 3: Verify WASM artifact"

    if [[ ! -d "$RELEASE_DIR" ]]; then
        log_error "Release directory not found: $RELEASE_DIR"
        exit 1
    fi

    if [[ ! -f "$RELEASE_DIR/$WASM_FILE" ]]; then
        log_error "WASM file not found: $RELEASE_DIR/$WASM_FILE"
        exit 1
    fi

    log_success "Release directory found: $RELEASE_DIR"
    log_success "WASM file found: $WASM_FILE"
    log_info "Release directory contents"
    ls -la "$RELEASE_DIR"
    log_info "WASM file information"
    file "$RELEASE_DIR/$WASM_FILE"

    WASM_SIZE=$(stat -c%s "$RELEASE_DIR/$WASM_FILE")
    WASM_SIZE_MB=$(echo "scale=2; $WASM_SIZE / 1024 / 1024" | bc)
    echo "File size: $WASM_SIZE bytes (~${WASM_SIZE_MB} MB)"
}

prepare_release_artifacts() {
    phase_banner "Step 4: Simulate release artifact preparation"
    mkdir -p "$OUTPUT_DIR"
    cp "$RELEASE_DIR/$WASM_FILE" "$OUTPUT_DIR/"
    log_success "WASM runtime copied to: $OUTPUT_DIR/$WASM_FILE"
}

generate_release_information() {
    phase_banner "Step 5: Generate release information"
    cat > "$OUTPUT_DIR/release-info.md" << EOF
# Parachain Runtime Release

This release contains the optimized WASM runtime for deployment with Omni Node.

## Runtime Details
- **File**: $WASM_FILE
- **Size**: $WASM_SIZE bytes (~${WASM_SIZE_MB} MB)
- **Profile**: Production build with LTO enabled
- **Target**: Built with standard wasm32-unknown-unknown target
- **Build Time**: ${BUILD_DURATION} seconds

## Usage with Omni Node
\`\`\`bash
# Use with polkadot-omni-node
polkadot-omni-node --chain=your-chain-spec.json
\`\`\`

## Verification
- File type: $(file "$RELEASE_DIR/$WASM_FILE" | cut -d: -f2)
- Build timestamp: $(date)
- Git commit: $(git rev-parse HEAD 2>/dev/null || echo "N/A")
EOF

    log_success "Release information generated: $OUTPUT_DIR/release-info.md"
}

final_verification() {
    phase_banner "Step 6: Final verification"
    if [[ -f "$OUTPUT_DIR/$WASM_FILE" ]]; then
        local copied_size
        copied_size=$(stat -c%s "$OUTPUT_DIR/$WASM_FILE")
        if [[ "$WASM_SIZE" -eq "$copied_size" ]]; then
            echo "File integrity verified: sizes match"
        else
            log_error "File size mismatch during copy"
            exit 1
        fi
    else
        log_error "Copied file not found"
        exit 1
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Local release workflow completed successfully"
    log_info "Release artifacts available at"
    echo "  Directory: $OUTPUT_DIR"
    echo "  WASM Runtime: $OUTPUT_DIR/$WASM_FILE"
    echo "  Release Info: $OUTPUT_DIR/release-info.md"
}

main() {
    parse_args "$@"
    phase_banner "DEOS local release workflow"
    check_prerequisites
    build_release_runtime
    verify_wasm_artifact
    prepare_release_artifacts
    generate_release_information
    final_verification
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
