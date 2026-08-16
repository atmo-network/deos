#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

AUDIT_SCOPE="${AUDIT_SCOPE:-changed}"
RUN_SIMULATOR="${RUN_SIMULATOR:-auto}"
RUN_CARGO_CHECK="${RUN_CARGO_CHECK:-auto}"
RUN_RUNTIME_TESTS="${RUN_RUNTIME_TESTS:-auto}"
REQUIRE_CONTEXT_SYNC="${REQUIRE_CONTEXT_SYNC:-1}"
LEDGER_DIR="$SKILL_DIR/ledgers"
HALLUCINATIONS_FILE="$LEDGER_DIR/hallucinations.jsonl"

declare -a CHANGED_PATHS=()
declare -a CHANGED_SHELL_PATHS=()

usage() {
    cat <<'EOF'
Usage: completion-gate.sh [OPTIONS]

Diff-aware DEOS completion gate for local delivery slices.
It validates the smallest meaningful scope for the current pass and blocks the next loop until the touched layer is green.

Options:
  --all-rust               Run architecture audit against the full pallet tree
  --skip-simulator         Do not run simulator validation
  --skip-cargo-check       Do not run cargo check validation
  --skip-runtime-tests     Do not run runtime unit tests
  --allow-no-context-sync  Warn instead of failing when context files were not updated
  -h, --help               Show this help message

Environment:
  AUDIT_SCOPE=changed|all
  RUN_SIMULATOR=auto|0|1
  RUN_CARGO_CHECK=auto|0|1
  RUN_RUNTIME_TESTS=auto|0|1
  REQUIRE_CONTEXT_SYNC=0|1
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
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
            --skip-runtime-tests)
                RUN_RUNTIME_TESTS="0"
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
    has_changed_path '^simulator/' || has_changed_path '^template/pallets/(tmc|router)/'
}

should_run_router_identity_audit() {
    has_changed_path '(^|/)(router|deos_router|pallet_deos_router)' || \
        has_changed_path '^template/primitives/src/(ecosystem|oracle)\.rs$' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-router-identity\.sh$'
}

should_run_strategic_governance_ingress_audit() {
    has_changed_path '^template/pallets/governance/' || \
        has_changed_path '^template/runtime/src/(configs/(governance|xcm)_config\.rs|chain_specs/|lib\.rs$)' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-strategic-governance-ingress\.sh$'
}

should_run_governance_structural_liveness_audit() {
    has_changed_path '^template/pallets/governance/' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-governance-structural-liveness\.sh$'
}

should_run_runtime_composition_dag_audit() {
    has_changed_path '^template/pallets/(governance|staking|router|oracle|actors)/Cargo\.toml$' || \
        has_changed_path '^template/runtime/src/configs/' || \
        has_changed_path '^docs/core\.architecture\.en\.md$' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-runtime-composition-dag\.sh$'
}

should_run_protocol_coherence_regression_audit() {
    has_changed_path '^template/(pallets|primitives)/' || \
        has_changed_path '^template/runtime/src/' || \
        has_changed_path '^docs/' || \
        has_changed_path '^simulator/' || \
        has_changed_path '^web-client/(src|docs|scripts)/' || \
        has_changed_path '^wiki/' || \
        has_changed_path '^scripts/' || \
        has_changed_path '^\.agents/skills/alignment/(rules/actors-identity-rules\.json|scripts/audit-(actors-identity|protocol-coherence-regressions)\.sh)$'
}

semantic_surface_anchor_changed() {
    local manifest="$PROJECT_ROOT/.agents/skills/alignment/semantic-surface.v1.json"
    local status
    if node -e '
        const fs = require("node:fs");
        try {
          const [manifestPath, ...changed] = process.argv.slice(1);
          const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
          if (manifest.schema !== "deos-error-narrowness-evidence/v2") {
            throw new Error(`unsupported semantic manifest schema: ${manifest.schema}`);
          }
          if (!Array.isArray(manifest.typedWitnesses)) {
            throw new Error("semantic manifest lacks typedWitnesses");
          }
          const anchors = new Set(manifest.typedWitnesses.flatMap((witness) =>
            (witness.anchors ?? []).map((anchor) => anchor.path),
          ));
          process.exit(changed.some((path) => anchors.has(path)) ? 0 : 1);
        } catch (error) {
          console.error(`Unable to derive semantic witness paths: ${error.message}`);
          process.exit(2);
        }
      ' "$manifest" "${CHANGED_PATHS[@]}"; then
        return 0
    else
        status=$?
        [[ "$status" -eq 1 ]] && return 1
        return 0
    fi
}

should_run_protocol_coherence_mutation_tests() {
    has_changed_path '^\.agents/skills/alignment/scripts/audit-protocol-coherence-regressions\.sh$'
}

should_run_semantic_surface_audit() {
    has_changed_path '^template/pallets/(governance|staking|actors|router|oracle)/src/' || \
        has_changed_path '^template/runtime/src/' || \
        has_changed_path '^web-client/src/lib/' || \
        semantic_surface_anchor_changed || \
        has_changed_path '^\.agents/skills/alignment/(semantic-surface\.v1\.json|scripts/audit-semantic-surface\.mjs)$'
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

should_run_runtime_tests() {
    if [[ "$RUN_RUNTIME_TESTS" == "1" ]]; then
        return 0
    fi
    if [[ "$RUN_RUNTIME_TESTS" == "0" ]]; then
        return 1
    fi
    has_changed_path '^template/runtime/src/.*\.rs$'
}

should_run_shell_syntax_check() {
    [[ ${#CHANGED_SHELL_PATHS[@]} -gt 0 ]]
}

should_run_markdown_table_audit() {
    has_changed_path '\.md$' || has_changed_path '^\.agents/skills/alignment/scripts/audit-markdown-tables\.sh$'
}

should_run_architecture_readability_audit() {
    has_changed_path '^docs/.*\.architecture\.en\.md$' || \
        has_changed_path '^AGENTS\.md$' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-architecture-readability\.sh$'
}

should_run_wiki_trust() {
    has_changed_path '^wiki/.*\.md$'
}

should_run_release_line_audit() {
    has_changed_path '^CHANGELOG\.md$' || has_changed_path '^template/Cargo\.lock$' || has_changed_path '^template/.*/Cargo\.toml$'
}

should_run_economic_claim_audit() {
    has_changed_path '^\.agents/skills/alignment/economic-claims\.json$' || has_changed_path '^docs/.*\.architecture\.en\.md$'
}

should_run_backlog_audit() {
    has_changed_path '^BACKLOG\.md$'
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
    phase_banner "DEOS completion gate"
    log_info "Layer 0: Architecture audit"
    log_info "Layer 1: Changed shell syntax"
    log_info "Layer 2: Mathematical truth"
    log_info "Layer 3: Behavioral truth"
    log_info "Layer 4: Markdown quality"
    log_info "Layer 5: Wiki trust"
    log_info "Layer 6: Economic claim integrity"
    log_info "Layer 7: Strategic governance ingress"
    log_info "Layer 8: Protocol coherence regressions"
    log_info "Layer 9: Error Narrowness identities and executable witnesses"
    log_info "Layer 9: Release-line consistency"
    log_info "Layer 10: Backlog open-work shape"
    log_info "Layer 11: Knowledge sync"
    log_info "Audit scope: $AUDIT_SCOPE"
    log_info "Changed paths: ${#CHANGED_PATHS[@]}"
    log_info "Changed shell scripts: ${#CHANGED_SHELL_PATHS[@]}"
    log_info "Simulator mode: $RUN_SIMULATOR"
    log_info "Cargo check mode: $RUN_CARGO_CHECK"
    log_info "Runtime tests mode: $RUN_RUNTIME_TESTS"
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
    if should_run_runtime_tests; then
        if ! run_shell_step "cargo test -p deos-runtime --lib" "" "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --lib"; then
            log_error "Runtime behavioral validation failed"
            exit 1
        fi
    else
        log_warning "Skipping runtime unit tests because no runtime source files changed"
    fi
}

run_markdown_table_validation() {
    phase_banner "Step 6: Markdown quality"
    if should_run_markdown_table_audit; then
        if ! "$SCRIPT_DIR/audit-markdown-tables.sh"; then
            log_error "Markdown table compactness validation failed"
            exit 1
        fi
    else
        log_warning "Skipping Markdown table audit because no Markdown files changed"
    fi
    if should_run_architecture_readability_audit; then
        if ! "$SCRIPT_DIR/audit-architecture-readability.sh"; then
            log_error "Architecture readability validation failed"
            exit 1
        fi
    else
        log_warning "Skipping architecture readability audit because no architecture documents changed"
    fi
}

run_wiki_trust_validation() {
    phase_banner "Step 7: Wiki trust"
    if ! should_run_wiki_trust; then
        log_warning "Skipping wiki validation because no wiki markdown files changed"
        return 0
    fi
    require_directory "$PROJECT_ROOT/web-client" "Web-client workspace"
    require_commands npm
    if ! run_shell_step "web-client wiki validation" "" "cd \"$PROJECT_ROOT/web-client\" && npm run validate:wiki"; then
        log_error "Wiki validation failed"
        exit 1
    fi
    log_success "Wiki validation passed"
}

run_economic_claim_validation() {
    phase_banner "Step 8: Economic claim integrity"
    if ! should_run_economic_claim_audit; then
        log_warning "Skipping economic claim audit because no claim inventory or architecture docs changed"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-economic-claims.sh"; then
        log_error "Economic claim validation failed"
        exit 1
    fi
}

run_router_identity_validation() {
    phase_banner "Step 9: DEOS Router identity"
    if ! should_run_router_identity_audit; then
        log_warning "Skipping Router identity audit because Router identity surfaces did not change"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-router-identity.sh"; then
        log_error "DEOS Router identity validation failed"
        exit 1
    fi
}

run_strategic_governance_ingress_validation() {
    phase_banner "Step 9: Strategic governance ingress"
    if ! should_run_strategic_governance_ingress_audit; then
        log_warning "Skipping strategic governance ingress audit because its authority surfaces did not change"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-strategic-governance-ingress.sh"; then
        log_error "Strategic governance ingress validation failed"
        exit 1
    fi
}

run_governance_structural_liveness_validation() {
    phase_banner "Step 9: Governance structural liveness"
    if ! should_run_governance_structural_liveness_audit; then
        log_warning "Skipping governance structural liveness audit because its surfaces did not change"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-governance-structural-liveness.sh"; then
        log_error "Governance structural liveness validation failed"
        exit 1
    fi
}

run_runtime_composition_dag_validation() {
    phase_banner "Step 9: Runtime composition DAG"
    if ! should_run_runtime_composition_dag_audit; then
        log_warning "Skipping runtime composition DAG audit because its ownership surfaces did not change"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-runtime-composition-dag.sh"; then
        log_error "Runtime composition DAG validation failed"
        exit 1
    fi
}

run_protocol_coherence_regression_validation() {
    phase_banner "Step 9: Protocol coherence regressions"
    if ! should_run_protocol_coherence_regression_audit; then
        log_warning "Skipping protocol coherence regression audit because its semantic-owner surfaces did not change"
        return 0
    fi
    if should_run_protocol_coherence_mutation_tests; then
        if ! "$SCRIPT_DIR/audit-protocol-coherence-regressions.sh" --self-test; then
            log_error "Protocol coherence repository-behavior mutation tests failed"
            exit 1
        fi
    fi
    if ! "$SCRIPT_DIR/audit-protocol-coherence-regressions.sh"; then
        log_error "Protocol coherence regression validation failed"
        exit 1
    fi
}

run_semantic_surface_validation() {
    phase_banner "Step 9: Error Narrowness evidence"
    if ! should_run_semantic_surface_audit; then
        log_warning "Skipping Error Narrowness audit because its checked source roots did not change"
        return 0
    fi
    require_commands node
    if ! node "$SCRIPT_DIR/audit-semantic-surface.mjs" \
        --check .agents/skills/alignment/semantic-surface.v1.json \
        --run-witnesses; then
        log_error "Error Narrowness evidence validation failed"
        exit 1
    fi
}

run_release_line_validation() {
    phase_banner "Step 9: Release-line consistency"
    if ! should_run_release_line_audit; then
        log_warning "Skipping release-line audit because no release marker files changed"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-release-line.sh"; then
        log_error "Release-line validation failed"
        exit 1
    fi
}

run_backlog_validation() {
    phase_banner "Step 10: Backlog open-work shape"
    if ! should_run_backlog_audit; then
        log_warning "Skipping backlog audit because BACKLOG.md did not change"
        return 0
    fi
    if ! "$SCRIPT_DIR/audit-backlog-open-work.sh"; then
        log_error "Backlog open-work validation failed"
        exit 1
    fi
}

run_knowledge_sync() {
    phase_banner "Step 11: Knowledge sync"
    if [[ "$REQUIRE_CONTEXT_SYNC" != "1" ]]; then
        log_warning "Context sync gate disabled"
        return 0
    fi
    if (( ${#CHANGED_PATHS[@]} == 0 )); then
        log_success "Clean candidate has no changed paths requiring context sync"
        return 0
    fi
    if has_changed_path '^(BACKLOG\.md|CHANGELOG\.md|AGENTS\.md|docs/|\.agents/skills/README\.md$|\.agents/skills/.*/SKILL\.md$)'; then
        log_success "Context files were updated in this pass"
        return 0
    fi
    log_error "Context sync missing: update CHANGELOG.md, AGENTS.md, BACKLOG.md, docs/, the project skill graph, or a touched SKILL.md before the next loop"
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
    run_markdown_table_validation
    run_wiki_trust_validation
    run_economic_claim_validation
    run_router_identity_validation
    run_strategic_governance_ingress_validation
    run_governance_structural_liveness_validation
    run_runtime_composition_dag_validation
    run_protocol_coherence_regression_validation
    run_semantic_surface_validation
    run_release_line_validation
    run_backlog_validation
    run_knowledge_sync
    phase_banner "Summary"
    log_success "Completion gate passed"
}

run_entrypoint() {
    if [[ "${1:-}" == "--internal" ]]; then
        shift
        main "$@"
        return
    fi
    local arg
    for arg in "$@"; do
        if [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
            main "$@"
            return
        fi
    done
    local script_path
    script_path="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
    run_command_step "DEOS completion gate" "" "$script_path" --internal "$@"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_entrypoint "$@"
fi
