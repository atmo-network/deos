#!/usr/bin/env bash

set -euo pipefail
VALIDATION_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
VALIDATION_PROJECT_ROOT="$(cd "$VALIDATION_SCRIPT_DIR/.." && pwd -P)"
export DEOS_PROJECT_ROOT="$VALIDATION_PROJECT_ROOT"
export DEOS_BINARY_DIR="$VALIDATION_PROJECT_ROOT/bin"
source "$VALIDATION_SCRIPT_DIR/_common.sh"

PROFILE=""
FRESH=0
ALIGNMENT_SCRIPT_DIR="$PROJECT_ROOT/.agents/skills/alignment/scripts"
EVIDENCE_HELPER="$PROJECT_ROOT/scripts/validation-evidence.mjs"

usage() {
    cat <<'EOF'
Usage: validate-local.sh [--fresh] <fast|heavy|full>

Runs the canonical DEOS release-validation profile. A clean whole-profile success
may be reused only when every declared evidence dimension is exactly identical.

Profiles:
  fast   Pinned script dependencies, repository audits, simulator truth, and
         complete Rust workspace CI.
  heavy  Everything in fast plus clean client validation, Actors assurance,
         benchmark compilation, and bounded-capacity evidence. Registry-backed
         dependency posture remains an explicit opt-in audit.
  full   Everything in heavy plus two clean deterministic production builds,
         metadata/client evidence regeneration, exact generated-artifact SHA-256
         comparison, and separate zero tracked drift checks.
         Pre-1.0 releases have no runtime-upgrade lineage.

Options:
  --fresh  Bypass an exact evidence hit, execute the whole profile, and replace
           the same-key success record atomically.

Environment:
  DEOS_VALIDATION_CACHE=0|1  Disable or enable both local lookup and storage.
  DEOS_VERBOSE=0|1          Stream nested command output; not evidence identity.
  DEOS_PROJECT_ROOT and DEOS_BINARY_DIR are always replaced with the resolved
  repository root and its bin/ directory; caller values cannot redirect validation.

Dirty candidates:
  full and every CI profile fail closed. Local fast/heavy execute uncached and
  never read or write evidence. Clean records live under the Git common dir,
  never in the repository candidate tree.

Inputs:
  Repository source, locks, generated artifacts, and pinned toolchain authorities.

Outputs:
  One profile-owned pass/fail result with compact retained failure logs and,
  for an eligible clean success, one atomic Git-common-dir evidence record.

Side effects:
  Prepares pinned toolchains and replaces web-client/node_modules via npm ci.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --fresh)
                if [[ "$FRESH" == "1" ]]; then
                    log_error "--fresh may be specified only once"
                    exit 2
                fi
                FRESH=1
                ;;
            fast|heavy|full)
                if [[ -n "$PROFILE" ]]; then
                    log_error "Specify exactly one validation profile"
                    exit 2
                fi
                PROFILE="$1"
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown validation argument: $1"
                usage
                exit 1
                ;;
        esac
        shift
    done
    if [[ -z "$PROFILE" ]]; then
        usage
        exit 1
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Validation profile"
    activate_pinned_node
    require_commands bash git node python3 rustup
    [[ -f "$EVIDENCE_HELPER" ]] || { log_error "Validation evidence helper not found: $EVIDENCE_HELPER"; exit 1; }
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
    run_script_step "Deterministic production runtime" "03-build-runtime.sh"
    run_script_step "Runtime metadata and descriptors" "export-papi-metadata.sh"
    run_shell_step "Runtime-derived client evidence" "" "cd '$PROJECT_ROOT/web-client' && npm run generate:actors-abi && npm run generate:ingress-evidence && npm run generate:observation-evidence"
    run_shell_step "Package-derived Actors evidence" "" "cd '$TEMPLATE_DIR' && cargo run -q --locked -p pallet-deos-actors --example semantic_manifest -- --check ../web-client/src/lib/automation/actors-semantic-manifest.json && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
}

require_tracked_zero_drift() {
    local label="$1"
    if [[ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]]; then
        log_error "$label changed the exact candidate worktree"
        git -C "$PROJECT_ROOT" status --short
        exit 1
    fi
    log_success "$label preserved tracked, staged, and untracked zero drift"
}

report_validation_boundary() {
    node "$EVIDENCE_HELPER" boundary "$1"
}

run_full() {
    local first_manifest second_manifest
    first_manifest="$(mktemp "${TMPDIR:-/tmp}/deos-full-artifacts-pass-1.XXXXXX.json")"
    second_manifest="$(mktemp "${TMPDIR:-/tmp}/deos-full-artifacts-pass-2.XXXXXX.json")"
    trap 'rm -f "$first_manifest" "$second_manifest"' EXIT

    run_fast_checks
    phase_banner "Step 4: Canonical full-profile identity"
    regenerate_full_artifacts
    report_validation_boundary "full.regeneration.canonical-pass-1"
    require_tracked_zero_drift "Canonical first regeneration"
    report_validation_boundary "full.tracked-zero-drift.pass-1"
    node "$EVIDENCE_HELPER" artifact-manifest --repo "$PROJECT_ROOT" --output "$first_manifest"

    run_heavy_checks

    phase_banner "Step 5: Full-profile reproducibility"
    regenerate_full_artifacts
    report_validation_boundary "full.regeneration.canonical-pass-2"
    require_tracked_zero_drift "Canonical second regeneration"
    report_validation_boundary "full.tracked-zero-drift.pass-2"
    node "$EVIDENCE_HELPER" artifact-manifest --repo "$PROJECT_ROOT" --output "$second_manifest"
    node "$EVIDENCE_HELPER" compare-artifacts "$first_manifest" "$second_manifest"
    report_validation_boundary "full.ignored-and-generated-artifacts.exact-sha256-compare"
    rm -f "$first_manifest" "$second_manifest"
    trap - EXIT
}

run_internal_profile() {
    case "$PROFILE" in
        fast) run_fast_checks ;;
        heavy)
            run_fast_checks
            run_heavy_checks
            ;;
        full) run_full ;;
    esac
    phase_banner "Summary"
    log_success "$PROFILE release-validation profile passed"
}

require_internal_mode() {
    if [[ "${DEOS_VALIDATION_INTERNAL:-0}" != "1" ]]; then
        log_error "Internal validation mode is owned by validation-evidence.mjs"
        exit 2
    fi
    if [[ $# -ne 1 ]]; then
        log_error "Internal validation mode requires one profile"
        exit 2
    fi
    case "$1" in
        fast|heavy|full) PROFILE="$1" ;;
        *) log_error "Unsupported internal validation profile: $1"; exit 2 ;;
    esac
}

run_with_evidence() {
    local -a command=(
        node "$EVIDENCE_HELPER" run
        --repo "$PROJECT_ROOT"
        --profile "$PROFILE"
    )
    if [[ "$FRESH" == "1" ]]; then
        command+=(--fresh)
    fi
    "${command[@]}"
}

main() {
    if [[ "${1:-}" == "--internal-prepare" ]]; then
        shift
        require_internal_mode "$@"
        cd "$PROJECT_ROOT"
        prepare_pinned_environment
        return
    fi
    if [[ "${1:-}" == "--internal-run" ]]; then
        shift
        require_internal_mode "$@"
        cd "$PROJECT_ROOT"
        run_internal_profile
        return
    fi

    parse_args "$@"
    cd "$PROJECT_ROOT"
    check_prerequisites
    run_with_evidence
}

main "$@"
