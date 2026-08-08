#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SELF_TEST=0

usage() {
    cat <<'EOF'
Usage: audit-aaa-funding-terminology.sh [OPTIONS]

Rejects retired AAA actor-state, funding-generation, scheduler-lane, and
lineage terminology in active package and integration documentation while
excluding historical delivery records.

Options:
  --self-test  Prove accepted accumulator prose and rejected legacy fixtures
  -h, --help   Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --self-test) SELF_TEST=1 ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

legacy_pattern=''
removed_surface_pattern=''

load_rule_inventory() {
    local inventory="$PROJECT_ROOT/.agents/skills/alignment/rules/aaa-drift-rules.json"
    legacy_pattern="$(node -e '
const fs = require("node:fs");
const inventory = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const rule = inventory.rules.find((candidate) => candidate.id === "aaa-funding-lineage-terms");
if (!rule || rule.kind !== "regex-any-case-insensitive" || !Array.isArray(rule.patterns)) process.exit(1);
process.stdout.write(rule.patterns.join("|"));
' "$inventory")"
    removed_surface_pattern="$(node -e '
const fs = require("node:fs");
const inventory = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const rule = inventory.rules.find((candidate) => candidate.id === "aaa-removed-surface-terms");
if (!rule || rule.kind !== "regex-any-case-insensitive" || !Array.isArray(rule.patterns)) process.exit(1);
process.stdout.write(rule.patterns.join("|"));
' "$inventory")"
    [[ -n "$legacy_pattern" ]] || { log_error "AAA funding terminology rule is empty"; return 1; }
    [[ -n "$removed_surface_pattern" ]] || { log_error "AAA removed-surface rule is empty"; return 1; }
}

check_paths_with_pattern() {
    local pattern="$1"
    shift
    local -a paths=("$@")
    local matches
    matches="$(rg -n -i "$pattern" "${paths[@]}" 2>/dev/null || true)"
    if [[ -n "$matches" ]]; then
        log_error "Retired AAA terminology or surface remains"
        printf '%s\n' "$matches"
        return 1
    fi
}

check_paths() {
    check_paths_with_pattern "$legacy_pattern" "$@"
}

run_self_tests() {
    phase_banner "Step 2: Funding terminology fixtures"
    local fixture_dir
    fixture_dir="$(mktemp -d)"
    trap "rm -rf '$fixture_dir'" RETURN
    printf '%s\n' 'Accepted ingress checked-adds into funding_accumulated; retry keeps one frozen snapshot.' > "$fixture_dir/accepted.md"
    check_paths "$fixture_dir/accepted.md"
    local legacy
    for legacy in \
        'first_eligible_at' \
        'FundingBatch' \
        'pending_amount' \
        'armed/pending funding batches' \
        'tracked/pending funding counts' \
        'successful completion promotes pending funding' \
        'keeps new funding pending' \
        'User/System FIFO lanes' \
        'class service guarantee' \
        'reopened lineage'; do
        printf '%s\n' "$legacy" > "$fixture_dir/rejected.md"
        if check_paths "$fixture_dir/rejected.md" >/dev/null 2>&1; then
            log_error "Legacy funding fixture passed unexpectedly: $legacy"
            return 1
        fi
    done
    printf '%s\n' 'Evaluation fees derive from generated WeightInfo through WeightToFee.' > "$fixture_dir/accepted.md"
    check_paths_with_pattern "$removed_surface_pattern" "$fixture_dir/accepted.md"
    for legacy in \
        'PercentageOfCurrentBalance' \
        'ProductiveRun' \
        'CurrentCacheEpoch' \
        'stepBaseFee' \
        'finalized_through'; do
        printf '%s\n' "$legacy" > "$fixture_dir/rejected.md"
        if check_paths_with_pattern "$removed_surface_pattern" "$fixture_dir/rejected.md" >/dev/null 2>&1; then
            log_error "Removed AAA surface fixture passed unexpectedly: $legacy"
            return 1
        fi
    done
    log_success "AAA terminology fixtures passed"
}

run_audit() {
    phase_banner "Step 2: Active AAA funding terminology"
    check_paths \
        "$TEMPLATE_DIR/pallets/aaa/docs" \
        "$PROJECT_ROOT/docs/aaa.integration.en.md" \
        "$PROJECT_ROOT/docs/core.architecture.en.md"
    check_paths_with_pattern "$removed_surface_pattern" \
        "$TEMPLATE_DIR/pallets/aaa/src" \
        "$TEMPLATE_DIR/pallets/aaa/README.md" \
        "$TEMPLATE_DIR/pallets/aaa/docs/architecture.en.md" \
        "$TEMPLATE_DIR/pallets/aaa/docs/embedding.md" \
        "$PROJECT_ROOT/docs/aaa.integration.en.md" \
        "$PROJECT_ROOT/docs/aaa-control-plane.contract.en.md" \
        "$PROJECT_ROOT/docs/core.architecture.en.md" \
        "$PROJECT_ROOT/docs/read-model.contract.en.md" \
        "$PROJECT_ROOT/web-client/src" \
        "$PROJECT_ROOT/web-client/scripts" \
        "$PROJECT_ROOT/web-client/docs" \
        "$PROJECT_ROOT/wiki"
    log_success "AAA terminology and removed-surface audit passed"
}

main() {
    parse_args "$@"
    phase_banner "Step 1: Prerequisites"
    require_commands node rg
    load_rule_inventory
    if [[ "$SELF_TEST" == "1" ]]; then
        run_self_tests
    else
        run_audit
    fi
}

main "$@"
