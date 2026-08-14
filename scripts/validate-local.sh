#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

PROFILE=""
ALIGNMENT_SCRIPT_DIR="$PROJECT_ROOT/.agents/skills/alignment/scripts"

usage() {
    cat <<'EOF'
Usage: validate-local.sh <fast|heavy|full>

Runs the canonical DEOS release-validation profile.

Profiles:
  fast   Pinned script dependencies, repository audits, simulator truth, and
         complete Rust workspace CI.
  heavy  Everything in fast plus clean client validation, Actors assurance,
         benchmark compilation, and bounded-capacity evidence. Registry-backed
         dependency posture remains an explicit opt-in audit.
  full   Everything in heavy plus a clean deterministic production build,
         metadata/client evidence regeneration, and zero tracked drift.
         Pre-1.0 releases have no runtime-upgrade lineage.

Inputs:
  Repository source, locks, generated artifacts, and pinned toolchain authorities.

Outputs:
  One profile-owned pass/fail result with compact retained failure logs.

Side effects:
  Prepares pinned toolchains and replaces web-client/node_modules via npm ci.
EOF
}

parse_args() {
    if [[ $# -ne 1 ]]; then
        usage
        exit 1
    fi
    case "$1" in
        fast|heavy|full) PROFILE="$1" ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "Unknown validation profile: $1"
            usage
            exit 1
            ;;
    esac
}

check_prerequisites() {
    phase_banner "Step 1: Validation profile"
    activate_pinned_node
    require_commands bash node python3 rustup
    if [[ "$PROFILE" == "full" && -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]]; then
        log_error "full requires a clean exact candidate worktree"
        exit 1
    fi
    log_info "Profile: $PROFILE"
}

run_alignment_script_step() {
    local label="$1"
    local script_name="$2"
    shift 2
    local script_path="$ALIGNMENT_SCRIPT_DIR/$script_name"
    [[ -x "$script_path" ]] || { log_error "Alignment proof not found: $script_name"; exit 1; }
    run_shell_step "$label" "" "'$script_path' $*"
}

run_fast() {
    phase_banner "Step 2: Fast profile"
    run_script_step "Pinned Rust environment" "setup-environment.sh" rust
    run_script_step "Pinned Node runtime" "setup-environment.sh" node
    run_script_step "Pinned script dependencies" "setup-environment.sh" client
    run_alignment_script_step "Script entrypoint contract" audit-script-entrypoints.sh
    run_alignment_script_step "Template readiness" audit-template-readiness.sh
    run_alignment_script_step "Numeric parsing" audit-numeric-parsing.sh
    run_alignment_script_step "Simulator determinism" audit-simulator-determinism.sh
    run_alignment_script_step "Simulator suite mirror" audit-simulator-consistency.sh
    run_alignment_script_step "Code suppressions" audit-code-suppressions.sh
    run_alignment_script_step "Backlog shape" audit-backlog-open-work.sh
    run_alignment_script_step "Release line" audit-release-line.sh
    run_alignment_script_step "Repository portability" audit-repo-portability.sh
    run_script_step "Rust workspace CI" "ci-local.sh"
}

run_heavy_checks() {
    phase_banner "Step 3: Heavy profile"
    run_shell_step "Clean web-client validation" "" "cd '$PROJECT_ROOT/web-client' && npm run validate:all"
    AUDIT_SCOPE=all RUN_SIMULATOR=1 RUN_CARGO_CHECK=1 RUN_RUNTIME_TESTS=1 run_alignment_script_step "Full-scope completion gate" completion-gate.sh --all-rust
    run_script_step "Actors assurance" "actors-assurance.sh"
    run_script_step "Benchmark compilation" "benchmarks.sh" --check
}

run_heavy() {
    run_fast
    run_heavy_checks
}

regenerate_full_artifacts() {
    run_script_step "Deterministic production runtime" "03-build-runtime.sh"
    run_script_step "Runtime metadata and descriptors" "export-papi-metadata.sh"
    run_shell_step "Runtime-derived client evidence" "" "cd '$PROJECT_ROOT/web-client' && npm run generate:actors-abi && npm run generate:ingress-evidence && npm run generate:observation-evidence"
    run_shell_step "Package-derived Actors evidence" "" "cd '$TEMPLATE_DIR' && cargo run -q --locked -p pallet-deos-actors --example semantic_manifest -- --check ../web-client/src/lib/automation/actors-semantic-manifest.json && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
}

run_full() {
    run_fast
    phase_banner "Step 3: Canonical full-profile identity"
    regenerate_full_artifacts
    if [[ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]]; then
        log_error "Canonical preflight regeneration changed the exact candidate worktree"
        git -C "$PROJECT_ROOT" status --short
        exit 1
    fi
    run_heavy_checks
    phase_banner "Step 4: Full-profile reproducibility"
    regenerate_full_artifacts
    if [[ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]]; then
        log_error "Full regeneration changed the exact candidate worktree"
        git -C "$PROJECT_ROOT" status --short
        exit 1
    fi
}

main() {
    parse_args "$@"
    cd "$PROJECT_ROOT"
    check_prerequisites
    case "$PROFILE" in
        fast) run_fast ;;
        heavy) run_heavy ;;
        full) run_full ;;
    esac
    phase_banner "Summary"
    log_success "$PROFILE release-validation profile passed"
}

main "$@"
