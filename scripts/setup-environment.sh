#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MODE=""
PACKAGE_JSON="$PROJECT_ROOT/web-client/package.json"

usage() {
    cat <<'EOF'
Usage: setup-environment.sh <rust|node|client|full>

Prepares one repository-pinned validation environment.

Modes:
  rust    Install the Rust toolchain, components, and targets from template/rust-toolchain.toml.
  node    Verify the repository-pinned Node runtime.
  client    Verify pinned Node, install pinned npm, clear generated SvelteKit state, and install locked client tooling.
  full      Prepare Rust and client environments.

Inputs:
  template/rust-toolchain.toml
  web-client/package.json volta.node and packageManager

Outputs:
  Installed Rust toolchain and clean web-client state.

Side effects:
  Downloads toolchains and dependencies through rustup and npm; client/full replaces generated web-client/.svelte-kit state. It does not run the optional OKF upstream freshness check.
EOF
}

parse_args() {
    if [[ $# -ne 1 ]]; then
        usage
        exit 1
    fi
    case "$1" in
        rust|node|client|full) MODE="$1" ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "Unknown setup mode: $1"
            usage
            exit 1
            ;;
    esac
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands python3
    # `tomllib` is the only reason this script needs Python at all, and it landed in 3.11.
    # Checking the version here turns a later traceback into a named prerequisite failure.
    if ! python3 -c 'import tomllib' 2>/dev/null; then
        log_error "python3 with tomllib (3.11 or newer) is required to read rust-toolchain.toml, found $(python3 --version 2>&1)"
        exit 1
    fi
    [[ -f "$TEMPLATE_DIR/rust-toolchain.toml" ]] || { log_error "Rust toolchain authority not found"; exit 1; }
    [[ -f "$PACKAGE_JSON" ]] || { log_error "Web-client package authority not found"; exit 1; }
    if [[ "$MODE" == "rust" || "$MODE" == "full" ]]; then
        require_commands rustup
    fi
    if [[ "$MODE" == "node" || "$MODE" == "client" || "$MODE" == "full" ]]; then
        require_commands node
    fi
    if [[ "$MODE" == "client" || "$MODE" == "full" ]]; then
        require_commands npm
    fi
    log_success "Setup prerequisites checked"
}

setup_rust() {
    phase_banner "Step 2: Rust toolchain"
    local channel profile
    local -a components targets command
    channel="$(python3 -c 'import pathlib,tomllib; print(tomllib.loads(pathlib.Path("template/rust-toolchain.toml").read_text())["toolchain"]["channel"])' )"
    profile="$(python3 -c 'import pathlib,tomllib; print(tomllib.loads(pathlib.Path("template/rust-toolchain.toml").read_text())["toolchain"].get("profile", "minimal"))' )"
    mapfile -t components < <(python3 -c 'import pathlib,tomllib; print(*tomllib.loads(pathlib.Path("template/rust-toolchain.toml").read_text())["toolchain"].get("components", []), sep="\n")')
    mapfile -t targets < <(python3 -c 'import pathlib,tomllib; print(*tomllib.loads(pathlib.Path("template/rust-toolchain.toml").read_text())["toolchain"].get("targets", []), sep="\n")')
    command=(rustup toolchain install "$channel" --profile "$profile")
    local value
    for value in "${components[@]}"; do
        [[ -n "$value" ]] && command+=(--component "$value")
    done
    for value in "${targets[@]}"; do
        [[ -n "$value" ]] && command+=(--target "$value")
    done
    "${command[@]}"
    (cd "$TEMPLATE_DIR" && rustup show active-toolchain)
    log_success "Repository Rust toolchain prepared"
}

expected_node_version() {
    node -e 'const p=require(process.argv[1]); process.stdout.write(p.volta?.node ?? "")' "$PACKAGE_JSON"
}

verify_node() {
    phase_banner "Step 3: Node runtime"
    local expected_node actual_node
    expected_node="$(expected_node_version)"
    actual_node="$(node --version)"
    actual_node="${actual_node#v}"
    if [[ "$actual_node" != "$expected_node" ]]; then
        log_error "Node version mismatch: expected $expected_node, found $actual_node"
        exit 1
    fi
    log_success "Repository Node runtime verified"
}

setup_client() {
    phase_banner "Step 4: Client environment"
    local expected_npm actual_npm
    expected_npm="$(node -e 'const p=require(process.argv[1]); process.stdout.write(String(p.packageManager).split("@").pop())' "$PACKAGE_JSON")"
    actual_npm="$(npm --version)"
    if [[ "$actual_npm" != "$expected_npm" ]]; then
        npm install --global "npm@$expected_npm"
        actual_npm="$(npm --version)"
    fi
    if [[ "$actual_npm" != "$expected_npm" ]]; then
        log_error "npm version mismatch: expected $expected_npm, found $actual_npm"
        exit 1
    fi
    rm -rf -- "$PROJECT_ROOT/web-client/.svelte-kit"
    (cd "$PROJECT_ROOT/web-client" && npm ci --ignore-scripts)
    log_success "Pinned client environment prepared"
}

main() {
    parse_args "$@"
    cd "$PROJECT_ROOT"
    if [[ "$MODE" == "node" || "$MODE" == "client" || "$MODE" == "full" ]]; then
        activate_pinned_node
    fi
    check_prerequisites
    if [[ "$MODE" == "rust" || "$MODE" == "full" ]]; then
        setup_rust
    fi
    if [[ "$MODE" == "node" || "$MODE" == "client" || "$MODE" == "full" ]]; then
        verify_node
    fi
    if [[ "$MODE" == "client" || "$MODE" == "full" ]]; then
        setup_client
    fi
}

main "$@"
