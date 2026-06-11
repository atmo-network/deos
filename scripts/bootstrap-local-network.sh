#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SKIP_DOWNLOAD="${SKIP_DOWNLOAD:-0}"
SKIP_TOOLS="${SKIP_TOOLS:-0}"
SKIP_BUILD="${SKIP_BUILD:-0}"
SKIP_CHAINSPEC="${SKIP_CHAINSPEC:-0}"
CHAIN_TYPE="${CHAIN_TYPE:-Development}"
WEB_CLIENT_PORT="${WEB_CLIENT_PORT:-5173}"

usage() {
    cat <<'EOF'
Usage: bootstrap-local-network.sh [OPTIONS]

Runs the local bootstrap chain for Zombienet: binaries -> tools -> runtime build -> chain spec -> network spawn.

Options:
  --skip-download    Skip 01-download-binaries.sh
  --skip-tools       Skip 02-install-tools.sh
  --skip-build       Skip 03-build-runtime.sh
  --skip-chain-spec  Skip 04-generate-chain-spec.sh
  -h, --help         Show this help message

Environment:
  SKIP_DOWNLOAD=0|1
  SKIP_TOOLS=0|1
  SKIP_BUILD=0|1
  SKIP_CHAINSPEC=0|1
  CHAIN_TYPE=Development|Local|Live
  WEB_CLIENT_PORT=<port>  Web-client dev server port to display (default: 5173)
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-download)
                SKIP_DOWNLOAD=1
                ;;
            --skip-tools)
                SKIP_TOOLS=1
                ;;
            --skip-build)
                SKIP_BUILD=1
                ;;
            --skip-chain-spec)
                SKIP_CHAINSPEC=1
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
}

# Display network endpoints
display_endpoints() {
    echo ""
    log_info "Network endpoints (once started):"
    echo "  Relay Chain (Alice): ws://localhost:9944"
    echo "  Relay Chain (Bob):   ws://localhost:9955"
    echo "  Parachain (Charlie): ws://localhost:9988"
    echo ""
    echo "  Polkadot.js Apps:"
    echo "    Relay: https://polkadot.js.org/apps/?rpc=ws://localhost:9944"
    echo "    Para:  https://polkadot.js.org/apps/?rpc=ws://localhost:9988"
    echo ""
    echo "  Web client (start separately): http://127.0.0.1:$WEB_CLIENT_PORT"
}

run_bootstrap_steps() {
    export CHAIN_TYPE

    if (( SKIP_DOWNLOAD == 0 )); then
        phase_banner "1/5: Download Polkadot binaries"
        run_script_step "Download Polkadot binaries" "01-download-binaries.sh"
    else
        log_warning "Skipping step 1: Download binaries"
    fi

    if (( SKIP_TOOLS == 0 )); then
        phase_banner "2/5: Install cargo tools"
        run_script_step "Install cargo tools" "02-install-tools.sh"
    else
        log_warning "Skipping step 2: Install tools"
    fi

    if (( SKIP_BUILD == 0 )); then
        phase_banner "3/5: Build parachain runtime"
        run_script_step "Build parachain runtime" "03-build-runtime.sh"
    else
        log_warning "Skipping step 3: Build runtime"
    fi

    if (( SKIP_CHAINSPEC == 0 )); then
        phase_banner "4/5: Generate chain spec"
        run_script_step "Generate chain spec" "04-generate-chain-spec.sh"
    else
        log_warning "Skipping step 4: Generate chain spec"
    fi
}

check_spawn_prerequisites() {
    phase_banner "Step 5: Spawn preflight"
    hydrate_local_tool_paths
    require_commands polkadot polkadot-omni-node zombienet
    log_success "Spawn prerequisites verified"
}

on_exit() {
    local exit_code=$?
    if (( exit_code != 0 )); then
        log_error "Local bootstrap failed"
    fi
}

main() {
    parse_args "$@"

    trap on_exit EXIT

    phase_banner "DEOS local bootstrap workflow"
    log_info "Chain type: $CHAIN_TYPE"
    log_info "Plan: skip_download=$SKIP_DOWNLOAD skip_tools=$SKIP_TOOLS skip_build=$SKIP_BUILD skip_chain_spec=$SKIP_CHAINSPEC"

    run_bootstrap_steps
    check_spawn_prerequisites

    phase_banner "6/6: Spawn Zombienet"
    display_endpoints
    log_info "Starting network (Ctrl+C to stop)..."
    echo ""

    "$SCRIPT_DIR/05-spawn-zombienet.sh"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
