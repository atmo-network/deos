#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

KEEP_NETWORK="${KEEP_NETWORK:-0}"
SESSION_TRANSITION="${SESSION_TRANSITION:-0}"
COMPOSED_PATH="${COMPOSED_PATH:-0}"
RELEASE_CANDIDATE_MODE="${RELEASE_CANDIDATE_MODE:-0}"
RELEASE_CANDIDATE_DIR="${RELEASE_CANDIDATE_DIR:-}"
RELEASE_CANDIDATE_MANIFEST_SHA256="${RELEASE_CANDIDATE_MANIFEST_SHA256:-}"
RELEASE_REPOSITORY_ID="${RELEASE_REPOSITORY_ID:-}"
RELEASE_TAG_REF="${RELEASE_TAG_REF:-}"
RELEASE_RUN_ID="${RELEASE_RUN_ID:-}"
RELEASE_RUN_ATTEMPT="${RELEASE_RUN_ATTEMPT:-}"
RELEASE_TOOL_LOCK="${RELEASE_TOOL_LOCK:-$PROJECT_ROOT/scripts/release-tools.v1.json}"
NETWORK_SUMMARY_PATH="${NETWORK_SUMMARY_PATH:-${TMPDIR:-/tmp}/deos-network-summary.json}"
NETWORK_PROOF_LEDGER="${NETWORK_PROOF_LEDGER:-${TMPDIR:-/tmp}/deos-network-proof-ledger.$$.$RANDOM.jsonl}"
PROCESS_CLEANUP_GRACE_ATTEMPTS="${PROCESS_CLEANUP_GRACE_ATTEMPTS:-10}"
RPC_READY_TIMEOUT_SEC="${RPC_READY_TIMEOUT_SEC:-300}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/deos-zombienet.log}"
DEOS_BINARY_DIR="${DEOS_BINARY_DIR:-$PROJECT_ROOT/bin}"
export DEOS_BINARY_DIR
BIN_DIR="$DEOS_BINARY_DIR"
ZOMBIENET_PID=""
RESTARTED_DAVE_PID=""
NETWORK_DIR=""
declare -a OWNED_PIDS=()

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
  DEOS_BINARY_DIR=/path/to/polkadot-binaries (default: PROJECT_ROOT/bin)
  SESSION_TRANSITION=0|1 (set 1 for the multi-hour session proof)
  COMPOSED_PATH=0|1 (set 1 for the mutating Router/Oracle/Burn Actor proof)
  RELEASE_CANDIDATE_MODE=0|1 (candidate mode consumes verified downloaded artifacts only)
  RELEASE_CANDIDATE_DIR, RELEASE_CANDIDATE_MANIFEST_SHA256, RELEASE_REPOSITORY_ID,
  RELEASE_TAG_REF, RELEASE_RUN_ID, RELEASE_RUN_ATTEMPT (required in candidate mode)
  RELEASE_TOOL_LOCK=scripts/release-tools.v1.json
  NETWORK_SUMMARY_PATH=/tmp/deos-network-summary.json
  BLOCK_TARGET=100 and other 06-network-smoke.sh environment controls
  SESSION_TIMEOUT_SEC=28800 and other 08-session-transition.sh controls

Inputs:
  Local mode uses clean repository locks/config, network access, and build capacity.
  Candidate mode uses an already verified exact handoff and immutable tool lock.

Outputs:
  Compact setup/build/network validation result plus retained Zombienet log;
  optional finalized session-transition evidence when SESSION_TRANSITION=1 and
  optional finalized composed-path evidence when COMPOSED_PATH=1.

Side effects:
  Local mode prepares dependencies, builds artifacts, and generates a chain spec.
  Candidate mode installs only immutable release tools, never builds or regenerates
  candidate artifacts, and verifies genesis :code. Both modes start local processes
  and submit the contracted transfers/path transactions.
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
    require_commands bash node npm rg pgrep kill curl jq sha256sum
    [[ "$KEEP_NETWORK" == "0" || "$KEEP_NETWORK" == "1" ]] || { log_error "KEEP_NETWORK must be 0 or 1"; exit 1; }
    [[ "$SESSION_TRANSITION" == "0" || "$SESSION_TRANSITION" == "1" ]] || { log_error "SESSION_TRANSITION must be 0 or 1"; exit 1; }
    [[ "$COMPOSED_PATH" == "0" || "$COMPOSED_PATH" == "1" ]] || { log_error "COMPOSED_PATH must be 0 or 1"; exit 1; }
    [[ "$RELEASE_CANDIDATE_MODE" == "0" || "$RELEASE_CANDIDATE_MODE" == "1" ]] || { log_error "RELEASE_CANDIDATE_MODE must be 0 or 1"; exit 1; }
    [[ "$PROCESS_CLEANUP_GRACE_ATTEMPTS" =~ ^[1-9][0-9]*$ ]] || { log_error "PROCESS_CLEANUP_GRACE_ATTEMPTS must be a positive integer"; exit 1; }
    if [[ "$RELEASE_CANDIDATE_MODE" == "1" ]]; then
        [[ "$COMPOSED_PATH" == "1" ]] || { log_error "Candidate mode requires COMPOSED_PATH=1"; exit 1; }
        [[ "$KEEP_NETWORK" == "0" ]] || { log_error "Candidate mode forbids KEEP_NETWORK=1 because cleanup is evidence-critical"; exit 1; }
        [[ ! -e "$NETWORK_SUMMARY_PATH" && ! -e "$NETWORK_PROOF_LEDGER" ]] || { log_error "Candidate summary/proof output already exists"; exit 1; }
        local value
        for value in "$RELEASE_CANDIDATE_DIR" "$RELEASE_CANDIDATE_MANIFEST_SHA256" "$RELEASE_REPOSITORY_ID" "$RELEASE_TAG_REF" "$RELEASE_RUN_ID" "$RELEASE_RUN_ATTEMPT"; do
            [[ -n "$value" ]] || { log_error "Candidate mode identity inputs are incomplete"; exit 1; }
        done
        require_directory "$RELEASE_CANDIDATE_DIR" "Verified candidate handoff"
        [[ -f "$RELEASE_TOOL_LOCK" ]] || { log_error "Immutable release tool lock not found: $RELEASE_TOOL_LOCK"; exit 1; }
    else
        require_commands rustup
    fi
}

record_proof() {
    [[ "$RELEASE_CANDIDATE_MODE" == "1" ]] || return 0
    node "$SCRIPT_DIR/release-evidence.mjs" append-proof --ledger "$NETWORK_PROOF_LEDGER" --id "$1"
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
    if [[ -n "$ZOMBIENET_PID" ]]; then OWNED_PIDS+=("$ZOMBIENET_PID"); collect_descendant_pids "$ZOMBIENET_PID"; fi
    [[ -n "$RESTARTED_DAVE_PID" ]] && OWNED_PIDS+=("$RESTARTED_DAVE_PID")
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
    [[ -n "$RESTARTED_DAVE_PID" ]] && wait "$RESTARTED_DAVE_PID" 2>/dev/null || true
    log_success "Every owned Zombienet, node, and restarted process is dead"
}

failure_cleanup() {
    local exit_code=$?
    trap - EXIT
    rm -f "$NETWORK_SUMMARY_PATH" "$NETWORK_PROOF_LEDGER"
    cleanup_owned_processes || exit_code=1
    log_error "Local network assurance failed"
    exit "$exit_code"
}

prepare() {
    phase_banner "Step 2: Pinned environment and artifacts"
    if [[ "$RELEASE_CANDIDATE_MODE" == "1" ]]; then
        node "$SCRIPT_DIR/release-evidence.mjs" verify \
            --input "$RELEASE_CANDIDATE_DIR" --repo "$PROJECT_ROOT" \
            --repository-id "$RELEASE_REPOSITORY_ID" --tag-ref "$RELEASE_TAG_REF" \
            --run-id "$RELEASE_RUN_ID" --run-attempt "$RELEASE_RUN_ATTEMPT" \
            --manifest-sha256 "$RELEASE_CANDIDATE_MANIFEST_SHA256" \
            --materialize "$PROJECT_ROOT"
        (cd "$PROJECT_ROOT/web-client" && npm ci --ignore-scripts)
        node "$SCRIPT_DIR/install-release-tools.mjs" install --lock "$RELEASE_TOOL_LOCK" --bin "$DEOS_BINARY_DIR"
        export PATH="$DEOS_BINARY_DIR:$PATH"
        node "$SCRIPT_DIR/install-release-tools.mjs" verify-path --lock "$RELEASE_TOOL_LOCK" --bin "$DEOS_BINARY_DIR"
        RUNTIME_WASM_PATH="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" \
            CHAIN_SPEC_PATH="$TEMPLATE_DIR/chain_spec.json" \
            run_script_step "Candidate chain spec" "04-generate-chain-spec.sh"
        node "$SCRIPT_DIR/release-evidence.mjs" verify-chain-code \
            --wasm "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" \
            --chain-spec "$TEMPLATE_DIR/chain_spec.json"
    else
        run_script_step "Pinned full environment" "setup-environment.sh" full
        add_path_if_dir "$DEOS_BINARY_DIR"
        run_script_step "Local network tools" "02-install-tools.sh"
        run_script_step "Production runtime" "03-build-runtime.sh"
        run_script_step "Local chain spec" "04-generate-chain-spec.sh"
    fi
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
    record_proof charliePauseDaveFinality
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
    record_proof persistedDaveRestart
}

verify_collator_participation() {
    local record="${1:-0}"
    NETWORK_DIR="$(rg -m1 -o "${TMPDIR:-/tmp}/zombie-[^ /]+" "$ZOMBIENET_LOG")"
    [[ -n "$NETWORK_DIR" && -d "$NETWORK_DIR" ]] || { log_error "Zombienet node-log directory not found"; exit 1; }
    local collator
    for collator in charlie dave; do
        local log_path="$NETWORK_DIR/$collator.log"
        [[ -f "$log_path" ]] || { log_error "$collator node log not found"; exit 1; }
        rg -q 'Prepared block for proposing' "$log_path" || { log_error "$collator produced no block-preparation evidence"; exit 1; }
        log_success "$collator produced block-preparation evidence"
        [[ "$record" == "1" ]] && record_proof "${collator}Authored"
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
    record_proof finalizedRelayAndTwoCollators
    verify_collator_participation 1
    if [[ "$SESSION_TRANSITION" == "1" ]]; then
        run_script_step "Finalized session transition" "08-session-transition.sh"
        verify_collator_participation 0
    fi
    verify_collator_failover
    run_script_step "Signed finalized network E2E" "07-network-e2e.sh"
    record_proof signedPreRestartTransfer
    restart_dave_with_persisted_state "$NETWORK_DIR"
    DEOS_WS_ENDPOINT="ws://127.0.0.1:9999" run_script_step "Post-restart signed finalized E2E" "07-network-e2e.sh"
    record_proof signedPostRestartTransfer
    if [[ "$COMPOSED_PATH" == "1" ]]; then
        WS_ENDPOINT="ws://127.0.0.1:9999" run_script_step "Finalized composed economic path through restarted Dave" "09-composed-economic-path.sh"
        record_proof routerOracleBurnActor
    fi
}

write_candidate_summary() {
    [[ "$RELEASE_CANDIDATE_MODE" == "1" ]] || return 0
    mkdir -p "$(dirname "$NETWORK_SUMMARY_PATH")"
    node "$SCRIPT_DIR/release-evidence.mjs" network-summary \
        --input "$RELEASE_CANDIDATE_DIR" --repo "$PROJECT_ROOT" \
        --repository-id "$RELEASE_REPOSITORY_ID" --tag-ref "$RELEASE_TAG_REF" \
        --run-id "$RELEASE_RUN_ID" --run-attempt "$RELEASE_RUN_ATTEMPT" \
        --manifest-sha256 "$RELEASE_CANDIDATE_MANIFEST_SHA256" \
        --wasm "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" \
        --chain-spec "$TEMPLATE_DIR/chain_spec.json" --tool-lock "$RELEASE_TOOL_LOCK" \
        --proof-ledger "$NETWORK_PROOF_LEDGER" --output "$NETWORK_SUMMARY_PATH"
}

main() {
    parse_args "$@"
    check_prerequisites
    trap failure_cleanup EXIT
    prepare
    run_network_proofs
    cleanup_owned_processes
    write_candidate_summary
    rm -f "$NETWORK_PROOF_LEDGER"
    trap - EXIT
    log_success "Local network assurance passed"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
