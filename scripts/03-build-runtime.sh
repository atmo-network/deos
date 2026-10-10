#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CANONICAL_BUILD_ROOT="/tmp/deos-runtime-production-source"
BUILD_PROJECT_ROOT=""
BUILD_TEMPLATE_DIR=""
BUILD_STAGE_OWNED=0
OUTPUT_WASM_PATH="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
CANONICAL_OUTPUT_WASM_PATH="$OUTPUT_WASM_PATH"
OUTPUT_EXCLUSIVE=0
SELF_TEST=0

usage() {
    cat <<'EOF'
Usage: 03-build-runtime.sh [OPTIONS]

Builds the current DEOS reference runtime (`deos-runtime`) Wasm artifact with substrate-wasm-builder's production profile.

Options:
  --output FILE    Write a new artifact instead of replacing canonical output
  --self-test      Check output/argument contracts offline without Rust or a build
  -h, --help       Show this help message

Inputs:
  Locked template Cargo workspace and repository-pinned Rust toolchain.

Outputs:
  Default: target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm.
  --output selects a new file in an existing directory, relative to the caller.
  Existing files, directories and symlinks are refused, including publication races.
  The canonical destination is excluded from --output, even when absent.

Side effects:
  Installs the pinned Wasm target when absent, builds in a fixed temporary
  source root, and atomically replaces canonical output after success by default.
  --output publishes exclusively without changing canonical output or identity pins.
  --self-test only creates/removes its own temporary fixtures; no build/install.

Notes:
  The production Wasm profile fixes fat LTO and one codegen unit. One physical
  build root stabilizes Cargo crate identity; canonical virtual source,
  Cargo-home, and Rustup-home prefixes remove machine paths from Wasm bytes.
  A concurrent or stale canonical build root fails closed rather than sharing
  mutable build state.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --output)
                [[ $# -ge 2 && -n "$2" && "$2" != --* && "$OUTPUT_EXCLUSIVE" == 0 ]] || {
                    log_error "--output requires one new path and cannot be repeated"
                    exit 1
                }
                OUTPUT_WASM_PATH="$2"
                OUTPUT_EXCLUSIVE=1
                shift
                ;;
            --self-test)
                [[ "$SELF_TEST" == 0 ]] || { log_error "Duplicate --self-test"; exit 1; }
                SELF_TEST=1
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
    if (( SELF_TEST == 1 && OUTPUT_EXCLUSIVE == 1 )); then
        log_error "--self-test must be used alone"
        exit 1
    fi
}

resolve_output() {
    if (( OUTPUT_EXCLUSIVE == 1 )); then
        local parent canonical_parent canonical
        parent="$(dirname -- "$OUTPUT_WASM_PATH")"
        require_directory "$parent" "New Wasm output parent"
        OUTPUT_WASM_PATH="$(cd -- "$parent" && pwd -P)/$(basename -- "$OUTPUT_WASM_PATH")"
        canonical_parent="$(dirname -- "$CANONICAL_OUTPUT_WASM_PATH")"
        if [[ -d "$canonical_parent" ]]; then
            canonical="$(cd -- "$canonical_parent" && pwd -P)/$(basename -- "$CANONICAL_OUTPUT_WASM_PATH")"
            [[ "$OUTPUT_WASM_PATH" != "$canonical" ]] || {
                log_error "--output cannot select the canonical Wasm destination"
                return 1
            }
        fi
        [[ ! -e "$OUTPUT_WASM_PATH" && ! -L "$OUTPUT_WASM_PATH" ]] || {
            log_error "New Wasm output already exists: $OUTPUT_WASM_PATH"
            return 1
        }
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands rustc cargo rustup du cut sha256sum tar mkdir rm cp mv link dirname basename mktemp
    resolve_output
    log_success "Runtime build prerequisites checked"
}

cleanup_build_stage() {
    if (( BUILD_STAGE_OWNED == 1 )); then
        rm -rf "$CANONICAL_BUILD_ROOT"
        BUILD_STAGE_OWNED=0
    fi
}

stage_runtime_source() {
    phase_banner "Step 2: Stage deterministic source"
    if ! mkdir "$CANONICAL_BUILD_ROOT"; then
        log_error "Canonical runtime build root is already present: $CANONICAL_BUILD_ROOT"
        echo "  Refusing to share or remove a build root not created by this process."
        exit 1
    fi
    BUILD_STAGE_OWNED=1
    trap cleanup_build_stage EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    if ! (cd "$PROJECT_ROOT" && tar --exclude='template/target' --exclude='template/chain_spec.json' -cf - template) \
        | (cd "$CANONICAL_BUILD_ROOT" && tar -xf -); then
        log_error "Unable to stage the template workspace at the canonical build root"
        exit 1
    fi
    BUILD_PROJECT_ROOT="$CANONICAL_BUILD_ROOT"
    BUILD_TEMPLATE_DIR="$CANONICAL_BUILD_ROOT/template"
    require_directory "$BUILD_TEMPLATE_DIR" "Staged template directory"
    log_success "Runtime source staged at the canonical physical build root"
}

setup_wasm_target() {
    phase_banner "Step 3: Configure Wasm target"
    log_info "Checking Wasm target..."
    if ! (cd "$BUILD_TEMPLATE_DIR" && rustup target list --installed) | grep -qx "wasm32-unknown-unknown"; then
        log_info "Installing wasm32-unknown-unknown target for the repository-pinned toolchain..."
        (cd "$BUILD_TEMPLATE_DIR" && rustup target add wasm32-unknown-unknown)
        log_success "Wasm target installed"
    else
        log_success "Wasm target already installed for the repository-pinned toolchain"
    fi
}

configure_reproducible_wasm_paths() {
    local cargo_home rustup_home path
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
    for path in "$BUILD_PROJECT_ROOT" "$cargo_home" "$rustup_home"; do
        [[ "$path" == /* ]] || { log_error "Reproducible Wasm path roots must be absolute: $path"; exit 1; }
        [[ ! "$path" =~ [[:space:]] ]] || { log_error "Reproducible Wasm path roots must not contain whitespace: $path"; exit 1; }
    done
    export WASM_BUILD_TYPE=production
    export WASM_BUILD_RUSTFLAGS="--remap-path-prefix=$BUILD_PROJECT_ROOT=/deos/source --remap-path-prefix=$cargo_home=/deos/cargo --remap-path-prefix=$rustup_home=/deos/rustup"
    export CARGO_INCREMENTAL=0
    log_success "Runtime Wasm uses the production profile and canonical build identity"
}

build_runtime() {
    phase_banner "Step 4: Build runtime"
    local built_wasm
    built_wasm="$BUILD_TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
    log_info "Building parachain runtime (this may take several minutes)..."

    # substrate-wasm-builder treats any presence of SKIP_WASM_BUILD, including `0`, as skip.
    unset SKIP_WASM_BUILD
    configure_reproducible_wasm_paths
    run_shell_step \
        "Build parachain runtime" \
        "" \
        "cd '$BUILD_TEMPLATE_DIR' && cargo build --release -p deos-runtime --locked"
    [[ -f "$built_wasm" ]] || { log_error "Staged runtime Wasm is unavailable: $built_wasm"; exit 1; }

    publish_runtime_wasm "$built_wasm"
    log_success "Runtime Wasm atomically written from the canonical build root"
}

publish_runtime_wasm() {
    local built_wasm="$1" output_dir temporary_output
    output_dir="$(dirname "$OUTPUT_WASM_PATH")"
    mkdir -p "$output_dir" || return 1
    temporary_output="$(mktemp "$output_dir/.deos_runtime.compact.compressed.wasm.XXXXXX")" || return 1
    if ! cp "$built_wasm" "$temporary_output"; then
        rm -f "$temporary_output"
        log_error "Unable to stage the successful runtime Wasm for publication"
        return 1
    fi
    if (( OUTPUT_EXCLUSIVE == 1 )); then
        # POSIX link targets the exact pathname: no overwrite or directory-following fallback.
        if ! link "$temporary_output" "$OUTPUT_WASM_PATH"; then
            rm -f "$temporary_output"
            log_error "Exclusive Wasm publication refused: $OUTPUT_WASM_PATH"
            return 1
        fi
        rm -f "$temporary_output"
        return
    fi
    mv "$temporary_output" "$OUTPUT_WASM_PATH"
}

verify_build() {
    phase_banner "Step 5: Verify output"
    local wasm_path="$OUTPUT_WASM_PATH"

    if [[ -f "$wasm_path" ]]; then
        local wasm_size
        local wasm_sha256
        wasm_size=$(du -h "$wasm_path" | cut -f1)
        wasm_sha256=$(sha256sum "$wasm_path" | cut -d' ' -f1)
        log_success "Runtime WASM artifact verified"
        echo "  Path:   $wasm_path"
        echo "  Size:   $wasm_size"
        echo "  SHA256: $wasm_sha256"
    else
        log_error "Runtime WASM not found at expected path: $wasm_path"
        exit 1
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Runtime build process completed successfully"
}

run_self_test() (
    require_commands mktemp mkdir rmdir rm cp mv link dirname basename cmp ln
    local fixture
    fixture="$(mktemp -d)"
    trap 'rm -rf "$fixture"' EXIT
    CANONICAL_OUTPUT_WASM_PATH="$fixture/canonical.wasm"
    printf 'current candidate\n' >"$fixture/source.wasm"
    printf 'historical canonical\n' >"$fixture/canonical.wasm"
    cp "$fixture/canonical.wasm" "$fixture/canonical.before"
    expect_refusal() {
        if (SELF_TEST=0; OUTPUT_EXCLUSIVE=0; "$@") >/dev/null 2>&1; then
            log_error "Expected refusal: $*"
            exit 1
        fi
    }
    expect_refusal parse_args --output
    expect_refusal parse_args --output ''
    expect_refusal parse_args --output --self-test
    expect_refusal parse_args --output a --output b
    expect_refusal parse_args --self-test --output a
    expect_refusal parse_args --unknown
    (SELF_TEST=0; OUTPUT_EXCLUSIVE=0; parse_args; [[ "$OUTPUT_EXCLUSIVE" == 0 ]])
    (
        cd "$fixture"
        SELF_TEST=0
        OUTPUT_EXCLUSIVE=0
        parse_args --output relative.wasm
        resolve_output
        [[ "$OUTPUT_WASM_PATH" == "$(pwd -P)/relative.wasm" ]]
        publish_runtime_wasm "$fixture/source.wasm"
        cmp "$fixture/source.wasm" relative.wasm
    )
    if (OUTPUT_EXCLUSIVE=1; OUTPUT_WASM_PATH="$fixture/missing/child.wasm"; resolve_output) >/dev/null 2>&1; then exit 1; fi
    if (OUTPUT_EXCLUSIVE=1; CANONICAL_OUTPUT_WASM_PATH="$fixture/absent-canonical.wasm"; OUTPUT_WASM_PATH="$CANONICAL_OUTPUT_WASM_PATH"; resolve_output) >/dev/null 2>&1; then exit 1; fi
    SELF_TEST=0
    parse_args --output "$fixture/new candidate.wasm"
    resolve_output
    publish_runtime_wasm "$fixture/source.wasm"
    cmp "$fixture/source.wasm" "$OUTPUT_WASM_PATH"
    cmp "$fixture/canonical.before" "$fixture/canonical.wasm"
    if resolve_output >/dev/null 2>&1 || publish_runtime_wasm "$fixture/source.wasm" >/dev/null 2>&1; then
        log_error "Existing output accepted"
        exit 1
    fi
    cmp "$fixture/source.wasm" "$OUTPUT_WASM_PATH"
    OUTPUT_WASM_PATH="$fixture/raced.wasm"
    resolve_output
    printf 'race winner\n' >"$OUTPUT_WASM_PATH"
    cp "$OUTPUT_WASM_PATH" "$fixture/race.before"
    if publish_runtime_wasm "$fixture/source.wasm" >/dev/null 2>&1; then exit 1; fi
    cmp "$fixture/race.before" "$OUTPUT_WASM_PATH"
    OUTPUT_WASM_PATH="$fixture/directory.wasm"
    resolve_output
    mkdir "$OUTPUT_WASM_PATH"
    if publish_runtime_wasm "$fixture/source.wasm" >/dev/null 2>&1; then exit 1; fi
    rmdir "$OUTPUT_WASM_PATH"
    OUTPUT_WASM_PATH="$fixture/link.wasm"
    resolve_output
    ln -s "$fixture/absent.wasm" "$OUTPUT_WASM_PATH"
    if resolve_output >/dev/null 2>&1 || publish_runtime_wasm "$fixture/source.wasm" >/dev/null 2>&1; then exit 1; fi
    [[ -L "$OUTPUT_WASM_PATH" && ! -e "$fixture/absent.wasm" ]]
    OUTPUT_WASM_PATH="$fixture/missing-source.wasm"
    if publish_runtime_wasm "$fixture/missing.wasm" >/dev/null 2>&1; then exit 1; fi
    [[ ! -e "$OUTPUT_WASM_PATH" ]]
    OUTPUT_EXCLUSIVE=0
    OUTPUT_WASM_PATH="$fixture/canonical.wasm"
    publish_runtime_wasm "$fixture/source.wasm"
    cmp "$fixture/source.wasm" "$OUTPUT_WASM_PATH"
    shopt -s nullglob
    local leftovers=("$fixture"/.deos_runtime.compact.compressed.wasm.*)
    [[ ${#leftovers[@]} == 0 ]]
    log_success "Runtime output self-test passed: exclusive creation/races, default replacement and cleanup; no build"
)

main() {
    parse_args "$@"
    if (( SELF_TEST == 1 )); then run_self_test; return; fi
    phase_banner "DEOS reference runtime build"
    check_prerequisites
    stage_runtime_source
    setup_wasm_target
    build_runtime
    verify_build
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
