#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-protocol-coherence-regressions.sh

Fail-closed source audit for semantic owners retired by the 0.7.17 coherence line:
legacy Actor identity, adaptive governance thresholds, unreserved strategic
capacity, block-cadenced or inferred staking rewards, secondary native-security
flags, raw Router-error retry inference, and placeholder public variants.
EOF
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
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        usage
        exit 0
    fi
    [[ "$#" == "0" ]] || { log_error "Unknown argument: $1"; usage; exit 1; }

    phase_banner "Step 1: Prerequisites"
    require_commands rg awk

    phase_banner "Step 2: Retired semantic owners"
    "$SCRIPT_DIR/audit-actors-identity.sh"

    local governance_src="$TEMPLATE_DIR/pallets/governance/src"
    local staking_src="$TEMPLATE_DIR/pallets/staking/src"
    local runtime_configs="$TEMPLATE_DIR/runtime/src/configs"
    reject_pattern '\b(?:AdaptiveApproval|AdaptiveTurnout|AdaptiveThreshold|ApprovalDecay|ThresholdDecay|VotingProgressThreshold)\b' \
        "Adaptive governance threshold owner reintroduced" "$governance_src"
    require_anchor 'type StrategicProposalReserve: Get<u32>' "$governance_src/lib.rs" \
        "Governance strategic capacity reserve owner is missing"
    require_anchor 'maximum\.saturating_sub\(T::StrategicProposalReserve::get\(\)\)' \
        "$governance_src/epoch_service.rs" "General governance capacity no longer withholds the strategic reserve"
    require_anchor 'general_proposal_cap_preserves_the_strategic_reserve' "$governance_src/tests.rs" \
        "Governance strategic-reserve regression evidence is missing"

    reject_pattern '\b(?:BlockNumberRewardEpoch|RewardEpochProvider|RewardPeriod|RewardRolloverCursor|RewardEventIngress|BalanceDeltaReward|InferredRewardFunding)\b' \
        "Retired block-cadenced or inferred staking reward owner reintroduced" "$staking_src"
    reject_pattern '\b(?:NativeSecurityEnabled|EnableNativeSecurity|LpBackedSelectionEnabled|EnableLpBackedSelection|NativeSecurityPhase|SecurityPhaseProvider)\b' \
        "Independent native-security phase flag reintroduced" "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client/src" "$PROJECT_ROOT/scripts"

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

    reject_pattern '^\s*(?:Legacy|Reserved|Deprecated|Unused)(?:\s*[({,])' \
        "Placeholder public variant reintroduced without an executable contract" \
        "$TEMPLATE_DIR/pallets/governance/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/staking/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/actors/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/actors/src/types.rs" \
        "$TEMPLATE_DIR/pallets/router/src/lib.rs" \
        "$TEMPLATE_DIR/pallets/router/src/types.rs" \
        "$TEMPLATE_DIR/pallets/oracle/src/lib.rs"
    require_anchor 'actor_scale_variant_names_are_stable' "$TEMPLATE_DIR/pallets/actors/src/tests.rs" \
        "Actors closed SCALE-surface regression evidence is missing"
    require_anchor 'adversarial_corpus_is_complete_unique_and_anchor_bound' "$TEMPLATE_DIR/pallets/router/src/tests.rs" \
        "Router executable failure-surface inventory is missing"
    require_anchor 'task_failure_defaults_unknown_errors_to_permanent' "$TEMPLATE_DIR/pallets/actors/src/tests.rs" \
        "Unknown Actor adapter failures no longer have fail-closed evidence"

    phase_banner "Step 4: Specification and implementation-map ownership"
    local core_architecture="$PROJECT_ROOT/docs/core.architecture.en.md"
    require_anchor 'Public Surface Closure Map' "$core_architecture" \
        "Cross-system public-surface closure map is missing"
    local subsystem
    for subsystem in governance staking actors router oracle; do
        [[ -f "$TEMPLATE_DIR/pallets/$subsystem/docs/specification.en.md" ]] || {
            log_error "$subsystem package specification is missing"
            exit 1
        }
        [[ -f "$TEMPLATE_DIR/pallets/$subsystem/docs/architecture.en.md" ]] || {
            log_error "$subsystem package architecture map is missing"
            exit 1
        }
        require_anchor "template/pallets/$subsystem/docs/specification\\.en\\.md" "$core_architecture" \
            "$subsystem is absent from the public-surface owner map"
    done

    log_success "Protocol coherence regression audit passed"
}

main "$@"
