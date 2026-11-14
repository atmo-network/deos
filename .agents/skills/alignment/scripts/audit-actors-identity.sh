#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SELF_TEST=0

usage() {
    cat <<'EOF'
Usage: audit-actors-identity.sh [OPTIONS]

Rejects retired Actors state, funding-generation, scheduler-lane, and lineage
terminology in active package and integration documentation while
excluding historical delivery records and third-party vendor namespaces.

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
identity_pattern='(?<![[:alnum:]])[Aa]{3}(?![[:lower:][:digit:]])|[Aa]{3}_|_[Aa]{3}'

load_rule_inventory() {
    local inventory="$PROJECT_ROOT/.agents/skills/alignment/rules/actors-identity-rules.json"
    legacy_pattern="$(node -e '
const fs = require("node:fs");
const inventory = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const rule = inventory.rules.find((candidate) => candidate.id === "actors-funding-lineage-terms");
if (!rule || rule.kind !== "regex-any-case-insensitive" || !Array.isArray(rule.patterns)) process.exit(1);
process.stdout.write(rule.patterns.join("|"));
' "$inventory")"
    removed_surface_pattern="$(node -e '
const fs = require("node:fs");
const inventory = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const rule = inventory.rules.find((candidate) => candidate.id === "actors-removed-surface-terms");
if (!rule || rule.kind !== "regex-any-case-insensitive" || !Array.isArray(rule.patterns)) process.exit(1);
process.stdout.write(rule.patterns.join("|"));
' "$inventory")"
    [[ -n "$legacy_pattern" ]] || { log_error "Actors funding terminology rule is empty"; return 1; }
    [[ -n "$removed_surface_pattern" ]] || { log_error "Actors removed-surface rule is empty"; return 1; }
}

check_paths_with_pattern() {
    local pattern="$1"
    shift
    local -a paths=("$@")
    local matches
    matches="$(rg -n -i "$pattern" "${paths[@]}" 2>/dev/null || true)"
    if [[ -n "$matches" ]]; then
        log_error "Retired Actors terminology or surface remains"
        printf '%s\n' "$matches"
        return 1
    fi
}

check_paths() {
    check_paths_with_pattern "$legacy_pattern" "$@"
}

policy_identity_matches() {
    local policy="$1" rendered
    rendered="$(node - "$policy" <<'NODE'
const fs = require('node:fs');
const policy = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
if (!Array.isArray(policy.exceptions)) throw new Error('Missing exception inventory');
for (const row of policy.exceptions) {
    if (row.id !== 'RUSTSEC-2026-0387' || row.sourceCallerRoot?.role !== 'root-workspace-normal-build') continue;
    for (const pkg of row.sourceCallerRoot.packages ?? []) {
        if (!pkg.source?.startsWith('registry+')) continue;
        // Third-party inventory filenames are evidence, not project identities.
        for (const file of pkg.files ?? []) file.path = '<third-party source path>';
    }
}
process.stdout.write(JSON.stringify(policy, null, 2));
NODE
)" || return 1
    printf '%s\n' "$rendered" | rg -n -P "$identity_pattern" | awk -v path="$policy" '{ print path ":" $0 }' || [[ "$?" == 1 ]]
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
    printf '%s\n' 'Actors ActorType actor_id' > "$fixture_dir/accepted.md"
    if rg -q -P "$identity_pattern" "$fixture_dir/accepted.md"; then
        log_error "Canonical identity fixture failed unexpectedly"
        return 1
    fi
    local retired_identity
    for retired_identity in "$(printf 'A%sType' 'aa')" "$(printf 'pallet_%s%s' 'a' 'aa')"; do
        printf '%s\n' "$retired_identity" > "$fixture_dir/rejected.md"
        if ! rg -q -P "$identity_pattern" "$fixture_dir/rejected.md"; then
            log_error "Retired identity fixture passed unexpectedly"
            return 1
        fi
    done
    local policy_fixture="$fixture_dir/policy.json" policy_matches
    printf '%s\n' '{"exceptions":[{"id":"RUSTSEC-2026-0387","sourceCallerRoot":{"role":"root-workspace-normal-build","packages":[{"source":"registry+fixture","files":[{"path":"openssl/crypto/x509/v3_'"$(printf 'a%s' 'aa')"'.c"},{"path":"samples/zonefile/'"$(printf 'AA%s' 'AA')"'.sample"}]}]}}]}' > "$policy_fixture"
    policy_matches="$(policy_identity_matches "$policy_fixture")"
    [[ -z "$policy_matches" ]] || { log_error "Third-party inventory paths were treated as identities"; return 1; }
    for location in reason local-path; do
        node - "$policy_fixture" "$location" "$(printf 'A%sType' 'aa')" <<'NODE'
const fs = require('node:fs');
const policy = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const row = policy.exceptions[0];
if (process.argv[3] === 'reason') row.reason = process.argv[4];
else {
    delete row.reason;
    row.sourceCallerRoot.packages[0].source = null;
    row.sourceCallerRoot.packages[0].files[0].path = process.argv[4];
}
fs.writeFileSync(process.argv[2], JSON.stringify(policy));
NODE
        policy_matches="$(policy_identity_matches "$policy_fixture")"
        [[ -n "$policy_matches" ]] || { log_error "Active policy identity escaped: $location"; return 1; }
    done
    printf '%s\n' 'Evaluation fees derive from generated WeightInfo through WeightToFee.' > "$fixture_dir/accepted.md"
    check_paths_with_pattern "$removed_surface_pattern" "$fixture_dir/accepted.md"
    for legacy in \
        'PercentBalance' \
        'ProductiveRun' \
        'CurrentCacheEpoch' \
        'stepBaseFee' \
        'finalized_through'; do
        printf '%s\n' "$legacy" > "$fixture_dir/rejected.md"
        if check_paths_with_pattern "$removed_surface_pattern" "$fixture_dir/rejected.md" >/dev/null 2>&1; then
            log_error "Removed Actors surface fixture passed unexpectedly: $legacy"
            return 1
        fi
    done
    log_success "Actors terminology fixtures passed"
}

check_retired_identity() {
    local content_matches path_matches policy_matches
    local policy="$PROJECT_ROOT/.agents/skills/release-assurance/config/dependency-provenance-exceptions.json"
    content_matches="$(rg -n -P "$identity_pattern" "$PROJECT_ROOT" \
        --hidden \
        -g '!.git/**' \
        -g '!target/**' \
        -g '!node_modules/**' \
        -g '!CHANGELOG.md' \
        -g '!template/Cargo.lock' \
        -g '!template/vendor/**' \
        -g '!.agents/skills/release-assurance/config/dependency-provenance-exceptions.json' 2>/dev/null || true)"
    if [[ -f "$policy" ]]; then
        policy_matches="$(policy_identity_matches "$policy")" || return 1
        [[ -z "$policy_matches" ]] || content_matches+="$policy_matches"
    fi
    # DNS record identifiers and CA subject names are not DEOS Actor identities.
    path_matches="$(git -C "$PROJECT_ROOT" ls-files -co --exclude-standard -- \
        ':(exclude)template/vendor/**' | rg '[Aa]{3}' || true)"
    if [[ -n "$content_matches" || -n "$path_matches" ]]; then
        log_error "Retired actor-runtime identity remains"
        [[ -z "$path_matches" ]] || printf '%s\n' "$path_matches"
        [[ -z "$content_matches" ]] || printf '%s\n' "$content_matches"
        return 1
    fi
}

run_audit() {
    phase_banner "Step 2: Retired identity"
    check_retired_identity
    phase_banner "Step 3: Active Actors funding terminology"
    check_paths \
        "$TEMPLATE_DIR/pallets/actors/docs" \
        "$PROJECT_ROOT/docs/deos/actors.integration.en.md" \
        "$PROJECT_ROOT/docs/deos/core.architecture.en.md"
    check_paths_with_pattern "$removed_surface_pattern" \
        "$TEMPLATE_DIR/pallets/actors/src" \
        "$TEMPLATE_DIR/pallets/actors/README.md" \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "$TEMPLATE_DIR/pallets/actors/docs/embedding.md" \
        "$PROJECT_ROOT/docs/deos/actors.integration.en.md" \
        "$PROJECT_ROOT/docs/deos/actors-control-plane.contract.en.md" \
        "$PROJECT_ROOT/docs/deos/core.architecture.en.md" \
        "$PROJECT_ROOT/docs/deos/read-model.contract.en.md" \
        "$PROJECT_ROOT/web-client/src" \
        "$PROJECT_ROOT/web-client/scripts" \
        "$PROJECT_ROOT/web-client/docs" \
        "$PROJECT_ROOT/wiki"
    log_success "Actors terminology and removed-surface audit passed"
}

main() {
    parse_args "$@"
    phase_banner "Step 1: Prerequisites"
    require_commands node rg awk
    load_rule_inventory
    if [[ "$SELF_TEST" == "1" ]]; then
        run_self_tests
    else
        run_audit
    fi
}

main "$@"
