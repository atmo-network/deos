#!/usr/bin/env bash
set -euo pipefail
skill_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$#" -gt 1 ]]; then
  printf 'error: expected at most one argument\n' >&2
  exit 1
fi
case "${1:-}" in
  ''|--self-test) ;;
  --help)
    printf 'Usage: %s [--self-test]\nValidates live Experiment Records, current-consumer rows, relations and links.\n--self-test exercises isolated positive and negative fixtures.\nSkill-private; never a project build dependency.\n' "${0##*/}"
    exit 0 ;;
  *) printf 'error: unknown argument: %s\n' "$1" >&2; exit 1 ;;
esac
exec node "$skill_dir/scripts/record-normalization.mjs" "$skill_dir" "${1:-}"
