#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

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
  -h, --help        Show this help message

Environment:
  SKIP_WASM_BUILD=0|1
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
    require_commands cargo rustup timeout grep find wc
    log_info "Working from: $TEMPLATE_DIR"
    cd "$TEMPLATE_DIR"

    if ! cargo clippy --version &>/dev/null; then
        log_info "Installing clippy"
        rustup component add clippy
    fi

    log_success "Prerequisites checked"
}

run_primary_checks() {
    phase_banner "Step 2: Primary checks"

    run_shell_step "Clippy (Linting)" \
        "30" \
        "cargo clippy --all-targets --all-features --locked --workspace --quiet -- -D warnings"

    run_shell_step "Tests" \
        "15" \
        "cargo test --workspace"

    run_shell_step "Documentation Build" \
        "15" \
        "cargo doc --workspace --no-deps"
}

run_additional_checks() {
    phase_banner "Step 3: Additional checks"

    log_info "Checking code formatting"
    if cargo fmt -- --check; then
        log_success "Code formatting is correct"
    else
        log_warning "Code formatting issues found (run 'cargo fmt' to fix)"
    fi

    run_shell_step \
        "Basic workspace consistency" \
        "" \
        "cd '$TEMPLATE_DIR' && cargo check --workspace --quiet"
}

print_summary() {
    local workspace_members
    local test_file_count
    local doc_file_count

    workspace_members=$(grep -c 'members.*=' Cargo.toml || echo 'N/A')
    test_file_count=$(find . -name '*.rs' -exec grep -l '#\[test\]' {} ';' | wc -l)
    doc_file_count=$(find target/doc -name '*.html' 2>/dev/null | wc -l)

    phase_banner "Summary"
    log_success "Local CI workflow completed successfully"
    log_info "Project statistics"
    echo "  Workspace members: $workspace_members"
    echo "  Test files: $test_file_count"
    echo "  Documentation: $doc_file_count HTML files generated"
}

main() {
    parse_args "$@"
    phase_banner "DEOS local CI workflow"
    check_prerequisites
    run_primary_checks
    run_additional_checks
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
