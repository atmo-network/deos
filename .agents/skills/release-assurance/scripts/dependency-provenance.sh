#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
PROJECT_ROOT="$(cd "$SKILL_DIR/../../.." && pwd -P)"
export DEOS_PROJECT_ROOT="$PROJECT_ROOT"
export DEOS_BINARY_DIR="$PROJECT_ROOT/bin"
source "$PROJECT_ROOT/scripts/_common.sh"

EXCEPTIONS="$SKILL_DIR/config/dependency-provenance-exceptions.json"
DENY_CONFIG="$SKILL_DIR/config/deny.toml"
CARGO_AUDIT_VERSION="0.22.2"
CARGO_DENY_VERSION="0.20.2"
CARGO_JSON=""
NPM_JSON=""

usage() {
    cat <<'EOF'
Usage: dependency-provenance.sh [OPTIONS]

Validates locked release inputs, exact toolchains, binary checksums, dependency
licenses/sources, and current Rust/npm vulnerability findings against the dated
reachability exception ledger.

Options:
  -h, --help        Show this help message

Inputs:
  template/Cargo.lock, template/rust-toolchain.toml
  web-client/package-lock.json, web-client/package.json
  release-assurance/config/deny.toml and dependency-provenance-exceptions.json
  Current RustSec and npm advisory services
  Existing checksum-verified ./bin bundle

Outputs:
  A concise pass/fail review; temporary audit JSON is deleted on exit.

Side effects:
  Refreshes the local RustSec advisory database and performs npm registry reads.
  It does not install packages, mutate lockfiles, or alter repository artifacts.
EOF
}

parse_args() {
    if [[ $# -gt 1 ]]; then
        usage
        exit 1
    fi
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
}

cleanup() {
    [[ -z "$CARGO_JSON" ]] || rm -f -- "$CARGO_JSON"
    [[ -z "$NPM_JSON" ]] || rm -f -- "$NPM_JSON"
}

command_has_version() {
    local cmd="$1"
    local expected="$2"
    "$cmd" --version 2>&1 | grep -Fq "$expected"
}

check_prerequisites() {
    phase_banner "Step 1: Pinned identities"
    activate_pinned_node
    require_commands cargo rustc node npm cargo-audit cargo-deny mktemp rm grep
    command_has_version cargo-audit "$CARGO_AUDIT_VERSION" || {
        log_error "cargo-audit $CARGO_AUDIT_VERSION is required; run $SCRIPT_DIR/prepare-tools.sh"
        exit 1
    }
    command_has_version cargo-deny "$CARGO_DENY_VERSION" || {
        log_error "cargo-deny $CARGO_DENY_VERSION is required; run $SCRIPT_DIR/prepare-tools.sh"
        exit 1
    }
    [[ -f "$EXCEPTIONS" ]] || { log_error "Dependency exception ledger is missing"; exit 1; }
    (cd "$TEMPLATE_DIR" && rustc --version | grep -Fq 'rustc 1.94.1 ')
    node -e '
      const p = require(process.argv[1]);
      if (process.version.slice(1) !== p.volta.node) process.exit(1);
      if (`npm@${process.argv[2]}` !== p.packageManager) process.exit(1);
    ' "$PROJECT_ROOT/web-client/package.json" "$(npm --version)" || {
        log_error "Node/npm do not match web-client/package.json"
        exit 1
    }
    "$PROJECT_ROOT/scripts/01-download-binaries.sh" --check
    log_success "Toolchain and binary identities verified"
}

check_locks_and_licenses() {
    phase_banner "Step 2: Locked graphs and licenses"
    run_shell_step "Cargo locked metadata" "5" \
        "cd '$TEMPLATE_DIR' && cargo metadata --locked --format-version 1 --no-deps >/dev/null"
    run_shell_step "npm locked graph" "5" \
        "cd '$PROJECT_ROOT/web-client' && npm ci --ignore-scripts --dry-run >/dev/null"
    run_shell_step "Cargo licenses and sources" "10" \
        "cd '$TEMPLATE_DIR' && cargo deny --config '$DENY_CONFIG' check licenses sources"
}

check_advisories() {
    phase_banner "Step 3: Vulnerability reachability review"
    CARGO_JSON="$(mktemp "${TMPDIR:-/tmp}/deos-cargo-audit.XXXXXX.json")"
    NPM_JSON="$(mktemp "${TMPDIR:-/tmp}/deos-npm-audit.XXXXXX.json")"
    (cd "$TEMPLATE_DIR" && cargo audit --json >"$CARGO_JSON") || true
    (cd "$PROJECT_ROOT/web-client" && npm audit --json >"$NPM_JSON") || true
    node "$SCRIPT_DIR/dependency-provenance.mjs" \
        "$CARGO_JSON" "$NPM_JSON" "$EXCEPTIONS" "$PROJECT_ROOT/web-client/package-lock.json"
}

main() {
    parse_args "$@"
    trap cleanup EXIT
    phase_banner "DEOS dependency provenance review"
    check_prerequisites
    check_locks_and_licenses
    check_advisories
    log_success "Dependency provenance review passed"
}

main "$@"
