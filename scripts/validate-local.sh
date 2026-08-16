#!/usr/bin/env bash

set -euo pipefail
VALIDATION_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
VALIDATION_PROJECT_ROOT="$(cd "$VALIDATION_SCRIPT_DIR/.." && pwd -P)"
export DEOS_PROJECT_ROOT="$VALIDATION_PROJECT_ROOT"
export DEOS_BINARY_DIR="$VALIDATION_PROJECT_ROOT/bin"
source "$VALIDATION_SCRIPT_DIR/_common.sh"

PROFILE=""
ALIGNMENT_SCRIPT_DIR="$PROJECT_ROOT/.agents/skills/alignment/scripts"

usage() {
    cat <<'EOF'
Usage: validate-local.sh <fast|heavy|full>

Runs one DEOS validation profile directly.

Profiles:
  fast   Prepare pinned repository dependencies, then run repository audits,
         simulator truth, and complete Rust workspace CI.
  heavy  Everything in fast plus client validation, Actors assurance,
         benchmark compilation, and full-scope changed-contract validation.
  full   Everything in heavy plus production runtime, metadata, descriptor,
         and generated client evidence regeneration with zero worktree drift.

Environment:
  DEOS_VERBOSE=0|1  Stream nested command output.

Inputs:
  Repository source, lockfiles, generated artifacts, and pinned toolchains.

Outputs:
  One direct profile pass/fail result with compact retained failure logs.

Side effects:
  Prepares pinned toolchains, replaces web-client/node_modules via npm ci, and
  in full mode regenerates production runtime and client artifacts.
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
    require_commands bash git node python3 rustup
    log_info "Profile: $PROFILE"
}

run_alignment_script_step() {
    local label="$1"
    local script_name="$2"
    shift 2
    local script_path="$ALIGNMENT_SCRIPT_DIR/$script_name"
    [[ -x "$script_path" ]] || { log_error "Alignment audit not found: $script_name"; exit 1; }
    run_shell_step "$label" "" "'$script_path' $*"
}

prepare_pinned_environment() {
    phase_banner "Step 2: Pinned validation environment"
    run_script_step "Pinned Rust environment" "setup-environment.sh" rust
    run_script_step "Pinned Node runtime" "setup-environment.sh" node
    run_script_step "Pinned script dependencies" "setup-environment.sh" client
}

run_fast_checks() {
    phase_banner "Step 3: Fast profile"
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
    phase_banner "Step 4: Heavy profile"
    run_shell_step "Clean web-client validation" "" "cd '$PROJECT_ROOT/web-client' && npm run validate:all"
    AUDIT_SCOPE=all RUN_SIMULATOR=1 RUN_CARGO_CHECK=1 RUN_RUNTIME_TESTS=1 run_alignment_script_step "Full-scope completion gate" completion-gate.sh --all-rust
    run_script_step "Actors assurance" "actors-assurance.sh"
    run_script_step "Benchmark compilation" "benchmarks.sh" --check
}

regenerate_full_artifacts() {
    phase_banner "Step 5: Full profile artifacts"
    run_script_step "Deterministic production runtime" "03-build-runtime.sh"
    run_script_step "Runtime metadata and descriptors" "export-papi-metadata.sh"
    run_shell_step "Runtime-derived client evidence" "" "cd '$PROJECT_ROOT/web-client' && npm run generate:actors-abi && npm run generate:ingress-evidence && npm run generate:observation-evidence"
    run_shell_step "Package-derived Actors evidence" "" "cd '$TEMPLATE_DIR' && cargo run -q --locked -p pallet-deos-actors --example semantic_manifest -- --check ../web-client/src/lib/automation/actors-semantic-manifest.json && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
    if [[ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]]; then
        log_error "Full artifact regeneration changed the candidate worktree"
        git -C "$PROJECT_ROOT" status --short
        exit 1
    fi
    log_success "Full artifact regeneration preserved zero worktree drift"
}

main() {
    parse_args "$@"
    cd "$PROJECT_ROOT"
    check_prerequisites
    prepare_pinned_environment
    run_fast_checks
    if [[ "$PROFILE" == "heavy" || "$PROFILE" == "full" ]]; then
        run_heavy_checks
    fi
    if [[ "$PROFILE" == "full" ]]; then
        regenerate_full_artifacts
    fi
    phase_banner "Summary"
    log_success "$PROFILE validation profile passed"
}

main "$@"
