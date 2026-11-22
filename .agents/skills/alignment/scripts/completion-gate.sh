#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

AUDIT_SCOPE="${AUDIT_SCOPE:-changed}"
RUN_SIMULATOR="${RUN_SIMULATOR:-auto}"
RUN_CARGO_CHECK="${RUN_CARGO_CHECK:-auto}"
RUN_RUNTIME_TESTS="${RUN_RUNTIME_TESTS:-auto}"
RUN_PALLET_TESTS="${RUN_PALLET_TESTS:-auto}"
REQUIRE_CONTEXT_SYNC="${REQUIRE_CONTEXT_SYNC:-1}"
BASE_REF=""
BASE_COMMIT=""
SELECTED_RANGE="HEAD -> worktree (including untracked paths)"
PLAN_ONLY=0
WORKSPACE_WIDE=0
CRATE_PACKAGES=""
STANDALONE_PACKAGES=""
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
  --self-test              Check routing without running project validation
  --base REF               Compare REF to committed HEAD; requires a clean checkout
  --plan                   Print the selected scope without running validation
  --all-rust               Run architecture audit against the full pallet tree
  --skip-simulator         Do not run simulator validation
  --skip-cargo-check       Do not run cargo check validation
  --skip-runtime-tests     Do not run runtime unit tests
  --allow-no-context-sync  Warn instead of failing when context files were not updated
  -h, --help               Show this help message

Changes to template/Cargo.toml or Cargo.lock select workspace library tests
(excluding the runtime, which runs with try-runtime) and all standalone pallet
checks. Package manifest changes select that package and, for a pallet, its
standalone native feature check. Guest checks, tools and vendor are owned by
the unpacked framework profile.

Environment:
  AUDIT_SCOPE=changed|all
  RUN_SIMULATOR=auto|0|1
  RUN_CARGO_CHECK=auto|0|1
  RUN_RUNTIME_TESTS=auto|0|1|release
  RUN_PALLET_TESTS=auto|0|1
  REQUIRE_CONTEXT_SYNC=0|1
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --base)
                [[ $# -ge 2 && -n "$2" ]] || { log_error "Missing value for --base"; exit 1; }
                BASE_REF="$2"
                shift
                ;;
            --plan)
                PLAN_ONLY=1
                ;;
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
    local candidate status path listing
    local -A seen=()
    local -a revs=(HEAD)
    if [[ -n "$BASE_REF" ]]; then
        BASE_COMMIT="$(git -C "$PROJECT_ROOT" rev-parse --verify --end-of-options "$BASE_REF^{commit}")" || return 1
        candidate="$(git -C "$PROJECT_ROOT" rev-parse --verify HEAD^{commit})" || return 1
        status="$(git -C "$PROJECT_ROOT" status --porcelain --untracked-files=normal)" || return 1
        [[ -z "$status" ]] || { log_error "--base requires a clean candidate checkout"; return 1; }
        revs=("$BASE_COMMIT" "$candidate")
        SELECTED_RANGE="$BASE_COMMIT..$candidate"
    fi
    listing="$(mktemp)" || return 1
    # Disable rename pairing so a move selects both source and destination owners.
    if ! git -C "$PROJECT_ROOT" diff --no-renames --name-only -z "${revs[@]}" -- > "$listing" ||
        { [[ -z "$BASE_REF" ]] && ! git -C "$PROJECT_ROOT" ls-files -z --others --exclude-standard >> "$listing"; }; then
        rm -f -- "$listing"
        log_error "Cannot collect Git paths for $SELECTED_RANGE"
        return 1
    fi
    while IFS= read -r -d '' path; do
        [[ -n "${seen[$path]:-}" ]] && continue
        seen["$path"]=1
        CHANGED_PATHS+=("$path")
        if [[ "$path" =~ \.sh$ && -f "$PROJECT_ROOT/$path" ]]; then
            CHANGED_SHELL_PATHS+=("$PROJECT_ROOT/$path")
        fi
    done < "$listing"
    rm -f -- "$listing"
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
    has_changed_path '^simulators/tmctol/' || has_changed_path '^template/pallets/(tmc|router)/'
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
        has_changed_path '^docs/deos/core\.architecture\.en\.md$' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-runtime-composition-dag\.sh$'
}

should_run_protocol_coherence_regression_audit() {
    has_changed_path '^template/(pallets|primitives)/' || \
        has_changed_path '^template/runtime/src/' || \
        has_changed_path '^docs/' || \
        has_changed_path '^simulators/tmctol/' || \
        has_changed_path '^web-client/(src|docs|scripts)/' || \
        has_changed_path '^wiki/' || \
        has_changed_path '^scripts/' || \
        has_changed_path '^\.agents/skills/alignment/(rules/actors-identity-rules\.json|scripts/audit-(actors-identity|protocol-coherence-regressions)\.sh)$'
}

should_run_protocol_coherence_mutation_tests() {
    has_changed_path '^\.agents/skills/alignment/scripts/audit-protocol-coherence-regressions\.sh$'
}

is_workspace_change() {
    [[ "$1" =~ ^template/.*\.rs$ || "$1" =~ ^template/(.*/)?Cargo\.toml$ || "$1" == template/Cargo.lock ]] &&
        [[ "$1" != template/tools/* && "$1" != template/vendor/* ]]
}

should_run_cargo_check() {
    [[ "$RUN_CARGO_CHECK" != 0 ]] || return 1
    [[ "$RUN_CARGO_CHECK" != 1 ]] || return 0
    local path
    for path in "${CHANGED_PATHS[@]}"; do
        if is_workspace_change "$path"; then return 0; fi
    done
    return 1
}

should_run_runtime_tests() {
    [[ "$RUN_RUNTIME_TESTS" != 0 ]] || return 1
    [[ "$RUN_RUNTIME_TESTS" == 1 || "$RUN_RUNTIME_TESTS" == release || "$WORKSPACE_WIDE" == 1 ]] ||
        has_changed_path '^template/runtime/(.*\.rs|Cargo\.toml)$'
}

package_name() {
    local manifest="$1" name
    name="$(awk '
        /^[[:space:]]*\[/ { in_package = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*(#.*)?$/) }
        in_package && /^[[:space:]]*name[[:space:]]*=/ {
            value = $0
            sub(/^[[:space:]]*name[[:space:]]*=[[:space:]]*/, "", value)
            if (value !~ /^"[A-Za-z0-9][A-Za-z0-9_-]*"[[:space:]]*(#.*)?$/) exit 1
            sub(/^"/, "", value)
            sub(/".*$/, "", value)
            name = value
            count++
        }
        END { if (count != 1) exit 1; print name }
    ' "$manifest")" && [[ "$name" =~ ^[A-Za-z0-9][A-Za-z0-9_-]*$ ]] ||
        { log_error "Cannot map crate to a package in $manifest"; return 1; }
    printf '%s\n' "$name"
}

resolve_package_plan() {
    local path directory package manifest
    local -A packages=() standalone=()
    WORKSPACE_WIDE=0
    if has_changed_path '^template/Cargo\.(toml|lock)$'; then WORKSPACE_WIDE=1; fi
    CRATE_PACKAGES=""
    STANDALONE_PACKAGES=""
    for path in "${CHANGED_PATHS[@]}"; do
        is_workspace_change "$path" || continue
        [[ "$path" != template/Cargo.toml && "$path" != template/Cargo.lock ]] || continue
        directory="$(dirname "$TEMPLATE_DIR/${path#template/}")"
        if [[ "$path" == */Cargo.toml ]]; then
            manifest="$directory/Cargo.toml"
        else
            while [[ "$directory" != "$TEMPLATE_DIR" && ! -f "$directory/Cargo.toml" ]]; do
                directory="$(dirname "$directory")"
            done
            manifest="$directory/Cargo.toml"
        fi
        package="$(package_name "$manifest")" || return 1
        if [[ "$package" != "deos-runtime" ]]; then packages["$package"]=1; fi
        if [[ "$path" =~ ^template/pallets/[^/]+/Cargo\.toml$ ]]; then standalone["$package"]=1; fi
    done
    if [[ "$WORKSPACE_WIDE" == 1 ]]; then
        for manifest in "$TEMPLATE_DIR"/pallets/*/Cargo.toml; do
            package="$(package_name "$manifest")" || return 1
            standalone["$package"]=1
        done
    fi
    if (( ${#packages[@]} )); then CRATE_PACKAGES="$(printf '%s\n' "${!packages[@]}" | LC_ALL=C sort)" || return 1; fi
    if (( ${#standalone[@]} )); then STANDALONE_PACKAGES="$(printf '%s\n' "${!standalone[@]}" | LC_ALL=C sort)" || return 1; fi
}

should_run_shell_syntax_check() {
    [[ ${#CHANGED_SHELL_PATHS[@]} -gt 0 ]]
}

should_run_markdown_table_audit() {
    has_changed_path '\.md$' || has_changed_path '^\.agents/skills/alignment/scripts/audit-markdown-tables\.sh$'
}

should_run_architecture_readability_audit() {
    has_changed_path '^docs/.*\.architecture\.en\.md$' || \
        has_changed_path '^template/pallets/[^/]+/docs/architecture\.en\.md$' || \
        has_changed_path '^AGENTS\.md$' || \
        has_changed_path '^\.agents/skills/alignment/scripts/audit-architecture-readability\.sh$'
}

should_run_release_line_audit() {
    has_changed_path '^CHANGELOG\.md$' || has_changed_path '^template/Cargo\.lock$' || has_changed_path '^template/.*/Cargo\.toml$'
}

should_run_economic_claim_audit() {
    has_changed_path '^\.agents/skills/alignment/economic-claims\.json$' || \
        has_changed_path '^docs/.*\.architecture\.en\.md$' || \
        has_changed_path '^template/pallets/[^/]+/docs/architecture\.en\.md$'
}

run_self_test() (
    local path expected_readability expected_economic readability economic
    while IFS='|' read -r path expected_readability expected_economic; do
        CHANGED_PATHS=("$path")
        readability=0
        economic=0
        if should_run_architecture_readability_audit; then readability=1; fi
        if should_run_economic_claim_audit; then economic=1; fi
        if [[ "$readability" != "$expected_readability" || "$economic" != "$expected_economic" ]]; then
            log_error "Audit routing mismatch for $path: readability=$readability economic=$economic"
            return 1
        fi
    done <<'CASES'
docs/deos/core.architecture.en.md|1|1
template/pallets/actors/docs/architecture.en.md|1|1
template/pallets/router/docs/architecture.en.md|1|1
AGENTS.md|1|0
.agents/skills/alignment/scripts/audit-architecture-readability.sh|1|0
.agents/skills/alignment/economic-claims.json|0|1
template/pallets/actors/docs/specification.en.md|0|0
template/pallets/actors/src/execution.rs|0|0
docs/deos/README.md|0|0
docs/README.md|0|0
CASES
    CHANGED_PATHS=()
    if should_run_architecture_readability_audit || should_run_economic_claim_audit; then
        log_error "Empty diff unexpectedly schedules architecture audits"
        return 1
    fi
    local REQUIRE_CONTEXT_SYNC=1 expected_status status
    while IFS='|' read -r path expected_status; do
        CHANGED_PATHS=("$path")
        status=0
        (run_knowledge_sync) >/dev/null 2>&1 || status=$?
        if [[ "$status" != "$expected_status" ]]; then
            log_error "Knowledge-sync routing mismatch for $path: status=$status expected=$expected_status"
            return 1
        fi
    done <<'CASES'
README.md|0
scripts/README.md|0
template/pallets/actors/README.md|0
.agents/skills/README.md|0
AGENTS.md|0
BACKLOG.md|0
CHANGELOG.md|0
docs/deos/core.architecture.en.md|0
.agents/skills/alignment/SKILL.md|0
scripts/_common.sh|1
scripts/README.md.bak|1
scripts/not-README.md|1
scripts/readme.md|1
CASES
    CHANGED_PATHS=()
    run_knowledge_sync >/dev/null
    CHANGED_PATHS=(scripts/_common.sh scripts/README.md)
    run_knowledge_sync >/dev/null
    CHANGED_PATHS=(scripts/_common.sh)
    REQUIRE_CONTEXT_SYNC=0 run_knowledge_sync >/dev/null
    run_behavior_routing_self_test
    log_success "Completion audit, behavioral and knowledge-sync routing self-test passed"
)

run_behavior_routing_self_test() (
    local RUN_CARGO_CHECK=auto RUN_RUNTIME_TESTS=auto BASE_REF=""
    local path check runtime wide crates standalone expected actual
    while IFS='|' read -r path check runtime wide crates standalone; do
        CHANGED_PATHS=("$path")
        resolve_package_plan
        actual="0|"
        if should_run_cargo_check; then actual="1|"; fi
        if should_run_runtime_tests; then actual+="1|"; else actual+="0|"; fi
        expected="$check|$runtime|$wide"
        if [[ "$actual$WORKSPACE_WIDE" != "$expected" ]]; then
            log_error "Behavioral routing mismatch for $path: $actual$WORKSPACE_WIDE != $expected"
            return 1
        fi
        [[ "$CRATE_PACKAGES" == "$crates" ]] || { log_error "Wrong crate tests for $path"; return 1; }
        if [[ "$wide" == 1 ]]; then
            [[ "$STANDALONE_PACKAGES" == *pallet-deos-governance* && "$STANDALONE_PACKAGES" == *pallet-deos-actors* ]] ||
                { log_error "Missing workspace standalone checks for $path"; return 1; }
        else
            [[ "$STANDALONE_PACKAGES" == "$standalone" ]] || { log_error "Wrong standalone checks for $path"; return 1; }
        fi
    done <<'CASES'
template/pallets/governance/Cargo.toml|1|0|0|pallet-deos-governance|pallet-deos-governance
template/runtime/Cargo.toml|1|1|0||
template/Cargo.toml|1|1|1||
template/Cargo.lock|1|1|1||
template/pallets/actors/src/execution.rs|1|0|0|pallet-deos-actors|
template/runtime/src/lib.rs|1|1|0||
template/runtime/build.rs|1|1|0||
docs/deos/core.architecture.en.md|0|0|0||
template/tools/pvf-validation/src/lib.rs|0|0|0||
template/vendor/pallet-assets/Cargo.toml|0|0|0||
CASES
    CHANGED_PATHS=(template/runtime/Cargo.toml)
    resolve_package_plan
    if RUN_RUNTIME_TESTS=0 should_run_runtime_tests || RUN_CARGO_CHECK=0 should_run_cargo_check; then
        log_error "Explicit behavioral skip did not override routing"
        return 1
    fi

    local fixture
    fixture="$(mktemp -d)"
    trap 'rm -rf -- "$fixture"' EXIT
    local PROJECT_ROOT="$fixture" TEMPLATE_DIR="$fixture/template"
    git -C "$fixture" -c init.defaultBranch=main init -q
    local -a commit=(git -C "$fixture" -c core.hooksPath=/dev/null -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm)
    "${commit[@]}" base --allow-empty
    mkdir -p "$TEMPLATE_DIR/pallets/governance"
    local manifest="$TEMPLATE_DIR/pallets/governance/Cargo.toml"
    printf '[package]\nname = "pallet-deos-governance"\n' > "$manifest"
    git -C "$fixture" add .
    "${commit[@]}" candidate
    collect_changed_paths
    [[ ${#CHANGED_PATHS[@]} == 0 ]] || { log_error "Fixture worktree is not clean"; return 1; }
    BASE_REF=HEAD^
    collect_changed_paths
    resolve_package_plan
    if [[ "${CHANGED_PATHS[*]}" != template/pallets/governance/Cargo.toml || "$CRATE_PACKAGES" != pallet-deos-governance ]] || ! should_run_cargo_check; then
        log_error "Clean committed diff failed to select Governance tests"
        return 1
    fi
    if (git() {
        if [[ "$3" == diff ]]; then return 128; fi
        command git "$@"
    }; collect_changed_paths) >/dev/null 2>&1; then
        log_error "Git diff failure accepted as a valid selection"
        return 1
    fi
    printf '[package]\n[lib]\nname = "not-the-package"\n' > "$manifest"
    BASE_REF=""
    collect_changed_paths
    if resolve_package_plan >/dev/null 2>&1; then
        log_error "Malformed crate mapping accepted as a valid selection"
        return 1
    fi
)

should_run_backlog_audit() {
    has_changed_path '^BACKLOG\.md$'
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$SKILL_DIR" "Skill directory"
    hydrate_local_tool_paths
    require_commands git
    collect_changed_paths
    resolve_package_plan
    if [[ "$PLAN_ONLY" == 0 ]]; then
        mkdir -p "$LEDGER_DIR"
        touch "$HALLUCINATIONS_FILE"
    fi
    log_success "Gate prerequisites checked"
}

plan() {
    phase_banner "DEOS completion gate"
    log_info "Selected range: $SELECTED_RANGE"
    log_info "Audit scope: $AUDIT_SCOPE"
    log_info "Changed paths: ${#CHANGED_PATHS[@]}"
    log_info "Changed shell scripts: ${#CHANGED_SHELL_PATHS[@]}"
    log_info "Simulator mode: $RUN_SIMULATOR"
    log_info "Cargo check mode: $RUN_CARGO_CHECK"
    log_info "Runtime tests mode: $RUN_RUNTIME_TESTS"
    log_info "Require context sync: $REQUIRE_CONTEXT_SYNC"
    if should_run_cargo_check; then
        log_info "Select workspace cargo check"
        if should_run_runtime_tests; then log_info "Select runtime library tests (try-runtime)"; else log_info "Skip runtime library tests"; fi
        if [[ "$RUN_PALLET_TESTS" == 0 ]]; then
            log_info "Skip crate library tests (explicit override)"
        elif [[ "$WORKSPACE_WIDE" == 1 ]]; then
            log_info "Select workspace library tests (excluding deos-runtime)"
        elif [[ -n "$CRATE_PACKAGES" ]]; then
            log_info "Select crate library tests: ${CRATE_PACKAGES//$'\n'/, }"
        else
            log_info "Skip crate library tests (no mapped package)"
        fi
        if [[ -n "$STANDALONE_PACKAGES" ]]; then
            log_info "Select standalone native checks: ${STANDALONE_PACKAGES//$'\n'/, }"
        else
            log_info "Skip standalone native checks"
        fi
    else
        log_info "Skip behavioral layer (no affected workspace input or explicit override)"
    fi
    log_info "Separate profiles: Wiki and unpacked framework (including guest, vendor and tools)"
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
    if ! "$SCRIPT_DIR/auditor.sh" --since "${BASE_COMMIT:-HEAD}"; then
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
    local path
    for path in "${CHANGED_SHELL_PATHS[@]}"; do
        if ! bash -n "$path"; then
            log_error "Shell syntax validation failed: $path"
            exit 1
        fi
    done
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
        log_warning "Skipping cargo check because no affected workspace input was selected or an explicit override skipped it"
        return 0
    fi
    require_directory "$TEMPLATE_DIR" "Template directory"
    require_commands cargo
    if ! run_shell_step "cargo check --workspace" "" "cd \"$TEMPLATE_DIR\" && cargo check --locked --workspace"; then
        log_error "Behavioral validation failed"
        exit 1
    fi
    if should_run_runtime_tests; then
        # The runtime unit suite costs about 13 minutes in release but far more in the debug
        # profile, so RUN_RUNTIME_TESTS=release keeps the canonical gate usable on a local
        # machine. It changes only the build profile; every assertion stays identical.
        local runtime_profile="" runtime_label=""
        if [[ "$RUN_RUNTIME_TESTS" == "release" ]]; then
            runtime_profile="--release"
            runtime_label=" (release)"
        fi
        # The `try-runtime` feature is what compiles the pallet `try_state` reconciliation hooks
        # into executable form; without it those assertions are dead weight. Measured on this tree at
        # 390s against 363s for the identical selection, so the gate pays about 7% for the whole
        # invariant layer instead of running the suite twice.
        if ! run_shell_step "cargo test -p deos-runtime --lib --features try-runtime$runtime_label" "" "cd \"$TEMPLATE_DIR\" && cargo test --locked $runtime_profile -p deos-runtime --lib --features try-runtime"; then
            log_error "Runtime behavioral validation failed"
            exit 1
        fi
    fi
    local package
    if [[ "$RUN_PALLET_TESTS" != 0 ]]; then
        if [[ "$WORKSPACE_WIDE" == 1 ]]; then
            run_shell_step "Workspace library tests (excluding deos-runtime)" "" \
                "cd \"$TEMPLATE_DIR\" && cargo test --locked --workspace --exclude deos-runtime --lib"
        elif [[ -n "$CRATE_PACKAGES" ]]; then
            while IFS= read -r package; do
                run_shell_step "cargo test -p $package --lib" "" \
                    "cd \"$TEMPLATE_DIR\" && cargo test --locked -p $package --lib"
            done <<< "$CRATE_PACKAGES"
        fi
    fi
    if [[ -n "$STANDALONE_PACKAGES" ]]; then
        while IFS= read -r package; do
            run_command_step "Independent pallet package $package" "" \
                "$PROJECT_ROOT/scripts/ci-local.sh" --only check --package "$package"
        done <<< "$STANDALONE_PACKAGES"
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
    if has_changed_path '(^|/)README\.md$|^(BACKLOG\.md|CHANGELOG\.md|AGENTS\.md|docs/|\.agents/skills/.*/SKILL\.md$)'; then
        log_success "Context files were updated in this pass"
        return 0
    fi
    log_error "Context sync missing: update CHANGELOG.md, AGENTS.md, BACKLOG.md, a root/subtree README.md, docs/, or a touched SKILL.md before the next loop"
    exit 1
}

main() {
    parse_args "$@"
    check_prerequisites
    plan
    if [[ "$PLAN_ONLY" == 1 ]]; then return; fi
    run_self_test
    run_architecture_audit
    run_shell_syntax_validation
    run_simulator_validation
    run_behavior_validation
    run_markdown_table_validation
    run_economic_claim_validation
    run_router_identity_validation
    run_strategic_governance_ingress_validation
    run_governance_structural_liveness_validation
    run_runtime_composition_dag_validation
    run_protocol_coherence_regression_validation
    run_release_line_validation
    run_backlog_validation
    run_knowledge_sync
    phase_banner "Summary"
    log_success "Completion gate passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    if [[ $# == 1 && "$1" == "--self-test" ]]; then
        run_self_test
    else
        main "$@"
    fi
fi
