#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MODE=""
PACKAGE_JSON="$PROJECT_ROOT/web-client/package.json"

usage() {
    cat <<'EOF'
Usage: setup-environment.sh <rust|node|framework|client|full>

Prepares one repository-pinned validation environment.

Modes:
  rust    Install the Rust toolchain, components, and targets from template/rust-toolchain.toml.
  node    Verify the repository-pinned Node runtime.
  client    Verify pinned Node, install pinned npm, clear generated SvelteKit state, and install locked client tooling.
  framework Prepare Rust and verify Node; never invokes npm or reads client files.
  full      Prepare framework and optional client environments.

Inputs:
  template/rust-toolchain.toml
  .node-version (shared), .npm-version (client/full scopes use the shared npm pin)
  web-client/package.json pin projections (client only)

Outputs:
  Pinned toolchains and, only for client/full, locked client dependencies.

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
        rust|node|framework|client|full) MODE="$1" ;;
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
    if [[ "$MODE" == rust || "$MODE" == framework || "$MODE" == full ]]; then
        require_commands python3 rustup
        python3 -c 'import tomllib' 2>/dev/null || { log_error "Python 3.11+ (tomllib) is required for Rust setup"; return 1; }
        [[ -f "$TEMPLATE_DIR/rust-toolchain.toml" ]] || { log_error "Rust toolchain authority not found"; return 1; }
    fi
    if [[ "$MODE" == client || "$MODE" == full ]]; then
        require_commands npm
        [[ -f "$PACKAGE_JSON" && -f "$PROJECT_ROOT/.npm-version" ]] || {
            log_error "Optional client workspace or npm pin missing"; return 1;
        }
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

setup_client() {
    phase_banner "Step 4: Client environment"
    local expected_npm
    expected_npm="$(<"$PROJECT_ROOT/.npm-version")"
    [[ "$expected_npm" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { log_error "Invalid npm pin"; return 1; }
    node -e '
        const p = require(process.argv[1]), [node, npm] = process.argv.slice(2);
        if (p.volta?.node !== node || p.volta?.npm !== npm || p.packageManager !== `npm@${npm}`)
            throw new Error("Client pin projections differ from .node-version/.npm-version");
    ' "$PACKAGE_JSON" "$(<"$PROJECT_ROOT/.node-version")" "$expected_npm"
    activate_pinned_npm
    rm -rf -- "$PROJECT_ROOT/web-client/.svelte-kit"
    (cd "$PROJECT_ROOT/web-client" && npm ci --ignore-scripts)
    log_success "Pinned client environment prepared"
}

main() {
    parse_args "$@"
    cd "$PROJECT_ROOT"
    check_prerequisites
    case "$MODE" in
        node|framework|client|full) activate_pinned_node ;;
    esac
    case "$MODE" in
        rust|framework|full) setup_rust ;;
    esac
    if [[ "$MODE" == client || "$MODE" == full ]]; then
        setup_client
    fi
}

main "$@"
