#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CHAIN_TYPE="${CHAIN_TYPE:-Development}"
PARA_ID="${PARA_ID:-2000}"
RELAY_CHAIN="${RELAY_CHAIN:-rococo-local}"
RUNTIME_WASM_PATH="${RUNTIME_WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm}"
CHAIN_SPEC_PATH="${CHAIN_SPEC_PATH:-$TEMPLATE_DIR/chain_spec.json}"

usage() {
    cat <<'EOF'
Usage: 04-generate-chain-spec.sh [OPTIONS]

Generates template/chain_spec.json directly from a complete runtime-owned genesis preset.

Options:
  -h, --help        Show this help message

Environment:
  CHAIN_TYPE=Development|Local
  PARA_ID=2000 (must match the runtime-owned reference presets)
  RELAY_CHAIN=rococo-local
  RUNTIME_WASM_PATH=template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm
  CHAIN_SPEC_PATH=template/chain_spec.json

Inputs:
  Selected DEOS runtime Wasm, chain-spec-builder, and profile values.

Outputs:
  The selected CHAIN_SPEC_PATH.

Side effects:
  Replaces the generated local chain spec; never deploys or starts a network.

Notes:
  Runtime presets own the complete genesis state. This script selects one preset
  and outer ChainSpec metadata without patching economic or authority policy.
  A Live profile requires a separately implemented production runtime preset.
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

resolve_chain_profile() {
    case "$CHAIN_TYPE" in
        Development)
            PRESET="development"
            BUILDER_CHAIN_TYPE="development"
            CHAIN_NAME="DEOS Development"
            CHAIN_ID="deos-dev"
            ;;
        Local)
            PRESET="local_testnet"
            BUILDER_CHAIN_TYPE="local"
            CHAIN_NAME="DEOS Local Testnet"
            CHAIN_ID="deos-local"
            ;;
        Live)
            log_error "CHAIN_TYPE=Live is unavailable until the runtime owns a production genesis preset"
            exit 1
            ;;
        *)
            log_error "Unknown CHAIN_TYPE: $CHAIN_TYPE (expected: Development or Local)"
            exit 1
            ;;
    esac
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands chain-spec-builder du cut mv mkdir dirname mktemp rm
    if [[ "$PARA_ID" != "2000" ]]; then
        log_error "PARA_ID=$PARA_ID does not match the runtime-owned reference preset (2000)"
        exit 1
    fi
    log_success "Chain spec prerequisites checked"
}

GENERATION_DIR=""

# The explicit exit paths below already remove the staging directory, but an interrupt between
# `mktemp -d` and those would leak it. The trap covers that window.
cleanup_generation_dir() {
    [[ -n "$GENERATION_DIR" && -d "$GENERATION_DIR" ]] && rm -rf "$GENERATION_DIR"
    return 0
}

generate_chain_spec() {
    phase_banner "Step 2: Generate chain spec"
    local generation_dir generated_path
    generation_dir="$(mktemp -d "${TMPDIR:-/tmp}/deos-chain-spec.XXXXXX")"
    GENERATION_DIR="$generation_dir"
    trap cleanup_generation_dir EXIT INT TERM
    generated_path="$generation_dir/chain_spec.json"

    log_info "Generating chain specification"
    echo "  Chain type: $CHAIN_TYPE"
    echo "  Preset: $PRESET"
    echo "  Para ID: $PARA_ID"
    echo "  Relay chain: $RELAY_CHAIN"
    echo "  WASM: $RUNTIME_WASM_PATH"
    echo "  Output: $CHAIN_SPEC_PATH"
    echo ""

    if [[ ! -f "$RUNTIME_WASM_PATH" ]]; then
        log_error "Runtime WASM artifact not found."
        echo "  Expected: $RUNTIME_WASM_PATH"
        exit 1
    fi

    if ! (
        cd "$generation_dir"
        chain-spec-builder create \
            --chain-name "$CHAIN_NAME" \
            --chain-id "$CHAIN_ID" \
            -t "$BUILDER_CHAIN_TYPE" \
            -c "$RELAY_CHAIN" \
            -p "$PARA_ID" \
            -r "$RUNTIME_WASM_PATH" \
            --properties tokenSymbol=NTVE,tokenDecimals=12,ss58Format=42,isEthereum=false \
            --verify \
            named-preset "$PRESET"
    ); then
        rm -rf "$generation_dir"
        log_error "chain-spec-builder failed"
        exit 1
    fi

    if [[ ! -f "$generated_path" ]]; then
        rm -rf "$generation_dir"
        log_error "chain-spec-builder did not produce $generated_path"
        exit 1
    fi
    mkdir -p "$(dirname "$CHAIN_SPEC_PATH")"
    mv "$generated_path" "$CHAIN_SPEC_PATH"
    rm -rf "$generation_dir"

    log_success "Chain specification generated from runtime preset"
}

verify_output() {
    phase_banner "Step 3: Verify output"
    if [[ -f "$CHAIN_SPEC_PATH" ]]; then
        local size
        size=$(du -h "$CHAIN_SPEC_PATH" | cut -f1)
        log_success "Chain spec file verified"
        echo "  Path: $CHAIN_SPEC_PATH"
        echo "  Size: $size"
        echo "  Chain type: $CHAIN_TYPE"
        echo "  Name: $CHAIN_NAME"
        echo "  ID: $CHAIN_ID"
        echo "  Para ID: $PARA_ID"
        echo "  Relay chain: $RELAY_CHAIN"
    else
        log_error "Chain specification not generated"
        exit 1
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Chain spec generation completed successfully"
}

main() {
    parse_args "$@"
    phase_banner "DEOS chain spec generation"
    resolve_chain_profile
    check_prerequisites
    generate_chain_spec
    verify_output
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
