#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ONLY_CHECK="all"
FIX_FORMAT=0
TARGET_PACKAGE=""
TEST_FILTER=""
FEATURE_MODE="auto"
SKIP_WASM_BUILD="${SKIP_WASM_BUILD:-1}"
if [[ "$SKIP_WASM_BUILD" == "1" ]]; then
    export SKIP_WASM_BUILD
elif [[ "$SKIP_WASM_BUILD" == "0" ]]; then
    unset SKIP_WASM_BUILD
else
    echo "SKIP_WASM_BUILD must be 0 or 1" >&2
    exit 2
fi

usage() {
    cat <<'EOF'
Usage: ci-local.sh [OPTIONS]

Runs the local CI workflow: clippy, tests, docs, formatting check, and workspace check.

Options:
  --only CHECK       Run one compact check: clippy, tests, docs, format, or check
  --package NAME     Scope clippy, tests, docs, or check to one Cargo package
  --test-filter NAME Scope --only tests to one Cargo test-name filter
  --all-features     Enable all Cargo features (Clippy default)
  --default-features Use package default features, including for Clippy
  --fix              Apply formatting; requires --only format
  -h, --help         Show this help message

Environment:
  SKIP_WASM_BUILD=0|1
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --only)
                if [[ $# -lt 2 ]]; then
                    log_error "--only requires a check name"
                    exit 2
                fi
                ONLY_CHECK="$2"
                case "$ONLY_CHECK" in
                    clippy|tests|docs|format|check) ;;
                    *)
                        log_error "Unsupported check: $ONLY_CHECK"
                        exit 2
                        ;;
                esac
                shift 2
                continue
                ;;
            --package)
                if [[ $# -lt 2 ]]; then
                    log_error "--package requires a package name"
                    exit 2
                fi
                TARGET_PACKAGE="$2"
                shift 2
                continue
                ;;
            --test-filter)
                if [[ $# -lt 2 ]]; then
                    log_error "--test-filter requires a test name"
                    exit 2
                fi
                TEST_FILTER="$2"
                shift 2
                continue
                ;;
            --all-features)
                FEATURE_MODE="all"
                ;;
            --default-features)
                FEATURE_MODE="default"
                ;;
            --fix)
                FIX_FORMAT=1
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
    if (( FIX_FORMAT == 1 )) && [[ "$ONLY_CHECK" != "format" ]]; then
        log_error "--fix requires --only format"
        exit 2
    fi
    if [[ -n "$TARGET_PACKAGE" && ! "$TARGET_PACKAGE" =~ ^[A-Za-z0-9_-]+$ ]]; then
        log_error "--package accepts a Cargo package name"
        exit 2
    fi
    if [[ -n "$TARGET_PACKAGE" && "$ONLY_CHECK" == "format" ]]; then
        log_error "--package does not apply to formatting"
        exit 2
    fi
    if [[ -n "$TEST_FILTER" && "$ONLY_CHECK" != "tests" ]]; then
        log_error "--test-filter requires --only tests"
        exit 2
    fi
    if [[ -n "$TEST_FILTER" && ! "$TEST_FILTER" =~ ^[A-Za-z0-9_:.-]+$ ]]; then
        log_error "--test-filter contains unsupported characters"
        exit 2
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands cargo rustup timeout grep find wc
    log_info "Working from: $TEMPLATE_DIR"
    cd "$TEMPLATE_DIR"

    if ! cargo clippy --version &>/dev/null; then
        log_info "Installing clippy"
        rustup component add clippy
    fi

    log_success "Prerequisites checked"
}

selected() {
    [[ "$ONLY_CHECK" == "all" || "$ONLY_CHECK" == "$1" ]]
}

cargo_scope_args() {
    if [[ -n "$TARGET_PACKAGE" ]]; then
        printf '%s' "--package '$TARGET_PACKAGE'"
    else
        printf '%s' "--workspace"
    fi
}

cargo_feature_args() {
    local check="$1"
    if [[ "$FEATURE_MODE" == "all" || ( "$FEATURE_MODE" == "auto" && "$check" == "clippy" ) ]]; then
        printf '%s' "--all-features"
    fi
}

run_primary_checks() {
    phase_banner "Step 2: Primary checks"
    local scope_args test_filter
    scope_args="$(cargo_scope_args)"
    test_filter="${TEST_FILTER:+ '$TEST_FILTER'}"

    if selected clippy; then
        local feature_args
        feature_args="$(cargo_feature_args clippy)"
        run_shell_step "Clippy (Linting)" \
            "30" \
            "cargo clippy --all-targets $feature_args --locked $scope_args --quiet -- -D warnings"
    fi

    if selected tests; then
        local feature_args
        feature_args="$(cargo_feature_args tests)"
        run_shell_step "Tests" \
            "15" \
            "cargo test $feature_args $scope_args$test_filter"
    fi

    if selected docs; then
        local feature_args
        feature_args="$(cargo_feature_args docs)"
        run_shell_step "Documentation Build" \
            "15" \
            "cargo doc $feature_args $scope_args --no-deps"
    fi
}

run_additional_checks() {
    phase_banner "Step 3: Additional checks"

    if selected format; then
        if (( FIX_FORMAT == 1 )); then
            run_shell_step \
                "Apply code formatting" \
                "" \
                "cd '$TEMPLATE_DIR' && cargo fmt"
        else
            run_shell_step \
                "Code formatting" \
                "" \
                "cd '$TEMPLATE_DIR' && cargo fmt -- --check"
        fi
    fi

    if selected check; then
        local scope_args feature_args
        scope_args="$(cargo_scope_args)"
        feature_args="$(cargo_feature_args check)"
        run_shell_step \
            "Basic workspace consistency" \
            "" \
            "cd '$TEMPLATE_DIR' && cargo check $feature_args $scope_args --quiet"
    fi
}

print_summary() {
    local workspace_members
    local test_file_count
    local doc_file_count

    phase_banner "Summary"
    log_success "Local CI workflow completed successfully (selection: $ONLY_CHECK)"
    if [[ "$ONLY_CHECK" == "all" ]]; then
        workspace_members=$(grep -c 'members.*=' Cargo.toml || echo 'N/A')
        test_file_count=$(find . -name '*.rs' -exec grep -l '#\[test\]' {} ';' | wc -l)
        doc_file_count=$(find target/doc -name '*.html' 2>/dev/null | wc -l)
        log_info "Project statistics"
        echo "  Workspace members: $workspace_members"
        echo "  Test files: $test_file_count"
        echo "  Documentation: $doc_file_count HTML files generated"
    fi
}

main() {
    parse_args "$@"
    phase_banner "DEOS local CI workflow"
    check_prerequisites
    run_primary_checks
    run_additional_checks
    print_summary
}

run_entrypoint() {
    if [[ "${1:-}" == "--internal" ]]; then
        shift
        main "$@"
        return
    fi
    local arg
    for arg in "$@"; do
        if [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
            main "$@"
            return
        fi
    done
    local script_path
    script_path="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
    run_command_step "DEOS local CI" "" "$script_path" --internal "$@"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_entrypoint "$@"
fi
