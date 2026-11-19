#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

export NETWORK_WORK_DIR="${NETWORK_WORK_DIR:-$TEMPLATE_DIR}"
export DEOS_NETWORK_REFERENCE="${DEOS_NETWORK_REFERENCE:-1}"
if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 ]]; then export DEOS_NETWORK_REFERENCE=0; fi
export DEOS_BINARY_DIR="$BIN_DIR"
if [[ "$DEOS_NETWORK_REFERENCE" == 1 ]]; then
    ZOMBIENET_CONFIG="${ZOMBIENET_CONFIG:-$NETWORK_WORK_DIR/zombienet.json}"
else
    ZOMBIENET_CONFIG="${ZOMBIENET_CONFIG:-$NETWORK_WORK_DIR/zombienet.toml}"
fi
export CHAIN_SPEC_PATH="${CHAIN_SPEC_PATH:-$NETWORK_WORK_DIR/chain_spec.json}"

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
  DEOS_NETWORK_REFERENCE=1  Default bounded accepted Linux reference route:
    schema-2 installed selection/acceptance proof, exact reviewed Node/CLI,
    JSON topology and accepted spec/CODE only. Missing default zombienet.json
    is generated from the accepted topology; no TOML/custom-profile fallback.
    DEOS_NETWORK_EVIDENCE_DIR must name a fresh owned evidence directory.
  DEOS_NETWORK_REFERENCE=0  General/custom capability, NOT reference qualification;
    broader CLI/ETH/BIP32/producer/platform/release gates remain unresolved.
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
    verify_guarded_node_environment
}

network_output_directory() {
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then diagnostic_environment || return 1; fi
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
    local candidate="${DEOS_DIAGNOSTIC_OMNI_PATH:-}"
    if [[ "$DEOS_NETWORK_REFERENCE" == 1 ]]; then
        candidate="$BIN_DIR/polkadot-omni-node-deos"
        verify_reference_omni_node "$candidate" || return 1
    fi
    [[ "$(realpath "$(command -v polkadot-omni-node-deos)")" == "$(realpath "$candidate")" ]] || { log_error "Selected candidate path drift" >&2; return 1; }
}

check_prerequisites() {
    [[ "$DEOS_NETWORK_REFERENCE" == 0 || "$DEOS_NETWORK_REFERENCE" == 1 ]] || { log_error 'DEOS_NETWORK_REFERENCE must be 0 or 1'; return 1; }
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then diagnostic_environment || return 1; fi
    phase_banner "Step 1: Prerequisites"
    hydrate_local_tool_paths
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then diagnostic_environment || return 1; fi
    require_commands polkadot zombienet jq node
    if [[ "$DEOS_NETWORK_REFERENCE" == 1 ]]; then verify_reference_omni_node "$(command -v polkadot-omni-node-deos || true)"; fi
    verify_network_omni_node "$(command -v polkadot-omni-node-deos || true)"
    log_success "Zombienet spawn prerequisites checked"
}

verify_prerequisites() {
    phase_banner "Step 2: Verify inputs"
    if [[ "$DEOS_NETWORK_REFERENCE" == 1 && ! -e "$ZOMBIENET_CONFIG" && ! -L "$ZOMBIENET_CONFIG" ]]; then
        diagnostic_environment || return 1
        node "$SCRIPT_DIR/network-assurance.mjs" reference-config "$(command -v zombienet)" "${DEOS_NETWORK_EVIDENCE_DIR:-}" "$ZOMBIENET_CONFIG"
    fi
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
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then
        diagnostic_environment
        require_commands node
        [[ -n "${DEOS_NETWORK_EVIDENCE_DIR:-}" && -d "$DEOS_NETWORK_EVIDENCE_DIR" ]] || { log_error 'An existing owned DEOS_NETWORK_EVIDENCE_DIR is required'; return 1; }
        [[ "$DEOS_NETWORK_REFERENCE" == 1 || -f "$DEOS_NETWORK_EVIDENCE_DIR/ACCEPTANCE.md" ]] || {
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

    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then diagnostic_environment || return 1; fi
    local output
    output="$(network_output_directory)" || return 1
    require_absent_network_output || return 1
    umask 077
    cd "$NETWORK_WORK_DIR"
    if [[ "${DEOS_NETWORK_DIAGNOSTIC:-0}" == 1 || "$DEOS_NETWORK_REFERENCE" == 1 ]]; then
        # Revalidate the frozen preflight without regenerating wx evidence. Keep
        # this check at the sole CLI startup boundary, including direct 05 use.
        verify_diagnostic_launch || return 1
        exec "$(command -v node)" "$(jq -er .cli "$DEOS_NETWORK_EVIDENCE_DIR/diagnostic-inputs.json")" --provider native --dir "$output" spawn "$ZOMBIENET_CONFIG"
    else
        exec zombienet --provider native --dir "$output" spawn "$ZOMBIENET_CONFIG"
    fi
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
