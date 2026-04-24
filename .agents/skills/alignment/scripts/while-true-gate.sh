#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

AUDIT_SCOPE="${AUDIT_SCOPE:-changed}"
RUN_SIMULATOR="${RUN_SIMULATOR:-auto}"
RUN_CARGO_CHECK="${RUN_CARGO_CHECK:-auto}"
REQUIRE_CONTEXT_SYNC="${REQUIRE_CONTEXT_SYNC:-1}"
LEDGER_DIR="$SKILL_DIR/ledgers"
HALLUCINATIONS_FILE="$LEDGER_DIR/hallucinations.jsonl"

declare -a CHANGED_PATHS=()
declare -a CHANGED_SHELL_PATHS=()

usage() {
    cat <<'EOF'
Usage: while-true-gate.sh [OPTIONS]

Diff-aware DEOS completion gate for autonomous loops.
It validates the smallest meaningful scope for the current pass and blocks the next loop until the touched layer is green.

Options:
  --all-rust               Run architecture audit against the full pallet tree
  --skip-simulator         Do not run simulator validation
  --skip-cargo-check       Do not run cargo check validation
  --allow-no-context-sync  Warn instead of failing when context files were not updated
  -h, --help               Show this help message

Environment:
  AUDIT_SCOPE=changed|all
  RUN_SIMULATOR=auto|0|1
  RUN_CARGO_CHECK=auto|0|1
  REQUIRE_CONTEXT_SYNC=0|1
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all-rust)
                AUDIT_SCOPE="all"
                ;;
            --skip-simulator)
                RUN_SIMULATOR="0"
                ;;
            --skip-cargo-check)
                RUN_CARGO_CHECK="0"
                ;;
            --allow-no-context-sync)
                REQUIRE_CONTEXT_SYNC="0"
                ;;
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

collect_changed_paths() {
    CHANGED_PATHS=()
    CHANGED_SHELL_PATHS=()
    local tracked
    local untracked
    local path
    tracked="$(git -C "$PROJECT_ROOT" diff --name-only HEAD || true)"
    untracked="$(git -C "$PROJECT_ROOT" ls-files --others --exclude-standard || true)"
    while IFS= read -r path; do
        [[ -z "$path" ]] && continue
        CHANGED_PATHS+=("$path")
        if [[ "$path" =~ \.sh$ && -f "$PROJECT_ROOT/$path" ]]; then
            CHANGED_SHELL_PATHS+=("$PROJECT_ROOT/$path")
        fi
    done < <(printf '%s\n%s\n' "$tracked" "$untracked" | awk 'NF' | sort -u)
}

has_changed_path() {
    local pattern="$1"
    local path
    for path in "${CHANGED_PATHS[@]}"; do
        if [[ "$path" =~ $pattern ]]; then
            return 0
        fi
    done
    return 1
}

should_run_architecture_audit() {
    if [[ "$AUDIT_SCOPE" == "all" ]]; then
        return 0
    fi
    has_changed_path '^template/pallets/.*\.rs$'
}

should_run_simulator() {
    if [[ "$RUN_SIMULATOR" == "1" ]]; then
        return 0
    fi
    if [[ "$RUN_SIMULATOR" == "0" ]]; then
        return 1
    fi
    has_changed_path '^simulator/' || has_changed_path '^template/pallets/(tmc|axial-router)/'
}

should_run_cargo_check() {
    if [[ "$RUN_CARGO_CHECK" == "1" ]]; then
        return 0
    fi
    if [[ "$RUN_CARGO_CHECK" == "0" ]]; then
        return 1
    fi
    has_changed_path '^template/.*\.(rs|toml)$' || has_changed_path '^template/Cargo.lock$'
}

should_run_shell_syntax_check() {
    [[ ${#CHANGED_SHELL_PATHS[@]} -gt 0 ]]
}

should_run_wiki_trust() {
    has_changed_path '^wiki/.*\.md$'
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$PROJECT_ROOT/.git" "Git repository"
    require_directory "$SKILL_DIR" "Skill directory"
    hydrate_local_tool_paths
    require_commands git
    mkdir -p "$LEDGER_DIR"
    touch "$HALLUCINATIONS_FILE"
    collect_changed_paths
    log_success "Gate prerequisites checked"
}

plan() {
    phase_banner "DEOS while-true gate"
    log_info "Layer 0: Architecture audit"
    log_info "Layer 1: Changed shell syntax"
    log_info "Layer 2: Mathematical truth"
    log_info "Layer 3: Behavioral truth"
    log_info "Layer 4: Wiki trust"
    log_info "Layer 5: Knowledge sync"
    log_info "Audit scope: $AUDIT_SCOPE"
    log_info "Changed paths: ${#CHANGED_PATHS[@]}"
    log_info "Changed shell scripts: ${#CHANGED_SHELL_PATHS[@]}"
    log_info "Simulator mode: $RUN_SIMULATOR"
    log_info "Cargo check mode: $RUN_CARGO_CHECK"
    log_info "Require context sync: $REQUIRE_CONTEXT_SYNC"
}

run_architecture_audit() {
    phase_banner "Step 2: Architecture"
    if ! should_run_architecture_audit; then
        log_warning "Skipping architecture audit because no pallet Rust files changed"
        return 0
    fi
    if [[ "$AUDIT_SCOPE" == "all" ]]; then
        if ! "$SCRIPT_DIR/auditor.sh" --all "$TEMPLATE_DIR/pallets"; then
            log_error "Architecture audit failed"
            exit 1
        fi
        return 0
    fi
    if ! "$SCRIPT_DIR/auditor.sh"; then
        log_error "Architecture audit failed"
        exit 1
    fi
}

run_shell_syntax_validation() {
    phase_banner "Step 3: Shell syntax"
    if ! should_run_shell_syntax_check; then
        log_warning "Skipping shell syntax validation because no shell scripts changed"
        return 0
    fi
    require_commands bash
    if ! bash -n "${CHANGED_SHELL_PATHS[@]}"; then
        log_error "Shell syntax validation failed"
        exit 1
    fi
    log_success "Shell syntax validation passed"
}

run_simulator_validation() {
    phase_banner "Step 4: Mathematical truth"
    if ! should_run_simulator; then
        log_warning "Skipping simulator validation because the touched scope is not math-coupled"
        return 0
    fi
    require_directory "$SIMULATOR_DIR" "Simulator directory"
    require_commands node
    if ! run_shell_step "Simulator test suite" "" "cd \"$SIMULATOR_DIR\" && node tests.js"; then
        log_error "Mathematical validation failed"
        exit 1
    fi
}

run_behavior_validation() {
    phase_banner "Step 5: Behavioral truth"
    if ! should_run_cargo_check; then
        log_warning "Skipping cargo check because no Rust workspace files changed"
        return 0
    fi
    require_directory "$TEMPLATE_DIR" "Template directory"
    require_commands cargo
    if ! run_shell_step "cargo check --workspace" "" "cd \"$TEMPLATE_DIR\" && cargo check --workspace"; then
        log_error "Behavioral validation failed"
        exit 1
    fi
}

run_wiki_trust_validation() {
    phase_banner "Step 6: Wiki trust"
    if ! should_run_wiki_trust; then
        log_warning "Skipping wiki trust validation because no wiki markdown files changed"
        return 0
    fi
    local wiki_validator="$PROJECT_ROOT/.agents/skills/wiki-sync/scripts/validate-wiki-trust.sh"
    if [[ ! -x "$wiki_validator" ]]; then
        log_warning "Wiki trust validator not found or not executable, skipping"
        return 0
    fi
    if ! "$wiki_validator"; then
        log_error "Wiki trust validation failed"
        exit 1
    fi
    log_success "Wiki trust validation passed"
}

run_knowledge_sync() {
    phase_banner "Step 7: Knowledge sync"
    if [[ "$REQUIRE_CONTEXT_SYNC" != "1" ]]; then
        log_warning "Context sync gate disabled"
        return 0
    fi
    if has_changed_path '^(BACKLOG\.md|CHANGELOG\.md|AGENTS\.md|docs/)'; then
        log_success "Context files were updated in this pass"
        return 0
    fi
    log_error "Context sync missing: update CHANGELOG.md, AGENTS.md, BACKLOG.md, or docs/ before the next loop"
    exit 1
}

main() {
    parse_args "$@"
    check_prerequisites
    plan
    run_architecture_audit
    run_shell_syntax_validation
    run_simulator_validation
    run_behavior_validation
    run_wiki_trust_validation
    run_knowledge_sync
    phase_banner "Summary"
    log_success "While-true gate passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
