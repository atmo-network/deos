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
HOST_DIR="$TEMPLATE_DIR/tools/pvf-validation"
HOST_TARGET="${CARGO_TARGET_DIR:-$HOST_DIR/target}"
HOST_TRIPLE=""

usage() {
    cat <<'EOF'
Usage: pvf-assurance.sh --fixtures DIR --para-id ID [OPTIONS]

Validate exported PVF candidates with the exact-tag SDK ValidationHost and pinned workers.

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
  Isolated locked host workspace at template/tools/pvf-validation (exact stable2609-2
  SDK commit; not the runtime registry graph); exported SCALE inputs and complete results;
  CARGO_TARGET_DIR may select an absolute host build/output directory (default: tool/target).
  The native compiler host triple is passed explicitly to Cargo and used for execution;
  ambient CARGO_BUILD_TARGET or Cargo build.target cannot select a stale native output.
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
    require_commands cargo rustc awk perl timeout sha256sum cut env
    [[ "$HOST_TARGET" == /* ]] || { log_error "CARGO_TARGET_DIR must be absolute"; exit 1; }
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
    build_host
    run_validation
    after="$(sha256sum "$WASM" | cut -d ' ' -f 1)"
    [[ "$before" == "$after" ]] || { log_error "PVF assurance changed selected Wasm"; exit 1; }
    log_success "PVF candidate assurance retained Wasm identity: $after"
}

native_host_triple() (
    cd "$HOST_DIR"
    rustc -vV | awk '/^host: / { print $2 }'
)

build_host() {
    HOST_TRIPLE="$(native_host_triple)" || return 1
    [[ "$HOST_TRIPLE" =~ ^[A-Za-z0-9_]+(-[A-Za-z0-9_]+)+$ ]] || {
        log_error "Cannot establish native PVF host target"; return 1;
    }
    (
        # An explicit target overrides both environment and Cargo build.target.
        cd "$HOST_DIR"
        run_command_step "Build exact-tag SDK PVF host" "" env SKIP_WASM_BUILD=1 \
            cargo build --locked --manifest-path "$HOST_DIR/Cargo.toml" \
            --target-dir "$HOST_TARGET" --target "$HOST_TRIPLE" -p deos-pvf-validation
    )
}

run_validation() {
    [[ -n "$HOST_TRIPLE" ]] || { log_error "PVF host build target was not established"; return 1; }
    local command=(timeout 450s "$HOST_TARGET/$HOST_TRIPLE/debug/deos-pvf-validation" "$WASM" "$FIXTURES" "$BIN_DIR" "$PARA_ID")
    (( INSECURE_LOCAL == 0 )) || command+=(--insecure-local)
    [[ -z "$CPU_TRACE" ]] || command+=(--cpu-trace "$CPU_TRACE")
    (( POSITIVE_ONLY == 0 )) || command+=(--positive-only)
    run_command_step "Validate candidates with real pinned workers" "" "${command[@]}"
}

self_test() (
    local fixture
    fixture="$(mktemp -d)"
    trap 'rm -rf "$fixture"' EXIT
    HOST_TARGET="$fixture/host target"
    native_host_triple() { printf '%s\n' x86_64-unknown-linux-gnu; }
    local execute_stub=0
    local -a captured_command=()
    check_prerequisites() { log_error "Self-test reached real prerequisites"; exit 1; }
    run_shell_step() { log_error "Self-test reached a build command"; exit 1; }
    run_command_step() {
        local label="$1"
        [[ -z "$2" ]] || exit 1
        shift 2
        case "$label" in
            "Build exact-tag SDK PVF host")
                [[ "$PWD" == "$HOST_DIR" ]] || exit 1
                printf '%s\n' "$@" > "$fixture/build.argv" ;;
            "Validate candidates with real pinned workers")
                captured_command=("$@")
                (( execute_stub == 0 )) || "${captured_command[@]}"
                ;;
            *) exit 1 ;;
        esac
    }
    build_host
    [[ "$(<"$fixture/build.argv")" == "$(printf '%s\n' env SKIP_WASM_BUILD=1 \
        cargo build --locked --manifest-path "$HOST_DIR/Cargo.toml" \
        --target-dir "$HOST_TARGET" --target x86_64-unknown-linux-gnu -p deos-pvf-validation)" ]] || {
        log_error "Host build did not select the isolated manifest/target"
        exit 1
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
        local -a expected=(timeout 450s "$HOST_TARGET/x86_64-unknown-linux-gnu/debug/deos-pvf-validation" \
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
    mkdir -p "$HOST_TARGET/debug" "$HOST_TARGET/x86_64-unknown-linux-gnu/debug"
    printf '#!/usr/bin/env bash\nprintf stale > %q\n' "$fixture/stale-executed" > "$HOST_TARGET/debug/deos-pvf-validation"
    printf '#!/usr/bin/env bash\nprintf native > %q\n' "$fixture/native-executed" > "$HOST_TARGET/x86_64-unknown-linux-gnu/debug/deos-pvf-validation"
    chmod +x "$HOST_TARGET/debug/deos-pvf-validation" "$HOST_TARGET/x86_64-unknown-linux-gnu/debug/deos-pvf-validation"
    (
        export CARGO_BUILD_TARGET=aarch64-apple-darwin
        build_host
        execute_stub=1
        expect_command 0 0 ""
        [[ -f "$fixture/native-executed" && ! -e "$fixture/stale-executed" ]]
        [[ "$(<"$fixture/build.argv")" == *$'--target\nx86_64-unknown-linux-gnu\n'* ]]
    ) || { log_error "Ambient target selected a stale PVF host"; exit 1; }
    reset_args
    parse_args --self-test
    (( SELF_TEST == 1 )) || exit 1
    log_success "PVF argv self-test passed (explicit native target, stale-output refusal, 5 routes, 10 refusals; no Cargo or workers)"
)

main "$@"
