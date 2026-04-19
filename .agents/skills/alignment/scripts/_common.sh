#!/usr/bin/env bash

LOCAL_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

find_project_root() {
    local dir="$LOCAL_SCRIPT_DIR"
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/AGENTS.md" && -d "$dir/scripts" && -d "$dir/template" ]]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

DEOS_PROJECT_ROOT="$(find_project_root || true)"
if [[ -z "$DEOS_PROJECT_ROOT" ]]; then
    echo "[ERROR] Unable to resolve DEOS project root from $LOCAL_SCRIPT_DIR" >&2
    exit 1
fi

source "$DEOS_PROJECT_ROOT/scripts/_common.sh"

SCRIPT_DIR="$LOCAL_SCRIPT_DIR"
PROJECT_ROOT="$DEOS_PROJECT_ROOT"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_SCRIPT_DIR="$PROJECT_ROOT/scripts"
TEMPLATE_DIR="$PROJECT_ROOT/template"
BIN_DIR="$PROJECT_ROOT/bin"
SIMULATOR_DIR="$PROJECT_ROOT/simulator"
