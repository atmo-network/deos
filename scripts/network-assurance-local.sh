#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

KEEP_NETWORK="${KEEP_NETWORK:-0}"
SESSION_TRANSITION="${SESSION_TRANSITION:-0}"
COMPOSED_PATH="${COMPOSED_PATH:-0}"
RPC_READY_TIMEOUT_SEC="${RPC_READY_TIMEOUT_SEC:-300}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/deos-zombienet.log}"
DEOS_BINARY_DIR="${DEOS_BINARY_DIR:-${TMPDIR:-/tmp}/deos-assurance-bin}"
export DEOS_BINARY_DIR
BIN_DIR="$DEOS_BINARY_DIR"
ZOMBIENET_PID=""
RESTARTED_DAVE_PID=""

usage() {
    cat <<'EOF'
Usage: network-assurance-local.sh [OPTIONS]

Prepare pinned dependencies and artifacts, spawn the canonical two-validator /
two-collator local topology, then run finalized-progress smoke and signed-transfer E2E.

Options:
  --keep-network  Leave the owned Zombienet process running after validation.
  -h, --help      Show this help message.

Environment:
  KEEP_NETWORK=0|1
  RPC_READY_TIMEOUT_SEC=300
  ZOMBIENET_LOG=/tmp/deos-zombienet.log
  DEOS_BINARY_DIR=/tmp/deos-assurance-bin
  SESSION_TRANSITION=0|1 (set 1 for the multi-hour session proof)
  COMPOSED_PATH=0|1 (set 1 for the mutating Router/Oracle/Burn Actor proof)
  BLOCK_TARGET=100 and other 06-network-smoke.sh environment controls
  SESSION_TIMEOUT_SEC=28800 and other 08-session-transition.sh controls

Inputs:
  Clean repository locks/config, network access for setup, and local build capacity.

Outputs:
  Compact setup/build/network validation result plus retained Zombienet log;
  optional finalized session-transition evidence when SESSION_TRANSITION=1 and
  optional finalized composed-path evidence when COMPOSED_PATH=1.

Side effects:
  Downloads pinned tools/dependencies/binaries, builds artifacts, generates the local
  chain spec, starts local node processes, and submits one Alice-to-Bob transfer.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --keep-network) KEEP_NETWORK=1 ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    activate_pinned_node
    require_commands bash node npm rustup rg pgrep kill curl jq
    [[ "$KEEP_NETWORK" == "0" || "$KEEP_NETWORK" == "1" ]] || { log_error "KEEP_NETWORK must be 0 or 1"; exit 1; }
    [[ "$SESSION_TRANSITION" == "0" || "$SESSION_TRANSITION" == "1" ]] || { log_error "SESSION_TRANSITION must be 0 or 1"; exit 1; }
    [[ "$COMPOSED_PATH" == "0" || "$COMPOSED_PATH" == "1" ]] || { log_error "COMPOSED_PATH must be 0 or 1"; exit 1; }
}

cleanup() {
    local exit_code=$?
    if [[ -n "$RESTARTED_DAVE_PID" ]] && kill -0 "$RESTARTED_DAVE_PID" 2>/dev/null; then
        kill "$RESTARTED_DAVE_PID" 2>/dev/null || true
        wait "$RESTARTED_DAVE_PID" 2>/dev/null || true
    fi
    stop_background_process "$ZOMBIENET_PID" "$KEEP_NETWORK" "$ZOMBIENET_LOG" "zombienet"
    (( exit_code == 0 )) || log_error "Local network assurance failed"
}

prepare() {
    phase_banner "Step 2: Pinned environment and artifacts"
    run_script_step "Pinned full environment" "setup-environment.sh" full
    add_path_if_dir "$DEOS_BINARY_DIR"
    run_script_step "Local network tools" "02-install-tools.sh"
    run_script_step "Production runtime" "03-build-runtime.sh"
    run_script_step "Local chain spec" "04-generate-chain-spec.sh"
}

finalized_block_number() {
    local rpc_url="$1"
    local finalized_hash block_hex
    finalized_hash="$(curl -fsS -H 'Content-Type: application/json' -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' "$rpc_url" | jq -er '.result')"
    block_hex="$(curl -fsS -H 'Content-Type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$finalized_hash\"]}" "$rpc_url" | jq -er '.result.number')"
    printf '%d\n' "$block_hex"
}

verify_collator_failover() {
    local charlie_pid
    local -a charlie_pids
    mapfile -t charlie_pids < <(pgrep -f 'polkadot-omni-node --name charlie')
    charlie_pid="${charlie_pids[0]:-}"
    [[ -n "$charlie_pid" ]] || { log_error "Charlie process not found"; exit 1; }
    local before after deadline
    before="$(finalized_block_number 'http://127.0.0.1:9999')"
    kill -STOP "$charlie_pid"
    log_info "Paused Charlie process $charlie_pid at finalized block $before"
    deadline=$(( $(date +%s) + 120 ))
    after="$before"
    while (( after <= before )); do
        if (( $(date +%s) > deadline )); then
            kill -CONT "$charlie_pid" || true
            log_error "Finality did not advance through Dave while Charlie was paused"
            exit 1
        fi
        sleep 3
        after="$(finalized_block_number 'http://127.0.0.1:9999')"
    done
    kill -CONT "$charlie_pid"
    log_success "Dave preserved finalized progress from $before to $after while Charlie was paused"
}

restart_dave_with_persisted_state() {
    local network_dir="$1"
    local recorded_hash before after dave_pid dave_command deadline
    recorded_hash="$(curl -fsS -H 'Content-Type: application/json' -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' 'http://127.0.0.1:9999' | jq -er '.result')"
    before="$(finalized_block_number 'http://127.0.0.1:9999')"
    dave_pid="$(jq -er '.client.processMap.dave.pid' "$network_dir/zombie.json")"
    dave_command="$(jq -er '.client.processMap.dave.cmd[0]' "$network_dir/zombie.json")"
    kill -9 "$dave_pid"
    local attempt
    for attempt in {1..30}; do
        kill -0 "$dave_pid" 2>/dev/null || break
        sleep 1
    done
    kill -0 "$dave_pid" 2>/dev/null && { log_error "Dave process did not stop"; exit 1; }
    bash -c "$dave_command" >> "$network_dir/dave.log" 2>&1 &
    RESTARTED_DAVE_PID="$!"
    wait_for_chain_rpc "http://127.0.0.1:9999" "$RPC_READY_TIMEOUT_SEC" "Restarted Dave RPC" "$RESTARTED_DAVE_PID" "$network_dir/dave.log"
    curl -fsS -H 'Content-Type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$recorded_hash\"]}" 'http://127.0.0.1:9999' | jq -e '.result.number != null' >/dev/null || { log_error "Restarted Dave cannot read the pre-restart finalized block"; exit 1; }
    deadline=$(( $(date +%s) + 120 ))
    after="$before"
    while (( after <= before )); do
        (( $(date +%s) <= deadline )) || { log_error "Finality did not continue after Dave restart"; exit 1; }
        sleep 3
        after="$(finalized_block_number 'http://127.0.0.1:9999')"
    done
    local database_lines database_paths
    database_lines="$(rg -c 'Database: RocksDb at ' "$network_dir/dave.log")"
    database_paths="$(rg 'Database: RocksDb at ' "$network_dir/dave.log" | sed -E 's/.*Database: RocksDb at ([^ ]+).*/\1/' | sort -u | wc -l | tr -d ' ')"
    (( database_lines >= 2 && database_paths == 1 )) || { log_error "Dave did not reopen one stable database path"; exit 1; }
    if rg -qi 'database[^[:cntrl:]]*(corrupt|repair|purge|reset)|state[^[:cntrl:]]*loss' "$network_dir/dave.log"; then
        log_error "Dave restart log reports database repair/reset/corruption"
        exit 1
    fi
    log_success "Dave reopened persisted state at $recorded_hash and finality advanced from $before to $after"
}

verify_collator_participation() {
    local network_dir
    network_dir="$(rg -m1 -o "${TMPDIR:-/tmp}/zombie-[^ /]+" "$ZOMBIENET_LOG")"
    [[ -n "$network_dir" && -d "$network_dir" ]] || { log_error "Zombienet node-log directory not found"; exit 1; }
    local collator
    for collator in charlie dave; do
        local log_path="$network_dir/$collator.log"
        [[ -f "$log_path" ]] || { log_error "$collator node log not found"; exit 1; }
        rg -q 'Prepared block for proposing' "$log_path" || { log_error "$collator produced no block-preparation evidence"; exit 1; }
        log_success "$collator produced block-preparation evidence"
    done
}

run_network_proofs() {
    phase_banner "Step 3: Local network proofs"
    start_background_script "zombienet" "05-spawn-zombienet.sh" "$ZOMBIENET_LOG" ZOMBIENET_PID
    wait_for_chain_rpc "http://127.0.0.1:9988" "$RPC_READY_TIMEOUT_SEC" "Primary collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    wait_for_chain_rpc "http://127.0.0.1:9999" "$RPC_READY_TIMEOUT_SEC" "Secondary collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    wait_for_chain_rpc "http://127.0.0.1:9944" "$RPC_READY_TIMEOUT_SEC" "Relay RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    run_script_step "Finalized network smoke" "06-network-smoke.sh"
    verify_collator_participation
    if [[ "$SESSION_TRANSITION" == "1" ]]; then
        run_script_step "Finalized session transition" "08-session-transition.sh"
        verify_collator_participation
    fi
    if [[ "$COMPOSED_PATH" == "1" ]]; then
        run_script_step "Finalized composed economic path" "09-composed-economic-path.sh"
    fi
    verify_collator_failover
    run_script_step "Signed finalized network E2E" "07-network-e2e.sh"
    local network_dir
    network_dir="$(rg -m1 -o "${TMPDIR:-/tmp}/zombie-[^ /]+" "$ZOMBIENET_LOG")"
    restart_dave_with_persisted_state "$network_dir"
    DEOS_WS_ENDPOINT="ws://127.0.0.1:9999" run_script_step "Post-restart signed finalized E2E" "07-network-e2e.sh"
}

main() {
    parse_args "$@"
    check_prerequisites
    trap cleanup EXIT
    prepare
    run_network_proofs
    log_success "Local network assurance passed"
}

main "$@"
