#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: teardown-local-network.sh [OPTIONS]

Stops local network processes and removes Zombienet temp directories.
This includes a foreground local `web-client` Vite dev server when one is running on the default port.

Only processes started from this repository are stopped: a match must either execute a binary under
the repository `bin/` directory or carry the repository path in its command line. This keeps an
unrelated Polkadot node, SDK build, or `@polkadot/*` tool on the same machine untouched.

Options:
      --all         Stop every matching process regardless of origin (dangerous)
  -h, --help        Show this help message
EOF
}

TEARDOWN_SCOPE="project"

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all)
                TEARDOWN_SCOPE="all"
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

check_prerequisites() {
    phase_banner "Step 1: Teardown plan"
    require_commands pgrep kill find rm ps
    if [[ "$TEARDOWN_SCOPE" == "all" ]]; then
        log_warning "--all stops every matching process on this machine, including unrelated ones"
    fi
    log_success "Teardown prerequisites checked"
}

# Emits the PIDs matching a pattern that this repository is responsible for. Ownership is proven by
# the executable living under BIN_DIR or by the repository path appearing in the command line, so a
# bare substring like "polkadot" can never reach an unrelated process.
repository_pids_for() {
    local pattern="$1"
    local pid exe command
    while IFS= read -r pid; do
        [[ -n "$pid" ]] || continue
        [[ "$pid" != "$$" ]] || continue
        if [[ "$TEARDOWN_SCOPE" == "all" ]]; then
            printf '%s\n' "$pid"
            continue
        fi
        if [[ -r "/proc/$pid/exe" ]]; then
            exe="$(readlink "/proc/$pid/exe" 2>/dev/null || true)"
            command="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
        else
            exe="$(ps -p "$pid" -o comm= 2>/dev/null || true)"
            command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
        fi
        if [[ -n "$exe" && "$exe" == "$BIN_DIR/"* ]]; then
            printf '%s\n' "$pid"
            continue
        fi
        if [[ -n "$command" && "$command" == *"$PROJECT_ROOT"* ]]; then
            printf '%s\n' "$pid"
        fi
    done < <(pgrep -f "$pattern" 2>/dev/null || true)
}

stop_matching_processes() {
    local pattern="$1"
    local pids=()
    while IFS= read -r pid; do
        [[ -n "$pid" ]] && pids+=("$pid")
    done < <(repository_pids_for "$pattern")
    [[ ${#pids[@]} -gt 0 ]] || return 1
    kill "${pids[@]}" 2>/dev/null || true
    return 0
}

stop_processes() {
    log_info "Stopping local network processes"

    local stopped_processes=()

    if stop_matching_processes "zombienet"; then
        stopped_processes+=("zombienet")
    fi

    if stop_matching_processes "polkadot"; then
        stopped_processes+=("polkadot")
    fi

    if stop_matching_processes "vite dev.*(--port 5173|5173)"; then
        stopped_processes+=("web-client")
    fi

    if [[ ${#stopped_processes[@]} -gt 0 ]]; then
        log_success "Stopped processes: ${stopped_processes[*]}"
    else
        log_info "No local network processes found"
    fi
}

clean_zombienet_temp() {
    log_info "Removing Zombienet temp directories"

    # Restricted to directories this user owns; a shared host may hold another user's run.
    local removed_count=0
    while IFS= read -r -d '' dir; do
        if rm -rf "$dir" 2>/dev/null; then
            removed_count=$((removed_count + 1))
        fi
    done < <(find /tmp -maxdepth 1 -type d -name "zombie-*" -user "$(id -u)" -print0 2>/dev/null || true)

    if [[ $removed_count -gt 0 ]]; then
        log_success "Removed $removed_count Zombienet temp directories"
    else
        log_info "No Zombienet temp directories found"
    fi
}

main() {
    parse_args "$@"
    phase_banner "DEOS local network teardown"
    check_prerequisites
    phase_banner "Step 2: Stop local network state"
    stop_processes
    clean_zombienet_temp
    log_success "Local network teardown completed successfully"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
