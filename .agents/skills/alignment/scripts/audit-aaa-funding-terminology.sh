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

legacy_pattern='first_eligible_at|FundingBatch|funding-batch|pending_amount|armed[ /-]+pending (funding )?(batches|generations)|tracked[ /-]+pending funding (counts|counters)|pending funding (counts|counters)|promotes? pending funding|funding promotion|keeps? new funding pending|User[ /-]+System FIFO lanes|class service guarantees?|reopen(ed|ing)? lineage|lineage reopen(ed|ing)?'

check_paths() {
    local -a paths=("$@")
    local matches
    matches="$(rg -n -i "$legacy_pattern" "${paths[@]}" 2>/dev/null || true)"
    if [[ -n "$matches" ]]; then
        log_error "Legacy AAA funding-generation terminology remains"
        printf '%s\n' "$matches"
        return 1
    fi
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
    log_success "Funding terminology fixtures passed"
}

run_audit() {
    phase_banner "Step 2: Active AAA funding terminology"
    check_paths \
        "$TEMPLATE_DIR/pallets/aaa/docs" \
        "$PROJECT_ROOT/docs/aaa.integration.en.md" \
        "$PROJECT_ROOT/docs/core.architecture.en.md"
    log_success "AAA funding terminology audit passed"
}

main() {
    parse_args "$@"
    phase_banner "Step 1: Prerequisites"
    require_commands rg
    if [[ "$SELF_TEST" == "1" ]]; then
        run_self_tests
    else
        run_audit
    fi
}

main "$@"
