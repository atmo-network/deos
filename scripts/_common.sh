#!/usr/bin/env bash
# Shared utilities for DEOS scripts
# Source this file: source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[1]:-${BASH_SOURCE[0]}}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TEMPLATE_DIR="$PROJECT_ROOT/template"
BIN_DIR="$PROJECT_ROOT/bin"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

phase_banner() {
    echo -e "\n${CYAN}--- $1 ---${NC}\n"
}

add_path_if_dir() {
    local dir="$1"
    if [[ ! -d "$dir" ]]; then
        return 0
    fi
    case ":$PATH:" in
        *":$dir:"*) return 0 ;;
    esac
    export PATH="$dir:$PATH"
    log_info "Added $dir to PATH"
}

hydrate_local_tool_paths() {
    add_path_if_dir "$BIN_DIR"
    add_path_if_dir "$HOME/.cargo/bin"
}

require_directory() {
    local dir="$1"
    local label="$2"
    if [[ ! -d "$dir" ]]; then
        log_error "$label not found: $dir"
        exit 1
    fi
}

require_local_script() {
    local script_name="$1"
    local script_path="$SCRIPT_DIR/$script_name"
    if [[ ! -x "$script_path" ]]; then
        log_error "Script not found or not executable: $script_name"
        exit 1
    fi
}

require_commands() {
    local missing=()
    local cmd
    for cmd in "$@"; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            missing+=("$cmd")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing required commands: ${missing[*]}"
        exit 1
    fi
}

run_script_step() {
    local label="$1"
    local script_name="$2"
    shift 2

    require_local_script "$script_name"
    log_info "Running: $label ($script_name)"
    local start_time
    start_time=$(date +%s)
    "$SCRIPT_DIR/$script_name" "$@"
    local end_time
    end_time=$(date +%s)
    log_success "$label completed in $((end_time - start_time))s"
}

run_shell_step() {
    local label="$1"
    local timeout_minutes="$2"
    local command="$3"

    log_info "Running: $label"
    log_info "Command: $command"

    local start_time
    local end_time
    local status
    start_time=$(date +%s)
    if [[ -n "$timeout_minutes" ]]; then
        if timeout "${timeout_minutes}m" bash -lc "$command"; then
            status=0
        else
            status=$?
        fi
    elif bash -lc "$command"; then
        status=0
    else
        status=$?
    fi
    end_time=$(date +%s)

    if [[ "$status" -ne 0 ]]; then
        log_error "$label failed with status $status after $((end_time - start_time))s"
        return "$status"
    fi
    log_success "$label completed in $((end_time - start_time))s"
}

start_background_script() {
    local label="$1"
    local script_name="$2"
    local log_path="$3"
    local pid_var="$4"
    local -n pid_ref="$pid_var"

    require_local_script "$script_name"
    log_info "Starting $label in background"
    log_info "Log file: $log_path"
    "$SCRIPT_DIR/$script_name" >"$log_path" 2>&1 &
    pid_ref="$!"
    log_success "$label started with PID $pid_ref"
}

start_background_command() {
    local label="$1"
    local workdir="$2"
    local command="$3"
    local log_path="$4"
    local pid_var="$5"
    local -n pid_ref="$pid_var"

    require_directory "$workdir" "$label working directory"
    log_info "Starting $label in background"
    log_info "Working directory: $workdir"
    log_info "Command: $command"
    log_info "Log file: $log_path"
    (
        cd "$workdir"
        bash -lc "$command"
    ) >"$log_path" 2>&1 &
    pid_ref="$!"
    log_success "$label started with PID $pid_ref"
}

wait_for_chain_rpc() {
    local http_uri="$1"
    local timeout_sec="$2"
    local label="$3"
    local pid="${4:-}"
    local log_path="${5:-}"
    local deadline=$((SECONDS + timeout_sec))

    while (( SECONDS < deadline )); do
        if curl -sS \
            -H 'Content-Type: application/json' \
            -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
            "$http_uri" >/dev/null 2>&1; then
            log_success "$label is reachable at $http_uri"
            return 0
        fi

        if [[ -n "$pid" ]] && ! kill -0 "$pid" >/dev/null 2>&1; then
            log_error "$label producer exited before RPC became ready"
            if [[ -n "$log_path" ]]; then
                log_info "Check log: $log_path"
            fi
            return 1
        fi

        sleep 2
    done

    log_error "Timed out waiting for $label at $http_uri (timeout=${timeout_sec}s)"
    if [[ -n "$log_path" ]]; then
        log_info "Check log: $log_path"
    fi
    return 1
}

wait_for_http() {
    local http_uri="$1"
    local timeout_sec="$2"
    local label="$3"
    local pid="${4:-}"
    local log_path="${5:-}"
    local deadline=$((SECONDS + timeout_sec))

    while (( SECONDS < deadline )); do
        if curl -fsS "$http_uri" >/dev/null 2>&1; then
            log_success "$label is reachable at $http_uri"
            return 0
        fi

        if [[ -n "$pid" ]] && ! kill -0 "$pid" >/dev/null 2>&1; then
            log_error "$label producer exited before HTTP became ready"
            if [[ -n "$log_path" ]]; then
                log_info "Check log: $log_path"
            fi
            return 1
        fi

        sleep 2
    done

    log_error "Timed out waiting for $label at $http_uri (timeout=${timeout_sec}s)"
    if [[ -n "$log_path" ]]; then
        log_info "Check log: $log_path"
    fi
    return 1
}

stop_background_process() {
    local pid="${1:-}"
    local keep_running="${2:-0}"
    local log_path="${3:-}"
    local label="${4:-process}"

    if [[ -z "$pid" ]]; then
        return 0
    fi

    if [[ "$keep_running" == "1" ]]; then
        log_warning "KEEP_NETWORK=1, leaving $label running (PID: $pid)"
        if [[ -n "$log_path" ]]; then
            log_info "Log file: $log_path"
        fi
        return 0
    fi

    if kill -0 "$pid" >/dev/null 2>&1; then
        log_info "Stopping $label (PID: $pid)"
        kill "$pid" >/dev/null 2>&1 || true
        for _ in {1..10}; do
            if ! kill -0 "$pid" >/dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        if kill -0 "$pid" >/dev/null 2>&1; then
            log_warning "$label did not stop gracefully, sending SIGKILL"
            kill -9 "$pid" >/dev/null 2>&1 || true
        fi
        log_success "$label stopped"
    fi
}
