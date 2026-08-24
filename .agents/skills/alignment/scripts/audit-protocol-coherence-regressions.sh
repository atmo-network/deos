#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SELF_TEST=0
FIXTURE_CHILD="${DEOS_PROTOCOL_COHERENCE_FIXTURE_CHILD:-0}"
FIXTURE_TEST_PAUSE="${DEOS_PROTOCOL_COHERENCE_TEST_PAUSE:-}"
FIXTURE_RUN_DIR=""
FIXTURE_LOCK_FD=""
FIXTURE_LOCK_WAITER_PID=""
declare -a FIXTURE_PATHS=()

plural_preconditions_pattern='\bPreconditions\b'
legacy_condition_pattern='MaxConditionsPerStep|skipped_conditions|ConditionsNotMet|setup_condition_assets|AutomationConditionEditor|ActorLocalConditionOutcome|SkippedCondition|evaluateCondition'
duplicate_error_pattern='\b(?:ActorEligibilityError|SimulationStepOutcome|SimulationStatus|ProtocolFailureClass|RuntimeFailureClass|UnifiedFailureClass|CrossPalletFailure)\b'
soft_signed_preimage_pattern='admission remains soft|MAY remain hash-only|[Oo]ptional (?:separate )?[Pp]reimage|Creates proposal .* payload hash .* only|hash-only advisory'
stale_phase_pattern='\b(?:Phase 1|Phase 2|Phase 3|Phase 4|phase1|phase2|phase3|phase4)\b'
dead_staking_view_pattern='\b(?:StakeExposure|passive_stake_value|delegated_stake_value|native_stake_value|passive_native_stake_value|delegated_native_stake_value|AccountNativeCollatorLpLocked|account_native_collator_lp_locked|NativeSecurityCapabilities|native_security_capabilities|securityCapabilities)\b'
router_retry_reconstruction_pattern='\b(?:RouterRetryPolicy|router_retry_policy|retry_from_router_error|retryFromRouterError|retryDispositionForRouterError)\b'
plural_detector_exception_pattern="^(?:.*/)?web-client/scripts/check-actors-normative-drift\\.mjs:[0-9]+:\\Q  entry.path?.includes('Preconditions'),\\E$|^(?:.*/)?web-client/scripts/check-actors-normative-drift\\.mjs:[0-9]+:\\Q  failures.push('plural Preconditions compatibility type remains in metadata');\\E$|^(?:.*/)?web-client/scripts/test-actors-normative-drift\\.mjs:[0-9]+:\\Q  assert.match(scriptSource, /plural Preconditions compatibility type remains/);\\E$|^(?:.*/)?web-client/scripts/test-actors-normative-drift\\.mjs:[0-9]+:\\Q  assert.match(scriptSource, /entry\\.path\\?\\.includes\\('Preconditions'\\)/);\\E$"
required_package_roots=(
    "$TEMPLATE_DIR/pallets"
    "$TEMPLATE_DIR/primitives"
)
required_family_surfaces=(
    "${required_package_roots[@]}"
    "$TEMPLATE_DIR/runtime/src"
    "$PROJECT_ROOT/docs"
    "$PROJECT_ROOT/simulator"
    "$PROJECT_ROOT/scripts"
    "$PROJECT_ROOT/web-client/src"
    "$PROJECT_ROOT/web-client/docs"
    "$PROJECT_ROOT/web-client/scripts"
    "$PROJECT_ROOT/wiki"
)

usage() {
    cat <<'EOF'
Usage: audit-protocol-coherence-regressions.sh

Fail-closed source audit for semantic owners retired by the 0.7.17 coherence line
and terminology retired by the 0.7.18 contraction line: legacy Actor identity,
adaptive governance thresholds, unreserved strategic capacity, soft signed-preimage
claims, block-cadenced or inferred staking rewards, secondary native-security flags,
raw Router-error retry inference, placeholder public variants, and parallel current
names for canonical contracts, Steps, Precondition, Predicates, security identity,
failure/retry facts, or Fee Sink.

Normative-drift ownership is fail closed: Actors contract/lifecycle type modules and
pallet facades own typed errors and bounds; Governance admission owns signed-preimage
policy; Staking owns its bounded views; Router owns cause plus RetryDisposition; the
runtime composition root may only consume those typed owners. Docs, client, and Wiki
may explain or project those facts but may not rename or reconstruct them. Scans cover
active DEOS source, specifications, architecture/integration docs, client projections,
simulator semantics, and generated Wiki surfaces. Immutable upstream references and
historical CHANGELOG text are outside those explicit roots; canonical code identifiers
and rejected-shortcut prose pass only when they do not match a retired owner.

Every required family scans the complete independently reusable package roots
(template/pallets and template/primitives), the complete runtime source, root docs,
simulator and root scripts, web-client source/docs/scripts, and the generated Wiki.
CHANGELOG history, immutable OKF references, and workflow-only
Skill phases are outside those active roots. The only active content exception
is the exact plural-Preconditions detector fixture in the web-client drift gate;
clearly marked rejected-shortcut prose requires a separately reviewed exact rule.

Options:
  --self-test  Mutate realistic active roots and invoke this real audit path
  -h, --help   Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --self-test) SELF_TEST=1 ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown argument: $1"; usage; exit 1 ;;
        esac
        shift
    done
}

register_fixture() {
    FIXTURE_PATHS+=("$1")
}

remove_fixture() {
    local path="$1"
    rm -f -- "$path"
    [[ ! -e "$path" ]] || {
        log_error "Protocol-coherence fixture cleanup failed: $path"
        return 1
    }
}

cleanup_fixtures() {
    local path
    local failed=0
    if [[ -n "$FIXTURE_LOCK_WAITER_PID" ]]; then
        kill -TERM "$FIXTURE_LOCK_WAITER_PID" 2>/dev/null || true
        wait "$FIXTURE_LOCK_WAITER_PID" 2>/dev/null || true
        FIXTURE_LOCK_WAITER_PID=""
    fi
    for path in "${FIXTURE_PATHS[@]}"; do
        rm -f -- "$path"
        if [[ -e "$path" ]]; then
            log_error "Protocol-coherence fixture residue remains: $path"
            failed=1
        fi
    done
    if [[ -n "$FIXTURE_RUN_DIR" ]]; then
        rm -rf -- "$FIXTURE_RUN_DIR"
        if [[ -e "$FIXTURE_RUN_DIR" ]]; then
            log_error "Protocol-coherence self-test directory remains: $FIXTURE_RUN_DIR"
            failed=1
        fi
    fi
    return "$failed"
}

handle_fixture_exit() {
    local status=$?
    trap - EXIT INT TERM HUP
    cleanup_fixtures || status=1
    exit "$status"
}

new_fixture() {
    local directory="$1"
    local family="$2"
    local extension="$3"
    NEW_FIXTURE_PATH="$directory/protocol-coherence-${family}-$(basename "$FIXTURE_RUN_DIR").${extension}"
    [[ ! -e "$NEW_FIXTURE_PATH" ]] || {
        log_error "Protocol-coherence fixture path collision: $NEW_FIXTURE_PATH"
        return 1
    }
    register_fixture "$NEW_FIXTURE_PATH"
    : >"$NEW_FIXTURE_PATH"
}

pause_fixture_test_if_requested() {
    local stage="$1"
    [[ "$FIXTURE_TEST_PAUSE" == "$stage" ]] || return 0
    : >"$FIXTURE_RUN_DIR/$stage.ready"
    while true; do sleep 1; done
}

acquire_fixture_lock() {
    flock "$FIXTURE_LOCK_FD" &
    FIXTURE_LOCK_WAITER_PID=$!
    local status=0
    wait "$FIXTURE_LOCK_WAITER_PID" || status=$?
    FIXTURE_LOCK_WAITER_PID=""
    [[ "$status" -eq 0 ]] || {
        log_error "Protocol-coherence mutation lock acquisition failed"
        return "$status"
    }
}

run_fixture_audit() {
    local output="$1"
    DEOS_PROTOCOL_COHERENCE_FIXTURE_CHILD=1 "${BASH_SOURCE[0]}" >"$output" 2>&1
}

require_legitimate_fixture_pass() {
    local path="$1"
    local content="$2"
    local output="$3"
    printf '%s\n' "$content" >"$path"
    if ! run_fixture_audit "$output"; then
        log_error "Legitimate protocol-coherence fixture failed in $path"
        tail -n 40 "$output" >&2
        return 1
    fi
}

require_mutation_failure() {
    local family="$1"
    local path="$2"
    local content="$3"
    local expected="$4"
    local output="$5"
    printf '%s\n' "$content" >"$path"
    if run_fixture_audit "$output"; then
        log_error "$family repository mutation passed unexpectedly: $path"
        return 1
    fi
    if ! rg -q -F "$expected" "$output"; then
        log_error "$family mutation failed outside the intended detector family"
        tail -n 40 "$output" >&2
        return 1
    fi
    remove_fixture "$path"
}

run_normative_drift_fixtures() {
    # Cleanup ownership and every terminating trap exist before any self-test
    # output, fixture, lock-owned artifact, or blocking flock operation.
    FIXTURE_PATHS=()
    FIXTURE_RUN_DIR=""
    FIXTURE_LOCK_FD=""
    FIXTURE_LOCK_WAITER_PID=""
    trap handle_fixture_exit EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    trap 'exit 129' HUP

    phase_banner "Step 2: Repository-behavior mutation fixtures"
    FIXTURE_RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/deos-protocol-coherence-self-test-$$-XXXXXXXX")"
    local output="$FIXTURE_RUN_DIR/audit.log"
    register_fixture "$output"
    : >"$output"
    exec {FIXTURE_LOCK_FD}>"$PROJECT_ROOT/.git/deos-protocol-coherence-mutation.lock"
    acquire_fixture_lock

    local plural_path condition_path duplicate_path soft_path phase_path staking_path retry_path near_miss_path
    new_fixture "$TEMPLATE_DIR/pallets/actors/src" plural rs; plural_path="$NEW_FIXTURE_PATH"
    new_fixture "$TEMPLATE_DIR/pallets/actors/docs" condition md; condition_path="$NEW_FIXTURE_PATH"
    new_fixture "$TEMPLATE_DIR/primitives/src" duplicate rs; duplicate_path="$NEW_FIXTURE_PATH"
    new_fixture "$PROJECT_ROOT/docs" signed-preimage md; soft_path="$NEW_FIXTURE_PATH"
    new_fixture "$TEMPLATE_DIR/pallets/staking/docs" numbered-phase md; phase_path="$NEW_FIXTURE_PATH"
    new_fixture "$TEMPLATE_DIR/pallets/staking/src" staking-view rs; staking_path="$NEW_FIXTURE_PATH"
    new_fixture "$PROJECT_ROOT/web-client/scripts" router-retry mjs; retry_path="$NEW_FIXTURE_PATH"
    new_fixture "$PROJECT_ROOT/web-client/scripts" plural-near-miss mjs; near_miss_path="$NEW_FIXTURE_PATH"
    pause_fixture_test_if_requested post-lock

    require_legitimate_fixture_pass "$plural_path" "One optional Precondition gates this Step." "$output"
    require_legitimate_fixture_pass "$condition_path" "type MaxPredicatesPerStep = ConstU32<8>;" "$output"
    require_legitimate_fixture_pass "$duplicate_path" "enum ActorClassificationError { Corrupt }" "$output"
    require_legitimate_fixture_pass "$soft_path" "Signed admission requires the exact bounded preimage." "$output"
    require_legitimate_fixture_pass "$phase_path" "TrustedSet mode preserves existing obligations." "$output"
    require_legitimate_fixture_pass "$staking_path" "fn native_security_view() -> NativeSecurityView;" "$output"
    require_legitimate_fixture_pass "$retry_path" "classifyRouterRetry(failure.retryDisposition, error);" "$output"
    require_legitimate_fixture_pass "$near_miss_path" "One optional Precondition remains singular." "$output"

    require_mutation_failure "Plural Precondition" "$plural_path" \
        "Optional Preconditions gate this Step." \
        "Plural Preconditions terminology reintroduced" "$output"
    require_mutation_failure "Retired Condition bound" "$condition_path" \
        "type MaxConditionsPerStep = ConstU32<8>;" \
        "Retired Actors Condition vocabulary or bound reintroduced" "$output"
    require_mutation_failure "Duplicate error vocabulary" "$duplicate_path" \
        "enum ProtocolFailureClass { Corrupt }" \
        "Duplicated or reconstructed typed error vocabulary reintroduced" "$output"
    require_mutation_failure "Soft signed preimage" "$soft_path" \
        "Signed proposal admission MAY remain hash-only." \
        "Soft signed-preimage claim contradicts canonical hard admission" "$output"
    require_mutation_failure "Numbered Phase" "$phase_path" \
        "Phase 2 enables permissionless operation." \
        "Stale numbered Phase terminology reintroduced" "$output"
    require_mutation_failure "Dead staking view" "$staking_path" \
        "type StakeExposure = Balance;" \
        "Dead or duplicated staking exposure/view owner reintroduced" "$output"
    require_mutation_failure "Router retry reconstruction" "$retry_path" \
        "function retryFromRouterError(error) { return error; }" \
        "Router retry policy reconstructed outside typed RetryDisposition ownership" "$output"
    require_mutation_failure "Plural Precondition near miss" "$near_miss_path" \
        "const entrySummary = 'Unrelated Preconditions prose';" \
        "Plural Preconditions terminology reintroduced" "$output"

    remove_fixture "$output"
    cleanup_fixtures
    flock -u "$FIXTURE_LOCK_FD"
    trap - EXIT INT TERM HUP
    log_success "Seven repository-behavior mutation families and the plural-exception near miss failed closed; exact detector lines, legitimate fixtures, and cleanup passed"
}

reject_pattern() {
    local pattern="$1"
    local message="$2"
    shift 2
    if rg -n -P "$pattern" "$@"; then
        log_error "$message"
        exit 1
    fi
}

reject_pattern_except() {
    local pattern="$1"
    local exception="$2"
    local message="$3"
    shift 3
    local matches
    local unexpected
    matches="$(rg -n -P "$pattern" "$@" || true)"
    [[ -z "$matches" ]] && return 0
    unexpected="$(printf '%s\n' "$matches" | rg -v -P "$exception" || true)"
    if [[ -n "$unexpected" ]]; then
        printf '%s\n' "$unexpected"
        log_error "$message"
        exit 1
    fi
}

require_anchor() {
    local pattern="$1"
    local path="$2"
    local message="$3"
    rg -q -P "$pattern" "$path" || {
        log_error "$message"
        exit 1
    }
}

main() {
    parse_args "$@"

    phase_banner "Step 1: Prerequisites"
    require_commands rg awk
    if [[ "$SELF_TEST" == "1" ]]; then
        require_commands flock
        [[ "$FIXTURE_CHILD" != "1" ]] || { log_error "Fixture child cannot recurse into --self-test"; exit 1; }
        run_normative_drift_fixtures
        return
    fi

    phase_banner "Step 2: Retired semantic owners"
    "$SCRIPT_DIR/audit-actors-identity.sh"

    local governance_src="$TEMPLATE_DIR/pallets/governance/src"
    local staking_src="$TEMPLATE_DIR/pallets/staking/src"
    local staking_lib="$staking_src/lib.rs"
    local staking_modules=(
        "$staking_src/pool.rs"
        "$staking_src/custody.rs"
        "$staking_src/security.rs"
        "$staking_src/views.rs"
        "$staking_src/invariants.rs"
    )
    local runtime_configs="$TEMPLATE_DIR/runtime/src/configs"
    reject_pattern '\b(?:AdaptiveApproval|AdaptiveTurnout|AdaptiveThreshold|ApprovalDecay|ThresholdDecay|VotingProgressThreshold)\b' \
        "Adaptive governance threshold owner reintroduced" "$governance_src"
    require_anchor 'T::ProposalMinimumTurnout::get\(\)' "$governance_src/proposal_resolution.rs" \
        "Governance resolution no longer consumes fixed minimum turnout"
    require_anchor 'T::ProposalApprovalThreshold::get\(\)' "$governance_src/proposal_resolution.rs" \
        "Governance resolution no longer consumes fixed approval threshold"
    reject_pattern '\b(?:pallet_staking|Staking::|RewardScheduler|schedule_reward|enqueue_reward|RewardPot)\b' \
        "Governance crossed the read-only participation boundary into staking or reward scheduling" \
        "$governance_src"
    require_anchor 'participation_coefficient_rotates_a_read_only_copy' "$governance_src/tests.rs" \
        "Governance read-only participation projection evidence is missing"
    require_anchor 'type StrategicProposalReserve: Get<u32>' "$governance_src/lib.rs" \
        "Governance strategic capacity reserve owner is missing"
    require_anchor '\.checked_sub\(T::StrategicProposalReserve::get\(\)\)' \
        "$governance_src/epoch_service.rs" "General governance capacity no longer withholds the strategic reserve with checked arithmetic"
    require_anchor 'general_proposal_cap_preserves_the_strategic_reserve' "$governance_src/tests.rs" \
        "Governance strategic-reserve regression evidence is missing"
    require_anchor 'signed_preimage_required: authority != ProposalSubmissionAuthority::AdminOnly' \
        "$governance_src/epoch_service.rs" "Governance signed-preimage policy owner is missing"
    require_anchor 'Self::ensure_payload_admission_witness' \
        "$governance_src/epoch_service.rs" "Signed proposal admission no longer requires the compact payload witness"
    require_anchor 'PayloadAdmissionWitnesses::<T>::get' \
        "$governance_src/epoch_service.rs" "Signed proposal admission no longer reads the canonical compact payload witness"
    reject_pattern 'validate_for_witness' \
        "Signed proposal admission regressed to loading and validating full preimage bytes" \
        "$governance_src/epoch_service.rs"
    require_anchor 'ProposalPayloadPreimageProvider::validate_for_witness' \
        "$governance_src/lib.rs" "Bounded witness preparation no longer validates the canonical preimage"
    require_anchor 'Hashing::hash\(bytes\)' \
        "$runtime_configs/governance_config.rs" "Runtime witness validation no longer binds supplied bytes to the noted preimage hash"
    reject_pattern 'get_preimage\(hash\)' \
        "Witness preparation regressed to loading the generic preimage value from storage" \
        "$runtime_configs/governance_config.rs"
    reject_pattern_except 'DispatchClass::' \
        'pallets/actors/src/lib\.rs:[0-9]+:[[:space:]]+DispatchClass::Mandatory,$' \
        "Custom public call changed dispatch class outside the runtime dispatchability matrix" \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/asset-registry/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/governance/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/oracle/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/router/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/staking/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/tmc/src/lib.rs"
    reject_pattern 'Pays::' \
        "Custom public call changed payment semantics outside the Actors-funded Manual occurrence boundary" \
        "$TEMPLATE_DIR/pallets/asset-registry/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/governance/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/oracle/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/router/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/staking/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/tmc/src/lib.rs"
    local actor_pays_count
    actor_pays_count="$(rg -N -o 'Pays::' "$TEMPLATE_DIR/pallets/actors/src/lib.rs" | wc -l | tr -d '[:space:]')"
    [[ "$actor_pays_count" == "5" ]] || {
        log_error "Actors public-call payment surface diverged from the audited Manual and mandatory-prepass boundaries"
        exit 1
    }
    require_anchor 'Call::actor_prepass' \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" "Mandatory Actor Prepass inherent lost its canonical call owner"
    require_anchor 'actor_type == ActorType::User && trigger_processed' \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" "Manual User occurrence no longer owns the sole Actors Pays::No success path"
    require_anchor 'actor_funded_success' \
        "$TEMPLATE_DIR/runtime/src/tests/dispatchability_matrix_tests.rs" "Actor-funded success payment is missing from the dispatchability matrix"
    require_anchor 'every_custom_runtime_call_family_fits_its_dispatch_envelope_at_maximum_input' \
        "$TEMPLATE_DIR/runtime/src/tests/dispatchability_matrix_tests.rs" "Custom-call dispatchability matrix is missing"
    local custom_call_count expected_custom_call_count
    custom_call_count="$(rg -N -o '#\[pallet::call_index\(' \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/asset-registry/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/governance/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/oracle/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/router/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/staking/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/tmc/src/lib.rs" | wc -l | tr -d '[:space:]')"
    custom_call_count="$((custom_call_count - 1))" # Payload-free Mandatory inherent is not a signed dispatchability-matrix family.
    expected_custom_call_count="$(awk \
        '$1 == "const" && $2 == "EXPECTED_CUSTOM_CALL_FAMILIES:" { gsub(/;/, "", $5); print $5 }' \
        "$TEMPLATE_DIR/runtime/src/tests/dispatchability_matrix_tests.rs")"
    [[ -n "$expected_custom_call_count" && "$custom_call_count" == "$expected_custom_call_count" ]] || {
        log_error "Custom-call source inventory ($custom_call_count) and dispatchability matrix ($expected_custom_call_count) diverged"
        exit 1
    }
    require_anchor 'signed_preimage_failures_precede_capacity_fee_events_and_state' \
        "$governance_src/tests.rs" "Hard signed-preimage rejection evidence is missing"
    reject_pattern '\b(?:proposal_submission_authority|proposal_opening_fee)\b' \
        "Redundant narrow governance admission view reintroduced" \
        "$governance_src" "$TEMPLATE_DIR/pallets/governance/docs" \
        "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/scripts"
    require_anchor 'proposal_admission_policy_view' "$governance_src/lib.rs" \
        "Canonical governance admission policy view is missing"
    reject_pattern "$soft_signed_preimage_pattern" \
        "Soft signed-preimage claim contradicts canonical hard admission" \
        "${required_family_surfaces[@]}"
    reject_pattern 'Current Divergence from (?:the )?Target Specification|most important current gaps' \
        "Governance implementation diary reintroduced into current architecture" \
        "$TEMPLATE_DIR/pallets/governance/docs/architecture.en.md"
    reject_pattern '\b(?:DirectTransfer|UnsupportedTarget|UnsupportedPayloadKind)\b' \
        "Constructor-free governance treasury variant reintroduced" \
        "$TEMPLATE_DIR/pallets/governance" "$TEMPLATE_DIR/runtime" \
        "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/wiki"
    require_anchor 'ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer' \
        "$TEMPLATE_DIR/runtime/src/configs/governance_config.rs" \
        "Canonical runtime governance treasury settlement constructor is missing"
    require_anchor 'runtime_executor_reaches_domain_and_call_failures_from_valid_preimages' \
        "$TEMPLATE_DIR/runtime/src/tests/governance_integration_tests.rs" \
        "Governance runtime execution-failure reachability evidence is missing"
    require_anchor 'PendingEnactment \{' "$governance_src/lib.rs" \
        "Governance pending-enactment status is missing"
    require_anchor 'struct ProposalApproval<Epoch>' "$governance_src/lib.rs" \
        "Governance shared approval identity is missing"
    require_anchor 'approval: ProposalApproval<Epoch>' "$governance_src/lib.rs" \
        "Governance pending enactment does not consume the shared approval identity"
    require_anchor 'proposal_status_reports_pending_enactment_until_delay_expires' \
        "$governance_src/tests.rs" \
        "Governance pending-enactment outcome-algebra evidence is missing"
    require_anchor 'pending_enactment_projection_fails_closed_on_non_approval_outcome' \
        "$governance_src/tests.rs" \
        "Governance pending-enactment corruption evidence is missing"
    require_anchor 'struct ProposalIdentity<DomainId, ItemId>' "$governance_src/lib.rs" \
        "Governance canonical proposal identity is missing"
    require_anchor 'struct FinalizedProposalRecord<AccountId, DomainId, Hash, Epoch>' \
        "$governance_src/lib.rs" "Governance canonical finalized record is missing"
    require_anchor 'pub finalization: FinalizedProposalRecord' "$governance_src/lib.rs" \
        "Governance recent history no longer projects the canonical finalized record"
    require_anchor 'outcome_algebra_reachability_and_projection_are_exhaustive' \
        "$governance_src/tests.rs" "Governance outcome-algebra reachability evidence is missing"
    require_anchor 'finalized_update_fails_closed_without_the_canonical_record' \
        "$governance_src/tests.rs" "Governance missing-finalization corruption evidence is absent"
    reject_pattern '\b(?:FinalizedProposalOutcomes|ProposalExecutionDetails)\b' \
        "Parallel governance finalization storage owner reintroduced" "$governance_src"
    reject_pattern 'FinalizedProposalOutcome::(?:Resolved|Enacted|ExecutionFailed|AdvisoryFinalized)' \
        "Duplicated governance approval/finalization fields reintroduced" "$governance_src"
    reject_pattern 'ProposalExecutionDetail::(?:Executed|ExecutionFailed|AdvisoryFinalized)' \
        "Governance execution detail reintroduced lifecycle identity or epoch ownership" \
        "$governance_src"

    reject_pattern '\b(?:BlockNumberRewardEpoch|RewardEpochProvider|RewardPeriod|RewardRolloverCursor|RewardEventIngress|BalanceDeltaReward|InferredRewardFunding)\b' \
        "Retired block-cadenced or inferred staking reward owner reintroduced" "$staking_src"
    for module in "${staking_modules[@]}"; do
        [[ -f "$module" ]] || { log_error "Staking implementation module is missing: $module"; exit 1; }
    done
    require_anchor '^mod pool;$' "$staking_lib" "Staking pool implementation owner is not wired through the facade"
    require_anchor '^mod custody;$' "$staking_lib" "Staking custody implementation owner is not wired through the facade"
    require_anchor '^mod security;$' "$staking_lib" "Staking security/reward implementation owner is not wired through the facade"
    require_anchor '^mod views;$' "$staking_lib" "Staking bounded-view implementation owner is not wired through the facade"
    require_anchor '^mod invariants;$' "$staking_lib" "Staking invariant implementation owner is not wired through the facade"
    require_anchor 'fn credit_stake_from\(' "$staking_src/pool.rs" "Staking pool module lacks the canonical share-vault mutation owner"
    require_anchor 'fn ensure_native_governance_unlocked\(' "$staking_src/custody.rs" "Staking custody module lacks governance-lock admission"
    require_anchor 'fn native_security_retention_state\(' "$staking_src/security.rs" "Staking security module lacks bounded epoch retention ownership"
    require_anchor 'fn record_native_security_reward_funding\(' "$staking_src/security.rs" "Staking security module lacks certified reward settlement ownership"
    require_anchor 'fn build_native_security_view\(' "$staking_src/views.rs" "Staking views module lacks canonical security projection construction"
    require_anchor 'fn ensure_native_security_reward_custody\(' "$staking_src/invariants.rs" "Staking invariants module lacks reward custody/liability reconciliation"
    reject_pattern '^\s*pub\s+(?:struct|enum|type)\b' \
        "Staking implementation module introduced a duplicate public SCALE model" "${staking_modules[@]}"
    reject_pattern '#\[pallet::(?:storage|call|event|error|view_functions)\]' \
        "Staking implementation module split FRAME macro ownership away from the facade" "${staking_modules[@]}"
    require_anchor '#\[pallet::storage\]' "$staking_lib" "Staking facade no longer owns FRAME storage declarations"
    require_anchor '#\[pallet::call\]' "$staking_lib" "Staking facade no longer owns dispatchable declarations"
    reject_pattern '\b(?:NativeSecurityEnabled|EnableNativeSecurity|LpBackedSelectionEnabled|EnableLpBackedSelection|NativeSecurityPhase|SecurityPhaseProvider)\b' \
        "Independent native-security phase flag reintroduced" "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/scripts"
    reject_pattern 'NativeSecurityReadiness::SnapshotOpenFailed' \
        "Attempted transition result reintroduced as native-security readiness" "$TEMPLATE_DIR"
    reject_pattern "$dead_staking_view_pattern" \
        "Dead or duplicated staking exposure/view owner reintroduced" \
        "${required_family_surfaces[@]}"
    require_anchor 'OperatorNativeLpLocked\[operator\].*O\(1\).*session ranking' \
        "$TEMPLATE_DIR/pallets/staking/docs/architecture.en.md" \
        "Operator LP aggregate lacks its O(1) session consumer provenance"
    require_anchor 'AccountNativeLpLocked\[account\].*O\(1\).*NativeVotePower' \
        "$TEMPLATE_DIR/pallets/staking/docs/architecture.en.md" \
        "Account LP aggregate lacks its O(1) governance consumer provenance"
    require_anchor 'TotalNativeLpLocked.*O\(1\).*turnout/supply' \
        "$TEMPLATE_DIR/pallets/staking/docs/architecture.en.md" \
        "Global LP aggregate lacks its O(1) governance consumer provenance"
    reject_pattern '\b(?:stake_native|NativeStakeRequiresDedicatedCall|StakeNative)\b' \
        "Dedicated native staking call or compatibility vocabulary reintroduced" \
        "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/scripts" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/wiki"
    reject_pattern "$stale_phase_pattern" \
        "Stale numbered Phase terminology reintroduced into active protocol truth" \
        "${required_family_surfaces[@]}"
    reject_pattern 'view\.Staking\.(?:native_security_mode|native_security_readiness|current_security_epoch)' \
        "Split native-security view reconstruction reintroduced" \
        "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/scripts"
    require_anchor 'pub enum NativeSecurityBoundaryOutcome' "$staking_src/lib.rs" \
        "Native-security boundary outcomes are missing their separate bounded owner"
    require_anchor 'pub fn native_security_view\(\) -> Result<NativeSecurityView, NativeSecurityViewError>' \
        "$staking_src/lib.rs" "Canonical bounded native-security view is missing"
    require_anchor 'session_retention_runs_four_claim_horizons_without_external_cleanup' \
        "$staking_src/tests.rs" "Staking four-horizon automatic retention evidence is missing"
    require_anchor 'expiry_atomically_settles_in_trusted_mode_and_removes_state' \
        "$staking_src/tests.rs" "Staking atomic expiry/Fee Sink settlement evidence is missing"
    require_anchor 'try_state_reconciles_native_security_reward_liability_and_custody' \
        "$staking_src/tests.rs" "Staking liability/custody conservation evidence is missing"
    require_anchor 'trusted_security_mode_session_boundary_settles_retained_reward_obligations' \
        "$TEMPLATE_DIR/runtime/src/tests/staking_integration_tests.rs" \
        "Runtime restart-safe session settlement evidence is missing"

    phase_banner "Step 3: Typed failure and closed public surfaces"
    local router_classifier
    router_classifier="$(awk '/pub\(crate\) fn classify_router_failure/{capture=1} capture{print} /pub struct TmctolLiquidityOps/{exit}' "$runtime_configs/actor_config.rs")"
    [[ "$router_classifier" == *"retry_disposition()"* ]] || {
        log_error "Actors Router boundary no longer consumes typed retry disposition"
        exit 1
    }
    if printf '%s\n' "$router_classifier" | rg -n 'match\s+error|if\s+error|error\s*==|matches!\(\s*error'; then
        log_error "Actors Router boundary infers retry from the raw downstream error"
        exit 1
    fi
    require_anchor 'error: pallet_deos_router::ExecutionError<Runtime>' \
        "$runtime_configs/actor_config.rs" "Actors Router boundary no longer receives typed execution failure"
    reject_pattern '^pallet-deos-router\s*=' \
        "Actors package directly depends on Router instead of the runtime composition root" \
        "$TEMPLATE_DIR/pallets/actors/Cargo.toml"
    reject_pattern '^pallet-deos-actors\s*=' \
        "Router package directly depends on Actors instead of the runtime composition root" \
        "$TEMPLATE_DIR/pallets/router/Cargo.toml"
    reject_pattern "$duplicate_error_pattern" \
        "Duplicated or reconstructed typed error vocabulary reintroduced outside its owner" \
        "${required_family_surfaces[@]}"
    require_anchor 'No adapter infers retry from error text, pallet identity, or a raw `DispatchError`' \
        "$PROJECT_ROOT/docs/core.architecture.en.md" \
        "Local failure ownership is missing from the Runtime Composition DAG"
    require_anchor 'adapter_failure_keeps_boundary_and_retry_independent' \
        "$TEMPLATE_DIR/pallets/router/src/tests.rs" \
        "Router independent cause/retry cross-product evidence is missing"
    require_anchor 'AdapterFailure::unknown' "$TEMPLATE_DIR/pallets/router/src/tests.rs" \
        "Router unknown-adapter permanent fallback evidence is missing"
    require_anchor 'router_failure_classifier_is_exhaustive_and_typed' \
        "$TEMPLATE_DIR/runtime/src/tests/actors_integration_tests.rs" \
        "Runtime typed Router-to-Actors classifier evidence is missing"
    require_anchor 'preimage_admission_error_core_maps_exhaustively_to_dispatch' \
        "$TEMPLATE_DIR/pallets/governance/src/tests.rs" \
        "Governance preimage-core error mapping evidence is missing"
    require_anchor 'market_execution_classifier_uses_the_concrete_cause' \
        "$TEMPLATE_DIR/runtime/src/tests/actors_integration_tests.rs" \
        "Runtime market-cause classification evidence is missing"
    local oracle_src="$TEMPLATE_DIR/pallets/oracle/src"
    reject_pattern '\b(?:Subscriber|Subscription|ObservationHistory|RetryQueue|Fanout|Strategy)\b' \
        "Oracle absorbed history, subscription, retry, fanout, or strategy ownership" "$oracle_src"
    require_anchor 'changed_hook_is_transactional_and_equal_refresh_is_hook_free' \
        "$oracle_src/tests.rs" "Oracle equal-refresh and transactional-hook evidence is missing"
    require_anchor 'revision_overflow_fails_without_refresh_or_hook' "$oracle_src/tests.rs" \
        "Oracle revision failure rollback evidence is missing"
    require_anchor 'lifecycle_controls_publication_without_reinterpreting_state' \
        "$oracle_src/tests.rs" "Oracle lifecycle-state reachability evidence is missing"
    require_anchor 'ema_uses_elapsed_weighting_and_direct_initialization' \
        "$oracle_src/tests.rs" "Oracle aggregation-variant reachability evidence is missing"
    require_anchor 'zero_can_be_an_initialized_value_when_the_feed_allows_it' \
        "$oracle_src/tests.rs" "Oracle zero-policy reachability evidence is missing"
    require_anchor 'scale_and_storage_contract_are_explicit' "$oracle_src/tests.rs" \
        "Oracle public SCALE/storage variant contract evidence is missing"
    require_anchor 'independent_runtime_registers_and_publishes_typed_feed' \
        "$TEMPLATE_DIR/pallets/oracle/embedding-runtime/src/lib.rs" \
        "Oracle independent embedding reachability evidence is missing"
    require_anchor 'stores only current scalar truth' \
        "$TEMPLATE_DIR/pallets/oracle/docs/architecture.en.md" \
        "Oracle current-truth-only architecture boundary is missing"
    reject_pattern '\bretry_class\(\)' "Legacy Router retry vocabulary reintroduced" \
        "$TEMPLATE_DIR/pallets/router" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/web-client/src"
    reject_pattern "$router_retry_reconstruction_pattern" \
        "Router retry policy reconstructed outside typed RetryDisposition ownership" \
        "${required_family_surfaces[@]}"

    reject_pattern '^\s*(?:Legacy|Reserved|Deprecated|Unused)(?:\s*[({,])' \
        "Placeholder public variant reintroduced without an executable contract" \
        "$TEMPLATE_DIR/pallets/governance/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/staking/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/actors/src/types" \
        "$TEMPLATE_DIR/pallets/router/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/router/src/types.rs" \
        "$TEMPLATE_DIR/pallets/oracle/src/lib.rs"
    require_anchor 'public_reachability_inventory_is_closed_and_canonical' "$TEMPLATE_DIR/pallets/actors/src/tests" \
        "Actors closed SCALE-surface regression evidence is missing"
    local router_tests="$TEMPLATE_DIR/pallets/router/src/tests.rs"
    require_anchor 'adversarial_corpus_is_complete_unique_and_anchor_bound' "$router_tests" \
        "Router executable failure-surface inventory is missing"
    require_anchor 'exact_input_outcomes_cover_all_weight_classes' "$router_tests" \
        "Router exact-input route/weight reachability evidence is missing"
    require_anchor 'router_exact_output_quote_and_execution_enforce_total_input_cap' "$router_tests" \
        "Router direct exact-output reachability evidence is missing"
    require_anchor 'router_exact_output_selects_bounded_native_anchored_path_without_search' \
        "$router_tests" "Router Native-anchored exact-output reachability evidence is missing"
    require_anchor 'every_router_error_has_stable_failure_and_retry_classes' "$router_tests" \
        "Router exhaustive error reachability/classification evidence is missing"
    require_anchor 'execution_error_exposes_only_router_core_or_adapter_failure' "$router_tests" \
        "Router narrow execution-error boundary evidence is missing"
    reject_pattern '\bNoMultiHopRoute\b' \
        "Constructor-free Router no-route duplicate reintroduced" \
        "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client" "$PROJECT_ROOT/docs"
    require_anchor '`PreparedRoute` remains a public Rust package type solely' \
        "$TEMPLATE_DIR/pallets/router/docs/architecture.en.md" \
        "Router prepared-versus-public representation boundary is missing"
    local actors_tests="$TEMPLATE_DIR/pallets/actors/src/tests"
    local actors_test_root="$TEMPLATE_DIR/pallets/actors/src/tests.rs"
    local actors_types="$TEMPLATE_DIR/pallets/actors/src/types"
    local actors_contract_types="$actors_types/contract.rs"
    local actors_lifecycle_types="$actors_types/lifecycle.rs"
    require_anchor '^pub use contract::\*;$' "$TEMPLATE_DIR/pallets/actors/src/types.rs" \
        "Actors canonical type facade no longer exports the Contract owner"
    require_anchor '^pub use lifecycle::\*;$' "$TEMPLATE_DIR/pallets/actors/src/types.rs" \
        "Actors canonical type facade no longer exports the Lifecycle owner"
    require_anchor '^pub use observation::\*;$' "$TEMPLATE_DIR/pallets/actors/src/types.rs" \
        "Actors canonical type facade no longer exports the Observation owner"
    require_anchor '^pub use scheduler::\*;$' "$TEMPLATE_DIR/pallets/actors/src/types.rs" \
        "Actors canonical type facade no longer exports the Scheduler owner"
    reject_pattern '^pub (?:struct|enum|type) ' \
        "Actors type facade reintroduced a duplicate semantic owner" \
        "$TEMPLATE_DIR/pallets/actors/src/types.rs"
    reject_pattern 'replace_segment' \
        "Actors type metadata paths must follow natural module ownership" \
        "$actors_types"
    require_anchor 'pub enum Task' "$actors_contract_types" \
        "Actors Contract type owner is missing the canonical Task surface"
    require_anchor 'pub struct ActorIdentity' "$actors_lifecycle_types" \
        "Actors Lifecycle type owner is missing canonical identity state"
    require_anchor 'pub struct DirtyObservationState' "$actors_types/observation.rs" \
        "Actors Observation type owner is missing canonical dirty state"
    require_anchor 'pub struct WakeupBucketState' "$actors_types/scheduler.rs" \
        "Actors Scheduler type owner is missing canonical wakeup state"
    require_anchor 'task_failure_defaults_unknown_errors_to_permanent' "$actors_tests" \
        "Unknown Actor adapter failures no longer have fail-closed evidence"
    require_anchor 'retry_later_is_mutable_only_at_creation_and_update' "$actors_tests" \
        "Actors Mutable-only retry admission evidence is missing"
    require_anchor 'retry_later_aborts_permanent_failure_without_executing_suffix' "$actors_tests" \
        "Actors Permanent-failure no-retry evidence is missing"
    require_anchor 'invalid_fresh_observation_fails_permanently_and_applies_step_policy' \
        "$actors_tests" "Actors invalid-observation Permanent-failure evidence is missing"
    require_anchor 'completed_failed_and_suspended_attempts_update_failure_streak_once' \
        "$actors_tests" "Actors cross-attempt failure-streak transition evidence is missing"
    require_anchor 'fn transition_failure_streak\(' \
        "$TEMPLATE_DIR/pallets/actors/src/execution.rs" \
        "Actors canonical failure-streak transition owner is missing"
    require_anchor '`transition_failure_streak` is the sole mutation formula' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors failure-streak transition ownership is missing from the implementation map"
    require_anchor 'retry_later_resets_local_attempt_count_after_cursor_advancement' \
        "$actors_tests" "Actors cursor-local unsuccessful-attempt reset evidence is missing"
    require_anchor 'retry_target_uses_only_cursor_local_count_and_last_attempt_block' \
        "$actors_tests" "Actors retry timing no longer proves its minimal stored inputs"
    reject_pattern '(?:pub\s+attempt:\s*u32|ActorRunState\.attempt|\brun_state\.attempt|\battempt:\s*u32)' \
        "Removed Actors cycle-global attempt ordinal reintroduced" \
        "$TEMPLATE_DIR/pallets/actors/src" "$TEMPLATE_DIR/pallets/actors/docs" \
        "$PROJECT_ROOT/docs/actors-control-plane.contract.en.md" \
        "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/web-client/docs"
    require_anchor 'run_attempts_have_unique_chain_coordinates_without_the_stored_ordinal' \
        "$actors_tests" "Actors attempt-identity contraction proof is missing"
    require_anchor 'pub enum ActorClassificationError' \
        "$actors_lifecycle_types" \
        "Actors canonical classification error core is missing"
    require_anchor 'classification_dispatch_error\(error: ActorClassificationError\)' \
        "$TEMPLATE_DIR/pallets/actors/src/scheduler.rs" \
        "Actors classification-to-dispatch boundary is missing"
    reject_pattern '\bActorEligibilityError\b' \
        "Duplicated Actors eligibility error wrapper reintroduced" \
        "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client" "$PROJECT_ROOT/docs"
    require_anchor 'Classification\(ActorClassificationError\)' \
        "$actors_lifecycle_types" \
        "Actors simulation classification wrapper is missing"
    require_anchor 'eligibility_projection_rejects_partial_active_partitions' "$actors_tests" \
        "Actors shared eligibility/simulation classification evidence is missing"
    reject_pattern 'SimulationError::(?:ActorInvariant|RunInvariant|ComputationOverflow)' \
        "Duplicated classification mapping reintroduced into simulation" \
        "$TEMPLATE_DIR/pallets/actors/src" "$TEMPLATE_DIR/pallets/actors/docs"
    require_anchor 'public_api_error_signatures_use_shared_typed_cores' "$actors_tests" \
        "Actors typed runtime-API signature evidence is missing"
    require_anchor 'public_reachability_inventory_is_closed_and_canonical' "$actors_tests" \
        "Actors fail-closed public reachability inventory is missing"
    require_anchor '### Public Inventory Evidence' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors implementation map no longer owns the reviewed public inventories"
    require_anchor 'metadata/spec drift checks freeze Event and pallet Error inventories' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors Event/Error inventory proof routing is missing"
    reject_pattern '\bRuntimeUpgrade\b' \
        "Constructor-free Actors runtime-upgrade cancellation placeholder reintroduced" \
        "$actors_types" \
        "$TEMPLATE_DIR/pallets/actors/docs/specification.en.md" \
        "$PROJECT_ROOT/web-client/src/lib/automation/actors-abi-manifest.json"
    reject_pattern 'ContextDependency::None' \
        "Constructor-free Actors semantic dependency placeholder reintroduced" \
        "$TEMPLATE_DIR/pallets/actors/src/contract.rs" \
        "$TEMPLATE_DIR/pallets/actors/examples/semantic_manifest.rs"
    require_anchor 'every_amount_resolution_has_a_live_task_policy_constructor' \
        "$TEMPLATE_DIR/pallets/actors/src/contract.rs" \
        "Actors amount-dependency constructor evidence is missing"
    require_anchor 'pub enum StepOutcome' "$actors_lifecycle_types" \
        "Actors canonical shared Step outcome is missing"
    require_anchor 'Failed\(crate::TaskFailure\)' "$actors_lifecycle_types" \
        "Actors Step failure no longer preserves concrete cause and retry disposition"
    require_anchor 'pub enum AttemptDisposition' "$actors_lifecycle_types" \
        "Actors canonical attempt disposition is missing"
    require_anchor 'fn resolve_step_control\(outcome: &StepOutcome' \
        "$TEMPLATE_DIR/pallets/actors/src/execution.rs" \
        "Actors Step policy interpreter no longer consumes the canonical outcome"
    require_anchor 'status: attempt.disposition' "$TEMPLATE_DIR/pallets/actors/src/execution.rs" \
        "Actors simulation no longer returns production finalization disposition"
    require_anchor 'cumulative_outcomes: attempt.outcomes' \
        "$TEMPLATE_DIR/pallets/actors/src/execution.rs" \
        "Actors simulation reconstructed counters outside production finalization"
    require_anchor 'run_simulation_preserves_retry_position_and_committed_state' \
        "$actors_tests" "Actors shared failure-cause simulation evidence is missing"
    reject_pattern '\b(?:SimulationStepOutcome|SimulationStatus)\b' \
        "Retired simulation-only Actors outcome vocabulary reintroduced" \
        "$TEMPLATE_DIR/pallets/actors" "$TEMPLATE_DIR/runtime" "$PROJECT_ROOT/docs" \
        "$PROJECT_ROOT/wiki" "$PROJECT_ROOT/web-client"
    require_anchor 'BoundedVec<Result<bool, PredicateError>, MaxOpeningPredicateResults>' \
        "$actors_lifecycle_types" \
        "Actors exact Opening predicate result storage is missing"
    reject_pattern '\bPredicateEvaluation\b' \
        "Tri-state Actors predicate evaluation reintroduced" \
        "$TEMPLATE_DIR/pallets/actors" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/web-client/src"
    require_anchor 'opening_and_current_predicates_observe_distinct_step_state' "$actors_tests" \
        "Actors explicit Opening/Current timing evidence is missing"
    require_anchor 'opening_predicate_result_is_reused_by_run_state' "$actors_tests" \
        "Actors frozen Opening truth evidence is missing"
    require_anchor 'unavailable_observation_skips_without_incrementing_failures' "$actors_tests" \
        "Actors false-precondition skip evidence is missing"
    require_anchor 'bounded_dnf_is_canonical_and_mode_distinct' "$actors_tests" \
        "Actors canonical bounded-DNF evidence is missing"
    require_anchor 'empty_outer_and_inner_precondition_forms_are_rejected' "$actors_tests" \
        "Actors invalid empty-form evidence is missing"
    require_anchor 'predicate_evaluator_visits_every_atom_and_preserves_first_error' "$actors_tests" \
        "Actors full predicate visitation evidence is missing"
    require_anchor 'admission_canonicalizes_dnf_and_equivalent_update_is_exact_noop' "$actors_tests" \
        "Actors exact DNF normalization evidence is missing"
    require_anchor 'admission_absorbs_exact_dnf_superset_clause' "$actors_tests" \
        "Actors exact DNF absorption evidence is missing"
    require_anchor 'type MaxPredicatesPerStep' "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "Actors canonical predicate bound is missing"
    require_anchor 'pub type ContractSteps<T>' "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "Actors canonical ContractSteps type is missing"
    require_anchor 'type MaxContractSteps: Get<u32>' "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "Actors canonical ContractSteps bound is missing"
    reject_pattern 'ExecutionPlanOf|MaxExecutionPlanSteps|execution_plan|executionPlan' \
        "Retired Actors execution-plan field vocabulary reintroduced" \
        "$TEMPLATE_DIR/pallets/actors/src" "$TEMPLATE_DIR/pallets/actors/embedding-runtime" \
        "$TEMPLATE_DIR/pallets/actors/examples" "$TEMPLATE_DIR/pallets/actors/docs" \
        "$TEMPLATE_DIR/pallets/actors/README.md" "$TEMPLATE_DIR/runtime" "$PROJECT_ROOT/docs" \
        "$PROJECT_ROOT/wiki" "$PROJECT_ROOT/web-client/src"
    require_anchor 'pub precondition_skips: u32' "$actors_lifecycle_types" \
        "Actors canonical precondition-skip counter is missing"
    require_anchor 'PreconditionFalse' "$actors_lifecycle_types" \
        "Actors canonical false-precondition reason is missing"
    reject_pattern "$legacy_condition_pattern" \
        "Retired Actors Condition vocabulary or bound reintroduced" \
        "${required_family_surfaces[@]}"
    require_anchor 'pub unsuccessful_attempt_streak: u32' "$actors_lifecycle_types" \
        "Actors exact unsuccessful-attempt streak field is missing"
    reject_pattern '\bconsecutive_failures\b' \
        "Retired ambiguous Actors failure-streak field reintroduced" \
        "$TEMPLATE_DIR" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/wiki" "$PROJECT_ROOT/web-client"
    require_anchor 'fn predicate_evaluation_weight' "$TEMPLATE_DIR/pallets/actors/src/execution.rs" \
        "Actors benchmark-domain predicate Weight chunking is missing"
    require_anchor 'generated_predicate_weight_scales_and_chunks_opening_plus_step_visits' "$actors_tests" \
        "Actors dual-phase predicate Weight evidence is missing"
    reject_pattern 'predicate_set_evaluation\(step\.preconditions\.evaluation_units\(\)\)' \
        "Actors dual-phase predicate Weight bypasses benchmark-domain chunking" \
        "$TEMPLATE_DIR/pallets/actors/src"
    reject_pattern 'BalanceExhausted|FeeBudgetExhausted' \
        "Actors active runtime/client surfaces revived retired mid-pipeline economic close reasons" \
        "$TEMPLATE_DIR/pallets/actors/src" \
        "$TEMPLATE_DIR/runtime/src" \
        "$PROJECT_ROOT/web-client/src/lib/automation/eligibility.ts"
    require_anchor 'underfunded_manual_occurrence_creates_no_fee_readiness_or_apoptosis' "$actors_tests" \
        "Actors underfunded Manual occurrence preservation evidence is missing"
    require_anchor 'manual_trigger_collection_failure_rolls_back_readiness_and_fee_movement' "$actors_tests" \
        "Actors Manual occurrence collection rollback evidence is missing"
    require_anchor 'address_event_charges_occurrence_before_pipeline_opening' "$actors_tests" \
        "Actors AddressEvent occurrence charging evidence is missing"
    require_anchor 'underfunded_address_event_advances_without_fee_readiness_or_apoptosis' "$actors_tests" \
        "Actors automatic AddressEvent underfunding evidence is missing"
    require_anchor 'address_event_collection_failure_preserves_source_progress_without_readiness' "$actors_tests" \
        "Actors automatic AddressEvent collection-failure progression evidence is missing"
    require_anchor 'address_event_trigger_occurrence' \
        "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" \
        "Actors generated AddressEvent Trigger Weight owner is missing"
    require_anchor 'observation_change_charges_occurrence_before_pipeline_opening' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/observations.rs" \
        "Actors ObservationChange occurrence charging evidence is missing"
    require_anchor 'underfunded_observation_change_advances_without_fee_readiness_or_apoptosis' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/observations.rs" \
        "Actors automatic ObservationChange underfunding evidence is missing"
    require_anchor 'observation_change_trigger_occurrence' \
        "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" \
        "Actors generated ObservationChange Trigger Weight owner is missing"
    require_anchor 'observation_crossing_fire_charges_before_readiness' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/crossing.rs" \
        "Actors ObservationCrossing fire occurrence charging evidence is missing"
    require_anchor 'underfunded_crossing_fire_advances_without_fee_readiness_or_apoptosis' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/crossing.rs" \
        "Actors automatic ObservationCrossing underfunding evidence is missing"
    require_anchor 'crossing_fire_collection_failure_advances_without_readiness' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/crossing.rs" \
        "Actors automatic ObservationCrossing collection-failure progression evidence is missing"
    require_anchor 'crossing_batch_falls_back_to_scalar_progress_for_an_underfunded_member' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/crossing.rs" \
        "Actors ObservationCrossing batch underfunding progress evidence is missing"
    require_anchor 'observation_crossing_trigger_occurrence' \
        "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" \
        "Actors generated ObservationCrossing Trigger Weight owner is missing"
    require_anchor 'at_time_occurrence_charges_once_consumes_deadline_and_latches_readiness' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors AtTime occurrence charging evidence is missing"
    require_anchor 'busy_at_time_occurrence_charges_and_preserves_independent_run_service' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors busy AtTime occurrence evidence is missing"
    require_anchor 'underfunded_at_time_occurrence_selects_prepaid_custody_neutral_apoptosis' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors automatic AtTime underfunded-apoptosis evidence is missing"
    require_anchor 'at_time_trigger_occurrence' \
        "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" \
        "Actors generated AtTime Trigger Weight owner is missing"
    require_anchor 'cadenced_latch_disables_detection_until_pipeline_opening' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors Cadenced useful-latch disable/re-arm evidence is missing"
    require_anchor 'busy_cadenced_occurrence_charges_and_preserves_independent_run_service' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors busy Cadenced occurrence evidence is missing"
    require_anchor 'underfunded_cadenced_occurrence_advances_without_fee_readiness_or_apoptosis' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/scheduling.rs" \
        "Actors automatic Cadenced underfunding evidence is missing"
    require_anchor 'pipeline_and_trigger_temporal_memberships_coexist_and_drain_independently' \
        "$TEMPLATE_DIR/pallets/actors/src/tests/wakeups.rs" \
        "Actors independent Pipeline/Trigger temporal topology evidence is missing"
    require_anchor 'cadenced_trigger_occurrence' \
        "$TEMPLATE_DIR/runtime/src/weights/pallet_deos_actors.rs" \
        "Actors generated Cadenced Trigger Weight owner is missing"
    reject_pattern 'Cadenced never reads or sets `pending_signal`|including while Running/Suspended or already pending|setting or coalescing readiness' \
        "Actors documentation revived paid latched activity or bypassed canonical cadence readiness" \
        "$TEMPLATE_DIR/pallets/actors/docs" "$PROJECT_ROOT/docs"
    reject_pattern '\bFresh (?:cohort|opportunity|opening|Active|activation|cycle)\b' \
        "Actors specification revived ambiguous Fresh scheduler vocabulary" \
        "$TEMPLATE_DIR/pallets/actors/docs/specification.en.md"
    require_anchor 'observation_fanout_blocked_page' \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "Actors ordinary fanout admission omits the measured blocked-fallback owner"
    require_anchor '`Attempt identity proof`' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors attempt-identity proof is missing from the implementation map"
    require_anchor 'checked_shl\(cursor_local_attempt\)' \
        "$TEMPLATE_DIR/pallets/actors/src/scheduler.rs" \
        "Actors retry backoff no longer uses checked capped exponentiation"
    require_anchor 'temporary_retry_backoff_is_one_two_four_eight_then_capped' \
        "$actors_tests" "Actors capped-exponential retry evidence is missing"
    require_anchor 'capped_exponential_balances_retry_pressure_and_recovery_at_maximum_occupancy' \
        "$actors_tests" "Actors maximum-occupancy backoff decision evidence is missing"
    require_anchor 'timer_jitter_removal_evidence_is_machine_readable_and_decisive' \
        "$actors_tests" "Actors timer-phase removal evidence is missing"
    require_anchor '"decision": "remove-timer-jitter"' \
        "$TEMPLATE_DIR/pallets/actors/tests/fixtures/timer-jitter-decision.v1.json" \
        "Actors timer-phase decision fixture is missing"
    require_anchor 'timer_wakeup_uses_exact_cadence_without_actor_phase' "$actors_tests" \
        "Actors exact-cadence wakeup evidence is missing"
    reject_pattern '\b(?:MaxTimerJitterBlocks|ActorMaxTimerJitterBlocks|cadence_phase_blocks|timer_jitter_blocks|phase_window|worst_case_phase|cadence_phase)\b' \
        "Retired Actors timer-phase surface reintroduced" \
        "$TEMPLATE_DIR/pallets/actors/src" "$TEMPLATE_DIR/pallets/actors/embedding-runtime" \
        "$TEMPLATE_DIR/runtime/src" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/docs"
    require_anchor '"decision": "retain-capped-exponential"' \
        "$TEMPLATE_DIR/pallets/actors/tests/fixtures/retry-backoff-decision.v1.json" \
        "Actors retry-backoff decision fixture is missing"
    require_anchor '`Backoff decision evidence`' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors backoff retention decision is missing from the implementation map"
    reject_pattern 'match\s+cursor_local_attempt' \
        "Actors retry lookup table reintroduced" "$TEMPLATE_DIR/pallets/actors/src/scheduler.rs"
    require_anchor 'canonical_fifo_uses_one_physical_ticket_sequence' \
        "$actors_tests" "Actors single global FIFO evidence is missing"
    require_anchor 'on_idle_fanout_feeds_the_existing_scheduler_without_direct_execution' \
        "$actors_tests" "Actors fanout-to-canonical-scheduler evidence is missing"
    require_anchor '`resolve_step_control` remains the only runtime transition owner' \
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md" \
        "Actors single step/lifecycle transition owner is missing"
    reject_pattern '\b(?:PriorityQueue|PriorityLane|ReadinessCache|RetryQueue|AlternateRetryOwner|SecondaryLifecycleClassifier)\b' \
        "Actors second scheduler or lifecycle owner reintroduced" "$TEMPLATE_DIR/pallets/actors/src"
    reject_pattern '\b(?:system_class_not_starved|preferred_queue|non_empty_class)\b' \
        "Retired scheduler-class vocabulary reintroduced" "$TEMPLATE_DIR/pallets/actors"
    require_anchor 'canonical_step_transition_matrix_has_production_simulation_parity' \
        "$PROJECT_ROOT/scripts/actors-assurance.sh" \
        "Actors assurance no longer executes exhaustive production/simulation Step parity"
    require_anchor 'fn canonical_step_transition_matrix_has_production_simulation_parity' \
        "$actors_tests" "Actors exhaustive production/simulation Step parity matrix is missing"
    require_anchor 'variant_count::<StepErrorPolicy>\(\)' "$actors_tests" \
        "Actors parity matrix is not fail-closed against StepErrorPolicy growth"
    require_anchor 'variant_count::<StepOutcome>\(\)' "$actors_tests" \
        "Actors parity matrix is not fail-closed against StepOutcome growth"
    require_anchor 'unsuccessful attempt increments the failure streak once' "$actors_tests" \
        "Actors parity matrix no longer checks production failure-streak effects"
    local specification_step_rows
    local matrix_step_rows
    specification_step_rows="$(
        awk '/Each step whose enclosing scheduler attempt commits selects exactly one row/,/A step increments at most one/' \
            "$TEMPLATE_DIR/pallets/actors/docs/specification.en.md" |
            grep -oE 'ST-[0-9]{2}' | sort -u
    )"
    matrix_step_rows="$(
        awk '/const STEP_TRANSITION_PARITY_MATRIX:/,/^];/' "$actors_test_root" |
            grep -oE 'ST-[0-9]{2}' | sort -u
    )"
    if [[ -z "$specification_step_rows" || "$specification_step_rows" != "$matrix_step_rows" ]]; then
        log_error "Actors canonical Step transition rows and typed parity matrix inventory diverged"
        diff -u <(printf '%s\n' "$specification_step_rows") <(printf '%s\n' "$matrix_step_rows") || true
        exit 1
    fi

    phase_banner "Step 4: Specification and implementation-map ownership"
    local core_architecture="$PROJECT_ROOT/docs/core.architecture.en.md"
    require_anchor 'Public Surface Closure Map' "$core_architecture" \
        "Cross-system public-surface closure map is missing"
    require_anchor 'Package architecture maps keep executable anchors beside their owning invariants' \
        "$core_architecture" "Core architecture no longer routes executable closure to package owners"
    reject_pattern '^```rust$' \
        "Decorative implementation syntax reintroduced into core composition architecture" \
        "$core_architecture"
    reject_pattern '\b(?:fn|pub fn)\s+[a-z][a-z0-9_]+\s*\(' \
        "Exhaustive implementation symbol reintroduced into core composition architecture" \
        "$core_architecture"
    local specification_docs=("$PROJECT_ROOT/docs/tmctol.specification.en.md")
    local architecture_docs=("$core_architecture")
    local subsystem
    for subsystem in governance staking actors router oracle; do
        specification_docs+=("$TEMPLATE_DIR/pallets/$subsystem/docs/specification.en.md")
        architecture_docs+=("$TEMPLATE_DIR/pallets/$subsystem/docs/architecture.en.md")
        [[ -f "$TEMPLATE_DIR/pallets/$subsystem/docs/specification.en.md" ]] || {
            log_error "$subsystem package specification is missing"
            exit 1
        }
        [[ -f "$TEMPLATE_DIR/pallets/$subsystem/docs/architecture.en.md" ]] || {
            log_error "$subsystem package architecture map is missing"
            exit 1
        }
        require_anchor '(?:Validation|Falsification|Evidence)' \
            "$TEMPLATE_DIR/pallets/$subsystem/docs/architecture.en.md" \
            "$subsystem implementation map has no executable evidence route"
        require_anchor "template/pallets/$subsystem/docs/specification\\.en\\.md" "$core_architecture" \
            "$subsystem is absent from the public-surface owner map"
    done
    reject_pattern '(?:0\.7\.|specification version|contract version|this version|target v1|\bv1\b|pre-launch|pre-release|release line)' \
        "Release or pre-release identity reintroduced into a standalone specification" \
        "${specification_docs[@]}"
    reject_pattern '(?:Specification Maintenance Meta-Layer|formatting-preserving count|equal-or-greater removal|mandatory blank-line|line-count limit|document-maintenance process|update procedure)' \
        "Documentation-maintenance process reintroduced into a normative specification" \
        "${specification_docs[@]}"
    reject_pattern '(?:\*\*Status\*\*:\s*Target Contract|Implementation status|shipped-runtime divergence)' \
        "Implementation-status narration reintroduced into a normative specification" \
        "${specification_docs[@]}"
    local integration_doc
    for integration_doc in "$PROJECT_ROOT"/docs/*.integration.en.md; do
        architecture_docs+=("$integration_doc")
        require_anchor 'owns only concrete DEOS composition' "$integration_doc" \
            "Integration document does not state its concrete-composition boundary"
        require_anchor 'template/pallets/.*/docs/specification\.en\.md' "$integration_doc" \
            "Integration document does not route reusable semantics to its package specification"
        require_anchor 'template/pallets/.*/docs/architecture\.en\.md' "$integration_doc" \
            "Integration document does not route package behavior to its implementation map"
    done
    reject_pattern '^## (?:Storage Topology|Call Surface|Error Surface|Package Modules)$' \
        "Package-local implementation inventory reintroduced into a root integration document" \
        "$PROJECT_ROOT/docs/actors.integration.en.md" "$PROJECT_ROOT/docs/oracle.integration.en.md"
    reject_pattern '^```rust$' \
        "Decorative Rust implementation syntax reintroduced outside a package owner" \
        "$core_architecture" "$PROJECT_ROOT/docs/actors.integration.en.md" \
        "$PROJECT_ROOT/docs/oracle.integration.en.md"
    reject_pattern '(?:0\.7\.|pre-launch|pre-release|release line|current slice|historical .* extraction gate)' \
        "Release diary reintroduced into current architecture or integration truth" \
        "${architecture_docs[@]}"

    phase_banner "Step 5: Canonical terminology and generated projection"
    local terminology_surfaces=(
        "$PROJECT_ROOT/docs/README.md"
        "$PROJECT_ROOT/docs/framework-instance.contract.en.md"
        "$PROJECT_ROOT/docs/core.architecture.en.md"
        "$PROJECT_ROOT/docs/actors.integration.en.md"
        "$PROJECT_ROOT/docs/actors-control-plane.contract.en.md"
        "$TEMPLATE_DIR/pallets/README.md"
        "$TEMPLATE_DIR/pallets/actors/README.md"
        "$TEMPLATE_DIR/pallets/actors/docs/specification.en.md"
        "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md"
        "$TEMPLATE_DIR/pallets/actors/docs/embedding.md"
        "$PROJECT_ROOT/web-client/docs/architecture.en.md"
        "$PROJECT_ROOT/web-client/src/lib/automation"
        "$PROJECT_ROOT/web-client/src/lib/widgets/AutomationWidget.svelte"
        "$PROJECT_ROOT/wiki"
    )
    reject_pattern '(?i)\b(?:actor[- ]programs?|execution[- ]plans?|cycle[- ]plans?)\b' \
        "Parallel current Actor Contract or Step terminology reintroduced" \
        "${terminology_surfaces[@]}"
    reject_pattern_except "$plural_preconditions_pattern" "$plural_detector_exception_pattern" \
        "Plural Preconditions terminology reintroduced where singular Precondition owns the optional DNF" \
        "${required_family_surfaces[@]}"
    reject_pattern '(?i)\b(?:all conditions|any condition|condition mode|condition controls|observation conditions|false conditions|condition evaluation)\b' \
        "Parallel current Precondition or Predicate terminology reintroduced" \
        "${terminology_surfaces[@]}"
    reject_pattern '(?i)\b(?:nomination reward epoch|reward epoch|security-epoch|native security phase|unified fee collector)\b' \
        "Parallel current SecurityEpoch, NativeSecurityMode, or Fee Sink terminology reintroduced" \
        "$TEMPLATE_DIR/pallets/staking/docs" "$TEMPLATE_DIR/pallets/staking/README.md" \
        "$PROJECT_ROOT/web-client/docs" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/wiki"
    require_anchor '^### Actor Contract$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical Actor Contract"
    require_anchor '^### Step$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical Step"
    require_anchor '^### Precondition$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical Precondition"
    require_anchor '^### Predicate$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical Predicate"
    require_anchor '^### `SecurityEpoch`$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical SecurityEpoch"
    require_anchor '^### `NativeSecurityMode`$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical NativeSecurityMode"
    require_anchor '^### `FailureClass`$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical FailureClass"
    require_anchor '^### `RetryDisposition`$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical RetryDisposition"
    require_anchor '^### Fee Sink$' "$PROJECT_ROOT/wiki/glossary/core-terms.en.md" \
        "Generated wiki glossary lacks canonical Fee Sink"
    require_anchor '"Actor Contract": "actor-system"' "$PROJECT_ROOT/wiki/_meta/aliases.json" \
        "Generated wiki alias metadata lacks canonical Actor Contract navigation"
    require_anchor '"SecurityEpoch": "staking"' "$PROJECT_ROOT/wiki/_meta/aliases.json" \
        "Generated wiki alias metadata lacks canonical SecurityEpoch navigation"
    reject_pattern '<!-- (?:BEGIN|END) MANUAL -->' \
        "Manual semantic overlay reintroduced into the generated wiki" "$PROJECT_ROOT/wiki"

    log_success "Protocol coherence regression audit passed"
}

main "$@"
