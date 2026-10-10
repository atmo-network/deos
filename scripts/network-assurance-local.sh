#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

KEEP_NETWORK="${KEEP_NETWORK:-0}"
PREPARED_MODE=""
SELF_TEST=0
SESSION_TRANSITION="${SESSION_TRANSITION:-0}"
COMPOSED_PATH="${COMPOSED_PATH:-0}"
PROCESS_CLEANUP_GRACE_ATTEMPTS="${PROCESS_CLEANUP_GRACE_ATTEMPTS:-10}"
RPC_READY_TIMEOUT_SEC="${RPC_READY_TIMEOUT_SEC:-300}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/deos-zombienet.log}"
DEOS_BINARY_DIR="${DEOS_BINARY_DIR:-$PROJECT_ROOT/bin}"
export DEOS_BINARY_DIR
BIN_DIR="$DEOS_BINARY_DIR"
ZOMBIENET_PID=""
RESTARTED_SECONDARY_PID=""
PAUSED_PRIMARY_PID=""
NETWORK_DIR=""
declare -a OWNED_PIDS=()

usage() {
    cat <<'EOF'
Usage: network-assurance-local.sh [OPTIONS]

Prepare pinned dependencies and artifacts, spawn the canonical two-validator /
two-collator local topology, then run finalized-progress smoke and signed-transfer E2E.

Options:
  --prepared-smoke  Reuse prepared artifacts; run only readonly smoke/participation.
                    Always clean up owned processes; reject optional mutation modes.
  --prepared-recovery  Reuse prepared inputs; add owned primary pause/secondary restart.
                       Compare complete historical TOP/header; never submit extrinsics.
  --self-test       Test routing/guards and snapshot parsing with jq; no network/state mutation.
  --keep-network    Leave the owned Zombienet process running after full validation.
  -h, --help        Show this help message.

Environment:
  KEEP_NETWORK=0|1
  RPC_READY_TIMEOUT_SEC=300
  ZOMBIENET_LOG=/tmp/deos-zombienet.log
  DEOS_BINARY_DIR=/path/to/polkadot-binaries (default: PROJECT_ROOT/bin)
  DEOS_OMNI_NODE_PATH=<path> and DEOS_OMNI_NODE_SHA256=<sha> select a qualified Omni
  SESSION_TRANSITION=0|1 (set 1 for the multi-hour session proof)
  COMPOSED_PATH=0|1 (set 1 for the mutating Router/Oracle/Burn Actor proof)
  BLOCK_TARGET=100 and other 06-network-smoke.sh environment controls
  NETWORK_WORK_DIR, ZOMBIENET_CONFIG, CHAIN_SPEC_PATH select prepared inputs
  SESSION_TIMEOUT_SEC=28800 and other 08-session-transition.sh controls

Inputs:
  Repository locks/config, network access, and local build capacity.

Outputs:
  Compact setup/build/network validation result plus retained Zombienet log;
  optional finalized session-transition evidence when SESSION_TRANSITION=1 and
  optional finalized composed-path evidence when COMPOSED_PATH=1.

Side effects:
  Default: prepares dependencies, builds artifacts, generates a chain spec, starts
  local processes, and submits the contracted transfers/path transactions.
  --prepared-smoke: skips preparation and all signed/session/composed/failover/restart
  proofs. Starts/stops only its temporary network; runtime genesis is not patched.
  --prepared-recovery: additionally pauses/resumes the owned primary, SIGKILLs and
  restarts the owned secondary at its existing base path, and captures historical TOP
  snapshots under the Zombienet directory. Prepared modes are mutually exclusive.
  These are finite liveness/recovery points, not transaction, sandbox or release acceptance.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --keep-network) KEEP_NETWORK=1 ;;
            --prepared-smoke|--prepared-recovery)
                local requested="${1#--prepared-}"
                [[ -z "$PREPARED_MODE" || "$PREPARED_MODE" == "$requested" ]] || { log_error "Select one prepared mode"; return 1; }
                PREPARED_MODE="$requested"
                ;;
            --self-test) SELF_TEST=1 ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

validate_mode() {
    if [[ -n "$PREPARED_MODE" && ( "$KEEP_NETWORK" != "0" || "$SESSION_TRANSITION" != "0" || "$COMPOSED_PATH" != "0" ) ]]; then
        log_error "Prepared modes require cleanup and exclude session/composed proofs"
        return 1
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    validate_mode
    activate_pinned_node
    require_commands bash node npm rg pgrep kill curl jq sha256sum cmp
    [[ "$KEEP_NETWORK" == "0" || "$KEEP_NETWORK" == "1" ]] || { log_error "KEEP_NETWORK must be 0 or 1"; exit 1; }
    [[ "$SESSION_TRANSITION" == "0" || "$SESSION_TRANSITION" == "1" ]] || { log_error "SESSION_TRANSITION must be 0 or 1"; exit 1; }
    [[ "$COMPOSED_PATH" == "0" || "$COMPOSED_PATH" == "1" ]] || { log_error "COMPOSED_PATH must be 0 or 1"; exit 1; }
    [[ "$PROCESS_CLEANUP_GRACE_ATTEMPTS" =~ ^[1-9][0-9]*$ ]] || { log_error "PROCESS_CLEANUP_GRACE_ATTEMPTS must be a positive integer"; exit 1; }
    require_commands rustup
}

collect_descendant_pids() {
    local parent="$1" child
    while IFS= read -r child; do
        [[ "$child" =~ ^[1-9][0-9]*$ ]] || continue
        OWNED_PIDS+=("$child")
        collect_descendant_pids "$child"
    done < <(pgrep -P "$parent" 2>/dev/null || true)
}

refresh_owned_pids() {
    local pid
    if [[ -z "$NETWORK_DIR" && -f "$ZOMBIENET_LOG" ]]; then
        NETWORK_DIR="$(rg -m1 -o "${TMPDIR:-/tmp}/zombie-[^ /]+" "$ZOMBIENET_LOG" || true)"
    fi
    if [[ -n "$ZOMBIENET_PID" ]]; then OWNED_PIDS+=("$ZOMBIENET_PID"); collect_descendant_pids "$ZOMBIENET_PID"; fi
    if [[ -n "$RESTARTED_SECONDARY_PID" ]]; then OWNED_PIDS+=("$RESTARTED_SECONDARY_PID"); collect_descendant_pids "$RESTARTED_SECONDARY_PID"; fi
    if [[ -n "$NETWORK_DIR" && -f "$NETWORK_DIR/zombie.json" ]]; then
        while IFS= read -r pid; do [[ "$pid" =~ ^[1-9][0-9]*$ ]] && OWNED_PIDS+=("$pid"); done < <(jq -r '.. | objects | .pid? // empty' "$NETWORK_DIR/zombie.json")
    fi
}

cleanup_owned_processes() {
    if [[ "$KEEP_NETWORK" == "1" ]]; then
        log_warning "KEEP_NETWORK=1, leaving the local network running"
        return 0
    fi
    refresh_owned_pids
    if [[ -n "$PAUSED_PRIMARY_PID" ]]; then
        kill -CONT "$PAUSED_PRIMARY_PID" 2>/dev/null || true
        PAUSED_PRIMARY_PID=""
    fi
    local -a unique=() live=()
    local pid seen
    for pid in "${OWNED_PIDS[@]}"; do
        seen=0
        local existing
        for existing in "${unique[@]}"; do [[ "$existing" == "$pid" ]] && seen=1; done
        (( seen == 1 )) || unique+=("$pid")
    done
    for pid in "${unique[@]}"; do kill -0 "$pid" 2>/dev/null && { kill -TERM "$pid" || return 1; live+=("$pid"); }; done
    local attempt
    for (( attempt = 1; attempt <= PROCESS_CLEANUP_GRACE_ATTEMPTS; attempt += 1 )); do
        live=()
        for pid in "${unique[@]}"; do kill -0 "$pid" 2>/dev/null && live+=("$pid"); done
        (( ${#live[@]} == 0 )) && break
        sleep 1
    done
    for pid in "${live[@]}"; do kill -KILL "$pid" || return 1; done
    sleep 1
    for pid in "${unique[@]}"; do kill -0 "$pid" 2>/dev/null && { log_error "Owned network process remains alive after SIGKILL: $pid"; return 1; }; done
    [[ -n "$ZOMBIENET_PID" ]] && wait "$ZOMBIENET_PID" 2>/dev/null || true
    [[ -n "$RESTARTED_SECONDARY_PID" ]] && wait "$RESTARTED_SECONDARY_PID" 2>/dev/null || true
    log_success "Every owned Zombienet, node, and restarted process is dead"
}

failure_cleanup() {
    local exit_code=$?
    trap - EXIT
    cleanup_owned_processes || exit_code=1
    log_error "Local network assurance failed"
    exit "$exit_code"
}

prepare() {
    phase_banner "Step 2: Pinned environment and artifacts"
    run_script_step "Pinned full environment" "setup-environment.sh" full
    add_path_if_dir "$DEOS_BINARY_DIR"
    run_script_step "Pinned node binaries" "01-download-binaries.sh"
    verify_deos_omni_node "$(command -v polkadot-omni-node-deos || true)"
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
    local primary_pid
    primary_pid="$(jq -er '.client.processMap["alice-1"].pid' "$NETWORK_DIR/zombie.json")"
    kill -0 "$primary_pid" 2>/dev/null || { log_error "Primary collator process not found"; exit 1; }
    local before after deadline preparations_before preparations_after
    preparations_before="$(rg -c 'Prepared block for proposing' "$NETWORK_DIR/bob-1.log")"
    preparations_after="$preparations_before"
    before="$(finalized_block_number 'http://127.0.0.1:9999')"
    PAUSED_PRIMARY_PID="$primary_pid"
    kill -STOP "$primary_pid"
    log_info "Paused primary collator process $primary_pid at finalized block $before; secondary preparations $preparations_before"
    deadline=$(( $(date +%s) + 120 ))
    after="$before"
    while (( after <= before || preparations_after <= preparations_before )); do
        if (( $(date +%s) > deadline )); then
            kill -CONT "$primary_pid" || true
            log_error "Finality did not advance through the secondary collator while the primary was paused"
            exit 1
        fi
        sleep 3
        after="$(finalized_block_number 'http://127.0.0.1:9999')"
        preparations_after="$(rg -c 'Prepared block for proposing' "$NETWORK_DIR/bob-1.log")"
    done
    kill -CONT "$primary_pid"
    PAUSED_PRIMARY_PID=""
    log_success "While primary was paused, secondary view finalized $before->$after and preparations $preparations_before->$preparations_after; author inclusion is separate"
}

canonical_top_pairs() {
    jq -cSe '.result | select(type == "array" and length > 0)
        | select(all(.[]; type == "array" and length == 2
            and all(.[]; type == "string" and test("^0x([0-9a-fA-F]{2})*$"))))
        | sort_by(.[0]) | select((map(.[0]) | unique | length) == length)
        | select(any(.[]; .[0] == "0x3a636f6465"))'
}

snapshot_top_at() {
    local hash="$1" output="$2"
    curl -fsS -H 'Content-Type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"state_getPairs\",\"params\":[\"0x\",\"$hash\"]}" 'http://127.0.0.1:9999' | canonical_top_pairs > "$output"
}

restart_secondary_with_persisted_state() {
    local network_dir="$1"
    local recorded_hash recorded_header restored_header before after secondary_pid secondary_command deadline
    recorded_hash="$(curl -fsS -H 'Content-Type: application/json' -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' 'http://127.0.0.1:9999' | jq -er '.result | select(type == "string" and test("^0x[0-9a-fA-F]{64}$"))')"
    recorded_header="$(curl -fsS -H 'Content-Type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$recorded_hash\"]}" 'http://127.0.0.1:9999' | jq -cSe '.result | select(type == "object")')"
    before="$(jq -er '.number | select(type == "string" and test("^0x[0-9a-fA-F]+$"))' <<< "$recorded_header")"
    before=$((16#${before#0x}))
    snapshot_top_at "$recorded_hash" "$network_dir/bob-1-before-restart.top.json"
    printf '%s\n' "$recorded_header" > "$network_dir/bob-1-before-restart.header.json"
    secondary_pid="$(jq -er '.client.processMap["bob-1"].pid' "$network_dir/zombie.json")"
    secondary_command="$(jq -er '.client.processMap["bob-1"].cmd[0]' "$network_dir/zombie.json")"
    kill -9 "$secondary_pid"
    local attempt
    for attempt in {1..30}; do
        kill -0 "$secondary_pid" 2>/dev/null || break
        sleep 1
    done
    kill -0 "$secondary_pid" 2>/dev/null && { log_error "Secondary collator process did not stop"; exit 1; }
    bash -c "$secondary_command" >> "$network_dir/bob-1.log" 2>&1 &
    RESTARTED_SECONDARY_PID="$!"
    wait_for_chain_rpc "http://127.0.0.1:9999" "$RPC_READY_TIMEOUT_SEC" "Restarted secondary collator RPC" "$RESTARTED_SECONDARY_PID" "$network_dir/bob-1.log"
    restored_header="$(curl -fsS -H 'Content-Type: application/json' -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$recorded_hash\"]}" 'http://127.0.0.1:9999' | jq -cSe '.result | select(type == "object")')"
    [[ "$restored_header" == "$recorded_header" ]] || { log_error "Restarted secondary header differs at retained finalized hash"; exit 1; }
    snapshot_top_at "$recorded_hash" "$network_dir/bob-1-after-restart.top.json"
    cmp "$network_dir/bob-1-before-restart.top.json" "$network_dir/bob-1-after-restart.top.json" || { log_error "Restarted secondary complete TOP differs"; exit 1; }
    log_success "Complete historical TOP/header match at $recorded_hash ($(jq 'length' "$network_dir/bob-1-before-restart.top.json") TOP pairs)"
    deadline=$(( $(date +%s) + 120 ))
    after="$before"
    while (( after <= before )); do
        (( $(date +%s) <= deadline )) || { log_error "Finality did not continue after Secondary collator restart"; exit 1; }
        sleep 3
        after="$(finalized_block_number 'http://127.0.0.1:9999')"
    done
    local database_lines database_paths
    database_lines="$(rg -c 'Database: RocksDb at ' "$network_dir/bob-1.log")"
    database_paths="$(rg 'Database: RocksDb at ' "$network_dir/bob-1.log" | sed -E 's/.*Database: RocksDb at ([^ ]+).*/\1/' | sort -u | wc -l | tr -d ' ')"
    (( database_lines >= 2 && database_paths == 1 )) || { log_error "Secondary collator did not reopen one stable database path"; exit 1; }
    if rg -qi 'database[^[:cntrl:]]*(corrupt|repair|purge|reset)|state[^[:cntrl:]]*loss' "$network_dir/bob-1.log"; then
        log_error "Secondary collator restart log reports database repair/reset/corruption"
        exit 1
    fi
    log_success "Secondary collator reopened persisted state at $recorded_hash and finality advanced from $before to $after"
}

verify_collator_participation() {
    NETWORK_DIR="$(rg -m1 -o "${TMPDIR:-/tmp}/zombie-[^ /]+" "$ZOMBIENET_LOG")"
    [[ -n "$NETWORK_DIR" && -d "$NETWORK_DIR" ]] || { log_error "Zombienet node-log directory not found"; exit 1; }
    local collator
    for collator in alice-1 bob-1; do
        local log_path="$NETWORK_DIR/$collator.log"
        [[ -f "$log_path" ]] || { log_error "$collator node log not found"; exit 1; }
        rg -q 'Prepared block for proposing' "$log_path" || { log_error "$collator produced no block-preparation evidence"; exit 1; }
        log_success "$collator produced block-preparation evidence"
    done
    refresh_owned_pids
}

run_network_proofs() {
    phase_banner "Step 3: Local network proofs"
    start_background_script "zombienet" "05-spawn-zombienet.sh" "$ZOMBIENET_LOG" ZOMBIENET_PID
    wait_for_chain_rpc "http://127.0.0.1:9988" "$RPC_READY_TIMEOUT_SEC" "Primary collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    wait_for_chain_rpc "http://127.0.0.1:9999" "$RPC_READY_TIMEOUT_SEC" "Secondary collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    wait_for_chain_rpc "http://127.0.0.1:9944" "$RPC_READY_TIMEOUT_SEC" "Relay RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    run_script_step "Finalized network smoke" "06-network-smoke.sh"
    verify_collator_participation
    if [[ "$PREPARED_MODE" == "smoke" ]]; then
        return
    elif [[ "$PREPARED_MODE" == "recovery" ]]; then
        verify_collator_failover
        restart_secondary_with_persisted_state "$NETWORK_DIR"
        return
    fi
    if [[ "$SESSION_TRANSITION" == "1" ]]; then
        run_script_step "Finalized session transition" "08-session-transition.sh"
        verify_collator_participation
    fi
    verify_collator_failover
    run_script_step "Signed finalized network E2E" "07-network-e2e.sh"
    restart_secondary_with_persisted_state "$NETWORK_DIR"
    DEOS_WS_ENDPOINT="ws://127.0.0.1:9999" run_script_step "Post-restart signed finalized E2E" "07-network-e2e.sh"
    if [[ "$COMPOSED_PATH" == "1" ]]; then
        WS_ENDPOINT="ws://127.0.0.1:9999" run_script_step "Finalized composed economic path through restarted secondary collator" "09-composed-economic-path.sh"
    fi
}

run_self_tests() {
    (
        local calls=""
        phase_banner() { :; }
        start_background_script() { calls+="spawn "; }
        wait_for_chain_rpc() { :; }
        run_script_step() { calls+="$2 "; }
        verify_collator_participation() { calls+="participation "; }
        verify_collator_failover() { calls+="failover "; }
        restart_secondary_with_persisted_state() { calls+="restart "; }
        KEEP_NETWORK=0 SESSION_TRANSITION=0 COMPOSED_PATH=0 PREPARED_MODE=smoke
        validate_mode
        run_network_proofs
        [[ "$calls" == "spawn 06-network-smoke.sh participation " ]] || exit 1
        calls="" PREPARED_MODE=recovery
        validate_mode
        run_network_proofs
        [[ "$calls" == "spawn 06-network-smoke.sh participation failover restart " ]] || exit 1
        calls="" PREPARED_MODE="" SESSION_TRANSITION=1 COMPOSED_PATH=1
        run_network_proofs
        [[ "$calls" == "spawn 06-network-smoke.sh participation 08-session-transition.sh participation failover 07-network-e2e.sh restart 07-network-e2e.sh 09-composed-economic-path.sh " ]] || exit 1
        local mode
        for mode in smoke recovery; do
            PREPARED_MODE="$mode" SESSION_TRANSITION=0 COMPOSED_PATH=0 KEEP_NETWORK=1
            if validate_mode >/dev/null 2>&1; then exit 1; fi
            KEEP_NETWORK=0 SESSION_TRANSITION=1
            if validate_mode >/dev/null 2>&1; then exit 1; fi
            SESSION_TRANSITION=0 COMPOSED_PATH=1
            if validate_mode >/dev/null 2>&1; then exit 1; fi
        done
        PREPARED_MODE=""
        if (parse_args --prepared-smoke --prepared-recovery) >/dev/null 2>&1; then exit 1; fi
        PREPARED_MODE=""
        if (parse_args --prepared-recovery --prepared-smoke) >/dev/null 2>&1; then exit 1; fi
        local sorted invalid
        sorted="$(canonical_top_pairs <<< '{"result":[["0x3a636f6465","0xab"],["0x01","0x"]]}')"
        [[ "$sorted" == '[["0x01","0x"],["0x3a636f6465","0xab"]]' ]] || exit 1
        for invalid in '{"error":{"message":"unavailable"}}' '{"result":[]}' '{"result":[["0x01","0xff"]]}' '{"result":[["0x3a636f6465","0xab"],["0x3a636f6465","0xcd"]]}' '{"result":[["0x3a636f6465","0xa"]]}' '{"result":[["0x3a636f6465",1]]}'; do
            if canonical_top_pairs <<< "$invalid" >/dev/null 2>&1; then exit 1; fi
        done
    ) || { log_error "Network assurance mode self-test failed"; return 1; }
    log_success "Smoke/recovery/full routing, excluded-mode guards and complete TOP parsing passed"
}

main() {
    parse_args "$@"
    if [[ "$SELF_TEST" == "1" ]]; then require_commands jq; run_self_tests; return; fi
    check_prerequisites
    trap failure_cleanup EXIT
    if [[ -n "$PREPARED_MODE" ]]; then
        hydrate_local_tool_paths
        require_commands polkadot zombienet
        verify_deos_omni_node "$(command -v polkadot-omni-node-deos || true)"
    else
        prepare
    fi
    run_network_proofs
    cleanup_owned_processes
    trap - EXIT
    if [[ -n "$PREPARED_MODE" ]]; then
        log_success "Prepared $PREPARED_MODE network proofs passed; signed/composed acceptance excluded"
    else
        log_success "Local network assurance passed"
    fi
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
