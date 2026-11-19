#!/usr/bin/env bash

WIKI_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
DEOS_PROJECT_ROOT="$(dirname "$WIKI_ROOT")"
source "$DEOS_PROJECT_ROOT/scripts/_common.sh"
