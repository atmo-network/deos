#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ONLY_CHECK="all"
FIX_FORMAT=0
TARGET_PACKAGE=""
TEST_FILTER=""
TEST_TARGET=""
IGNORED_TEST=0
SELF_TEST=0
RELEASE_TESTS=0
FEATURE_MODE="auto"
CUSTOM_FEATURES=""
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
  --test-target NAME Select one integration test; requires --only tests --package
  --ignored          Run one exact ignored test; requires --only tests --package
                     and --test-filter with its complete test name; empty selection fails
  --self-test        Check argument/command contracts without running Cargo
  --release          Use optimized tests; requires --only tests
  --all-features     Enable all Cargo features (Clippy default)
  --default-features Use package default features, including for Clippy
  --features LIST    Enable a comma-separated NAME or PACKAGE/NAME list,
                     retaining Cargo defaults; names use ASCII letters, digits,
                     underscores and hyphens (not a leading hyphen)
                     Feature selectors are exclusive/nonrepeated; not for format
  --fix              Apply formatting; requires --only format
  -h, --help         Show this help message

Environment:
  DEOS_TEST_PROFILE=release  Run the workspace tests in the optimized profile
  DEOS_TEST_TIMEOUT_MINUTES=N  Test step deadline in minutes (default: 15)
  SKIP_WASM_BUILD=0|1
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

select_feature_mode() {
    if [[ "$FEATURE_MODE" != "auto" ]]; then
        log_error "Cargo feature selectors are mutually exclusive and cannot repeat"
        exit 2
    fi
    FEATURE_MODE="$1"
}

parse_args() {
    local argument_count=$#
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
            --test-target)
                if [[ $# -lt 2 || -z "$2" ]]; then
                    log_error "--test-target requires a Cargo integration test name"
                    exit 2
                fi
                TEST_TARGET="$2"
                shift 2
                continue
                ;;
            --ignored)
                (( IGNORED_TEST == 0 )) || { log_error "Duplicate --ignored"; exit 2; }
                IGNORED_TEST=1
                ;;
            --self-test)
                SELF_TEST=1
                ;;
            --release)
                RELEASE_TESTS=1
                ;;
            --all-features)
                select_feature_mode all
                ;;
            --default-features)
                select_feature_mode default
                ;;
            --features)
                if [[ $# -lt 2 ]]; then
                    log_error "--features requires a comma-separated feature list"
                    exit 2
                fi
                local feature_pattern='[A-Za-z0-9_][A-Za-z0-9_-]*(/[A-Za-z0-9_][A-Za-z0-9_-]*)?'
                if [[ ! "$2" =~ ^${feature_pattern}(,${feature_pattern})*$ ]]; then
                    log_error "--features accepts comma-separated NAME or PACKAGE/NAME entries"
                    exit 2
                fi
                select_feature_mode custom
                CUSTOM_FEATURES="$2"
                shift 2
                continue
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
    if (( SELF_TEST == 1 && argument_count != 1 )); then
        log_error "--self-test must be used alone"
        exit 2
    fi
    if [[ -n "$TEST_TARGET" ]]; then
        if [[ "$ONLY_CHECK" != "tests" || -z "$TARGET_PACKAGE" ]]; then
            log_error "--test-target requires --only tests and --package"
            exit 2
        fi
        if [[ ! "$TEST_TARGET" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]*$ ]]; then
            log_error "--test-target accepts a simple Cargo integration test name"
            exit 2
        fi
    fi
    if (( IGNORED_TEST == 1 )) && { [[ "$ONLY_CHECK" != "tests" || -z "$TARGET_PACKAGE" ]] || [[ ! "$TEST_FILTER" =~ ^[A-Za-z_][A-Za-z0-9_:]*$ ]]; }; then
        log_error "--ignored requires --only tests, --package and a complete --test-filter name"
        exit 2
    fi
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
    if [[ "$FEATURE_MODE" != "auto" && "$ONLY_CHECK" == "format" ]]; then
        log_error "Cargo feature selectors do not apply to formatting"
        exit 2
    fi
    if (( RELEASE_TESTS == 1 )) && [[ "$ONLY_CHECK" != "tests" ]]; then
        log_error "--release requires --only tests"
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
    elif [[ "$FEATURE_MODE" == "custom" ]]; then
        # The restricted grammar excludes quotes; preserve the list as one Cargo argument.
        printf '%s' "--features '$CUSTOM_FEATURES'"
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
        local feature_args profile_args="" target_args=""
        feature_args="$(cargo_feature_args tests)"
        if [[ -n "$TEST_TARGET" ]]; then
            target_args=" --test '$TEST_TARGET'"
        fi
        # `DEOS_TEST_PROFILE=release` lets the composed validation profiles run the workspace tests
        # in the optimized profile without the scoped-only `--release` flag; every assertion is
        # identical and the default stays the debug profile.
        if (( RELEASE_TESTS == 1 )) || [[ "${DEOS_TEST_PROFILE:-}" == "release" ]]; then
            profile_args="--release"
        fi
        local command="cargo test --locked $profile_args $feature_args $scope_args$target_args$test_filter"
        if (( IGNORED_TEST == 1 )); then
            # Use the same profile/features/target for discovery and execution; zero tests is not acceptance.
            command="listing=\$($command -- --ignored --exact --list) || exit \$?
if [[ \$(printf '%s\\n' \"\$listing\" | grep -Fxc -- '$TEST_FILTER: test' || true) != 1 ]]; then
    printf '%s\\n' 'Expected exactly one ignored test: $TEST_FILTER' >&2
    exit 1
fi
$command -- --ignored --exact --nocapture"
        fi
        run_shell_step "Tests" \
            "${DEOS_TEST_TIMEOUT_MINUTES:-15}" \
            "$command"
    fi

    if selected docs; then
        local feature_args
        feature_args="$(cargo_feature_args docs)"
        run_shell_step "Documentation Build" \
            "15" \
            "cargo doc --locked $feature_args $scope_args --no-deps"
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
            "cd '$TEMPLATE_DIR' && cargo check --locked $feature_args $scope_args --quiet"
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

self_test() (
    local captured_command=""
    run_shell_step() {
        case "$1" in
            Tests) [[ "$2" =~ ^[0-9]+$ ]] || exit 1 ;;
            'Clippy (Linting)') [[ "$2" == 30 ]] || exit 1 ;;
            'Documentation Build') [[ "$2" == 15 ]] || exit 1 ;;
            'Basic workspace consistency') [[ -z "$2" ]] || exit 1 ;;
            *) exit 1 ;;
        esac
        captured_command="$3"
    }
    reset_test_args() {
        ONLY_CHECK="all"
        FIX_FORMAT=0
        TARGET_PACKAGE=""
        TEST_FILTER=""
        TEST_TARGET=""
        IGNORED_TEST=0
        SELF_TEST=0
        RELEASE_TESTS=0
        FEATURE_MODE="auto"
        CUSTOM_FEATURES=""
        captured_command=""
    }
    expect_command() {
        local expected="$1"
        shift
        reset_test_args
        parse_args "$@"
        run_primary_checks
        run_additional_checks
        [[ "$captured_command" == "$expected" ]] || {
            log_error "Unexpected test command: $captured_command"
            exit 1
        }
    }
    expect_refusal() {
        reset_test_args
        if (parse_args "$@") >/dev/null 2>&1; then
            log_error "Expected argument refusal: $*"
            exit 1
        fi
    }
    # The default expectations assert the debug-profile shape, so the profile knob is cleared
    # here and exercised explicitly below.
    unset DEOS_TEST_PROFILE
    expect_command "cargo test --locked   --workspace" --only tests
    DEOS_TEST_PROFILE=release expect_command "cargo test --locked --release  --workspace" --only tests
    expect_command "cargo test --locked   --package 'pallet-deos-actors' 'behavior_runtime'" \
        --only tests --package pallet-deos-actors --test-filter behavior_runtime
    expect_command "cargo test --locked   --package 'pallet-deos-actors' --test 'current_state_semantic_oracle'" \
        --only tests --package pallet-deos-actors --test-target current_state_semantic_oracle --default-features
    expect_command "cargo test --locked --release --all-features --package 'pallet-deos-actors' --test 'current_state_semantic_oracle' 'q1'" \
        --only tests --package pallet-deos-actors --test-target current_state_semantic_oracle --test-filter q1 --release --all-features
    expect_command "cargo test --locked --release --features 'runtime-benchmarks,deos-runtime/try-runtime' --package 'deos-runtime' --test 'signed_wide_close' 'profile'" \
        --only tests --package deos-runtime --test-target signed_wide_close --test-filter profile --release \
        --features runtime-benchmarks,deos-runtime/try-runtime
    expect_command "cargo clippy --all-targets --all-features --locked --workspace --quiet -- -D warnings" --only clippy
    expect_command "cargo clippy --all-targets  --locked --workspace --quiet -- -D warnings" --only clippy --default-features
    expect_command "cargo clippy --all-targets --features 'runtime-benchmarks' --locked --package 'deos-runtime' --quiet -- -D warnings" \
        --only clippy --package deos-runtime --features runtime-benchmarks
    expect_command "cargo doc --locked --features 'runtime-benchmarks' --package 'deos-runtime' --no-deps" \
        --only docs --package deos-runtime --features runtime-benchmarks
    expect_command "cd '$TEMPLATE_DIR' && cargo check --locked --features 'runtime-benchmarks' --package 'deos-runtime' --quiet" \
        --only check --package deos-runtime --features runtime-benchmarks
    local option features
    for option in --all-features --default-features; do
        expect_refusal --only tests "$option" "$option"
        expect_refusal --only tests "$option" --features runtime-benchmarks
        expect_refusal --only tests --features runtime-benchmarks "$option"
        expect_refusal --only format "$option"
    done
    expect_refusal --only tests --all-features --default-features
    expect_refusal --only tests --default-features --all-features
    expect_refusal --only tests --features runtime-benchmarks --features runtime-benchmarks
    expect_refusal --only format --features runtime-benchmarks
    expect_refusal --features runtime-benchmarks --only format
    expect_refusal --only tests --features
    expect_refusal --only tests --features --all-features
    for features in '' '-flag' ',a' 'a,' 'a,,b' '/a' 'a/' 'a/b/c' 'a b' 'a;false' "a'b" 'a$(false)' $'a\nb'; do
        expect_refusal --only tests --features "$features"
    done
    expect_refusal --only tests --package pallet-deos-actors --test-target
    expect_refusal --only tests --package pallet-deos-actors --test-target ""
    expect_refusal --only tests --package pallet-deos-actors --test-target --release
    expect_refusal --only tests --test-target current_state_semantic_oracle
    expect_refusal --package pallet-deos-actors --test-target current_state_semantic_oracle
    expect_refusal --only check --package pallet-deos-actors --test-target current_state_semantic_oracle
    expect_refusal --only tests --package pallet-deos-actors --test-target '../oracle'
    expect_refusal --only tests --package pallet-deos-actors --test-target "oracle'; touch unsafe"
    expect_refusal --self-test --only tests
    expect_refusal --ignored
    expect_refusal --only tests --ignored --test-filter exact_test
    expect_refusal --only tests --package deos-runtime --ignored
    expect_refusal --only check --package deos-runtime --test-filter exact_test --ignored
    expect_refusal --only tests --package deos-runtime --test-filter exact_test --ignored --ignored
    expect_refusal --only tests --package deos-runtime --test-filter '-flag' --ignored
    expect_refusal --only tests --package deos-runtime --test-filter exact_test --ignored=true

    # Execute only a private stub to falsify empty/duplicate/list-failure acceptance and argv drift.
    local fixture profile='tests::bridge::exact_ignored'
    fixture="$(mktemp -d)"
    trap 'rm -rf -- "$fixture"' EXIT
    cat >"$fixture/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${*: -1}" == --list ]]; then
    printf '%s\n' "$@" >"$DEOS_CI_STUB_DISCOVERY"
    [[ "$*" == *' -- --ignored --exact --list' ]] || exit 41
    case "$DEOS_CI_STUB_SELECTION" in
        one) printf '%s: test\n' "$DEOS_CI_STUB_NAME" ;;
        missing) printf 'other::%s: test\n' "$DEOS_CI_STUB_NAME" ;;
        duplicate) printf '%s: test\n%s: test\n' "$DEOS_CI_STUB_NAME" "$DEOS_CI_STUB_NAME" ;;
        failed) exit 42 ;;
        *) exit 43 ;;
    esac
else
    printf '%s\n' "$@" >"$DEOS_CI_STUB_EXECUTION"
fi
STUB
    chmod +x "$fixture/cargo"
    local command selection
    local -a feature_args=() actual=() expected=() discovery=()
    for features in '' 'runtime-benchmarks,deos-runtime/try-runtime'; do
        reset_test_args
        feature_args=(--default-features)
        expected=(test --locked)
        if [[ -n "$features" ]]; then
            feature_args=(--features "$features")
            expected+=(--features "$features")
        fi
        parse_args --only tests --package deos-runtime --test-filter "$profile" --ignored "${feature_args[@]}"
        run_primary_checks
        command="$captured_command"
        expected+=(--package deos-runtime "$profile" -- --ignored --exact)
        for selection in missing duplicate failed one; do
            rm -f "$fixture/executed" "$fixture/discovered"
            if PATH="$fixture:$PATH" DEOS_CI_STUB_SELECTION="$selection" DEOS_CI_STUB_NAME="$profile" \
                DEOS_CI_STUB_DISCOVERY="$fixture/discovered" DEOS_CI_STUB_EXECUTION="$fixture/executed" \
                bash -c "$command" >/dev/null 2>&1; then
                [[ "$selection" == one ]] || { log_error "Ignored selection accepted $selection"; exit 1; }
            else
                [[ "$selection" != one ]] || { log_error 'Valid ignored selection refused'; exit 1; }
            fi
            mapfile -t discovery <"$fixture/discovered"
            [[ "${discovery[*]}" == "${expected[*]} --list" && "${#discovery[@]}" == "$((${#expected[@]} + 1))" ]] || exit 1
            if [[ "$selection" == one ]]; then
                mapfile -t actual <"$fixture/executed"
                [[ "${actual[*]}" == "${expected[*]} --nocapture" && "${#actual[@]}" == "$((${#expected[@]} + 1))" ]] || exit 1
            else
                [[ ! -e "$fixture/executed" ]] || { log_error 'Refused discovery dispatched a test'; exit 1; }
            fi
        done
    done
    log_success "Local CI self-test passed (feature modes, command contracts, exact ignored discovery/execution argv and refusals; no real Cargo)"
)

main() {
    parse_args "$@"
    if (( SELF_TEST == 1 )); then
        self_test
        return
    fi
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
