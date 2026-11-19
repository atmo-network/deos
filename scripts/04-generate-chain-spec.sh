#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CHAIN_TYPE="${CHAIN_TYPE:-Development}"
PARA_ID="${PARA_ID:-2000}"
RELAY_CHAIN="${RELAY_CHAIN:-rococo-local}"
RUNTIME_WASM_PATH="${RUNTIME_WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm}"
CHAIN_SPEC_PATH="${CHAIN_SPEC_PATH:-$TEMPLATE_DIR/chain_spec.json}"
SELF_TEST=0
BUILDER_INPUTS="$SCRIPT_DIR/inputs/chain-spec-builder"

usage() {
    cat <<'EOF'
Usage: 04-generate-chain-spec.sh [OPTIONS]

Generates template/chain_spec.json as raw storage from a complete runtime-owned genesis preset.

Options:
  --self-test       Check builder provenance/refusal without product execution
  -h, --help        Show this help message

Environment:
  CHAIN_TYPE=Development|Local
  PARA_ID=2000 (must match the runtime-owned reference presets)
  RELAY_CHAIN=rococo-local
  RUNTIME_WASM_PATH=template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm
  CHAIN_SPEC_PATH=template/chain_spec.json

Inputs:
  Selected DEOS runtime Wasm, qualified chain-spec-builder with adjacent
  chain-spec-builder.producer.json, Node, and profile values. Unknown builder
  producers refuse before execution, even if --version matches. Builder19
  source/API security dispositions must be accepted independently of provenance.

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
            --self-test) SELF_TEST=1 ;;
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
    require_commands chain-spec-builder du cut mv mkdir dirname mktemp rm node
    [[ -z "${NODE_OPTIONS:-}${NODE_PATH:-}${LD_PRELOAD:-}" ]] || { log_error 'Loader injection is unsupported'; return 1; }
    activate_pinned_node
    SELECTED_BUILDER="$(command -v chain-spec-builder)"
    [[ "$SELECTED_BUILDER" == /* ]] || { log_error 'Builder must resolve to an absolute path'; return 1; }
    verify_builder_producer
    node "$BUILDER_INPUTS/provenance.mjs" check-output-path "$CHAIN_SPEC_PATH"
    if [[ "$PARA_ID" != "2000" ]]; then
        log_error "PARA_ID=$PARA_ID does not match the runtime-owned reference preset (2000)"
        exit 1
    fi
    log_success "Chain spec prerequisites checked"
}

verify_builder_producer() {
    node "$BUILDER_INPUTS/provenance.mjs" verify "${SELECTED_BUILDER}.producer.json" "$SELECTED_BUILDER"
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
    local generation_dir generated_path output_parent
    node "$BUILDER_INPUTS/provenance.mjs" check-output-path "$CHAIN_SPEC_PATH" || return 1
    mkdir -p "$(dirname "$CHAIN_SPEC_PATH")"
    output_parent="$(cd "$(dirname "$CHAIN_SPEC_PATH")" && pwd -P)"
    generation_dir="$(mktemp -d "$output_parent/.deos-chain-spec.XXXXXX")"
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
        verify_builder_producer || exit 1
        "$SELECTED_BUILDER" create \
            --chain-name "$CHAIN_NAME" \
            --chain-id "$CHAIN_ID" \
            -t "$BUILDER_CHAIN_TYPE" \
            -c "$RELAY_CHAIN" \
            -p "$PARA_ID" \
            -r "$RUNTIME_WASM_PATH" \
            --properties tokenSymbol=NTVE,tokenDecimals=12,ss58Format=42,isEthereum=false \
            --raw-storage \
            --verify \
            named-preset "$PRESET"
    ); then
        rm -rf "$generation_dir"
        log_error "chain-spec-builder failed"
        exit 1
    fi

    if ! node "$BUILDER_INPUTS/provenance.mjs" check-raw-spec "$generated_path" "$RUNTIME_WASM_PATH" \
        || ! node "$BUILDER_INPUTS/provenance.mjs" check-output-path "$CHAIN_SPEC_PATH"; then
        rm -rf "$generation_dir"
        log_error 'Builder output is not a complete raw ChainSpec or destination is unsafe; previous output retained'
        return 1
    fi
    mkdir -p "$(dirname "$CHAIN_SPEC_PATH")"
    mv -f "$generated_path" "$CHAIN_SPEC_PATH" || return 1
    rm -rf "$generation_dir"

    log_success "Chain specification generated from runtime preset"
}

verify_output() {
    phase_banner "Step 3: Verify output"
    if node "$BUILDER_INPUTS/provenance.mjs" check-raw-spec "$CHAIN_SPEC_PATH" "$RUNTIME_WASM_PATH"; then
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

run_self_tests() (
    node "$BUILDER_INPUTS/provenance.mjs" self-test || exit 1
    local fixture
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/deos-builder-integration.XXXXXX")"
    trap 'rm -rf "$fixture"' EXIT
    SELECTED_BUILDER="$fixture/builder"
    RUNTIME_WASM_PATH="$fixture/runtime.wasm"
    CHAIN_SPEC_PATH="$fixture/output.json"
    PRESET=development
    BUILDER_CHAIN_TYPE=development
    CHAIN_NAME='DEOS Development'
    CHAIN_ID=deos-dev
    printf 'fixture' >"$RUNTIME_WASM_PATH"
    printf 'original' >"$CHAIN_SPEC_PATH"
    cat >"$SELECTED_BUILDER" <<'BUILDER'
#!/usr/bin/env bash
set -eu
printf 'invoked' >"${0}.invoked"
printf '%s\n' "$@" >"${0}.argv"
if [[ -f "${0}.candidate" ]]; then
    cp "${0}.candidate" chain_spec.json
else
    printf '%s\n' '{"name":"Fixture","id":"fixture","chainType":"Development","bootNodes":[],"properties":{},"genesis":{"raw":{"top":{"0x3a636f6465":"0x66697874757265"},"childrenDefault":{}}}}' >chain_spec.json
fi
BUILDER
    chmod +x "$SELECTED_BUILDER"
    if (generate_chain_spec) >"$fixture/missing.log" 2>&1; then exit 1; fi
    [[ ! -e "${SELECTED_BUILDER}.invoked" && "$(<"$CHAIN_SPEC_PATH")" == original ]] || exit 1
    printf '{' >"${SELECTED_BUILDER}.producer.json"
    if (generate_chain_spec) >"$fixture/malformed.log" 2>&1; then exit 1; fi
    [[ ! -e "${SELECTED_BUILDER}.invoked" && "$(<"$CHAIN_SPEC_PATH")" == original ]] || exit 1
    # This stub proves routing/publication only, never product qualification.
    verify_builder_producer() { printf x >>"$fixture/checks"; }
    verify_builder_producer
    (generate_chain_spec) >"$fixture/positive.log" 2>&1 || exit 1
    [[ "$(<"$fixture/checks")" == xx && -f "${SELECTED_BUILDER}.invoked" ]] || exit 1
    grep -Fxq -- '--verify' "${SELECTED_BUILDER}.argv" || exit 1
    grep -Fxq -- '--raw-storage' "${SELECTED_BUILDER}.argv" || exit 1
    grep -Fxq -- 'named-preset' "${SELECTED_BUILDER}.argv" || exit 1
    grep -Fxq -- 'development' "${SELECTED_BUILDER}.argv" || exit 1
    node "$BUILDER_INPUTS/provenance.mjs" check-raw-spec "$CHAIN_SPEC_PATH" "$RUNTIME_WASM_PATH" || exit 1
    local candidate
    for candidate in '' '{' '{}' '{"genesis":{"runtime":{}}}' \
        '{"name":"Fixture","id":"fixture","chainType":"Development","bootNodes":[],"properties":{},"genesis":{"raw":{"top":{},"childrenDefault":{}}}}'; do
        printf original >"$CHAIN_SPEC_PATH"
        printf '%s' "$candidate" >"${SELECTED_BUILDER}.candidate"
        if (generate_chain_spec) >"$fixture/partial-${#candidate}.log" 2>&1; then exit 1; fi
        [[ "$(<"$CHAIN_SPEC_PATH")" == original ]] || exit 1
    done
    rm "${SELECTED_BUILDER}.candidate"
    local unsafe
    for unsafe in directory fifo symlink; do
        case "$unsafe" in
            directory) mkdir "$fixture/$unsafe" ;;
            fifo) mkfifo "$fixture/$unsafe" ;;
            symlink) ln -s "$CHAIN_SPEC_PATH" "$fixture/$unsafe" ;;
        esac
        if (CHAIN_SPEC_PATH="$fixture/$unsafe" generate_chain_spec) >"$fixture/$unsafe.log" 2>&1; then exit 1; fi
        [[ "$(<"$CHAIN_SPEC_PATH")" == original ]] || exit 1
    done
    local -a leftovers
    shopt -s nullglob
    leftovers=("$fixture"/.deos-chain-spec.*)
    [[ "${#leftovers[@]}" == 0 ]] || exit 1
    log_success 'Both receipt checks, valid raw publication, exit-zero partial/invalid raw refusal, candidate cleanup and unsafe destination preservation passed (private stubs only)'
)

main() {
    parse_args "$@"
    if [[ "$SELF_TEST" == 1 ]]; then
        run_self_tests
        return
    fi
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
