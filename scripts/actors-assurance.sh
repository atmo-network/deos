#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CARGO_PROFILE="${CARGO_PROFILE:-release}"
INCLUDE_OCCUPANCY_PROFILE="${INCLUDE_OCCUPANCY_PROFILE:-1}"
QUICK_MODE="${QUICK_MODE:-0}"
INTEGRATED_W0_W1=0
INTEGRATED_W1_ACTOR_ONLY=0
INTEGRATED_W1_CONTINUOUS_USER=0
INTEGRATED_W2_SCHEDULES=0
INTEGRATED_W3_OPENING_MATRIX=0
INTEGRATED_W4_HETEROGENEOUS_EFFECTS=0
INTEGRATED_W5_LIFECYCLE_RETRY_CLEANUP=0
INTEGRATED_W6_MIXED_ARRIVAL_LIFECYCLE=0
INTEGRATED_W7_DUE_ONLY_ACTIVE_FRONTIER=0
INTEGRATED_W8_TOMBSTONE_PREFIX_CHUNK_PRESSURE=0
INTEGRATED_W9_RESOURCE_INDEPENDENCE=0
INTEGRATED_CONTROL_ATTRIBUTION=0
INTEGRATED_FUNDED_USER_ACTION=0
BACKPRESSURE_AUDIT=0
EXACT_HEAVY_PROFILE=""
PRODUCTION_REFERENCE_REPLAY=0
REQUIRE_WASM_IDENTITY="${REQUIRE_WASM_IDENTITY:-1}"
EVIDENCE_SOURCE_IDENTITY=""
EVIDENCE_WEIGHT_IDENTITY=""
EVIDENCE_WASM_IDENTITY=""
EVIDENCE_METADATA_IDENTITY=""
WASM_ARTIFACT="$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
WASM_SNAPSHOT=""

REQUIRED_HEAVY_PROFILES=(
    "scheduler_stress_fifo_over_capacity_fairness_matrix"
    "scheduler_stress_fifo_dense_vs_sparse_topology_matrix"
    "scheduler_stress_fifo_sparse_topology_long_run_liveness"
    "stress_10k_actors_queue_scheduler"
    "checkpoint_a_s6_dense_10k_wakeups_converge_without_drops"
    "control_only_10k_first_traversal_with_continuous_user_dispatch"
    "transfer_10k_homogeneous_predicate_attribution"
    "swapout_10k_manual_and_reactive_first_traversal"
    "swapout_10k_first_traversal_with_continuous_user_dispatch"
    "swapout_10k_first_traversal_with_ref_time_heavy_user_dispatch"
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
       actors-assurance.sh self-test

Runs the DEOS Actors assurance contract across the package archive, external-consumer fixture, and deos-runtime.

Source identity hashes tracked and untracked source bytes and executable mode in
template, scripts, web-client, simulator, docs, .github, .cargo, and root build configuration.
It includes staged changes and committed content, is independent of checkout path
and commit packaging, and excludes build outputs, .papi metadata/descriptors, and Skills.
Weight, production Wasm, and metadata retain their separate artifact identities.

self-test checks source identity in an isolated temporary Git repository, without
Cargo, network access, or build artifacts. The assurance gate also runs it automatically.

Options:
  --skip-occupancy-profile   Skip the gating 10k occupancy profile
  --quick                    Run only fast checks (Clippy + light tests)
  --exact-heavy-profile NAME Run one declared runtime/package heavy profile
  --production-reference-replay
                             Run the current pinned production-Wasm block/proof replay
  --integrated-w0-w1              Run only EXP-0066's full-Executive W0/W1 preparation gate
  --integrated-w1-actor-only      Run the exact-Wasm 100-block Actor-only W1 campaign
  --integrated-w1-continuous-user Run the exact-Wasm 100-block continuous-user W1 campaign
  --integrated-w2-schedules       Run exact-Wasm 100-block Manual-only and Cadenced-only W2 campaigns
  --integrated-w3-opening-matrix  Run the exact-Wasm W3 Opening-predicate/mixed-length matrix
  --integrated-w4-heterogeneous-effects
                                  Run exact-Wasm Actor-only and valid-user W4 effects
  --integrated-w5-lifecycle-retry-cleanup
                                  Run native and exact-Wasm W5 lifecycle/retry/cleanup
  --integrated-w6-mixed-arrival-lifecycle
                                  Run native and exact-Wasm W6 clocks/Triggers/churn
  --integrated-w7-due-only-active-frontier
                                  Run native and exact-Wasm W7 active-frontier scaling
  --integrated-w8-tombstone-prefix-chunk-pressure
                                  Run native and exact-Wasm W8 queue-prefix pressure
  --integrated-w9-resource-independence
                                  Run native and exact-Wasm W9 User-resource isolation
  --integrated-control-attribution
                                  Run native and exact-Wasm schedule-Control phase attribution
  --integrated-funded-user-action
                                  Run native and exact-Wasm funded User Action fee evidence
  --backpressure-audit            Run EXP-0100 existing-path atomicity/retry falsifiers

The preparation gate authors real ordered inherents, finalizes actor-only and
continuous-valid-user-demand blocks, then independently replays the exact production Wasm.
The W1 campaign preserves the reference preset's 15 System Actor identities (three
active and twelve dormant), fills remaining identity capacity with 9,985 mixed
Manual/Cadenced workload Actors, and proof-replays 100 linked finalized blocks,
reporting committed Cycles against the retained historical 10,000-Cycle/100-block
reference horizon; shortfall inside that horizon is reported, not gated. The
continuous-user campaign additionally fills each User base turn with valid signed
remarks and proves the next call is inadmissible. The W2 schedule campaign
separately measures Manual-only and Cadenced-only actor-demand cohorts against the
same reference horizon and exact production binding.
The W3 campaign runs equal 0/2/4 Opening-predicate by 1/2/3-Step Manual-only
cells to completion and reports Opening, middle, and final Steps separately.
The W4 campaign preserves all 15 reference identities, fills the remaining slots
with 9,485 Transfer, 400 Router SwapOut, and 100 StopCycle Actors, and runs both
Actor-only and continuously saturated valid-user demand for 100 linked blocks.
The W5 campaign completes one five-Actor Manual matrix across zero-Step,
Opening/middle/final Running, retry exhaustion, productive cleanup, minimal
admission apoptosis, and uncompensated committed-prefix durability.
The W6 campaign combines dense Manual readiness, sparse block windows, AtTime
tick deadlines and seeded Cadenced periods with pause/resume, close-created
tombstones and normal Crossing generation rotation without stale execution.
Its timeline distinguishes deadline, materialization and service: distinct short
tick periods can become due together in the same produced block.
The W7 campaign compares one 100-due-Actor control with the same due frontier at
10,000 total identities: 4,943 future, 4,942 unsignaled, and 15 retained reference
identities form the 9,900 non-due population. Both profiles must preserve exact
FIFO/Q1 throughput and the complete-attempt Control frontier.
The W8 campaign measures legal closed-Actor tombstone prefixes of
1/4/8/16/32/64/128 entries ahead of the same 100 due Manual Actors. Each complete
block must reclaim the exact prefix, preserve the live FIFO/Q1 committed prefix,
and stop remaining live work only at the Actor Control frontier without opening a
page-layout comparison.
The W9 campaign compares the same 100-due-Actor FIFO frontier under Actor-only,
proof-saturated remarks, and RefTime-heavy valid Router demand. It requires equal
Actor service/accounting and records the explicit fallback when production-valid
business calls still reach User ProofSize before the RefTime frontier.
The Control-attribution campaign compares matched 100-Actor Manual and cadence-one
fixtures for nine linked blocks, records mandatory-Prepass versus final Control,
and proves temporal occurrence materialization is Prepass-owned and cannot service
newly materialized work past that block's captured FIFO cutoff. It also pins the
production-Weight read/write ledger for fixed coordination, dual-clock probes,
retained/removed wakeup consumption, Cadenced rearm topology, and close/fault
admission contingencies without treating additive envelopes as actual block proof.
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
            --exact-heavy-profile)
                [[ $# -ge 2 ]] || {
                    log_error "--exact-heavy-profile requires a profile name"
                    exit 2
                }
                EXACT_HEAVY_PROFILE="$2"
                shift
                ;;
            --production-reference-replay)
                PRODUCTION_REFERENCE_REPLAY=1
                ;;
            --integrated-w0-w1)
                INTEGRATED_W0_W1=1
                ;;
            --integrated-w1-actor-only)
                INTEGRATED_W1_ACTOR_ONLY=1
                ;;
            --integrated-w1-continuous-user)
                INTEGRATED_W1_CONTINUOUS_USER=1
                ;;
            --integrated-w2-schedules)
                INTEGRATED_W2_SCHEDULES=1
                ;;
            --integrated-w3-opening-matrix)
                INTEGRATED_W3_OPENING_MATRIX=1
                ;;
            --integrated-w4-heterogeneous-effects)
                INTEGRATED_W4_HETEROGENEOUS_EFFECTS=1
                ;;
            --integrated-w5-lifecycle-retry-cleanup)
                INTEGRATED_W5_LIFECYCLE_RETRY_CLEANUP=1
                ;;
            --integrated-w6-mixed-arrival-lifecycle)
                INTEGRATED_W6_MIXED_ARRIVAL_LIFECYCLE=1
                ;;
            --integrated-w7-due-only-active-frontier)
                INTEGRATED_W7_DUE_ONLY_ACTIVE_FRONTIER=1
                ;;
            --integrated-w8-tombstone-prefix-chunk-pressure)
                INTEGRATED_W8_TOMBSTONE_PREFIX_CHUNK_PRESSURE=1
                ;;
            --integrated-w9-resource-independence)
                INTEGRATED_W9_RESOURCE_INDEPENDENCE=1
                ;;
            --integrated-control-attribution)
                INTEGRATED_CONTROL_ATTRIBUTION=1
                ;;
            --integrated-funded-user-action)
                INTEGRATED_FUNDED_USER_ACTION=1
                ;;
            --backpressure-audit)
                BACKPRESSURE_AUDIT=1
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
    local integrated_count=$((INTEGRATED_W0_W1 + INTEGRATED_W1_ACTOR_ONLY + INTEGRATED_W1_CONTINUOUS_USER + INTEGRATED_W2_SCHEDULES + INTEGRATED_W3_OPENING_MATRIX + INTEGRATED_W4_HETEROGENEOUS_EFFECTS + INTEGRATED_W5_LIFECYCLE_RETRY_CLEANUP + INTEGRATED_W6_MIXED_ARRIVAL_LIFECYCLE + INTEGRATED_W7_DUE_ONLY_ACTIVE_FRONTIER + INTEGRATED_W8_TOMBSTONE_PREFIX_CHUNK_PRESSURE + INTEGRATED_W9_RESOURCE_INDEPENDENCE + INTEGRATED_CONTROL_ATTRIBUTION + INTEGRATED_FUNDED_USER_ACTION))
    if [[ "$integrated_count" -gt 1 ]]; then
        log_error "Select exactly one integrated EXP-0066 cohort"
        exit 2
    fi
    if [[ "$QUICK_MODE" == "1" && ( "$integrated_count" -gt 0 || "$BACKPRESSURE_AUDIT" == "1" ) ]]; then
        log_error "Selected focused cohorts and --quick are mutually exclusive"
        exit 2
    fi
    if [[ "$BACKPRESSURE_AUDIT" == "1" && ( "$integrated_count" -gt 0 || "$PRODUCTION_REFERENCE_REPLAY" == "1" ) ]]; then
        log_error "--backpressure-audit selects one independent assurance cohort"
        exit 2
    fi
    if [[ -n "$EXACT_HEAVY_PROFILE" && ( "$QUICK_MODE" == "1" || "$integrated_count" -gt 0 || "$PRODUCTION_REFERENCE_REPLAY" == "1" || "$BACKPRESSURE_AUDIT" == "1" ) ]]; then
        log_error "--exact-heavy-profile selects one independent assurance cohort"
        exit 2
    fi
    if [[ "$PRODUCTION_REFERENCE_REPLAY" == "1" && ( "$QUICK_MODE" == "1" || "$integrated_count" -gt 0 ) ]]; then
        log_error "--production-reference-replay selects one independent assurance cohort"
        exit 2
    fi
    if [[ -n "$EXACT_HEAVY_PROFILE" ]]; then
        local exact_matches=0 profile
        [[ "$EXACT_HEAVY_PROFILE" == "$OCCUPANCY_HEAVY_PROFILE" ]] && exact_matches=$((exact_matches + 1))
        for profile in "${REQUIRED_HEAVY_PROFILES[@]}" "${PACKAGE_HEAVY_PROFILES[@]}"; do
            [[ "$EXACT_HEAVY_PROFILE" == "$profile" ]] && exact_matches=$((exact_matches + 1))
        done
        if [[ "$exact_matches" -ne 1 ]]; then
            log_error "Exact heavy profile '$EXACT_HEAVY_PROFILE' is not declared exactly once"
            exit 2
        fi
    fi
    if [[ "$REQUIRE_WASM_IDENTITY" != "1" && ( "$integrated_count" -gt 0 || "$PRODUCTION_REFERENCE_REPLAY" == "1" ) ]]; then
        log_error "The selected production replay cohort requires the exact production Wasm identity"
        exit 2
    fi
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
        [[ -f "$WASM_ARTIFACT" ]] || {
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
    (
        cd "$PROJECT_ROOT" || exit 1
        git ls-files --cached --others --exclude-standard -z -- \
            template scripts web-client simulator docs .github .cargo \
            ':/*.toml' ':/*.json' .gitignore .gitattributes .npmrc \
            ':(exclude,glob)**/target/**' ':(exclude,glob)**/node_modules/**' \
            ':(exclude,glob)**/build/**' ':(exclude,glob)**/dist/**' \
            ':(exclude,glob)**/.svelte-kit/**' ':(exclude)web-client/.papi' \
            ':(exclude)template/chain_spec.json' \
            | LC_ALL=C sort -zu \
            | while IFS= read -r -d '' path; do
                if [[ -L "$path" ]]; then
                    log_error "Source identity cannot certify a symlink: $path" >&2
                    exit 1
                fi
                # A tracked deletion has the same source tree before and after staging.
                [[ -e "$path" ]] || continue
                if [[ -x "$path" ]]; then
                    printf 'executable\0'
                else
                    printf 'regular\0'
                fi
                sha256sum --zero -- "$path" || exit 1
            done \
            | sha256sum | awk '{print $1}'
    )
}

run_self_test() (
    local fixture
    require_commands bash git sha256sum sort mktemp
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/deos-source-identity.XXXXXX")"
    trap 'rm -rf -- "$fixture"' EXIT

    source_identity() { PROJECT_ROOT="$fixture" worktree_evidence_identity; }
    assert_equal() { [[ "$1" == "$2" ]] || { log_error "$3"; exit 1; }; }
    assert_changed() { [[ "$1" != "$2" ]] || { log_error "$3"; exit 1; }; }

    git -C "$fixture" init -q
    git -C "$fixture" config user.name 'Source identity test'
    git -C "$fixture" config user.email 'source-identity@example.invalid'
    mkdir -p "$fixture/template/runtime/src"
    local source="$fixture/template/runtime/src/lib.rs"
    printf 'baseline\n' > "$source"
    git -C "$fixture" add .
    git -C "$fixture" -c commit.gpgsign=false commit -qm baseline
    local baseline committed staged unstaged untracked deleted
    baseline="$(source_identity)"
    printf 'committed successor\n' > "$source"
    git -C "$fixture" add .
    git -C "$fixture" -c commit.gpgsign=false commit -qm successor
    committed="$(source_identity)"
    assert_changed "$baseline" "$committed" 'Different clean committed sources shared an identity'
    git -C "$fixture" -c commit.gpgsign=false commit --allow-empty -qm packaging
    assert_equal "$committed" "$(source_identity)" 'Commit packaging changed identical source bytes'
    printf 'staged successor\n' > "$source"
    git -C "$fixture" add .
    staged="$(source_identity)"
    assert_changed "$committed" "$staged" 'Staged source change was omitted'
    printf 'unstaged successor\n' > "$source"
    unstaged="$(source_identity)"
    assert_changed "$staged" "$unstaged" 'Unstaged source change was omitted'
    local unusual="$fixture/template/runtime/src/line"$'\n'"break.rs"
    printf 'untracked bytes\n' > "$unusual"
    untracked="$(source_identity)"
    assert_changed "$unstaged" "$untracked" 'Untracked source change was omitted'
    git -C "$fixture" add .
    assert_equal "$untracked" "$(source_identity)" 'Staging identical source bytes changed identity'
    rm -- "$unusual"
    deleted="$(source_identity)"
    assert_changed "$untracked" "$deleted" 'Deleted source remained in identity'
    git -C "$fixture" add -u
    assert_equal "$deleted" "$(source_identity)" 'Staging deletion changed identical source tree'
    mkdir -p "$fixture/template/target" "$fixture/web-client/.papi" "$fixture/.agents/skills"
    printf 'artifact\n' > "$fixture/template/target/runtime.wasm"
    printf 'metadata\n' > "$fixture/web-client/.papi/deos.scale"
    printf 'skill\n' > "$fixture/.agents/skills/SKILL.md"
    assert_equal "$deleted" "$(source_identity)" 'Excluded artifacts or Skills changed source identity'
    chmod +x "$source"
    assert_changed "$deleted" "$(source_identity)" 'Executable-mode source change was omitted'
    chmod -x "$source"
    assert_equal "$deleted" "$(source_identity)" 'Restoring executable mode changed source identity'
    ln -s lib.rs "$fixture/template/runtime/src/link.rs"
    if source_identity >/dev/null 2>&1; then
        log_error 'Symlink source must fail closed instead of certifying external bytes'
        exit 1
    fi
    log_success 'Actors source identity self-test passed'
)

capture_evidence_identity() {
    EVIDENCE_SOURCE_IDENTITY="$(worktree_evidence_identity)"
    EVIDENCE_WEIGHT_IDENTITY="$(sha256sum "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" | awk '{print $1}')"
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        EVIDENCE_WASM_IDENTITY="$(sha256sum "$WASM_ARTIFACT" | awk '{print $1}')"
    else
        EVIDENCE_WASM_IDENTITY="not-required"
    fi
    EVIDENCE_METADATA_IDENTITY="$(sha256sum "$PROJECT_ROOT/web-client/.papi/metadata/deos.scale" | awk '{print $1}')"
    log_info "Evidence source:   $EVIDENCE_SOURCE_IDENTITY"
    log_info "Evidence Weight:   $EVIDENCE_WEIGHT_IDENTITY"
    log_info "Evidence Wasm:     $EVIDENCE_WASM_IDENTITY"
    log_info "Evidence metadata: $EVIDENCE_METADATA_IDENTITY"
}

snapshot_wasm_artifact() {
    if [[ "$REQUIRE_WASM_IDENTITY" != "1" ]]; then
        return
    fi
    WASM_SNAPSHOT="$(mktemp)"
    cp "$WASM_ARTIFACT" "$WASM_SNAPSHOT"
}

restore_wasm_artifact() {
    if [[ -z "$WASM_SNAPSHOT" || ! -f "$WASM_SNAPSHOT" ]]; then
        return
    fi
    local replacement="${WASM_ARTIFACT}.assurance-restore"
    cp "$WASM_SNAPSHOT" "$replacement"
    mv "$replacement" "$WASM_ARTIFACT"
    rm -f "$WASM_SNAPSHOT"
    WASM_SNAPSHOT=""
}

verify_evidence_identity_unchanged() {
    local source_identity weight_identity wasm_identity metadata_identity
    source_identity="$(worktree_evidence_identity)"
    weight_identity="$(sha256sum "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" | awk '{print $1}')"
    if [[ "$REQUIRE_WASM_IDENTITY" == "1" ]]; then
        wasm_identity="$(sha256sum "$WASM_ARTIFACT" | awk '{print $1}')"
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

run_integrated_w0_w1_gate() {
    local native_profile="full_executive_w0_w1_fixture_covers_actor_only_and_valid_user_demand"
    local wasm_profile="full_executive_w0_w1_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W0/W1 fixture" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W0/W1 replay" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_w1_campaign_gate() {
    local demand="$1"
    local profile label
    case "$demand" in
        actor-only)
            profile="full_executive_w1_actor_only_100_block_campaign_replays_exact_production_wasm"
            label="Actor-only"
            ;;
        continuous-user)
            profile="full_executive_w1_continuous_valid_user_100_block_campaign_replays_exact_production_wasm"
            label="continuous valid-user"
            ;;
        *)
            log_error "Unknown EXP-0066 W1 campaign: $demand"
            return 2
            ;;
    esac
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test --release -p deos-runtime --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm ${label} W1 campaign" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$profile' -- --ignored --nocapture"
}

run_integrated_w2_schedule_gate() {
    local profile="full_executive_w2_manual_and_cadenced_only_100_block_campaigns_replay_exact_production_wasm"
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test --release -p deos-runtime --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm Manual-only and Cadenced-only W2 campaigns" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$profile' -- --ignored --nocapture"
}

run_integrated_w3_opening_matrix_gate() {
    local profile="full_executive_w3_opening_predicate_mixed_length_campaign_replays_exact_production_wasm"
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test --release -p deos-runtime --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W3 Opening-predicate and mixed-length matrix" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$profile' -- --ignored --nocapture"
}

run_integrated_w4_heterogeneous_effects_gate() {
    local profile="full_executive_w4_heterogeneous_effect_campaigns_replay_exact_production_wasm"
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test --release -p deos-runtime --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W4 heterogeneous effect campaigns" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$profile' -- --ignored --nocapture"
}

run_integrated_w5_lifecycle_retry_cleanup_gate() {
    local native_profile="full_executive_w5_lifecycle_retry_cleanup_fixture_preserves_prefixes"
    local wasm_profile="full_executive_w5_lifecycle_retry_cleanup_campaign_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W5 lifecycle/retry/cleanup fixture" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W5 lifecycle/retry/cleanup campaign" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_funded_user_action_gate() {
    local native_profile="full_executive_funded_action_collection_witness"
    local wasm_profile="full_executive_funded_action_collection_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Funded User Action profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0097 gate: native funded User Action monetary/resource witness" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0097 gate: exact production-Wasm funded User Action replay" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_w6_mixed_arrival_lifecycle_gate() {
    local native_profile="full_executive_w6_mixed_arrival_lifecycle_fixture_handles_stale_generations"
    local wasm_profile="full_executive_w6_mixed_arrival_lifecycle_campaign_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W6 clocks/Triggers/lifecycle fixture" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W6 clocks/Triggers/lifecycle campaign" \
        "" \
        "cd \"$TEMPLATE_DIR\" && DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_w7_due_only_active_frontier_gate() {
    local native_profile="full_executive_w7_due_only_active_frontier_fixture_scales_independently"
    local wasm_profile="full_executive_w7_due_only_active_frontier_campaign_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W7 due-only active-frontier fixture" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W7 due-only active-frontier campaign" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_w8_tombstone_prefix_chunk_pressure_gate() {
    local native_profile="full_executive_w8_tombstone_prefix_chunk_pressure_fixture_preserves_fifo"
    local wasm_profile="full_executive_w8_tombstone_prefix_chunk_pressure_campaign_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W8 tombstone-prefix/chunk pressure" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W8 tombstone-prefix/chunk pressure" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_w9_resource_independence_gate() {
    local native_profile="full_executive_w9_resource_independence_fixture_preserves_actor_service"
    local wasm_profile="full_executive_w9_resource_independence_campaign_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native full-Executive W9 resource independence" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm W9 resource independence" \
        "" \
        "cd \"$TEMPLATE_DIR\" && RUST_TEST_THREADS=1 DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_integrated_control_attribution_gate() {
    local native_profile="full_executive_control_phase_attribution_fixture_isolates_materialization"
    local wasm_profile="full_executive_control_phase_attribution_replays_exact_production_wasm"
    local listing profile matches
    listing="$(cd "$TEMPLATE_DIR" && SKIP_WASM_BUILD=1 cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    for profile in "$native_profile" "$wasm_profile"; do
        matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
        if [[ "$matches" -ne 1 ]]; then
            log_error "Integrated profile '${profile}' resolved to ${matches} test(s); expected exactly 1"
            return 1
        fi
    done
    run_shell_step \
        "EXP-0066 gate: native schedule-Control phase attribution" \
        "" \
        "cd \"$TEMPLATE_DIR\" && SKIP_WASM_BUILD=1 RUST_TEST_THREADS=1 cargo test -p deos-runtime --locked '$native_profile' -- --nocapture"
    run_shell_step \
        "EXP-0066 gate: exact production-Wasm schedule-Control phase attribution" \
        "" \
        "cd \"$TEMPLATE_DIR\" && SKIP_WASM_BUILD=1 RUST_TEST_THREADS=1 DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$wasm_profile' -- --ignored --nocapture"
}

run_production_reference_replay() {
    local profile="reference_full_block_replays_in_production_wasm_with_verified_storage_proof"
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && SKIP_WASM_BUILD=1 cargo test -p deos-runtime --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${profile}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Production reference replay '$profile' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "Actors gate: current production-Wasm reference block replay" \
        "" \
        "cd \"$TEMPLATE_DIR\" && SKIP_WASM_BUILD=1 DEOS_PRODUCTION_WASM='$WASM_SNAPSHOT' cargo test --release -p deos-runtime --locked '$profile' -- --ignored --nocapture"
}

run_exact_heavy_profile() {
    local package="deos-runtime"
    local profile
    for profile in "${PACKAGE_HEAVY_PROFILES[@]}"; do
        if [[ "$EXACT_HEAVY_PROFILE" == "$profile" ]]; then
            package="pallet-deos-actors"
            break
        fi
    done
    local listing matches
    listing="$(cd "$TEMPLATE_DIR" && SKIP_WASM_BUILD=1 cargo test -p "$package" --locked -- --list 2>/dev/null)"
    matches="$(printf '%s\n' "$listing" | grep -c "${EXACT_HEAVY_PROFILE}:" || true)"
    if [[ "$matches" -ne 1 ]]; then
        log_error "Exact heavy profile '$EXACT_HEAVY_PROFILE' resolved to ${matches} test(s); expected exactly 1"
        return 1
    fi
    run_shell_step \
        "Actors gate: exact heavy profile ${EXACT_HEAVY_PROFILE}" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo test --$CARGO_PROFILE -p '$package' --locked '$EXACT_HEAVY_PROFILE' -- --ignored --nocapture"
}

run_gate() {
    run_self_test
    run_shell_step "Actors gate: fee-envelope vector freshness" "" "cd \"$TEMPLATE_DIR\" && cargo run -q --locked -p pallet-deos-actors --example fee_envelope_vectors -- --metadata ../web-client/.papi/metadata/deos.scale --weights runtime/src/weights/pallet_deos_actors.rs --check ../web-client/src/lib/automation/actors-fee-envelope-vectors.json"
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

run_backpressure_audit() {
    local runtime_profiles=(
        temporary_oracle_capacity_failure_rolls_back_economics_and_has_one_retry_owner
        swap_exact_out_liquidity_boundary_fails_without_partial_execution
        xcm_deposit_failure_rolls_back_precommit_events_and_exact_holding
        oracle_publication_rolls_back_when_crossing_transition_queue_is_full
        internal_asset_transfer_rolls_back_when_funding_pending_overflows
        actor_fee_collector_rolls_back_completely_on_ledger_failure
        runtime_simulation_core_rolls_back_deos_adapter_effects
        anchored_split_transfer_rolls_back_when_a_later_recipient_is_unavailable
    )
    local package_profiles=(
        fanout_structural_fault_is_bounded_and_requires_repair_before_resume
        user_retry_insolvency_closes_before_effect_capacity_deferral
        retry_later_resumes_same_cursor_without_replaying_committed_prefix
    )
    local profile
    for profile in "${runtime_profiles[@]}"; do
        run_shell_step \
            "EXP-0100 runtime falsifier: ${profile}" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test -p deos-runtime --locked '$profile'"
    done
    for profile in "${package_profiles[@]}"; do
        run_shell_step \
            "EXP-0100 package falsifier: ${profile}" \
            "" \
            "cd \"$TEMPLATE_DIR\" && cargo test -p pallet-deos-actors --locked '$profile'"
    done
}

main() {
    parse_args "$@"
    phase_banner "DEOS Actors assurance"
    check_prerequisites
    capture_evidence_identity
    snapshot_wasm_artifact
    trap restore_wasm_artifact EXIT
    log_info "Profile: $CARGO_PROFILE | quick: $QUICK_MODE | exact-heavy-profile: ${EXACT_HEAVY_PROFILE:-none} | production-reference-replay: $PRODUCTION_REFERENCE_REPLAY | occupancy: $INCLUDE_OCCUPANCY_PROFILE | integrated-w0-w1: $INTEGRATED_W0_W1 | integrated-w1-actor-only: $INTEGRATED_W1_ACTOR_ONLY | integrated-w1-continuous-user: $INTEGRATED_W1_CONTINUOUS_USER | integrated-w2-schedules: $INTEGRATED_W2_SCHEDULES | integrated-w3-opening-matrix: $INTEGRATED_W3_OPENING_MATRIX | integrated-w4-heterogeneous-effects: $INTEGRATED_W4_HETEROGENEOUS_EFFECTS | integrated-w5-lifecycle-retry-cleanup: $INTEGRATED_W5_LIFECYCLE_RETRY_CLEANUP | integrated-w6-mixed-arrival-lifecycle: $INTEGRATED_W6_MIXED_ARRIVAL_LIFECYCLE | integrated-w7-due-only-active-frontier: $INTEGRATED_W7_DUE_ONLY_ACTIVE_FRONTIER | integrated-w8-tombstone-prefix-chunk-pressure: $INTEGRATED_W8_TOMBSTONE_PREFIX_CHUNK_PRESSURE | integrated-w9-resource-independence: $INTEGRATED_W9_RESOURCE_INDEPENDENCE | integrated-control-attribution: $INTEGRATED_CONTROL_ATTRIBUTION | integrated-funded-user-action: $INTEGRATED_FUNDED_USER_ACTION | backpressure-audit: $BACKPRESSURE_AUDIT"
    if [[ "$BACKPRESSURE_AUDIT" == "1" ]]; then
        run_backpressure_audit
    elif [[ -n "$EXACT_HEAVY_PROFILE" ]]; then
        run_exact_heavy_profile
    elif [[ "$PRODUCTION_REFERENCE_REPLAY" == "1" ]]; then
        run_production_reference_replay
    elif [[ "$INTEGRATED_W0_W1" == "1" ]]; then
        run_integrated_w0_w1_gate
    elif [[ "$INTEGRATED_W1_ACTOR_ONLY" == "1" ]]; then
        run_integrated_w1_campaign_gate actor-only
    elif [[ "$INTEGRATED_W1_CONTINUOUS_USER" == "1" ]]; then
        run_integrated_w1_campaign_gate continuous-user
    elif [[ "$INTEGRATED_W2_SCHEDULES" == "1" ]]; then
        run_integrated_w2_schedule_gate
    elif [[ "$INTEGRATED_W3_OPENING_MATRIX" == "1" ]]; then
        run_integrated_w3_opening_matrix_gate
    elif [[ "$INTEGRATED_W4_HETEROGENEOUS_EFFECTS" == "1" ]]; then
        run_integrated_w4_heterogeneous_effects_gate
    elif [[ "$INTEGRATED_W5_LIFECYCLE_RETRY_CLEANUP" == "1" ]]; then
        run_integrated_w5_lifecycle_retry_cleanup_gate
    elif [[ "$INTEGRATED_W6_MIXED_ARRIVAL_LIFECYCLE" == "1" ]]; then
        run_integrated_w6_mixed_arrival_lifecycle_gate
    elif [[ "$INTEGRATED_W7_DUE_ONLY_ACTIVE_FRONTIER" == "1" ]]; then
        run_integrated_w7_due_only_active_frontier_gate
    elif [[ "$INTEGRATED_W8_TOMBSTONE_PREFIX_CHUNK_PRESSURE" == "1" ]]; then
        run_integrated_w8_tombstone_prefix_chunk_pressure_gate
    elif [[ "$INTEGRATED_W9_RESOURCE_INDEPENDENCE" == "1" ]]; then
        run_integrated_w9_resource_independence_gate
    elif [[ "$INTEGRATED_CONTROL_ATTRIBUTION" == "1" ]]; then
        run_integrated_control_attribution_gate
    elif [[ "$INTEGRATED_FUNDED_USER_ACTION" == "1" ]]; then
        run_integrated_funded_user_action_gate
    else
        run_gate
    fi
    restore_wasm_artifact
    trap - EXIT
    verify_evidence_identity_unchanged
    phase_banner "Summary"
    log_success "Actors scheduler assurance completed successfully"
}

run_entrypoint() {
    if [[ "${1:-}" == "self-test" ]]; then
        shift
        case "$#:${1:-}" in
            0:) run_self_test ;;
            1:--help|1:-h) usage ;;
            *) usage >&2; return 2 ;;
        esac
        return
    fi
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
