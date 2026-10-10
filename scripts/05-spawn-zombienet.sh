#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

NETWORK_WORK_DIR="${NETWORK_WORK_DIR:-$TEMPLATE_DIR}"
ZOMBIENET_CONFIG="${ZOMBIENET_CONFIG:-$NETWORK_WORK_DIR/zombienet.toml}"
CHAIN_SPEC_PATH="${CHAIN_SPEC_PATH:-$NETWORK_WORK_DIR/chain_spec.json}"

usage() {
    cat <<'EOF'
Usage: 05-spawn-zombienet.sh [OPTIONS]

Spawns the local Zombienet network from the prepared chain spec and config.

Options:
  -h, --help        Show this help message

Environment:
  NETWORK_WORK_DIR=template
  ZOMBIENET_CONFIG=<NETWORK_WORK_DIR>/zombienet.toml
  CHAIN_SPEC_PATH=<NETWORK_WORK_DIR>/chain_spec.json
  DEOS_BINARY_DIR=<repository>/bin  SDK bundle and selected Omni receipt

Inputs:
  Existing Zombienet config and matching raw-storage chain spec in the selected network work
  directory, plus node binaries on PATH. The reference topology uses
  polkadot-omni-node-deos selected through 01-download-binaries.sh; its receipt,
  SDK marker and SHA-256 must agree. Custom topologies own their node commands.

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
    require_commands polkadot zombienet jq
    verify_deos_omni_node "$(command -v polkadot-omni-node-deos || true)"
    log_success "Zombienet spawn prerequisites checked"
}

verify_prerequisites() {
    phase_banner "Step 2: Verify inputs"
    if [[ ! -f "$ZOMBIENET_CONFIG" ]]; then
        log_error "Zombienet config not found: $ZOMBIENET_CONFIG"
        exit 1
    fi

    if [[ ! -d "$NETWORK_WORK_DIR" ]]; then
        log_error "Network work directory not found: $NETWORK_WORK_DIR"
        exit 1
    fi

    if [[ ! -f "$CHAIN_SPEC_PATH" ]]; then
        log_error "Chain spec not found."
        echo "  Expected: $CHAIN_SPEC_PATH"
        exit 1
    fi

    if ! jq -e '.genesis.raw.top | type == "object"' "$CHAIN_SPEC_PATH" >/dev/null; then
        log_error "Raw-storage chain spec required to preserve runtime-owned genesis; regenerate with 04-generate-chain-spec.sh."
        exit 1
    fi

    log_info "Prerequisites verified"
}

spawn_network() {
    phase_banner "Step 3: Spawn Zombienet"
    log_info "Spawning Zombienet network"
    echo "  Work dir: $NETWORK_WORK_DIR"
    echo "  Config: $ZOMBIENET_CONFIG"
    echo "  Chain spec: $CHAIN_SPEC_PATH"
    echo "  polkadot: $(command -v polkadot)"
    echo "  polkadot-omni-node-deos: $(command -v polkadot-omni-node-deos)"
    echo "  zombienet: $(command -v zombienet)"
    echo ""

    log_info "Starting network (Ctrl+C to stop)..."
    echo ""

    cd "$NETWORK_WORK_DIR"
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
