#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd -P)"
export DEOS_PROJECT_ROOT="$PROJECT_ROOT"
export DEOS_BINARY_DIR="$PROJECT_ROOT/bin"
source "$PROJECT_ROOT/scripts/_common.sh"

CARGO_AUDIT_VERSION="0.22.2"
CARGO_DENY_VERSION="0.20.2"

usage() {
    cat <<'EOF'
Usage: prepare-tools.sh

Installs the exact Cargo Audit and Cargo Deny versions used by the private
release-assurance dependency review.

Inputs:
  Cargo and network access when an exact tool version is absent.

Outputs:
  cargo-audit 0.22.2 and cargo-deny 0.20.2 in the active Cargo binary root.

Side effects:
  Replaces a mismatched installed version. It does not alter project files.
EOF
}

parse_args() {
    case "${1:-}" in
        "") ;;
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
    [[ $# -le 1 ]] || {
        usage
        exit 1
    }
}

command_has_version() {
    local cmd="$1"
    local expected="$2"
    command -v "$cmd" &>/dev/null && "$cmd" --version 2>&1 | grep -Fq "$expected"
}

install_cargo_version() {
    local cmd="$1"
    local package="$2"
    local version="$3"
    if command_has_version "$cmd" "$version"; then
        log_info "Reusing $cmd $version"
        return
    fi
    cargo install --locked --force --version "=$version" "$package"
    command_has_version "$cmd" "$version" || {
        log_error "$cmd did not report pinned version $version after installation"
        exit 1
    }
}

check_prerequisites() {
    activate_pinned_node
    hydrate_local_tool_paths
    require_commands cargo grep
}

main() {
    parse_args "$@"
    phase_banner "Release-assurance tool preparation"
    check_prerequisites
    install_cargo_version cargo-audit cargo-audit "$CARGO_AUDIT_VERSION"
    install_cargo_version cargo-deny cargo-deny "$CARGO_DENY_VERSION"
    log_success "Private release-assurance tools match pinned identities"
}

main "$@"
