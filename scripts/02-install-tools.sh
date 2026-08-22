#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

ZOMBIENET_VERSION="1.3.138"
ZOMBIENET_INTEGRITY="sha512-hZ3n7y4SOwSP2C6sBL880b7qSK+EbSt39R2d/23fu90+VvH4bu3fkYRZb2sk6hio7zgIyBwQHpzqhhLL7DOJ1Q=="
CHAIN_SPEC_BUILDER_VERSION="19.0.0"
TRY_RUNTIME_VERSION="0.10.1"
TRY_RUNTIME_REVISION="6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be"
usage() {
    cat <<'EOF'
Usage: 02-install-tools.sh [OPTIONS]

Installs the exact repository-pinned operator tools.

Options:
  -h, --help  Show this help message

Tools:
  @zombienet/cli 1.3.138
  staging-chain-spec-builder 19.0.0
  try-runtime-cli 0.10.1 at Git revision 6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be

Inputs:
  Cargo, npm, and network access when an exact tool version is absent.

Outputs:
  The listed executables in the active npm/Cargo installation roots.

Side effects:
  Replaces mismatched tool versions; never accepts an arbitrary command already
  present on PATH and never installs from a mutable Git branch or tag.
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
    activate_pinned_node
    hydrate_local_tool_paths
    require_commands cargo npm
    log_success "Pinned tool installation prerequisites checked"
}

command_has_version() {
    local cmd="$1"
    local expected="$2"
    command -v "$cmd" &>/dev/null && "$cmd" --version 2>&1 | grep -Fq "$expected"
}

zombienet_has_version() {
    command -v zombienet &>/dev/null \
        && zombienet version 2>&1 | grep -Fq "$ZOMBIENET_VERSION"
}

install_cargo_version() {
    local cmd="$1"
    local pkg="$2"
    local version="$3"
    if command_has_version "$cmd" "$version"; then
        log_info "Reusing $cmd $version"
        return
    fi
    cargo install --locked --force --version "=$version" "$pkg"
    command_has_version "$cmd" "$version" || {
        log_error "$cmd did not report pinned version $version after installation"
        exit 1
    }
}

install_zombienet() {
    if zombienet_has_version; then
        log_info "Reusing zombienet $ZOMBIENET_VERSION"
        return
    fi
    local registry_integrity
    registry_integrity="$(npm view "@zombienet/cli@$ZOMBIENET_VERSION" dist.integrity)"
    [[ "$registry_integrity" == "$ZOMBIENET_INTEGRITY" ]] || {
        log_error "@zombienet/cli registry integrity does not match the repository pin"
        exit 1
    }
    npm install --global --save-exact "@zombienet/cli@$ZOMBIENET_VERSION"
    zombienet_has_version || {
        log_error "zombienet did not report pinned version $ZOMBIENET_VERSION after installation"
        exit 1
    }
}

install_try_runtime() {
    if command_has_version try-runtime "$TRY_RUNTIME_VERSION"; then
        log_info "Reusing try-runtime $TRY_RUNTIME_VERSION"
        return
    fi
    cargo install --git https://github.com/paritytech/try-runtime-cli \
        --rev "$TRY_RUNTIME_REVISION" --locked --force try-runtime-cli
    command_has_version try-runtime "$TRY_RUNTIME_VERSION" || {
        log_error "try-runtime did not report pinned version $TRY_RUNTIME_VERSION after installation"
        exit 1
    }
}

install_tools() {
    phase_banner "Step 2: Install exact tools"
    install_zombienet
    install_cargo_version chain-spec-builder staging-chain-spec-builder "$CHAIN_SPEC_BUILDER_VERSION"
    install_try_runtime
}

print_summary() {
    phase_banner "Summary"
    log_success "Requested repository tools match pinned identities"
}

main() {
    parse_args "$@"
    phase_banner "DEOS pinned tool installation"
    check_prerequisites
    install_tools
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
