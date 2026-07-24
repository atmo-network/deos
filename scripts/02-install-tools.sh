#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: 02-install-tools.sh [OPTIONS]

Installs local cargo-based tooling used by the repository.

Options:
  -h, --help        Show this help message

Tools:
  zombienet
  chain-spec-builder (package: staging-chain-spec-builder)
  try-runtime

Inputs:
  Cargo and network access when a requested tool is absent.

Outputs:
  The listed executables in the active Cargo installation root.

Side effects:
  Installs only missing tools; preserves commands already available on PATH.
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
    require_commands cargo
    log_success "Tool installation prerequisites checked"
}

install_if_missing() {
    local cmd="$1"
    local pkg="${2:-$1}"

    if command -v "$cmd" &>/dev/null; then
        log_warning "$cmd already installed at: $(command -v "$cmd")"
    else
        log_info "Installing $pkg..."
        if cargo install --locked "$pkg"; then
            log_success "$cmd installed successfully"
        else
            log_error "Failed to install $pkg"
            exit 1
        fi
    fi
}

install_git_if_missing() {
    local cmd="$1"
    local repo="$2"

    if command -v "$cmd" &>/dev/null; then
        log_warning "$cmd already installed at: $(command -v "$cmd")"
    else
        log_info "Installing $cmd from $repo..."
        if cargo install --git "$repo" --locked; then
            log_success "$cmd installed successfully"
        else
            log_error "Failed to install $cmd from $repo"
            exit 1
        fi
    fi
}

install_tools() {
    phase_banner "Step 2: Install cargo tools"
    install_if_missing "zombienet" "zombienet"
    install_if_missing "chain-spec-builder" "staging-chain-spec-builder"
    install_git_if_missing "try-runtime" "https://github.com/paritytech/try-runtime-cli"
}

print_summary() {
    phase_banner "Summary"
    log_success "All cargo tools installation complete"
}

main() {
    parse_args "$@"
    phase_banner "DEOS cargo tool installation"
    check_prerequisites
    install_tools
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
