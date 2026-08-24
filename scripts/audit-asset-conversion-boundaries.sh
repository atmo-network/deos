#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/_common.sh
source "$SCRIPT_DIR/_common.sh"

usage() {
  cat <<'USAGE'
Usage: audit-asset-conversion-boundaries.sh [--help]

Fail closed unless production runtime code has exactly one direct
AssetConversion::create_pool owner (the atomic DEOS lifecycle) and no external LP-binding repair.

Inputs: repository source tree.
Outputs: concise audit result.
Side effects: none.
USAGE
}

parse_args() {
  if [[ $# -gt 1 ]]; then
    usage >&2
    exit 2
  fi
  case "${1:-}" in
    "") ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
}

check_prerequisites() {
  require_commands rg
}

main() {
  local runtime="$PROJECT_ROOT/template/runtime/src"
  local creation_matches repair_matches
  creation_matches="$(rg -n 'AssetConversion::create_pool' "$runtime" \
    --glob '*.rs' --glob '!**/tests/**' --glob '!**/tests.rs' || true)"
  if [[ "$(printf '%s\n' "$creation_matches" | grep -c . || true)" -ne 1 ]] \
    || [[ "$creation_matches" != *"configs/assets_config.rs:"* ]]; then
    printf '%s\n' 'Unowned direct Asset Conversion pool creation detected:' >&2
    printf '%s\n' "$creation_matches" >&2
    exit 1
  fi
  repair_matches="$(rg -n 'register_pool_lp_pair' "$runtime" \
    --glob '*.rs' --glob '!**/configs/assets_config.rs' --glob '!**/tests/**' --glob '!**/tests.rs' || true)"
  if [[ -n "$repair_matches" ]]; then
    printf '%s\n' 'LP-binding mutation exists outside the lifecycle owner:' >&2
    printf '%s\n' "$repair_matches" >&2
    exit 1
  fi
  printf '%s\n' 'Asset Conversion boundary audit passed: one lifecycle creation owner, no external LP repair.'
}

parse_args "$@"
check_prerequisites
main
