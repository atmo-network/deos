#!/usr/bin/env bash

set -euo pipefail
VALIDATION_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
VALIDATION_PROJECT_ROOT="$(cd "$VALIDATION_SCRIPT_DIR/.." && pwd -P)"
export DEOS_PROJECT_ROOT="$VALIDATION_PROJECT_ROOT"
export DEOS_BINARY_DIR="$VALIDATION_PROJECT_ROOT/bin"
source "$VALIDATION_SCRIPT_DIR/_common.sh"

PROFILE=""
SELF_TEST=0

usage() {
    cat <<'EOF'
Usage: validate-local.sh <framework|client|wiki|--self-test>

Scopes (independent results):
  framework  Pinned Rust and Node setup, tool checks, TMCTOL tests, workspace
             format/Clippy/tests/docs/check, no-std checks and isolated PVF host.
             No npm dependencies, browser workspace, Skills or Git checkout.
  client     Optional browser setup and smoke/build checks; does not run framework checks.
  wiki       Optional Wiki corpus/trust checks, pinned mdBook build, structural output checks and
             native search data tests; Node only, no browser. Uses WIKI_BASE_PATH and
             WIKI_SITE_URL from wiki/scripts/build-site.sh.

Options:
  --self-test  Check independent-workspace routing without preparation or Cargo
  -h, --help   Show this help message

Environment:
  DEOS_VERBOSE=0|1     Stream nested command output.
  CC_wasm32v1_none     Wasm-capable C frontend (framework default: clang).
  AR_wasm32v1_none     Wasm-capable archiver (framework default: llvm-ar).

Inputs:
  Selected source, toolchain pins and lockfiles. Framework uses the pinned
  Rust guest target for no-std checks; see export-papi-metadata.sh for actual-Wasm metadata.

Outputs:
  One direct scope pass/fail result with compact retained failure logs.

Side effects:
  Prepares pinned toolchains and updates only the selected workspace's dependencies.
  Wiki validation writes ignored wiki/site output and does not publish.
EOF
}

parse_args() {
    if [[ $# -ne 1 ]]; then
        usage
        exit 1
    fi
    case "$1" in
        framework|client|wiki) PROFILE="$1" ;;
        --self-test) SELF_TEST=1 ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "Unknown validation profile: $1"
            usage
            exit 1
            ;;
    esac
}

prepare_pinned_environment() {
    phase_banner "Pinned $PROFILE validation environment"
    run_script_step "Pinned $PROFILE environment" "setup-environment.sh" "$PROFILE"
    activate_pinned_node
}

run_framework_checks() {
    phase_banner "Framework validation"
    run_script_step "Pinned binary selection self-test" "01-download-binaries.sh" --self-test
    run_script_step "Operator tool selection self-test" "02-install-tools.sh" --self-test
    run_script_step "Local CI argument/command self-test" "ci-local.sh" --self-test
    run_script_step "Validation composition self-test" "validate-local.sh" --self-test
    run_script_step "Runtime output self-test" "03-build-runtime.sh" --self-test
    run_script_step "Corrected-node build contract self-test" "build-corrected-node.sh" --self-test
    run_script_step "Chain spec producer self-test" "04-generate-chain-spec.sh" --self-test
    run_script_step "Independent PVF wrapper self-test" "pvf-assurance.sh" --self-test
    run_shell_step "Simulator tests" "" "node '$PROJECT_ROOT/simulators/tmctol/tests.js'"
    # Actual guest artifact generation/export are separate, explicit operations.
    SKIP_WASM_BUILD=1 run_script_step "Rust workspace CI" "ci-local.sh"
    local crate
    for crate in deos-runtime pallet-deos-actors-embedding-fixture pallet-deos-router-embedding-fixture pallet-oracle-embedding-fixture; do
        # The SDK Wasm builder sets this cfg to select its guest-only dependencies.
        SKIP_WASM_BUILD=1 CARGO_ENCODED_RUSTFLAGS=$'--cfg\x1fsubstrate_runtime' \
            CC_wasm32v1_none="${CC_wasm32v1_none:-clang}" AR_wasm32v1_none="${AR_wasm32v1_none:-llvm-ar}" \
            run_script_step "$crate guest no-std" "ci-local.sh" \
            --only check --package "$crate" --no-default-features --target wasm32v1-none
    done
    run_shell_step "Selected Assets vendor regressions" "${DEOS_TEST_TIMEOUT_MINUTES:-15}" \
        "cd '$TEMPLATE_DIR/vendor/pallet-assets' && SKIP_WASM_BUILD=1 cargo test --locked --release"
    local check
    for check in clippy tests docs check; do
        SKIP_WASM_BUILD=1 run_script_step "Canonical PVF host $check" "ci-local.sh" \
            --only "$check" --package deos-pvf-validation \
            --manifest-path "$TEMPLATE_DIR/tools/pvf-validation/Cargo.toml"
    done
    run_shell_step "Canonical PVF host formatting" "" \
        "cd '$TEMPLATE_DIR/tools/pvf-validation' && cargo fmt -- --check"
}

run_wiki_checks() {
    local builder="$PROJECT_ROOT/wiki/scripts/build-site.sh"
    run_command_step "Pinned Wiki environment" "" bash "$builder" --prepare
    run_command_step "Wiki validation" "" bash "$builder" --validate
}

test_client_free_tools() (
    local root
    root="$(mktemp -d)"
    trap 'rm -rf "$root"' EXIT
    mkdir -p "$root/template" "$root/bin"
    cp "$TEMPLATE_DIR/rust-toolchain.toml" "$root/template/"
    cp "$PROJECT_ROOT/.node-version" "$root/"
    printf '#!/bin/sh\nexit 91\n' >"$root/bin/npm"
    printf '#!/bin/sh\nexit 0\n' >"$root/bin/rustup"
    printf '#!/bin/sh\nprintf "v%%s\\n" "$(cat "$DEOS_PROJECT_ROOT/.node-version")"\n' >"$root/bin/node"
    printf '#!/bin/sh\nprintf "%%s\\n" "$@" >"$DEOS_PROJECT_ROOT/argv"\n' >"$root/bin/cargo"
    chmod +x "$root/bin/"*
    mkdir "$root/home"
    export DEOS_PROJECT_ROOT="$root" DEOS_BINARY_DIR="$root/bin" HOME="$root/home" PATH="$root/bin:$PATH"
    bash "$VALIDATION_SCRIPT_DIR/setup-environment.sh" framework >/dev/null || return 1
    if bash "$VALIDATION_SCRIPT_DIR/setup-environment.sh" client >/dev/null 2>&1; then return 1; fi
    cd "$root"
    if bash "$VALIDATION_SCRIPT_DIR/export-papi-metadata.sh" --output metadata.scale --wasm absent >/dev/null 2>&1; then return 1; fi
    [[ ! -e argv ]] || return 1
    touch input.wasm
    if bash "$VALIDATION_SCRIPT_DIR/export-papi-metadata.sh" --output input.wasm --wasm input.wasm >/dev/null 2>&1; then return 1; fi
    [[ ! -e argv ]] || return 1
    bash "$VALIDATION_SCRIPT_DIR/export-papi-metadata.sh" --output metadata.scale --wasm input.wasm >/dev/null || return 1
    printf '%s\n' "$root/metadata.scale" 16 --wasm "$root/input.wasm" >expected
    tail -n 4 argv | cmp - expected
)

self_test() (
    local -a routed=() shell_steps=()
    phase_banner() { :; }
    activate_pinned_node() { :; }
    run_script_step() {
        local script="$2"
        shift 2
        routed+=("$script $*")
    }
    run_shell_step() { shell_steps+=("$3"); }
    PROFILE=framework prepare_pinned_environment
    run_framework_checks
    [[ "${routed[0]}" == "setup-environment.sh framework" ]] || return 1
    [[ "${routed[*]} ${shell_steps[*]}" != *web-client* && "${shell_steps[*]}" != *npm* && "${routed[*]} ${shell_steps[*]}" != *wiki/* ]] || return 1
    local check count command
    for check in root clippy tests docs check; do
        if [[ "$check" == root ]]; then
            command="ci-local.sh "
        else
            command="ci-local.sh --only $check --package deos-pvf-validation --manifest-path $TEMPLATE_DIR/tools/pvf-validation/Cargo.toml"
        fi
        count=0
        local item
        for item in "${routed[@]}"; do
            if [[ "$item" == "$command" ]]; then count=$((count + 1)); fi
        done
        [[ "$count" == 1 ]] || { log_error "Missing or duplicate canonical host route: $check"; return 1; }
    done
    [[ "${shell_steps[*]}" == *"cd '$TEMPLATE_DIR/tools/pvf-validation' && cargo fmt -- --check"* ]] || return 1
    local crate
    for crate in deos-runtime pallet-deos-actors-embedding-fixture pallet-deos-router-embedding-fixture pallet-oracle-embedding-fixture; do
        [[ " ${routed[*]} " == *" ci-local.sh --only check --package $crate --no-default-features --target wasm32v1-none "* ]] || return 1
    done
    test_client_free_tools || return 1
    local -a wiki_steps=()
    run_command_step() { wiki_steps+=("$3 $4 $5"); }
    run_wiki_checks
    [[ "${#wiki_steps[@]}" == 2 &&
        "${wiki_steps[0]}" == "bash $PROJECT_ROOT/wiki/scripts/build-site.sh --prepare" &&
        "${wiki_steps[1]}" == "bash $PROJECT_ROOT/wiki/scripts/build-site.sh --validate" ]] || return 1
    log_success "Validation composition self-test passed (framework and independent Wiki routing)"
)

main() {
    parse_args "$@"
    if (( SELF_TEST == 1 )); then
        self_test
        return
    fi
    cd "$PROJECT_ROOT"
    if [[ "$PROFILE" == framework ]]; then
        require_commands "${CC_wasm32v1_none:-clang}" "${AR_wasm32v1_none:-llvm-ar}"
    fi
    if [[ "$PROFILE" != wiki ]]; then prepare_pinned_environment; fi
    case "$PROFILE" in
        framework) run_framework_checks ;;
        client) run_shell_step "Optional browser smoke/build" "" "cd '$PROJECT_ROOT/web-client' && npm run validate" ;;
        wiki) run_wiki_checks ;;
    esac
    phase_banner "Summary"
    log_success "$PROFILE validation profile passed"
}

main "$@"
