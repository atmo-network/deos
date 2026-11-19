#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
export DEOS_PROJECT_ROOT="$(cd "$SKILL_DIR/../../.." && pwd -P)"
export DEOS_BINARY_DIR="$DEOS_PROJECT_ROOT/bin"
source "$DEOS_PROJECT_ROOT/scripts/_common.sh"

REVIEW_DIR=""
CARGO_AUDIT_VERSION=0.22.2
CARGO_DENY_VERSION=0.20.2
VALIDATOR="$SCRIPT_DIR/dependency-provenance.mjs"
LEDGER="$SKILL_DIR/config/dependency-provenance-exceptions.json"

usage() {
    cat <<'EOF'
Usage: dependency-provenance.sh <--framework|--root|--host|--client|--self-test>

  --framework  Report both selected Cargo roles independently; fail if either fails.
  --root       Root Cargo licenses/sources, actual and version-view advisory decisions.
  --host       Isolated Linux PVF Cargo role and pinned workers (not producer clearance).
  --client     Experimental browser locked npm graph, licenses/sources and advisory decisions.
  --self-test  Exercise refusal fixtures without building or fetching products.

The Cargo roles do not read client inputs or require npm. Their JavaScript
helpers use only Node's standard library. The Zombienet/npm node-launch route
and SDK-native node producers are separate, unqualified operator selections.
Development-tool or source decisions do not qualify secret-bearing Native paths,
external producer binaries, or consensus resource bounds. The Host proof requires
its reviewed local evidence and exact advisory reports. Refusal is not clearance.

Side effects: temporary reports and reads from advisory registries; no package
installations, lockfile edits, network launches, or artifact regeneration.
EOF
}

check_toolchain() {
    require_commands cargo rustc node cargo-audit cargo-deny
    [[ "$(cargo audit --version)" == *" $CARGO_AUDIT_VERSION" ]] || { log_error "Expected cargo-audit $CARGO_AUDIT_VERSION"; return 1; }
    [[ "$(cargo deny --version)" == *" $CARGO_DENY_VERSION" ]] || { log_error "Expected cargo-deny $CARGO_DENY_VERSION"; return 1; }
    rustc --version | grep -F 'rustc 1.94.1 '
}

review_cargo() {
    local scope="$1" workspace="$TEMPLATE_DIR" projection=--project exceptions="$LEDGER"
    local -a external=()
    if [[ "$scope" == --host ]]; then
        workspace="$TEMPLATE_DIR/tools/pvf-validation"
        projection=--project-external-workspace
        exceptions="$SKILL_DIR/config/canonical-pvf-host-source/exceptions.json"
        external=(--external-workspace)
    fi
    cd "$workspace"
    node "$VALIDATOR" --check-cargo-environment "$PWD"
    check_toolchain
    cargo metadata --locked --all-features --format-version 1 >"$REVIEW_DIR/metadata.json"
    if [[ "$scope" == --root ]]; then
        node -e 'if (require(process.argv[1]).packages.some(p => p.source?.startsWith("git+"))) throw new Error("Unreviewed Git input to Root")' "$REVIEW_DIR/metadata.json"
    fi
    node "$VALIDATOR" "$projection" "$workspace/Cargo.lock" "$REVIEW_DIR/metadata.json" "$REVIEW_DIR/version-view.lock"
    # Both views must be genuine and from the same current RustSec DB. Audit
    # returns nonzero on findings; malformed, incomplete or mismatched reports
    # are rejected by the validator, not treated as zero findings.
    cargo audit --json >"$REVIEW_DIR/actual-audit.json" || :
    cargo audit --file "$REVIEW_DIR/version-view.lock" --json >"$REVIEW_DIR/version-audit.json" || :
    local result=0
    node "$VALIDATOR" --framework "$REVIEW_DIR/actual-audit.json" "$REVIEW_DIR/version-audit.json" \
        "$exceptions" "$workspace/Cargo.lock" "$REVIEW_DIR/metadata.json" "$REVIEW_DIR/version-view.lock" \
        "$PROJECT_ROOT" "${external[@]}" || result=1
    cargo deny --config "$SKILL_DIR/config/deny.toml" check licenses sources || result=1
    if [[ "$scope" == --host ]]; then
        "$PROJECT_ROOT/scripts/01-download-binaries.sh" --check \
            --only polkadot-prepare-worker --only polkadot-execute-worker || result=1
    fi
    return "$result"
}

review_client() {
    require_commands npm
    cd "$PROJECT_ROOT/web-client"
    [[ "$(npm --version)" == "$(<"$PROJECT_ROOT/.npm-version")" ]] || { log_error "npm pin mismatch"; return 1; }
    node -e 'const p = require("./package.json"); if (p.volta?.node !== process.versions.node || p.volta?.npm !== process.argv[1] || p.packageManager !== `npm@${process.argv[1]}`) process.exit(1);' "$(npm --version)"
    npm ci --ignore-scripts --dry-run --include=dev --include=optional --include=peer >/dev/null
    npm audit --json --include=dev --include=optional --include=peer >"$REVIEW_DIR/npm.json" || :
    node "$VALIDATOR" --client "$REVIEW_DIR/npm.json" "$LEDGER" "$PWD/package-lock.json" "$PROJECT_ROOT"
}

main() {
    [[ $# -eq 1 ]] || { usage; return 1; }
    case "$1" in
        --self-test) activate_pinned_node; node "$VALIDATOR" --self-test; return ;;
        -h|--help) usage; return ;;
        --framework)
            local failed=0
            "$SCRIPT_DIR/dependency-provenance.sh" --root || failed=1
            "$SCRIPT_DIR/dependency-provenance.sh" --host || failed=1
            return "$failed" ;;
        --root|--host|--client) ;;
        *) usage; return 1 ;;
    esac
    phase_banner "Dependency provenance $1"
    activate_pinned_node
    REVIEW_DIR="$(mktemp -d "${TMPDIR:-/tmp}/deos-dependency-review.XXXXXX")"
    trap 'rm -rf -- "$REVIEW_DIR"' EXIT
    case "$1" in
        --root|--host) review_cargo "$1" ;;
        --client) review_client ;;
    esac
    log_success "Dependency provenance $1 passed"
}

main "$@"
