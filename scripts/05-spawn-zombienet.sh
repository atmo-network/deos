#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

export NETWORK_WORK_DIR="${NETWORK_WORK_DIR:-$TEMPLATE_DIR}"
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
  DEOS_NETWORK_DIAGNOSTIC=1  Explicit non-promoting prepared diagnostic only
  DEOS_DIAGNOSTIC_OMNI_PATH, DEOS_DIAGNOSTIC_OMNI_SHA256,
  DEOS_DIAGNOSTIC_OMNI_VERSION  Exact caller-selected candidate identity
  DEOS_NETWORK_EVIDENCE_DIR  Required diagnostic preflight/evidence directory
  Diagnostic only: reviewed Linux x86_64 Node 22.22.0 / CLI 1.3.138 tree,
  JSON native topology and frozen input checks before preflight and final exec.
  Injection environments are refused before Node; OS/Bash startup is trusted.

Inputs:
  Existing Zombienet config and matching raw-storage chain spec in the selected network work
  directory, plus node binaries on PATH. The reference topology uses
  polkadot-omni-node-deos selected through 01-download-binaries.sh; its receipt,
  SDK marker and SHA-256 must agree. Custom topologies own their node commands.

Outputs:
  Foreground local Zombienet process and its runtime logs.
  <NETWORK_WORK_DIR>/zombienet-network is the authoritative machine output path.
  Work directory must be plain absolute, canonical, owned and non-symlinked;
  the output path must not exist (including old databases or dangling symlinks).

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

diagnostic_environment() {
    # This admission runs before Node, not before OS loader or Bash startup.
    local key
    for key in NODE_OPTIONS NODE_PATH NODE_EXTRA_CA_CERTS NODE_REPL_EXTERNAL_MODULE LD_PRELOAD LD_LIBRARY_PATH DYLD_INSERT_LIBRARIES BASH_ENV ENV ZOMBIENET_IMAGE RUN_IN_CONTAINER PUSHGATEWAY_URL ${!ZOMBIE_@}; do
        [[ -z "${!key:-}" ]] || { log_error "Diagnostic injection environment: $key" >&2; return 1; }
    done
    if env | grep '^BASH_FUNC_' >/dev/null; then log_error "Diagnostic inherited shell function" >&2; return 1; fi
    [[ "$(sha256sum "$(command -v node)" | cut -d ' ' -f1)" == "1bec56ef7cfa9a76f3e0b7c0a87f220eb73f23102b9c0b4c7529a3f7c3ce7c31" ]] || {
        log_error "Diagnostic requires the reviewed Node 22.22.0 executable" >&2; return 1;
    }
}

network_output_directory() {
    local work="${NETWORK_WORK_DIR:-$TEMPLATE_DIR}"
    node "$SCRIPT_DIR/network-assurance.mjs" network-output "$work"
}

require_absent_network_output() {
    local output
    output="$(network_output_directory)" || return 1
    [[ ! -e "$output" && ! -L "$output" ]] || {
        log_error "Network output already exists; select a fresh owned work directory: $output" >&2; return 1;
    }
}

verify_diagnostic_launch() {
    diagnostic_environment || return 1
    "$(command -v node)" "$SCRIPT_DIR/network-assurance.mjs" launch-check "$(command -v zombienet)" "$DEOS_NETWORK_EVIDENCE_DIR" "$ZOMBIENET_CONFIG" --provider native --dir "$(network_output_directory)" spawn "$ZOMBIENET_CONFIG" || return 1
    [[ "$(realpath "$(command -v polkadot)")" == "$(jq -er '.binaries.polkadot.path' "$DEOS_NETWORK_EVIDENCE_DIR/diagnostic-inputs.json")" ]] || { log_error "Selected relay path drift" >&2; return 1; }
    [[ "$(realpath "$(command -v polkadot-omni-node-deos)")" == "$(realpath "$DEOS_DIAGNOSTIC_OMNI_PATH")" ]] || { log_error "Selected candidate path drift" >&2; return 1; }
}

check_prerequisites() {
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == "1" ]]; then diagnostic_environment || return 1; fi
    phase_banner "Step 1: Prerequisites"
    hydrate_local_tool_paths
    require_commands polkadot zombienet jq node
    verify_network_omni_node "$(command -v polkadot-omni-node-deos || true)"
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

    require_absent_network_output || return 1
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == "1" ]]; then
        diagnostic_environment
        require_commands node
        [[ -n "${DEOS_NETWORK_EVIDENCE_DIR:-}" && -f "$DEOS_NETWORK_EVIDENCE_DIR/ACCEPTANCE.md" ]] || {
            log_error "Diagnostic requires a predeclared ACCEPTANCE.md in DEOS_NETWORK_EVIDENCE_DIR"
            return 1
        }
        node "$SCRIPT_DIR/network-assurance.mjs" preflight "$(command -v zombienet)" "$DEOS_NETWORK_EVIDENCE_DIR" "$ZOMBIENET_CONFIG"
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

    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == "1" ]]; then diagnostic_environment || return 1; fi
    local output
    output="$(network_output_directory)" || return 1
    require_absent_network_output || return 1
    umask 077
    cd "$NETWORK_WORK_DIR"
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == "1" ]]; then
        # Revalidate the frozen preflight without regenerating wx evidence. Keep
        # this check at the sole CLI startup boundary, including direct 05 use.
        verify_diagnostic_launch || return 1
        exec "$(command -v node)" "$(jq -er .cli "$DEOS_NETWORK_EVIDENCE_DIR/diagnostic-inputs.json")" --provider native --dir "$output" spawn "$ZOMBIENET_CONFIG"
    fi
    exec zombienet --provider native --dir "$output" spawn "$ZOMBIENET_CONFIG"
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
