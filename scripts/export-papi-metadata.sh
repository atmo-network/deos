#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

METADATA_VERSION=16
OUTPUT_PATH=""
WASM_PATH=""
NATIVE_ONLY=0

usage() {
    cat <<'EOF'
Usage: export-papi-metadata.sh --output PATH [--wasm PATH | --native-only] [--metadata-version N]

Export SCALE runtime metadata independently of any browser workspace. Relative
paths are resolved from the caller. By default use template/target/release/wbuild/
deos-runtime/deos_runtime.compact.compressed.wasm from this distribution.

The exporter calls the selected Wasm's Metadata API, refuses native fallback,
and requires byte equality before writing. --native-only is an explicit schema
preparation route, not Wasm evidence. No npm or descriptor generation is run;
a PAPI consumer may use the output as input to its own generator.

Options:
  --output PATH         Required metadata destination
  --wasm PATH           Explicit Wasm input
  --native-only        Export native metadata without Wasm verification
  --metadata-version N  Metadata version (default: 16)
  -h, --help            Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --output|--wasm|--metadata-version)
                [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || { log_error "$1 requires a value"; return 1; }
                case "$1" in
                    --output) OUTPUT_PATH="$2" ;;
                    --wasm) WASM_PATH="$2" ;;
                    --metadata-version) METADATA_VERSION="$2" ;;
                esac
                shift 2 ;;
            --native-only) NATIVE_ONLY=1; shift ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; return 1 ;;
        esac
    done
    [[ -n "$OUTPUT_PATH" ]] || { log_error "--output is required"; return 1; }
    [[ "$METADATA_VERSION" =~ ^[1-9][0-9]*$ ]] || { log_error "Expected positive metadata version"; return 1; }
    [[ "$NATIVE_ONLY" == 0 || -z "$WASM_PATH" ]] || { log_error "--wasm and --native-only are exclusive"; return 1; }
    [[ "$OUTPUT_PATH" = /* ]] || OUTPUT_PATH="$PWD/$OUTPUT_PATH"
    if [[ "$NATIVE_ONLY" == 0 ]]; then
        WASM_PATH="${WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm}"
        [[ "$WASM_PATH" = /* ]] || WASM_PATH="$PWD/$WASM_PATH"
    fi
}

check_prerequisites() {
    hydrate_local_tool_paths
    require_commands cargo
    require_directory "$TEMPLATE_DIR" "Template workspace"
    if [[ "$NATIVE_ONLY" == 0 ]]; then
        [[ -f "$WASM_PATH" ]] || { log_error "Selected Wasm is missing: $WASM_PATH"; return 1; }
        [[ ! "$OUTPUT_PATH" -ef "$WASM_PATH" ]] || { log_error "Metadata output would overwrite selected Wasm"; return 1; }
    fi
}

main() {
    parse_args "$@"
    check_prerequisites
    local -a wasm_args=()
    if [[ "$NATIVE_ONLY" == 0 ]]; then
        wasm_args=(--wasm "$WASM_PATH")
    else
        log_warning "Native-only metadata is schema preparation, not Wasm evidence"
    fi
    (
        cd "$TEMPLATE_DIR"
        run_command_step "Export DEOS runtime metadata v$METADATA_VERSION" "" \
            env SKIP_WASM_BUILD=1 cargo run --release --locked -p deos-runtime --example export_metadata -- \
            "$OUTPUT_PATH" "$METADATA_VERSION" "${wasm_args[@]}"
    )
}

main "$@"
