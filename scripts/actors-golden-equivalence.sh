#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

EXECUTE=0
CARGO_PROFILE="dev"
BASELINE_COMMIT=""
BASELINE_WORKTREE=""
TEMP_DIR=""
GOLDEN_FIXTURE="$PROJECT_ROOT/template/pallets/actors/tests/fixtures/golden-equivalence.v1.json"

usage() {
    cat <<'EOF'
Usage: actors-golden-equivalence.sh [OPTIONS]

Validate the pinned DEOS 0.7.17 Actors golden oracle. With --execute, run the
complete retained reactive corpus and semantic behavior anchors against both
the pinned 0.7.17 implementation and the current candidate, then compare each
implementation's generated semantic manifest to the immutable oracle.

Options:
  --check      Validate oracle identity, corpus freshness, mappings, and anchors
  --execute    Execute the complete cross-version equivalence route
  --release    Use Cargo release profile for --execute
  -h, --help   Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --check)
                EXECUTE=0
                ;;
            --execute)
                EXECUTE=1
                ;;
            --release)
                CARGO_PROFILE="release"
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage >&2
                exit 2
                ;;
        esac
        shift
    done
}

check_prerequisites() {
    require_commands cargo git node
    BASELINE_COMMIT="$(node -e 'const fixture=require(process.argv[1]); if(!fixture.baseline?.commit) process.exit(1); process.stdout.write(fixture.baseline.commit)' "$GOLDEN_FIXTURE")" || {
        log_error "Immutable 0.7.17 baseline is missing from the golden-equivalence fixture"
        exit 1
    }
    [[ -f "$SCRIPT_DIR/validate-actors-golden-equivalence.mjs" ]] || {
        log_error "Golden-equivalence validator is missing"
        exit 1
    }
}

cleanup() {
    if [[ -n "$BASELINE_WORKTREE" && -d "$BASELINE_WORKTREE" ]]; then
        git -C "$PROJECT_ROOT" worktree remove --force "$BASELINE_WORKTREE" >/dev/null 2>&1 || true
    fi
    if [[ -n "$TEMP_DIR" && -d "$TEMP_DIR" ]]; then
        rm -rf "$TEMP_DIR"
    fi
}

run_static_contract() {
    run_shell_step \
        "Golden equivalence: pinned oracle and freshness" \
        "" \
        "cd \"$PROJECT_ROOT\" && node scripts/validate-actors-golden-equivalence.mjs"
    run_shell_step \
        "Golden equivalence: current reactive corpus contract" \
        "" \
        "\"$PROJECT_ROOT/scripts/reactive-operations-corpus.sh\""
    run_shell_step \
        "Golden equivalence: current semantic manifest freshness" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo run -q --locked -p pallet-deos-actors --example semantic_manifest -- --check ../web-client/src/lib/automation/actors-semantic-manifest.json"
}

run_executable_contract() {
    git -C "$PROJECT_ROOT" cat-file -e "${BASELINE_COMMIT}^{commit}" || {
        log_error "Immutable 0.7.17 baseline commit is unavailable: $BASELINE_COMMIT"
        exit 1
    }
    TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/deos-actors-golden.XXXXXX")"
    BASELINE_WORKTREE="$TEMP_DIR/baseline"
    trap cleanup EXIT
    run_shell_step \
        "Golden equivalence: materialize pinned 0.7.17 implementation" \
        "" \
        "git -C \"$PROJECT_ROOT\" worktree add --detach \"$BASELINE_WORKTREE\" \"$BASELINE_COMMIT\""

    local profile_flag=""
    [[ "$CARGO_PROFILE" == "dev" ]] || profile_flag="--release"
    run_shell_step \
        "Golden equivalence: baseline semantic manifest" \
        "" \
        "cd \"$BASELINE_WORKTREE/template\" && cargo run -q --locked $profile_flag -p pallet-deos-actors --example semantic_manifest > \"$TEMP_DIR/baseline-semantic.json\""
    run_shell_step \
        "Golden equivalence: current semantic manifest" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo run -q --locked $profile_flag -p pallet-deos-actors --example semantic_manifest > \"$TEMP_DIR/current-semantic.json\""
    run_shell_step \
        "Golden equivalence: independent semantic outputs equal immutable oracle" \
        "" \
        "cmp \"$TEMP_DIR/baseline-semantic.json\" \"$PROJECT_ROOT/web-client/src/lib/automation/actors-semantic-manifest.json\" && cmp \"$TEMP_DIR/current-semantic.json\" \"$PROJECT_ROOT/web-client/src/lib/automation/actors-semantic-manifest.json\""

    run_shell_step \
        "Golden equivalence: complete baseline reactive corpus" \
        "" \
        "cd \"$BASELINE_WORKTREE\" && ./scripts/reactive-operations-corpus.sh --execute $profile_flag"
    run_shell_step \
        "Golden equivalence: complete current reactive corpus" \
        "" \
        "cd \"$PROJECT_ROOT\" && ./scripts/reactive-operations-corpus.sh --execute $profile_flag"

    local anchors
    anchors="$(cd "$PROJECT_ROOT" && node scripts/validate-actors-golden-equivalence.mjs --anchors)"
    while IFS=$'\t' read -r baseline_symbol current_symbol scenario_id; do
        [[ -n "$baseline_symbol" && -n "$current_symbol" ]] || continue
        run_shell_step \
            "Golden equivalence baseline semantic anchor: $scenario_id" \
            "" \
            "cd \"$BASELINE_WORKTREE/template\" && cargo test -q $profile_flag -p pallet-deos-actors --locked --lib \"$baseline_symbol\""
        run_shell_step \
            "Golden equivalence current semantic anchor: $scenario_id" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test -q $profile_flag -p pallet-deos-actors --locked --lib \"$current_symbol\""
    done <<< "$anchors"
    log_success "Pinned 0.7.17 and current Actors implementations satisfy the same immutable corpora and normalized semantic anchors"
}

main() {
    parse_args "$@"
    check_prerequisites
    phase_banner "DEOS Actors golden equivalence"
    run_static_contract
    if [[ "$EXECUTE" -eq 1 ]]; then
        run_executable_contract
    fi
    phase_banner "Summary"
    log_success "Actors golden-equivalence contract passed"
}

main "$@"
