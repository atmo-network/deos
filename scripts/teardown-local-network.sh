#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: teardown-local-network.sh [OPTIONS]

Stops local network processes and removes Zombienet temp directories.
This includes a foreground local `web-client` Vite dev server when one is running on the default port.

Options:
  -h, --help        Show this help message
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
    phase_banner "Step 1: Teardown plan"
    require_commands pgrep pkill find rm
    log_success "Teardown prerequisites checked"
}

stop_processes() {
    log_info "Stopping local network processes"

    local stopped_processes=()

    if pgrep -f "zombienet" &>/dev/null; then
        pkill -f "zombienet" || true
        stopped_processes+=("zombienet")
    fi

    if pgrep -f "polkadot" &>/dev/null; then
        pkill -f "polkadot" || true
        stopped_processes+=("polkadot")
    fi

    if pgrep -f "vite dev.*(--port 5173|5173)" &>/dev/null; then
        pkill -f "vite dev.*(--port 5173|5173)" || true
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

    local removed_count=0
    while IFS= read -r -d '' dir; do
        if rm -rf "$dir" 2>/dev/null; then
            ((removed_count++))
        fi
    done < <(find /tmp -maxdepth 1 -type d -name "zombie-*" -print0 2>/dev/null || true)

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
