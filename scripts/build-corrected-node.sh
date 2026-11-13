#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CACHE="${DEOS_NODE_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/deos-native-convergence}"
SOURCE=
COMMIT=9448fe4f252936d241b8d2663506813b884f94f3
TREE=97a1f73479286fdbd94e4bed0c5062999fab3073
LOCK=f12148367523411902f1cceade3d28d14b94101049dce41f5eb9644775bc0356
LEAF="$SCRIPT_DIR/build-corrected-node.mjs"
MODE= WORK=

usage() {
    cat <<'HELP'
Usage: build-corrected-node.sh --prepare --sdk-source SDK_PATH --work-dir NEW_PATH
       build-corrected-node.sh --build --work-dir PREPARED_PATH
       build-corrected-node.sh --self-test|--help

Build-only diagnostic corrected Omni route; never installs or selects a node.
--prepare exclusively creates a new workspace beneath
  DEOS_NODE_CACHE_DIR (default: $XDG_CACHE_HOME/deos-native-convergence or
  $HOME/.cache/deos-native-convergence); offline copies SDK stable2609-2,
  applies the reviewed checked-decrement correction and workspace-root Crypto,
  Hickory and roots patches plus coupled Wasmtime 36.0.20/Cranelift 0.123.20,
  retains nine exact compatible SDK dependency-parent selections, verifies
  the accepted graph modulo the coupled compiler patches, and freezes source,
  isolated lock, graph evidence and script hashes. No compile.
  Cargo may fetch missing required dependencies. No broad SDK update is made;
  prepared.json exposes intersecting versions for separate security review.
--build validates the frozen workspace then runs ONE Cargo build with Rust
  1.94.1, jobs 2, profile production (fat LTO/cgu1/opt3/unwind), features
  runtime-benchmarks, SDK Wasm enabled. Requires explicit later admission by
  the caller; preparation is not security approval or artifact qualification.
  Each workspace permits one build attempt; failure logs/artifacts are kept.

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
            --help|-h) [[ $# == 1 && -z "$MODE$WORK$SOURCE" ]] || fail '--help is exclusive'; usage; exit 0 ;;
            --prepare|--build|--self-test) [[ -z "$MODE" ]] || fail 'Choose exactly one mode'; MODE="$1"; shift ;;
            --work-dir) [[ $# -ge 2 && -n "$2" && -z "$WORK" ]] || fail '--work-dir requires one path'; WORK="$2"; shift 2 ;;
            --sdk-source) [[ $# -ge 2 && -n "$2" && -z "$SOURCE" ]] || fail '--sdk-source requires one path'; SOURCE="$2"; shift 2 ;;
            *) fail "Unknown argument: $1" ;;
        esac
    done
    [[ -n "$MODE" ]] || fail 'Choose --prepare, --build or --self-test'
    if [[ "$MODE" == --self-test ]]; then [[ -z "$WORK$SOURCE" ]] || fail '--self-test is exclusive'; return; fi
    if [[ "$MODE" == --prepare ]]; then
        [[ "$SOURCE" == /* && -d "$SOURCE" && "$(realpath "$SOURCE")" == "$SOURCE" ]] || fail '--prepare requires a canonical --sdk-source directory'
    else
        [[ -z "$SOURCE" ]] || fail '--sdk-source is prepare-only'
    fi
    [[ "$WORK" == "$CACHE/"* && "$WORK" != "$CACHE/" && "$WORK" != *$'\n'* ]] || fail 'Workspace must be a new absolute path beneath the owned cache'
    [[ "$(realpath -m -- "$WORK")" == "$WORK" ]] || fail 'Workspace must be canonical, without symlink or dot components'
}

git_read() { git -c protocol.allow=never --no-optional-locks "$@"; }

check_prerequisites() {
    require_commands node git cargo rustup sha256sum realpath cp tee diff cmp
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
    if [[ "$MODE" == --prepare ]]; then
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
    cargo +1.94.1 metadata --format-version 1 --locked --features polkadot-omni-node/runtime-benchmarks >"$WORK/$label-metadata.json" 2>"$WORK/$label-metadata.log"
    logged "$WORK/$label-tree.log" cargo +1.94.1 tree --locked -p polkadot-omni-node --features runtime-benchmarks --edges normal,build --prefix depth --format '{p}|{f}'
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
    for pkg in crypto-bigint hickory-proto webpki-roots sp-api-proc-macro; do
        cp -a "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg"
        diff -qr "$TEMPLATE_DIR/vendor/$pkg" "$WORK/vendor/$pkg" >/dev/null || fail "Copy changed: $pkg"
    done
    cd "$WORK/sdk"
    cargo_graph baseline
    node "$LEAF" patch "$WORK/sdk" "$WORK/vendor"
    logged "$WORK/resolve.log" cargo +1.94.1 update -p crypto-bigint@0.5.2 --precise 0.5.5
    logged "$WORK/resolve-hickory.log" cargo +1.94.1 update -p hickory-proto@0.24.1 --precise 0.24.4
    logged "$WORK/resolve-roots.log" cargo +1.94.1 update -p webpki-roots@0.25.2 --precise 0.25.4
    logged "$WORK/resolve-wasmtime.log" cargo +1.94.1 update -p wasmtime@36.0.7 --precise 36.0.20
    node "$LEAF" retain-parents "$WORK"
    cargo_graph candidate
    cp Cargo.lock "$WORK/candidate-before-sync.lock"
    logged "$WORK/offline-sync.log" cargo +1.94.1 update --workspace --offline
    cmp Cargo.lock "$WORK/candidate-before-sync.lock" || fail 'Offline workspace sync changed candidate lock'
    git_read -C "$WORK/sdk" diff -- Cargo.lock >"$WORK/lock-delta.patch"
    node "$LEAF" freeze "$WORK" "$SCRIPT_DIR/build-corrected-node.sh" "$SCRIPT_DIR/_common.sh"
    node "$LEAF" verify "$WORK"
    log_success "Prepared only (no build): $WORK/prepared.json"
}

build() {
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
    mkdir "$WORK/build-admission"
    cd "$WORK/sdk"
    { rustup run 1.94.1 rustc -Vv; cargo +1.94.1 --version; gcc --version; g++ --version; cmake --version; "$PROTOC" --version; } >"$WORK/build-tools.txt" 2>&1
    node "$LEAF" environment "$WORK"
    logged "$WORK/build.log" cargo +1.94.1 build --verbose --locked --profile production -p polkadot-omni-node --bin polkadot-omni-node --features runtime-benchmarks -j 2
    node "$LEAF" verify "$WORK"
    sha256sum "$WORK/target/production/polkadot-omni-node" | tee "$WORK/binary.sha256"
    log_success 'Diagnostic binary only; no install, canonical selection or acceptance performed'
}

self_test() {
    node "$LEAF" self-test
    local arg
    for arg in '--unknown' '--prepare --build' '--build' '--self-test --work-dir /tmp/no' '--self-test --sdk-source /tmp/no' '--prepare --work-dir /tmp/no' "--prepare --work-dir $CACHE/../unsafe"; do
        if bash "$SCRIPT_DIR/build-corrected-node.sh" $arg >/dev/null 2>&1; then fail "Invalid CLI accepted: $arg"; fi
    done
    log_success 'CLI rejection and structural self-tests passed; no SDK compile'
}

main() {
    parse_args "$@"
    check_prerequisites
    [[ "$MODE" != --self-test ]] || { self_test; return; }
    unset SKIP_WASM_BUILD
    export CARGO_TARGET_DIR="$WORK/target" WASM_BUILD_WORKSPACE_HINT="$WORK/sdk" WASM_BUILD_TOOLCHAIN=1.94.1 CARGO_BUILD_JOBS=2
    if [[ "$MODE" == --prepare ]]; then prepare; else build; fi
}
main "$@"
