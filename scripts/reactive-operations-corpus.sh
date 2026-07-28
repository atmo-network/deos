#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

FAMILY=""
LIST=0
EXECUTE=0
CARGO_PROFILE="dev"

usage() {
    cat <<'EOF'
Usage: reactive-operations-corpus.sh [OPTIONS]

Validate the machine-readable AAA reactive-operations scenario contract,
generated runtime identity, and runtime-test evidence anchors. This command
does not execute the anchored Rust tests.

Options:
  --family <name>  Validate one scenario family
  --list           List scenario ids and families after validation
  --execute        Execute every selected anchored Rust test after validation
  --release        Use Cargo release profile for --execute
  -h, --help       Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --family)
                [[ $# -ge 2 ]] || { log_error "--family requires a value"; exit 2; }
                FAMILY="$2"
                shift 2
                ;;
            --list)
                LIST=1
                shift
                ;;
            --execute)
                EXECUTE=1
                shift
                ;;
            --release)
                CARGO_PROFILE="release"
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage >&2
                exit 2
                ;;
        esac
    done
}

check_prerequisites() {
    require_commands node
    if [[ "$EXECUTE" -eq 1 ]]; then
        require_commands cargo
        require_directory "$TEMPLATE_DIR" "Template directory"
    fi
    [[ -f "$SCRIPT_DIR/validate-reactive-operations-corpus.mjs" ]] || {
        log_error "Corpus validator implementation is missing"
        exit 1
    }
}

main() {
    parse_args "$@"
    check_prerequisites
    local args=()
    [[ -z "$FAMILY" ]] || args+=(--family "$FAMILY")
    [[ "$LIST" -eq 0 ]] || args+=(--list)
    [[ "$EXECUTE" -eq 0 ]] || args+=(--anchors)
    phase_banner "AAA reactive operations corpus"
    local output
    output="$(node "$SCRIPT_DIR/validate-reactive-operations-corpus.mjs" "${args[@]}")"
    printf '%s\n' "$output"
    log_success "Reactive operations corpus contract passed"

    if [[ "$EXECUTE" -eq 1 ]]; then
        local profile_flag=""
        [[ "$CARGO_PROFILE" == "dev" ]] || profile_flag="--release"
        while IFS=$'\t' read -r package_name test_symbol; do
            if [[ "$package_name" != "pallet-deos-aaa" && "$package_name" != "deos-runtime" ]]; then
                continue
            fi
            run_shell_step \
                "Reactive corpus: $test_symbol" \
                "" \
                "cd \"$TEMPLATE_DIR\" && cargo test -q $profile_flag -p $package_name --locked --lib $test_symbol"
        done <<< "$output"
        log_success "Selected reactive operations anchors passed"
    fi
}

main "$@"
