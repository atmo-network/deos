#!/usr/bin/env bash

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
PROJECT_ROOT="$(cd "$SKILL_DIR/../../.." && pwd -P)"
export DEOS_PROJECT_ROOT="$PROJECT_ROOT"
export DEOS_BINARY_DIR="$PROJECT_ROOT/bin"
source "$PROJECT_ROOT/scripts/_common.sh"

MODE="generate"

usage() {
    cat <<'EOF'
Usage: weight-delta-ledger.sh [--check]

Generates or verifies the v0.7.20-to-candidate production Weight delta ledger.

Options:
  --check           Fail when the committed ledger differs from current weights
  -h, --help        Show this help message

Inputs:
  Git tag v0.7.20 and all seven candidate custom-pallet production weight files.

Outputs:
  .agents/skills/release-assurance/evidence/runtime-weight-delta-ledger.md

Side effects:
  Generate mode replaces only the ledger. Check mode is read-only.
EOF
}

parse_args() {
    if [[ $# -gt 1 ]]; then
        usage
        exit 1
    fi
    case "${1:-}" in
        "") ;;
        --check) MODE="check" ;;
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

check_prerequisites() {
    require_commands git node
    git -C "$PROJECT_ROOT" rev-parse --verify v0.7.20 >/dev/null
}

main() {
    parse_args "$@"
    check_prerequisites
    local -a args=()
    [[ "$MODE" == "check" ]] && args+=(--check)
    node "$SCRIPT_DIR/generate-weight-delta-ledger.mjs" "${args[@]}"
}

main "$@"
