#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-simulator-consistency.sh [OPTIONS]

Checks that simulator documentation mirrors the executable simulator test suite.
The tests.md total and ordered test headings must match the runTest(...) cases in
tests.js.

Options:
  -h, --help  Show this help message
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
    require_directory "$SIMULATOR_DIR" "simulator directory"
    require_commands rg grep tr
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Simulator suite mirror"
    local tests_js="$SIMULATOR_DIR/tests.js"
    local tests_md="$SIMULATOR_DIR/tests.md"
    if [[ ! -f "$tests_js" || ! -f "$tests_md" ]]; then
        log_error "simulator/tests.js and simulator/tests.md are required"
        exit 1
    fi

    local executable_count
    executable_count="$(rg -c '^runTest\(' "$tests_js" | tr -d ' ')"
    local documented_count
    documented_count="$(grep -E '^[-*] Total Tests: [0-9]+$' "$tests_md" | head -1 | grep -Eo '[0-9]+' || true)"

    if [[ -z "$documented_count" ]]; then
        log_error "simulator/tests.md is missing '- Total Tests: <n>'"
        exit 1
    fi
    if [[ "$documented_count" != "$executable_count" ]]; then
        log_error "simulator/tests.md total does not match executable tests"
        echo "tests.js runTest count: $executable_count"
        echo "tests.md Total Tests: $documented_count"
        exit 1
    fi

    TESTS_JS="$tests_js" TESTS_MD="$tests_md" node <<'EOF'
const fs = require('fs');
const testsJs = fs.readFileSync(process.env.TESTS_JS, 'utf8');
const testsMd = fs.readFileSync(process.env.TESTS_MD, 'utf8');
const executableNames = [...testsJs.matchAll(/^runTest\("([^"]+)"/gm)].map((match) => match[1]);
const documentedNames = [...testsMd.matchAll(/^### \d+\.\d+ (.+)$/gm)].map((match) => match[1]);
const maxLength = Math.max(executableNames.length, documentedNames.length);
const mismatches = [];
for (let index = 0; index < maxLength; index += 1) {
  if (executableNames[index] !== documentedNames[index]) {
    mismatches.push(
      `${index + 1}: tests.js=${executableNames[index] ?? '<missing>'} | tests.md=${documentedNames[index] ?? '<missing>'}`,
    );
  }
}
if (mismatches.length > 0) {
  console.error('[ERROR] simulator/tests.md headings do not mirror tests.js runTest order');
  for (const line of mismatches) console.error(`  - ${line}`);
  process.exit(1);
}
EOF
    log_success "Simulator suite mirror is current ($executable_count tests)"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
