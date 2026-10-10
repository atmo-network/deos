#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

FIXTURES=""
PARA_ID=""
WASM="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
INSECURE_LOCAL=0
CPU_TRACE=""

usage() {
    cat <<'EOF'
Usage: pvf-assurance.sh --fixtures DIR --para-id ID [OPTIONS]

Validate exported PVF candidates with the current SDK ValidationHost and pinned workers.

Options:
  --fixtures DIR       Existing actor_finalization_close/actor_retention_close export
  --para-id ID         Declared local candidate parachain identity (required)
  --wasm PATH          Explicit Wasm (default: canonical production-profile output)
  --insecure-local     Explicitly permit best-effort hardening for trusted local code
  --cpu-trace FILE     Create a new diagnostic JSONL file of SDK events and child CPU
  -h, --help           Show this help

Inputs:
  Locked Cargo workspace; exported SCALE inputs and complete expected results;
  checksum-pinned prepare/execute workers selected through DEOS_BINARY_DIR;
  unrelated relay/Omni/bencher executables and the full-bundle marker are not required.
  Native SDK build prerequisites: C/C++ toolchain, Perl core modules and protoc.
  Standard PERL5LIB/PROTOC/LD_LIBRARY_PATH overrides may select an isolated toolchain.

Outputs:
  Typed pass/fail for each valid, corrupt-seal, parent-root and compact-proof case;
  code identity, worker version and available security features.
  Optional trace: whole-job child user+system CPU only after exact positive acceptance;
  missing/ambiguous events fail closed. No complete record means incomplete assurance.
  This is not wall time, isolated root cost, a calibrated bound or a production tariff.

Side effects:
  Builds the offline host tool (Cargo may fetch locked build dependencies), creates
  an owned short-lived worker/cache directory and starts/stops local workers.
  No executable-bundle download, account, runtime source, Wasm, deployment or
  generated Weight mutation. Candidate execution has no network connection.
  --fixtures must already exist; candidate generation is a separate public example.
  --cpu-trace never overwrites files; failed runs retain partial diagnostics.
EOF
}

parse_args() {
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
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
    done
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
    phase_banner "DEOS offline PVF assurance"
    check_prerequisites
    local before after
    before="$(sha256sum "$WASM" | cut -d ' ' -f 1)"
    run_shell_step "Build offline SDK PVF host" ""         "cd '$TEMPLATE_DIR' && SKIP_WASM_BUILD=1 cargo build --locked -p deos-pvf-validation"
    local command=(timeout 450s "$TEMPLATE_DIR/target/debug/deos-pvf-validation" "$WASM" "$FIXTURES" "$BIN_DIR" "$PARA_ID")
    (( INSECURE_LOCAL == 0 )) || command+=(--insecure-local)
    [[ -z "$CPU_TRACE" ]] || command+=(--cpu-trace "$CPU_TRACE")
    run_command_step "Validate candidates with real pinned workers" "" "${command[@]}"
    after="$(sha256sum "$WASM" | cut -d ' ' -f 1)"
    [[ "$before" == "$after" ]] || { log_error "PVF assurance changed selected Wasm"; exit 1; }
    log_success "PVF candidate assurance retained Wasm identity: $after"
}

main "$@"
