#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

FIXTURES=""
PARA_ID=""
WASM="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
INSECURE_LOCAL=0
CPU_TRACE=""
POSITIVE_ONLY=0
SELF_TEST=0

usage() {
    cat <<'EOF'
Usage: pvf-assurance.sh --fixtures DIR --para-id ID [OPTIONS]

Validate exported PVF candidates with the current SDK ValidationHost and pinned workers.

Options:
  --fixtures DIR       Existing proof-bound params/PVD/block/expected-result export
  --para-id ID         Declared local candidate parachain identity (required)
  --wasm PATH          Explicit Wasm (default: canonical production-profile output)
  --insecure-local     Explicitly permit best-effort hardening for trusted local code
  --cpu-trace FILE     Create a new diagnostic JSONL file of SDK events and child CPU
  --positive-only      Require exactly one fixture; validate only its positive case
  --self-test          Check argument/argv contracts offline, without Cargo or workers
  -h, --help           Show this help

Inputs:
  Locked Cargo workspace; exported SCALE inputs and complete expected results;
  checksum-pinned prepare/execute workers selected through DEOS_BINARY_DIR;
  unrelated relay/Omni/bencher executables and the full-bundle marker are not required.
  Native SDK build prerequisites: C/C++ toolchain, Perl core modules and protoc.
  Standard PERL5LIB/PROTOC/LD_LIBRARY_PATH overrides may select an isolated toolchain.

Outputs:
  Default: typed pass/fail for each valid, corrupt-seal, parent-root and compact-proof
  case of every fixture; code identity, worker version and available security features.
  --positive-only: one complete positive check, no corrupt cases or negative-suite claim.
  Rust owns fixture-count admission before its worker probes, trace/cache and host startup;
  wrapper prerequisites/build still run. This flag is not a dry run.
  Optional trace: whole-job child user+system CPU only after exact positive acceptance;
  missing/ambiguous events fail closed. No complete record means incomplete assurance.
  This is not wall time, isolated root cost, a calibrated bound or a production tariff.

Side effects:
  Builds the offline host tool (Cargo may fetch locked build dependencies), creates
  an owned short-lived worker/cache directory and starts/stops local workers.
  No executable-bundle download, account, runtime source, Wasm, deployment or
  generated Weight mutation. Candidate execution has no network connection.
  --fixtures must already exist; generation is a separate public example or native export test.
  --cpu-trace never overwrites files; failed runs retain partial diagnostics.
  --self-test must be used alone; only its owned temporary fixtures and argv stubs run.
EOF
}

parse_args() {
    local argument_count=$#
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --fixtures|--para-id|--wasm|--cpu-trace)
                [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || {
                    log_error "$1 requires a value"
                    exit 1
                }
                case "$1" in
                    --fixtures) FIXTURES="$2" ;;
                    --para-id) PARA_ID="$2" ;;
                    --wasm) WASM="$2" ;;
                    --cpu-trace)
                        [[ -z "$CPU_TRACE" ]] || { log_error "Duplicate --cpu-trace"; exit 1; }
                        CPU_TRACE="$2" ;;
                esac
                shift 2 ;;
            --insecure-local) INSECURE_LOCAL=1; shift ;;
            --positive-only)
                (( POSITIVE_ONLY == 0 )) || { log_error "Duplicate --positive-only"; exit 1; }
                POSITIVE_ONLY=1; shift ;;
            --self-test)
                (( argument_count == 1 )) || { log_error "--self-test must be used alone"; exit 1; }
                SELF_TEST=1; shift ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
    done
    (( SELF_TEST == 0 )) || return 0
    [[ -n "$FIXTURES" && "$PARA_ID" =~ ^[0-9]+$ ]] || {
        log_error "--fixtures and a decimal --para-id are required"
        exit 1
    }
}

check_prerequisites() {
    hydrate_local_tool_paths
    require_commands cargo perl timeout sha256sum cut env
    require_directory "$FIXTURES" "Exported PVF fixtures"
    [[ -f "$WASM" ]] || { log_error "Selected Wasm is missing: $WASM"; exit 1; }
    perl -MFindBin -MIPC::Cmd -e 1 || {
        log_error "Native SDK compilation requires Perl core modules (FindBin and IPC::Cmd)"
        exit 1
    }
    local protoc="${PROTOC:-protoc}"
    command -v "$protoc" >/dev/null || { log_error "Native SDK compilation requires protoc or PROTOC"; exit 1; }
    env -u DEOS_OMNI_NODE_PATH -u DEOS_OMNI_NODE_SHA256 \
        "$SCRIPT_DIR/01-download-binaries.sh" --check \
        --only polkadot-prepare-worker --only polkadot-execute-worker
    FIXTURES="$(cd "$FIXTURES" && pwd)"
    WASM="$(cd "$(dirname "$WASM")" && pwd)/$(basename "$WASM")"
}

main() {
    parse_args "$@"
    if (( SELF_TEST == 1 )); then
        self_test
        return 0
    fi
    phase_banner "DEOS offline PVF assurance"
    check_prerequisites
    local before after
    before="$(sha256sum "$WASM" | cut -d ' ' -f 1)"
    run_shell_step "Build offline SDK PVF host" ""         "cd '$TEMPLATE_DIR' && SKIP_WASM_BUILD=1 cargo build --locked -p deos-pvf-validation"
    run_validation
    after="$(sha256sum "$WASM" | cut -d ' ' -f 1)"
    [[ "$before" == "$after" ]] || { log_error "PVF assurance changed selected Wasm"; exit 1; }
    log_success "PVF candidate assurance retained Wasm identity: $after"
}

run_validation() {
    local command=(timeout 450s "$TEMPLATE_DIR/target/debug/deos-pvf-validation" "$WASM" "$FIXTURES" "$BIN_DIR" "$PARA_ID")
    (( INSECURE_LOCAL == 0 )) || command+=(--insecure-local)
    [[ -z "$CPU_TRACE" ]] || command+=(--cpu-trace "$CPU_TRACE")
    (( POSITIVE_ONLY == 0 )) || command+=(--positive-only)
    run_command_step "Validate candidates with real pinned workers" "" "${command[@]}"
}

self_test() (
    local fixture
    fixture="$(mktemp -d)"
    trap 'rm -rf "$fixture"' EXIT
    local -a captured_command=()
    check_prerequisites() { log_error "Self-test reached real prerequisites"; exit 1; }
    run_shell_step() { log_error "Self-test reached a build command"; exit 1; }
    run_command_step() {
        [[ "$1" == "Validate candidates with real pinned workers" && -z "$2" ]] || exit 1
        shift 2
        captured_command=("$@")
    }
    reset_args() {
        FIXTURES=""
        PARA_ID=""
        WASM="$fixture/selected code.wasm"
        INSECURE_LOCAL=0
        CPU_TRACE=""
        POSITIVE_ONLY=0
        SELF_TEST=0
        captured_command=()
    }
    expect_command() {
        local positive="$1" insecure="$2" trace="$3"
        shift 3
        reset_args
        parse_args --fixtures "$fixture/candidates with spaces" --para-id 2000 \
            --wasm "$fixture/selected code.wasm" "$@"
        run_validation
        local -a expected=(timeout 450s "$TEMPLATE_DIR/target/debug/deos-pvf-validation" \
            "$fixture/selected code.wasm" "$fixture/candidates with spaces" "$BIN_DIR" 2000)
        (( insecure == 0 )) || expected+=(--insecure-local)
        [[ -z "$trace" ]] || expected+=(--cpu-trace "$trace")
        (( positive == 0 )) || expected+=(--positive-only)
        [[ "${#captured_command[@]}" == "${#expected[@]}" ]] || exit 1
        local index
        for index in "${!expected[@]}"; do
            [[ "${captured_command[$index]}" == "${expected[$index]}" ]] || {
                log_error "Unexpected argv token $index"
                exit 1
            }
        done
    }
    expect_refusal() {
        reset_args
        if (parse_args "$@") >/dev/null 2>&1; then
            log_error "Expected argument refusal: $*"
            exit 1
        fi
    }
    expect_command 0 0 ""
    expect_command 1 0 "" --positive-only
    expect_command 0 1 "$fixture/trace path" --insecure-local --cpu-trace "$fixture/trace path"
    expect_command 1 1 "$fixture/trace path" --cpu-trace "$fixture/trace path" --positive-only --insecure-local
    expect_command 1 0 "$fixture/trace path" --positive-only --cpu-trace "$fixture/trace path"
    expect_refusal --fixtures f --para-id 1 --positive-only --positive-only
    expect_refusal --fixtures f --para-id 1 --positive-only=true
    expect_refusal --fixtures f --para-id 1 --positive-only true
    expect_refusal --self-test --positive-only
    expect_refusal --positive-only --self-test
    expect_refusal --self-test --self-test
    expect_refusal --self-test --help
    expect_refusal --fixtures f --para-id 1 --cpu-trace
    expect_refusal --fixtures f --para-id 1 --cpu-trace a --cpu-trace b
    expect_refusal --fixtures f --para-id 1 --unknown
    reset_args
    parse_args --self-test
    (( SELF_TEST == 1 )) || exit 1
    log_success "PVF argv self-test passed (5 routes, 10 refusals; no Cargo or workers)"
)

main "$@"
