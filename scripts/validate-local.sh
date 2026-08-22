#!/usr/bin/env bash

set -euo pipefail
VALIDATION_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
VALIDATION_PROJECT_ROOT="$(cd "$VALIDATION_SCRIPT_DIR/.." && pwd -P)"
export DEOS_PROJECT_ROOT="$VALIDATION_PROJECT_ROOT"
export DEOS_BINARY_DIR="$VALIDATION_PROJECT_ROOT/bin"
source "$VALIDATION_SCRIPT_DIR/_common.sh"

PROFILE=""

usage() {
    cat <<'EOF'
Usage: validate-local.sh <fast|heavy|full>

Runs one DEOS validation profile directly.

Profiles:
  fast   Prepare pinned repository dependencies, then run simulator tests and
         complete Rust workspace CI.
  heavy  Everything in fast plus client validation, Actors assurance, and
         benchmark compilation.
  full   Everything in heavy plus production runtime, metadata, descriptor,
         and generated client evidence regeneration with zero worktree drift.

Environment:
  DEOS_VERBOSE=0|1  Stream nested command output.

Inputs:
  Repository source, lockfiles, generated artifacts, tool and binary pins,
  and recorded executable checksums.

Outputs:
  One direct profile pass/fail result with compact retained failure logs.

Side effects:
  Prepares pinned toolchains, replaces web-client/node_modules via npm ci, and
  in full mode prepares the binary bundle and regenerates production
  runtime and client artifacts.
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
    require_commands bash git node python3 rustup sha256sum
    log_info "Profile: $PROFILE"
}

prepare_pinned_environment() {
    phase_banner "Step 2: Pinned validation environment"
    run_script_step "Pinned Rust and client environment" "setup-environment.sh" full
    if [[ "$PROFILE" == "full" ]]; then
        run_script_step "Checksum-verified binary bundle" "01-download-binaries.sh"
    fi
}

run_fast_checks() {
    phase_banner "Step 3: Fast profile"
    run_shell_step "Simulator tests" "" "node '$PROJECT_ROOT/simulator/tests.js'"
    SKIP_WASM_BUILD=1 run_script_step "Rust workspace CI" "ci-local.sh"
}

run_heavy_checks() {
    phase_banner "Step 4: Heavy profile"
    run_shell_step "Clean web-client validation" "" "cd '$PROJECT_ROOT/web-client' && npm run validate"
    run_script_step "Actors assurance" "actors-assurance.sh"
    run_script_step "Benchmark compilation" "benchmarks.sh" --check
}

worktree_fingerprint() {
    {
        git -C "$PROJECT_ROOT" diff --binary --no-ext-diff HEAD --
        local path
        while IFS= read -r -d '' path; do
            printf 'untracked %s\0' "$path"
            sha256sum "$PROJECT_ROOT/$path"
        done < <(git -C "$PROJECT_ROOT" ls-files --others --exclude-standard -z)
    } | sha256sum | awk '{print $1}'
}

regenerate_full_artifacts() {
    phase_banner "Step 5: Full profile artifacts"
    local fingerprint_before fingerprint_after
    fingerprint_before="$(worktree_fingerprint)"
    run_script_step "Deterministic production runtime" "03-build-runtime.sh"
    run_script_step "Runtime metadata and descriptors" "export-papi-metadata.sh"
    run_shell_step "Runtime-derived client evidence" "" "cd '$PROJECT_ROOT/web-client' && npm run generate:actors-abi && npm run generate:ingress-evidence && npm run generate:observation-evidence"
    run_shell_step "Package-derived Actors evidence" "" "cd '$TEMPLATE_DIR' && cargo run -q --locked -p pallet-deos-actors --example semantic_manifest -- --check ../web-client/src/lib/automation/actors-semantic-manifest.json && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
    fingerprint_after="$(worktree_fingerprint)"
    if [[ "$fingerprint_after" != "$fingerprint_before" ]]; then
        log_error "Full artifact regeneration changed the candidate worktree"
        git -C "$PROJECT_ROOT" status --short
        exit 1
    fi
    log_success "Full artifact regeneration preserved the exact candidate worktree"
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
