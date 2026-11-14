#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CACHE="${DEOS_NODE_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/deos-native-convergence}"
SOURCE=
COMMIT=9448fe4f252936d241b8d2663506813b884f94f3
TREE=97a1f73479286fdbd94e4bed0c5062999fab3073
LOCK=f12148367523411902f1cceade3d28d14b94101049dce41f5eb9644775bc0356
LEAF="$SCRIPT_DIR/build-corrected-node.mjs"
MODE= WORK= EVIDENCE=

usage() {
    cat <<'HELP'
Usage: build-corrected-node.sh --prepare --sdk-source SDK_PATH --work-dir NEW_PATH
       build-corrected-node.sh --explore --sdk-source SDK_PATH --work-dir NEW_PATH
       build-corrected-node.sh --source-preflight --sdk-source COMPOSED_SDK --work-dir NEW_EVIDENCE_PATH
       build-corrected-node.sh --freeze-existing --work-dir COMPOSED_PATH --source-evidence PREFLIGHT_PATH
       build-corrected-node.sh --build --work-dir PREPARED_PATH
       build-corrected-node.sh --self-test|--help

Build-only diagnostic corrected Omni route; never installs or selects a node.
--prepare exclusively creates a new workspace beneath
  DEOS_NODE_CACHE_DIR (default: $XDG_CACHE_HOME/deos-native-convergence or
  $HOME/.cache/deos-native-convergence); offline copies SDK stable2609-2,
  applies the reviewed checked-decrement correction and workspace-root Crypto,
  Hickory, roots and str0m-openssl 0.6.0 IoBuffer patches plus coupled
  Wasmtime 36.0.20/Cranelift 0.123.20,
  retains nine exact compatible SDK dependency-parent selections, verifies
  the accepted graph modulo the coupled compiler patches, and freezes source,
  isolated lock, graph evidence and script hashes. No compile.
  Resolution is offline; missing cached dependencies fail. No broad SDK update is made;
  prepared.json exposes intersecting versions for separate security review.
--explore first verifies the old010/provider baseline, then composes the
  reviewed DNS/TLS/HTTP, IO/cache/build and exact tracing 0.3.20 remedies.
  It leaves a mutable, unqualified full SDK without prepared.json or build
  admission. --prepare currently refuses final freeze pending the independently
  approved current Native source-role guard and paired audit; exact composed
  source acceptance alone is not admission. It never freezes the old graph as
  the new one.
--source-preflight reads an existing exact accepted composition (SDK plus sibling
  vendor), verifies fresh offline graphs and fixed compiler/configuration, and
  records full selected consumer/codegen inventories in a NEW evidence directory.
  It writes no source, prepared.json, build receipt or ELF and clears no finding.
  Its inventories are unreviewed evidence, not an installed Native operation proof.
--freeze-existing technically freezes the SAME existing unqualified composed
  workspace using fresh source-preflight evidence for that exact source root.
  It rechecks offline graphs, consumer inputs, refs, configuration and current
  producers before publishing a distinct source-only prepared manifest. It never
  reconstructs source or overwrites prepared work. This is NOT security approval,
  Native finding clearance or build admission; independent review is required
  before invocation. Skills/private acceptance helpers are not dependencies.
--build validates the matching legacy or composed frozen manifest, then runs one Cargo build with Rust
  1.94.1, jobs 2, profile production (fat LTO/cgu1/opt3/unwind), features
  runtime-benchmarks, SDK Wasm enabled. Requires explicit later admission by
  the caller; preparation is not security approval or artifact qualification.
  Each explicitly admitted invocation creates an exclusive numbered attempt
  with separate logs, environment and exit status. No concurrent build or
  rebuild after success. Failed attempts remain intact; changed frozen inputs
  require a new workspace, not an overwritten manifest. Retry only after an
  evidenced correction; no automatic retry or acceptance is performed.
  Only a canonical decimal status 1..255 followed by exactly one LF proves
  failure. Missing, truncated, malformed or zero receipts refuse retry.

Inputs: explicitly supplied exact read-only SDK Git checkout and reviewed
  template/vendor files. Neither original source nor canonical graph changes.
Outputs: workspace/sdk, vendor, target, baseline/candidate metadata/tree/lock
  evidence, prepared.json and full persistent Cargo logs. Future binary:
  WORK/target/production/polkadot-omni-node. No receipt, /bin or canonical output.
Build prerequisites: Linux x86_64, Rust 1.94.1 incl wasm32-unknown-unknown,
  gcc/g++, cmake, bindgen libclang, protoc and Perl. PROTOC, PERL5LIB,
  LD_LIBRARY_PATH and LIBCLANG_PATH accept explicit per-command inputs;
  DEOS_NATIVE_PREREQUISITES defaults to the existing user-cache prerequisites.
  Native tool versions, environment and full verbose Cargo output are retained.
  No privileged installation is performed.
Ambient Rust/profile overrides are rejected; SKIP_WASM_BUILD is unset.
This source profile follows SDK Cargo.toml and release-20_build-rc.yml Omni
  features, not published-binary equivalence. Main DEOS Wasm is untouched.
--self-test uses isolated tiny fixtures only; no SDK copy/resolve/build.
HELP
}

fail() { log_error "$*" >&2; exit 1; }

parse_args() {
    while (($#)); do
        case "$1" in
            --help|-h) [[ $# == 1 && -z "$MODE$WORK$SOURCE$EVIDENCE" ]] || fail '--help is exclusive'; usage; exit 0 ;;
            --prepare|--explore|--source-preflight|--freeze-existing|--build|--self-test) [[ -z "$MODE" ]] || fail 'Choose exactly one mode'; MODE="$1"; shift ;;
            --work-dir) [[ $# -ge 2 && -n "$2" && -z "$WORK" ]] || fail '--work-dir requires one path'; WORK="$2"; shift 2 ;;
            --source-evidence) [[ $# -ge 2 && -n "$2" && -z "$EVIDENCE" ]] || fail '--source-evidence requires one path'; EVIDENCE="$2"; shift 2 ;;
            --sdk-source) [[ $# -ge 2 && -n "$2" && -z "$SOURCE" ]] || fail '--sdk-source requires one path'; SOURCE="$2"; shift 2 ;;
            *) fail "Unknown argument: $1" ;;
        esac
    done
    [[ -n "$MODE" ]] || fail 'Choose --prepare, --build or --self-test'
    if [[ "$MODE" == --self-test ]]; then [[ -z "$WORK$SOURCE$EVIDENCE" ]] || fail '--self-test is exclusive'; return; fi
    if [[ "$MODE" == --freeze-existing ]]; then
        [[ "$EVIDENCE" == /* && -d "$EVIDENCE" && "$(realpath "$EVIDENCE")" == "$EVIDENCE" ]] || fail 'Freeze requires canonical --source-evidence'
    else
        [[ -z "$EVIDENCE" ]] || fail '--source-evidence is freeze-existing-only'
    fi
    if [[ "$MODE" == --prepare || "$MODE" == --explore || "$MODE" == --source-preflight ]]; then
        [[ "$SOURCE" == /* && -d "$SOURCE" && "$(realpath "$SOURCE")" == "$SOURCE" ]] || fail 'Source modes require a canonical --sdk-source directory'
    else
        [[ -z "$SOURCE" ]] || fail '--sdk-source is prepare-only'
    fi
    [[ "$WORK" == "$CACHE/"* && "$WORK" != "$CACHE/" && "$WORK" != *$'\n'* ]] || fail 'Workspace must be a new absolute path beneath the owned cache'
    [[ "$(realpath -m -- "$WORK")" == "$WORK" ]] || fail 'Workspace must be canonical, without symlink or dot components'
}

git_read() { git -c protocol.allow=never --no-optional-locks "$@"; }

check_prerequisites() {
    require_commands node git cargo rustup sha256sum realpath cp tee diff cmp stat
    [[ "$MODE" != --self-test ]] || return 0
    local name
    for name in RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTC CARGO_BUILD_TARGET CARGO_HOME RUSTUP_TOOLCHAIN; do
        [[ -z "${!name:-}" ]] || fail "Unset ambient override $name"
    done
    while IFS= read -r name; do
        case "$name" in CARGO_PROFILE_*|CARGO_TARGET_*_RUSTFLAGS|WASM_BUILD_RUSTFLAGS|WASM_BUILD_NO_COLOR|SKIP_BUILD_STD) fail "Unset ambient override $name" ;; esac
    done < <(compgen -e)
    [[ "$(rustup run 1.94.1 rustc --version)" == 'rustc 1.94.1 '* ]] || fail 'Rust 1.94.1 required'
    [[ -z "${CARGO_BUILD_RUSTFLAGS:-}${WASM_BUILD_STD:-}${WASM_BUILD_TARGET:-}${WASM_BUILD_TYPE:-}" ]] || fail 'Unset ambient build overrides'
    if [[ "$MODE" == --freeze-existing ]]; then
        [[ -d "$WORK" && ! -L "$WORK" && ! -e "$WORK/prepared.json" && ! -L "$WORK/prepared.json" ]] || fail 'Freeze requires existing unprepared workspace'
        return
    fi
    if [[ "$MODE" == --source-preflight ]]; then
        [[ ! -e "$WORK" && ! -L "$WORK" ]] || fail 'Refusing existing preflight evidence'
        return
    fi
    if [[ "$MODE" == --prepare || "$MODE" == --explore ]]; then
        [[ "$MODE" != --prepare ]] || fail 'Current Native source-role guard and paired audit admission pending; use --source-preflight, no final freeze/build admission'
        [[ "$(git_read -C "$SOURCE" rev-parse HEAD)" == "$COMMIT" ]] || fail 'SDK commit mismatch'
        [[ "$(git_read -C "$SOURCE" rev-parse HEAD^{tree})" == "$TREE" ]] || fail 'SDK tree mismatch'
        [[ "$(sha256sum "$SOURCE/Cargo.lock" | cut -d' ' -f1)" == "$LOCK" ]] || fail 'SDK original lock mismatch'
        [[ ! -e "$WORK" && ! -L "$WORK" ]] || fail 'Refusing existing workspace'
    else
        [[ "$(git_read -C "$WORK/sdk" rev-parse HEAD)" == "$COMMIT" ]] || fail 'Prepared SDK HEAD changed'
        [[ "$(git_read -C "$WORK/sdk" show-ref | sha256sum | cut -d' ' -f1)" == "$(sha256sum "$WORK/git-refs.txt" | cut -d' ' -f1)" ]] || fail 'Prepared source version refs changed'
        [[ -f "$WORK/.corrected-node-owner" && -f "$WORK/prepared.json" ]] || fail 'Not a prepared owned workspace'
        node "$LEAF" verify "$WORK"
    fi
}

# Full child output is retained even on failure; no success-log cleanup applies.
logged() {
    local log="$1"; shift
    [[ ! -e "$log" ]] || fail "Refusing existing log: $log"
    if [[ "${DEOS_VERBOSE:-0}" == 1 ]]; then
        "$@" 2>&1 | tee "$log"
    else
        if "$@" >"$log" 2>&1; then log_info "Completed; full log: $log";
        else local status=$?; tail -n 80 "$log" >&2; return "$status"; fi
    fi
}

cargo_graph() {
    local label="$1"
    cargo +1.94.1 metadata --offline --format-version 1 --locked --features polkadot-omni-node/runtime-benchmarks >"$WORK/$label-metadata.json" 2>"$WORK/$label-metadata.log"
    logged "$WORK/$label-tree.log" cargo +1.94.1 tree --offline --locked -p polkadot-omni-node --features runtime-benchmarks --edges normal,build --prefix depth --format '{p}|{f}'
    cp "$WORK/$label-tree.log" "$WORK/$label-tree.txt"
    node "$LEAF" graph "$WORK" "$label"
}

prepare() {
    if [[ ! -e "$CACHE" ]]; then mkdir "$CACHE"; fi
    [[ -d "$CACHE" && ! -L "$CACHE" && "$(realpath "$CACHE")" == "$CACHE" ]] || fail 'Unsafe cache root'
    # No recursive mkdir: parents must already be owned/canonical; creation is exclusive.
    mkdir "$WORK"
    printf 'deos-corrected-node-v1\n' >"$WORK/.corrected-node-owner"
    logged "$WORK/clone.log" git -c protocol.allow=never -c protocol.file.allow=always --no-optional-locks clone --local --no-hardlinks --no-checkout "$SOURCE" "$WORK/sdk"
    logged "$WORK/checkout.log" git_read -C "$WORK/sdk" checkout --detach "$COMMIT"
    [[ "$(git_read -C "$WORK/sdk" rev-parse HEAD^{tree})" == "$TREE" ]] || fail 'Copied tree mismatch'
    git_read -C "$WORK/sdk" show-ref >"$WORK/git-refs.txt"
    cp "$WORK/sdk/Cargo.lock" "$WORK/baseline.lock"
    mkdir "$WORK/vendor"
    local pkg
    for pkg in crypto-bigint hickory-proto webpki-roots sp-api-proc-macro str0m-openssl; do
        cp -a "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg"
        diff -qr "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg" >/dev/null || fail "Copy changed: $pkg"
    done
    cd "$WORK/sdk"
    cargo_graph baseline
    node "$LEAF" patch "$WORK/sdk" "$WORK/vendor"
    logged "$WORK/resolve.log" cargo +1.94.1 update --offline -p crypto-bigint@0.5.2 --precise 0.5.5
    logged "$WORK/resolve-hickory.log" cargo +1.94.1 update --offline -p hickory-proto@0.24.1 --precise 0.24.4
    logged "$WORK/resolve-roots.log" cargo +1.94.1 update --offline -p webpki-roots@0.25.2 --precise 0.25.4
    logged "$WORK/resolve-wasmtime.log" cargo +1.94.1 update --offline -p wasmtime@36.0.7 --precise 36.0.20
    node "$LEAF" retain-parents "$WORK"
    node "$LEAF" patch-provider "$WORK/sdk" "$WORK/vendor"
    logged "$WORK/resolve-provider.log" cargo +1.94.1 update --offline -p str0m-openssl@0.6.0
    cargo_graph candidate
    cp Cargo.lock "$WORK/candidate-before-sync.lock"
    logged "$WORK/offline-sync.log" cargo +1.94.1 update --workspace --offline
    cmp Cargo.lock "$WORK/candidate-before-sync.lock" || fail 'Offline workspace sync changed candidate lock'
    git_read -C "$WORK/sdk" diff -- Cargo.lock >"$WORK/lock-delta.patch"
    if [[ "$MODE" == --explore ]]; then
        compose_sources
        log_success "Exploratory composed source only; mutable/unqualified, no freeze or build: $WORK"
        return
    fi
    node "$LEAF" freeze "$WORK" "$SCRIPT_DIR/build-corrected-node.sh" "$SCRIPT_DIR/_common.sh"
    node "$LEAF" verify "$WORK"
    log_success "Prepared only (no build): $WORK/prepared.json"
}

compose_sources() {
    local pkg spec name version
    cp Cargo.lock "$WORK/provider-baseline.lock"
    for pkg in hickory-proto-026 lru-0.7.8 lru-0.12.3 memmap2-0.5.10; do
        cp -a "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg"
        diff -qr "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg" >/dev/null || fail "Copy changed: $pkg"
    done
    node "$LEAF" compose-patches "$WORK"
    # Exact selections, not a broad update. Unlock coupled CXX build first.
    for spec in hickory-resolver@0.26.1=0.26.2 hickory-net@0.26.1=0.26.2 rustls@0.23.31=0.23.46 rustls-webpki@0.103.4=0.103.16 h2@0.4.5=0.4.16 rustls-native-certs@0.8.0=0.8.1 tokio@1.45.0=1.53.3 rustix@1.0.8=1.1.6 crossbeam-epoch@0.9.15=0.9.20 anyhow@1.0.98=1.0.103 cxx-build@1.0.106=1.0.196 cxx@1.0.106=1.0.196 memmap2@0.9.3=0.9.11 zstd@0.13.3=0.13.4; do
        name="${spec%=*}"; version="${spec#*=}"
        # A preceding coupled update may already have selected this exact fix.
        if ! cargo +1.94.1 pkgid --offline -p "$name" >/dev/null 2>&1; then
            name="${name%@*}@$version"
            cargo +1.94.1 pkgid --offline -p "$name" >/dev/null
        fi
        logged "$WORK/compose-${spec%=*}.log" cargo +1.94.1 update --offline -p "$name" --precise "$version"
    done
    # Optional full-workspace AWS constraints, not Linux provider activation.
    logged "$WORK/compose-aws-rs.log" cargo +1.94.1 update --offline -p aws-lc-rs --precise 1.18.0
    logged "$WORK/compose-aws-sys.log" cargo +1.94.1 update --offline -p aws-lc-sys --precise 0.44.0
    node "$LEAF" compose-minimums "$WORK"
    logged "$WORK/compose-tracing.log" cargo +1.94.1 update --offline -p tracing-subscriber@0.3.19 --precise 0.3.20
    node "$LEAF" compose-retain "$WORK"
    cargo +1.94.1 metadata --offline --locked --format-version 1 --features polkadot-omni-node/runtime-benchmarks >"$WORK/composed-metadata.json" 2>"$WORK/composed-metadata.log"
    logged "$WORK/composed-tree.log" cargo +1.94.1 tree --offline --locked -p polkadot-omni-node --features runtime-benchmarks --edges normal,build --prefix depth --format '{p}|{f}'
    cp "$WORK/composed-tree.log" "$WORK/composed-tree.txt"
    node "$LEAF" graph "$WORK" composed
    node "$LEAF" compose-report "$WORK"
    git_read -C "$WORK/sdk" diff -- Cargo.toml Cargo.lock >"$WORK/composed-selection.patch"
}

known_failed_attempt() {
    local status bytes
    [[ -f "$1" && ! -L "$1" ]] || return 1
    # The writer emits one canonical decimal status plus LF. Bound bytes before
    # reading: Bash read skips NULs, so its character limit alone is insufficient.
    bytes="$(stat --format=%s -- "$1")" || return 1
    [[ "$bytes" =~ ^[234]$ ]] || return 1
    IFS= read -r -n 4 status <"$1" || return 1
    [[ "$status" =~ ^([1-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])$ ]] || return 1
    cmp -s -- "$1" <(printf '%s\n' "$status")
}

admit_attempt() {
    [[ ! -e "$WORK/binary.sha256" ]] || fail 'Legacy completed build; prepare new inputs instead'
    mkdir "$WORK/build-admission" || fail 'Build active or interrupted; inspect its evidence before recovery'
    trap 'status=$?; if [[ -n "${ATTEMPT:-}" ]]; then (set -o noclobber; printf "%s\n" "$status" >"$ATTEMPT/exit-status"); fi; rmdir "$WORK/build-admission"' EXIT
    local number=1 prior next
    mkdir -p "$WORK/attempts"
    for prior in "$WORK"/attempts/*; do
        [[ -d "$prior" ]] || continue
        known_failed_attempt "$prior/exit-status" || fail 'Previous attempt succeeded or has unknown outcome; no redundant rebuild'
        ((number+=1))
    done
    printf -v next '%s/attempts/%04d' "$WORK" "$number"
    mkdir "$next" || fail 'Refusing existing attempt'
    ATTEMPT="$next"
    cp "$WORK/prepared.json" "$ATTEMPT/frozen-inputs.json"
}

build() (
    require_commands gcc g++ perl cmake
    [[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || fail 'This diagnostic build route currently qualifies Linux x86_64 only'
    rustup target list --installed --toolchain 1.94.1 | grep -qx wasm32-unknown-unknown || fail 'Install Rust 1.94.1 wasm32-unknown-unknown first'
    local prerequisites="${DEOS_NATIVE_PREREQUISITES:-${XDG_CACHE_HOME:-$HOME/.cache}/deos-native-prerequisites}"
    export PROTOC="${PROTOC:-$prerequisites/root/usr/bin/protoc}"
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-$prerequisites/root/usr/lib64}"
    export PERL5LIB="${PERL5LIB:-$prerequisites/root-fc45/usr/lib64/perl5:$prerequisites/root-fc45/usr/lib64/perl5/vendor_perl:$prerequisites/root/usr/share/perl5:$prerequisites/root/usr/share/perl5/vendor_perl}"
    export LIBCLANG_PATH="${LIBCLANG_PATH:-/usr/lib64}"
    [[ -x "$PROTOC" && -d "$LIBCLANG_PATH" ]] || fail 'Missing declared native prerequisites'
    "$PROTOC" --version
    perl -MFindBin -MIPC::Cmd -e 1
    local ATTEMPT=
    admit_attempt
    cd "$WORK/sdk"
    { rustup run 1.94.1 rustc -Vv; cargo +1.94.1 --version; gcc --version; g++ --version; cmake --version; "$PROTOC" --version; } >"$ATTEMPT/build-tools.txt" 2>&1
    node "$LEAF" environment "$WORK" "$ATTEMPT"
    logged "$ATTEMPT/build.log" cargo +1.94.1 build --verbose --locked --profile production -p polkadot-omni-node --bin polkadot-omni-node --features runtime-benchmarks -j 2
    node "$LEAF" verify "$WORK"
    sha256sum "$WORK/target/production/polkadot-omni-node" | tee "$ATTEMPT/binary.sha256"
    log_success 'Diagnostic binary only; no install, canonical selection or acceptance performed'
)

self_test() {
    node "$LEAF" self-test
    local arg
    for arg in '--unknown' '--prepare --build' '--build' '--self-test --work-dir /tmp/no' '--self-test --sdk-source /tmp/no' '--prepare --work-dir /tmp/no' "--prepare --work-dir $CACHE/../unsafe"; do
        if bash "$SCRIPT_DIR/build-corrected-node.sh" $arg >/dev/null 2>&1; then fail "Invalid CLI accepted: $arg"; fi
    done
    local fixture
    fixture="$(mktemp -d)"
    (
        WORK="$fixture"; ATTEMPT=
        local status
        for status in 1 9 10 99 100 127 128 254 255; do
            printf '%s\n' "$status" >"$WORK/status"
            known_failed_attempt "$WORK/status" || fail "Known failure refused: $status"
        done
        for status in '' 0 00 01 -1 +1 256 999 1000 99999999999999999999999999 ' 1' '1 ' $'1\t' $'1\r' $'1\n' $'1\n2' garbage $'garbage\n1'; do
            printf '%s\n' "$status" >"$WORK/status"
            if known_failed_attempt "$WORK/status"; then fail "Unknown/success receipt accepted: $status"; fi
        done
        for status in '' 1 25 255; do
            printf '%s' "$status" >"$WORK/status"
            if known_failed_attempt "$WORK/status"; then fail 'Truncated receipt accepted'; fi
        done
        printf '1\0\n' >"$WORK/status"
        if known_failed_attempt "$WORK/status"; then fail 'NUL receipt accepted'; fi
        if known_failed_attempt "$WORK/missing"; then fail 'Missing receipt accepted'; fi
        ln -s status "$WORK/status-link"
        if known_failed_attempt "$WORK/status-link"; then fail 'Symlink receipt accepted'; fi
        printf 'fixture\n' >"$WORK/prepared.json"
        if ( admit_attempt; exit 7 ); then fail 'Fixture failure lost'; fi
        [[ "$(<"$WORK/attempts/0001/exit-status")" == 7 ]] || fail 'Failure record lost'
        local malformed
        for malformed in '' 0 256 garbage $'1\n2'; do
            printf '%s\n' "$malformed" >"$WORK/attempts/0001/exit-status"
            if ( admit_attempt ); then fail 'Unknown/success outcome admitted retry'; fi
            [[ ! -e "$WORK/attempts/0002" && ! -e "$WORK/build-admission" ]] || fail 'Refused retry changed attempt state'
        done
        rm "$WORK/attempts/0001/exit-status"
        if ( admit_attempt ); then fail 'Missing outcome admitted retry'; fi
        printf '7\n' >"$WORK/attempts/0001/exit-status"
        ( admit_attempt; exit 0 )
        [[ "$(<"$WORK/attempts/0002/exit-status")" == 0 ]] || fail 'Retry status lost'
        cmp "$WORK/prepared.json" "$WORK/attempts/0001/frozen-inputs.json"
        if ( admit_attempt ); then fail 'Redundant success rebuild admitted'; fi
        mkdir "$WORK/build-admission"
        if ( admit_attempt ); then fail 'Concurrent build admitted'; fi
        [[ -d "$WORK/build-admission" ]] || fail 'Foreign admission removed'
    )
    rm -r "$fixture"
    log_success 'CLI, exclusive attempt/failure/retry and structural self-tests passed; no SDK compile'
}

main() {
    parse_args "$@"
    check_prerequisites
    [[ "$MODE" != --self-test ]] || { self_test; return; }
    if [[ "$MODE" == --source-preflight ]]; then
        node "$LEAF" source-preflight "$SOURCE" "$WORK" "$SCRIPT_DIR/build-corrected-node.sh" "$SCRIPT_DIR/_common.sh"
        return
    fi
    if [[ "$MODE" == --freeze-existing ]]; then
        node "$LEAF" freeze-existing "$WORK" "$EVIDENCE"
        return
    fi
    unset SKIP_WASM_BUILD
    export CARGO_TARGET_DIR="$WORK/target" WASM_BUILD_WORKSPACE_HINT="$WORK/sdk" WASM_BUILD_TOOLCHAIN=1.94.1 CARGO_BUILD_JOBS=2
    if [[ "$MODE" == --prepare || "$MODE" == --explore ]]; then prepare; else build; fi
}
main "$@"
