#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

METADATA_VERSION="16"
OUTPUT_PATH="$PROJECT_ROOT/web-client/.papi/metadata/deos.scale"
GENERATE_DESCRIPTORS="1"

usage() {
    cat <<EOF
Usage: $0 [options]

Export DEOS runtime metadata for the web-client PAPI descriptor pipeline.

Options:
  --metadata-version <version>  Runtime metadata version to export (default: $METADATA_VERSION)
  --output <path>              Metadata output path (default: $OUTPUT_PATH)
  --skip-generate              Only export metadata; do not run npm run papi:generate
  --help                       Show this help message

Examples:
  $0
  $0 --skip-generate
  $0 --metadata-version 16 --output web-client/.papi/metadata/deos.scale
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --metadata-version)
                if [[ $# -lt 2 ]]; then
                    log_error "--metadata-version requires a value"
                    exit 1
                fi
                METADATA_VERSION="$2"
                shift 2
                ;;
            --output)
                if [[ $# -lt 2 ]]; then
                    log_error "--output requires a path"
                    exit 1
                fi
                OUTPUT_PATH="$2"
                shift 2
                ;;
            --skip-generate)
                GENERATE_DESCRIPTORS="0"
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

normalize_output_path() {
    if [[ "$OUTPUT_PATH" != /* ]]; then
        OUTPUT_PATH="$PROJECT_ROOT/$OUTPUT_PATH"
    fi
}

check_prerequisites() {
    phase_banner "Prerequisites"
    hydrate_local_tool_paths
    require_commands cargo
    require_directory "$TEMPLATE_DIR" "Template workspace"
    require_directory "$PROJECT_ROOT/web-client" "Web-client workspace"
    if [[ "$GENERATE_DESCRIPTORS" == "1" ]]; then
        require_commands npm
    fi
    if [[ ! "$METADATA_VERSION" =~ ^[0-9]+$ ]]; then
        log_error "Metadata version must be a positive integer: $METADATA_VERSION"
        exit 1
    fi
    log_success "Metadata export prerequisites checked"
}

main() {
    normalize_output_path
    check_prerequisites
    phase_banner "Export runtime metadata"
    run_shell_step \
        "Export DEOS runtime metadata v$METADATA_VERSION" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo run -p deos-runtime --example export_metadata -- '$OUTPUT_PATH' '$METADATA_VERSION'"
    if [[ "$GENERATE_DESCRIPTORS" == "1" ]]; then
        phase_banner "Generate PAPI descriptors"
        run_shell_step \
            "Generate web-client PAPI descriptors" \
            "" \
            "cd '$PROJECT_ROOT/web-client' && npm run papi:generate"
    else
        log_info "Skipping PAPI descriptor generation by request"
    fi
    log_success "PAPI metadata export complete"
}

parse_args "$@"
main
