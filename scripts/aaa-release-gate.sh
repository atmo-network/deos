#!/usr/bin/env bash
# Skill-Owner: aaa-delivery
# Skill-Entrypoint: scripts/release-gate.sh

set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "$PROJECT_ROOT/.agents/skills/aaa-delivery/scripts/release-gate.sh" "$@"
