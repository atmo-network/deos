#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CARGO_PROFILE="${CARGO_PROFILE:-release}"
INCLUDE_OCCUPANCY_PROFILE="${INCLUDE_OCCUPANCY_PROFILE:-1}"
QUICK_MODE="${QUICK_MODE:-0}"
REQUIRE_WASM_IDENTITY="${REQUIRE_WASM_IDENTITY:-1}"
EVIDENCE_SOURCE_IDENTITY=""
EVIDENCE_WEIGHT_IDENTITY=""
EVIDENCE_WASM_IDENTITY=""
EVIDENCE_METADATA_IDENTITY=""

REQUIRED_HEAVY_PROFILES=(
    "scheduler_stress_fifo_over_capacity_fairness_matrix"
    "scheduler_stress_fifo_dense_vs_sparse_topology_matrix"
    "scheduler_stress_fifo_sparse_topology_long_run_liveness"
    "stress_10k_actors_queue_scheduler"
    "checkpoint_a_s6_dense_10k_wakeups_converge_without_drops"
    "transfer_10k_manual_and_reactive_first_traversal"
    "transfer_10k_first_traversal_with_continuous_user_dispatch"
    "mixed_9500_transfer_400_swapout_100_control_first_traversal"
)
PACKAGE_HEAVY_PROFILES=(
    "crossing_scale_10k_zero_match_small_cohort_and_maximum_herd"
    "breaker_materializes_maximum_mixed_wakeup_crossing_and_broad_fanout_without_execution_loss"
    "crossing_mixed_dense_sparse_directional_lifecycle_profile"
    "maximum_dormant_identity_population_adds_no_idle_scan"
)
OCCUPANCY_HEAVY_PROFILE="profile_scheduler_queue_wakeup_occupancy_10k"
DIAGNOSTIC_HEAVY_PROFILES=("profile_scheduler_wallclock_matrix")

usage() {
    cat <<'EOF'
Usage: actors-assurance.sh [OPTIONS]

Runs the DEOS Actors assurance contract across the package archive, external-consumer fixture, and deos-runtime.

Options:
  --skip-occupancy-profile   Skip the gating 10k occupancy profile
  --quick                    Run only fast checks (Clippy + light tests)

The wall-clock matrix remains diagnostic and does not run in this contract.
  -h, --help                 Show this help message

Environment:
  CARGO_PROFILE=release|dev
  INCLUDE_OCCUPANCY_PROFILE=0|1
  QUICK_MODE=0|1
  REQUIRE_WASM_IDENTITY=0|1  Require and preserve production Wasm (default: 1)
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-occupancy-profile)
                INCLUDE_OCCUPANCY_PROFILE=0
                ;;
            --quick)
                QUICK_MODE=1
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

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands cargo npm git sha256sum
    [[ -f "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" ]] || {
        log_error "Actors production Weight not found"
        return 1
    }
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        [[ -f "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" ]] || {
            log_error "Production runtime Wasm not found"
            return 1
        }
    else
        log_warning "Production Wasm identity disabled by the caller; source, Weight, metadata, and behavioral assurance remain required"
    fi
    [[ -f "$PROJECT_ROOT/web-client/.papi/metadata/deos.scale" ]] || {
        log_error "Runtime metadata not found"
        return 1
    }
    log_success "Release gate prerequisites checked"
}

worktree_evidence_identity() {
    {
        git -C "$PROJECT_ROOT" diff --binary --no-ext-diff
        while IFS= read -r -d '' path; do
            sha256sum "$PROJECT_ROOT/$path"
        done < <(git -C "$PROJECT_ROOT" ls-files --others --exclude-standard -z | sort -z)
    } | sha256sum | awk '{print $1}'
}

capture_evidence_identity() {
    EVIDENCE_SOURCE_IDENTITY="$(worktree_evidence_identity)"
    EVIDENCE_WEIGHT_IDENTITY="$(sha256sum "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" | awk '{print $1}')"
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        EVIDENCE_WASM_IDENTITY="$(sha256sum "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" | awk '{print $1}')"
    else
        EVIDENCE_WASM_IDENTITY="not-required"
    fi
    EVIDENCE_METADATA_IDENTITY="$(sha256sum "$PROJECT_ROOT/web-client/.papi/metadata/deos.scale" | awk '{print $1}')"
    log_info "Evidence source:   $EVIDENCE_SOURCE_IDENTITY"
    log_info "Evidence Weight:   $EVIDENCE_WEIGHT_IDENTITY"
    log_info "Evidence Wasm:     $EVIDENCE_WASM_IDENTITY"
    log_info "Evidence metadata: $EVIDENCE_METADATA_IDENTITY"
}

verify_evidence_identity_unchanged() {
    local source_identity weight_identity wasm_identity metadata_identity
    source_identity="$(worktree_evidence_identity)"
    weight_identity="$(sha256sum "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" | awk '{print $1}')"
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        wasm_identity="$(sha256sum "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm" | awk '{print $1}')"
    else
        wasm_identity="not-required"
    fi
    metadata_identity="$(sha256sum "$PROJECT_ROOT/web-client/.papi/metadata/deos.scale" | awk '{print $1}')"
    if [[ "$source_identity" != "$EVIDENCE_SOURCE_IDENTITY" ]]; then
        log_error "Actors assurance changed the source identity"
        return 1
    fi
    if [[ "$weight_identity" != "$EVIDENCE_WEIGHT_IDENTITY" ]]; then
        log_error "Actors assurance changed the Weight identity"
        return 1
    fi
    if [[ "$wasm_identity" != "$EVIDENCE_WASM_IDENTITY" ]]; then
        log_error "Actors assurance changed the Wasm identity"
        return 1
    fi
    if [[ "$metadata_identity" != "$EVIDENCE_METADATA_IDENTITY" ]]; then
        log_error "Actors assurance changed the metadata identity"
        return 1
    fi
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        log_success "Actors assurance retained exact source, Weight, Wasm, and metadata identities"
    else
        log_success "Actors assurance retained exact source, Weight, and metadata identities; caller explicitly omitted Wasm identity"
    fi
}

required_heavy_profiles() {
    printf '%s\n' "${REQUIRED_HEAVY_PROFILES[@]}"
    if [[ "$INCLUDE_OCCUPANCY_PROFILE" == "1" ]]; then
        printf '%s\n' "$OCCUPANCY_HEAVY_PROFILE"
    fi
}

# Lists every test in the deos-runtime test harness and fails unless each
# required heavy profile resolves to exactly one test. The same inventory owns
# execution below, so a renamed/deleted/duplicated profile cannot turn green.
verify_heavy_profiles_resolve_exactly_once() {
    phase_banner "Step 1b: Exact heavy-profile resolution"
    local profile
    local required_profiles=()
    mapfile -t required_profiles < <(required_heavy_profiles)
    local listing
    listing="$(cd "$TEMPLATE_DIR" && cargo test --$CARGO_PROFILE -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "${required_profiles[@]}"; do
        local matches
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Heavy profile '${profile}' resolved to ${matches} test(s); expected exactly 1. Zero-match success is impossible by design."
            return 1
        fi
        log_info "  exact profile: ${profile} (1 test)"
    done
    log_success "All required heavy profiles resolve to exactly one test"

    listing="$(cd "$TEMPLATE_DIR" && cargo test --$CARGO_PROFILE -p pallet-deos-actors --locked -- --list 2>/dev/null)"
    for profile in "${PACKAGE_HEAVY_PROFILES[@]}"; do
        local matches
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Package heavy profile '${profile}' resolved to ${matches} test(s); expected exactly 1."
            return 1
        fi
        log_info "  exact package profile: ${profile} (1 test)"
    done
    log_success "All package heavy profiles resolve to exactly one test"
}

run_gate() {
    run_shell_step "Actors gate: fee-envelope vector freshness" "" "cd \"$TEMPLATE_DIR\" && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
    run_shell_step "Actors gate: runtime cost vector freshness" "" "cd \"$TEMPLATE_DIR\" && cargo run -q --locked -p deos-runtime --example actor_cost_vectors -- --check ../web-client/src/lib/automation/actors-cost-vectors.json"
    run_shell_step "Actors gate: ABI manifest drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:actors-abi"
    run_shell_step "Actors gate: normative surface drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:actors-normative-drift"
    run_shell_step "Actors gate: observation runtime evidence drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:observation-evidence"
    run_shell_step "Actors gate: certified ingress evidence drift" "" "cd \"$PROJECT_ROOT/web-client\" && npm run check:ingress-evidence"
    run_shell_step "Actors gate: cross-language semantic contract" "" "cd \"$PROJECT_ROOT/web-client\" && npm run test:automation"
    run_shell_step "Actors gate: exhaustive production/simulation Step parity" "" "cd \"$TEMPLATE_DIR\" && cargo test -q --locked -p pallet-deos-actors --lib canonical_step_transition_matrix_has_production_simulation_parity"

    if [[ "$QUICK_MODE" == "1" ]]; then
        run_shell_step "Actors quick gate: Clippy" "" "cd \"$TEMPLATE_DIR\" && cargo clippy --locked -p pallet-deos-actors -p deos-runtime -p pallet-deos-actors-embedding-fixture --all-targets -- -D warnings"
        run_shell_step "Actors quick gate: basic tests" "" "cd \"$TEMPLATE_DIR\" && cargo test -q --locked -p pallet-deos-actors --lib && cargo test -q --locked -p pallet-deos-actors-embedding-fixture --lib"
        run_shell_step "Actors quick gate: package archive surface" "" "cd \"$TEMPLATE_DIR\" && cargo package -p pallet-deos-actors --allow-dirty --locked --list"
        return
    fi

    run_shell_step \
        "Actors gate: pallet package archive" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo package -p pallet-deos-actors --allow-dirty --locked"

    run_shell_step \
        "Actors gate: independent embedding default profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-deos-actors-embedding-fixture --locked --lib"

    run_shell_step \
        "Actors gate: independent embedding DEX profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-deos-actors-embedding-fixture --locked --lib --features dex-fixture"

    run_shell_step \
        "Actors gate: independent embedding try-runtime profile" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-deos-actors-embedding-fixture --locked --lib --features try-runtime"

    run_shell_step \
        "Actors gate: independent embedding no-std contract" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo check --$CARGO_PROFILE -p pallet-deos-actors-embedding-fixture --locked --no-default-features"

    if [[ "$QUICK_MODE" != "1" ]]; then
        verify_heavy_profiles_resolve_exactly_once
    fi

    local profile
    local required_profiles=()
    mapfile -t required_profiles < <(required_heavy_profiles)
    for profile in "${required_profiles[@]}"; do
        run_shell_step \
            "Actors gate: exact heavy profile ${profile}" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p deos-runtime --locked ${profile} -- --ignored --nocapture"
    done
    for profile in "${PACKAGE_HEAVY_PROFILES[@]}"; do
        run_shell_step \
            "Actors gate: exact package heavy profile ${profile}" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p pallet-deos-actors --locked ${profile} -- --ignored --nocapture"
    done
    if [[ "$INCLUDE_OCCUPANCY_PROFILE" != "1" ]]; then
        log_warning "Skipping occupancy profile"
    fi
    log_info "Non-gating diagnostics (not resolved or executed by this gate): ${DIAGNOSTIC_HEAVY_PROFILES[*]}"
}

main() {
    parse_args "$@"
    phase_banner "DEOS Actors assurance"
    check_prerequisites
    capture_evidence_identity
    log_info "Profile: $CARGO_PROFILE | quick: $QUICK_MODE | occupancy: $INCLUDE_OCCUPANCY_PROFILE"
    run_gate
    verify_evidence_identity_unchanged
    phase_banner "Summary"
    log_success "Actors scheduler assurance completed successfully"
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
    run_command_step "DEOS Actors assurance" "" "$script_path" --internal "$@"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_entrypoint "$@"
fi
