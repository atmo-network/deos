#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ZOMBIENET_CONFIG="${ZOMBIENET_CONFIG:-$TEMPLATE_DIR/zombienet.toml}"

usage() {
    cat <<'EOF'
Usage: 05-spawn-zombienet.sh [OPTIONS]

Spawns the local Zombienet network from the prepared chain spec and config.

Options:
  -h, --help        Show this help message

Environment:
  ZOMBIENET_CONFIG=template/zombienet.toml

Inputs:
  Existing Zombienet config, template/chain_spec.json, and node binaries on PATH.

Outputs:
  Foreground local Zombienet process and its runtime logs.

Side effects:
  Starts local relay/parachain processes and creates Zombienet temporary state.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
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
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    hydrate_local_tool_paths
    require_commands polkadot polkadot-omni-node zombienet
    log_success "Zombienet spawn prerequisites checked"
}

verify_prerequisites() {
    phase_banner "Step 2: Verify inputs"
    if [[ ! -f "$ZOMBIENET_CONFIG" ]]; then
        log_error "Zombienet config not found: $ZOMBIENET_CONFIG"
        exit 1
    fi

    if [[ ! -f "$TEMPLATE_DIR/chain_spec.json" ]]; then
        log_error "Chain spec not found."
        echo "  Expected: $TEMPLATE_DIR/chain_spec.json"
        exit 1
    fi

    log_info "Prerequisites verified"
}

spawn_network() {
    phase_banner "Step 3: Spawn Zombienet"
    log_info "Spawning Zombienet network"
    echo "  Config: $ZOMBIENET_CONFIG"
    echo "  Chain spec: $TEMPLATE_DIR/chain_spec.json"
    echo "  polkadot: $(command -v polkadot)"
    echo "  polkadot-omni-node: $(command -v polkadot-omni-node)"
    echo "  zombienet: $(command -v zombienet)"
    echo ""

    log_info "Starting network (Ctrl+C to stop)..."
    echo ""

    cd "$TEMPLATE_DIR"
    exec zombienet --provider native spawn "$ZOMBIENET_CONFIG"
}

main() {
    parse_args "$@"
    phase_banner "DEOS Zombienet spawn"
    check_prerequisites
    verify_prerequisites
    spawn_network
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
