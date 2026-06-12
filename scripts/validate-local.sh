#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

RUN_SCRIPT_AUDIT="${RUN_SCRIPT_AUDIT:-1}"
RUN_TEMPLATE_AUDIT="${RUN_TEMPLATE_AUDIT:-1}"
RUN_NUMERIC_AUDIT="${RUN_NUMERIC_AUDIT:-1}"
RUN_SIMULATOR_DETERMINISM_AUDIT="${RUN_SIMULATOR_DETERMINISM_AUDIT:-1}"
RUN_SIMULATOR_CONSISTENCY_AUDIT="${RUN_SIMULATOR_CONSISTENCY_AUDIT:-1}"
RUN_CODE_SUPPRESSION_AUDIT="${RUN_CODE_SUPPRESSION_AUDIT:-1}"
RUN_BACKLOG_AUDIT="${RUN_BACKLOG_AUDIT:-1}"
RUN_RELEASE_LINE_AUDIT="${RUN_RELEASE_LINE_AUDIT:-1}"
RUN_PORTABILITY_AUDIT="${RUN_PORTABILITY_AUDIT:-1}"
RUN_DOMAIN_DAG_AUDIT="${RUN_DOMAIN_DAG_AUDIT:-1}"
RUN_WIKI_TRUST_AUDIT="${RUN_WIKI_TRUST_AUDIT:-1}"
RUN_DEPENDENCY_AUDIT="${RUN_DEPENDENCY_AUDIT:-0}"
RUN_CI="${RUN_CI:-1}"
RUN_BUILD="${RUN_BUILD:-1}"
RUN_E2E="${RUN_E2E:-0}"
PREPARE_E2E="${PREPARE_E2E:-0}"
KEEP_NETWORK="${KEEP_NETWORK:-0}"
RPC_READY_TIMEOUT_SEC="${RPC_READY_TIMEOUT_SEC:-240}"
ZOMBIENET_LOG="${ZOMBIENET_LOG:-/tmp/deos-zombienet.log}"

ZOMBIENET_PID=""
ALIGNMENT_SCRIPT_DIR="$PROJECT_ROOT/.agents/skills/alignment/scripts"

usage() {
    cat <<'EOF2'
Usage: validate-local.sh [OPTIONS]

DEOS local validation orchestrator

Options:
  --all            Run script audit + template audit + numeric audit + simulator determinism/consistency audits + code suppression audit + backlog/release-line/portability audits + domain DAG audit + wiki trust audit + CI + runtime build + E2E
  --audit-only     Run only fast script/template/numeric/simulator-determinism/simulator-consistency/code-suppression/backlog/release-line/portability/domain-DAG/wiki-trust audits
  --dependency-audit  Add report-only dependency posture audit (uses npm registry/audit network calls)
  --ci-only        Run only CI validation
  --build-only     Run only runtime build validation
  --e2e-only       Run only E2E validation
  --with-e2e       Add E2E validation to current plan
  --prepare-e2e    Run 01..04 setup before E2E (implies --with-e2e)
  --no-script-audit    Disable script entrypoint audit
  --no-template-audit  Disable template readiness audit
  --no-numeric-audit   Disable numeric parsing audit
  --no-simulator-determinism-audit  Disable simulator determinism audit
  --no-simulator-consistency-audit   Disable simulator tests.js/tests.md mirror audit
  --no-code-suppression-audit  Disable code suppression audit
  --no-backlog-audit  Disable backlog open-work audit
  --no-release-line-audit  Disable release-line consistency audit
  --no-portability-audit  Disable repo portability audit
  --no-domain-dag-audit  Disable web-client domain DAG audit
  --no-wiki-trust-audit  Disable trusted wiki markdown audit
  --no-dependency-audit  Disable dependency posture audit
  --no-ci          Disable CI validation
  --no-build       Disable runtime build validation
  --no-e2e         Disable E2E validation
  --keep-network   Keep zombienet running after completion
  -h, --help       Show this help message

Environment flags:
  RUN_SCRIPT_AUDIT=0|1
  RUN_TEMPLATE_AUDIT=0|1
  RUN_NUMERIC_AUDIT=0|1
  RUN_SIMULATOR_DETERMINISM_AUDIT=0|1
  RUN_SIMULATOR_CONSISTENCY_AUDIT=0|1
  RUN_CODE_SUPPRESSION_AUDIT=0|1
  RUN_BACKLOG_AUDIT=0|1
  RUN_RELEASE_LINE_AUDIT=0|1
  RUN_PORTABILITY_AUDIT=0|1
  RUN_DOMAIN_DAG_AUDIT=0|1
  RUN_WIKI_TRUST_AUDIT=0|1
  RUN_DEPENDENCY_AUDIT=0|1
  RUN_CI=0|1
  RUN_BUILD=0|1
  RUN_E2E=0|1
  PREPARE_E2E=0|1
  KEEP_NETWORK=0|1
  RPC_READY_TIMEOUT_SEC=<seconds>
  ZOMBIENET_LOG=<path>
EOF2
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all)
                RUN_SCRIPT_AUDIT=1
                RUN_TEMPLATE_AUDIT=1
                RUN_NUMERIC_AUDIT=1
                RUN_SIMULATOR_DETERMINISM_AUDIT=1
                RUN_SIMULATOR_CONSISTENCY_AUDIT=1
                RUN_CODE_SUPPRESSION_AUDIT=1
                RUN_BACKLOG_AUDIT=1
                RUN_RELEASE_LINE_AUDIT=1
                RUN_PORTABILITY_AUDIT=1
                RUN_DOMAIN_DAG_AUDIT=1
                RUN_WIKI_TRUST_AUDIT=1
                RUN_DEPENDENCY_AUDIT=1
                RUN_CI=1
                RUN_BUILD=1
                RUN_E2E=1
                ;;
            --audit-only)
                RUN_SCRIPT_AUDIT=1
                RUN_TEMPLATE_AUDIT=1
                RUN_NUMERIC_AUDIT=1
                RUN_SIMULATOR_DETERMINISM_AUDIT=1
                RUN_SIMULATOR_CONSISTENCY_AUDIT=1
                RUN_CODE_SUPPRESSION_AUDIT=1
                RUN_BACKLOG_AUDIT=1
                RUN_RELEASE_LINE_AUDIT=1
                RUN_PORTABILITY_AUDIT=1
                RUN_DOMAIN_DAG_AUDIT=1
                RUN_WIKI_TRUST_AUDIT=1
                RUN_DEPENDENCY_AUDIT=0
                RUN_CI=0
                RUN_BUILD=0
                RUN_E2E=0
                ;;
            --ci-only)
                RUN_SCRIPT_AUDIT=0
                RUN_TEMPLATE_AUDIT=0
                RUN_NUMERIC_AUDIT=0
                RUN_SIMULATOR_DETERMINISM_AUDIT=0
                RUN_SIMULATOR_CONSISTENCY_AUDIT=0
                RUN_CODE_SUPPRESSION_AUDIT=0
                RUN_BACKLOG_AUDIT=0
                RUN_RELEASE_LINE_AUDIT=0
                RUN_PORTABILITY_AUDIT=0
                RUN_DOMAIN_DAG_AUDIT=0
                RUN_WIKI_TRUST_AUDIT=0
                RUN_DEPENDENCY_AUDIT=0
                RUN_CI=1
                RUN_BUILD=0
                RUN_E2E=0
                ;;
            --build-only)
                RUN_SCRIPT_AUDIT=0
                RUN_TEMPLATE_AUDIT=0
                RUN_NUMERIC_AUDIT=0
                RUN_SIMULATOR_DETERMINISM_AUDIT=0
                RUN_SIMULATOR_CONSISTENCY_AUDIT=0
                RUN_CODE_SUPPRESSION_AUDIT=0
                RUN_BACKLOG_AUDIT=0
                RUN_RELEASE_LINE_AUDIT=0
                RUN_PORTABILITY_AUDIT=0
                RUN_DOMAIN_DAG_AUDIT=0
                RUN_WIKI_TRUST_AUDIT=0
                RUN_DEPENDENCY_AUDIT=0
                RUN_CI=0
                RUN_BUILD=1
                RUN_E2E=0
                ;;
            --e2e-only)
                RUN_SCRIPT_AUDIT=0
                RUN_TEMPLATE_AUDIT=0
                RUN_NUMERIC_AUDIT=0
                RUN_SIMULATOR_DETERMINISM_AUDIT=0
                RUN_SIMULATOR_CONSISTENCY_AUDIT=0
                RUN_CODE_SUPPRESSION_AUDIT=0
                RUN_BACKLOG_AUDIT=0
                RUN_RELEASE_LINE_AUDIT=0
                RUN_PORTABILITY_AUDIT=0
                RUN_DOMAIN_DAG_AUDIT=0
                RUN_WIKI_TRUST_AUDIT=0
                RUN_DEPENDENCY_AUDIT=0
                RUN_CI=0
                RUN_BUILD=0
                RUN_E2E=1
                ;;
            --dependency-audit)
                RUN_DEPENDENCY_AUDIT=1
                ;;
            --with-e2e)
                RUN_E2E=1
                ;;
            --prepare-e2e)
                RUN_E2E=1
                PREPARE_E2E=1
                ;;
            --no-script-audit)
                RUN_SCRIPT_AUDIT=0
                ;;
            --no-template-audit)
                RUN_TEMPLATE_AUDIT=0
                ;;
            --no-numeric-audit)
                RUN_NUMERIC_AUDIT=0
                ;;
            --no-simulator-determinism-audit)
                RUN_SIMULATOR_DETERMINISM_AUDIT=0
                ;;
            --no-simulator-consistency-audit)
                RUN_SIMULATOR_CONSISTENCY_AUDIT=0
                ;;
            --no-code-suppression-audit)
                RUN_CODE_SUPPRESSION_AUDIT=0
                ;;
            --no-backlog-audit)
                RUN_BACKLOG_AUDIT=0
                ;;
            --no-release-line-audit)
                RUN_RELEASE_LINE_AUDIT=0
                ;;
            --no-portability-audit)
                RUN_PORTABILITY_AUDIT=0
                ;;
            --no-domain-dag-audit)
                RUN_DOMAIN_DAG_AUDIT=0
                ;;
            --no-wiki-trust-audit)
                RUN_WIKI_TRUST_AUDIT=0
                ;;
            --no-dependency-audit)
                RUN_DEPENDENCY_AUDIT=0
                ;;
            --no-ci)
                RUN_CI=0
                ;;
            --no-build)
                RUN_BUILD=0
                ;;
            --no-e2e)
                RUN_E2E=0
                ;;
            --keep-network)
                KEEP_NETWORK=1
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

check_plan() {
    phase_banner "Step 1: Validation plan"
    if (( RUN_SCRIPT_AUDIT == 0 && RUN_TEMPLATE_AUDIT == 0 && RUN_NUMERIC_AUDIT == 0 && RUN_SIMULATOR_DETERMINISM_AUDIT == 0 && RUN_SIMULATOR_CONSISTENCY_AUDIT == 0 && RUN_CODE_SUPPRESSION_AUDIT == 0 && RUN_BACKLOG_AUDIT == 0 && RUN_RELEASE_LINE_AUDIT == 0 && RUN_PORTABILITY_AUDIT == 0 && RUN_DOMAIN_DAG_AUDIT == 0 && RUN_WIKI_TRUST_AUDIT == 0 && RUN_DEPENDENCY_AUDIT == 0 && RUN_CI == 0 && RUN_BUILD == 0 && RUN_E2E == 0 )); then
        log_error "Nothing to run. Enable at least one validation stage"
        exit 1
    fi
    log_info "Plan: script_audit=$RUN_SCRIPT_AUDIT template_audit=$RUN_TEMPLATE_AUDIT numeric_audit=$RUN_NUMERIC_AUDIT simulator_determinism_audit=$RUN_SIMULATOR_DETERMINISM_AUDIT simulator_consistency_audit=$RUN_SIMULATOR_CONSISTENCY_AUDIT code_suppression_audit=$RUN_CODE_SUPPRESSION_AUDIT backlog_audit=$RUN_BACKLOG_AUDIT release_line_audit=$RUN_RELEASE_LINE_AUDIT portability_audit=$RUN_PORTABILITY_AUDIT domain_dag_audit=$RUN_DOMAIN_DAG_AUDIT wiki_trust_audit=$RUN_WIKI_TRUST_AUDIT dependency_audit=$RUN_DEPENDENCY_AUDIT"
    log_info "Plan: ci=$RUN_CI build=$RUN_BUILD e2e=$RUN_E2E prepare_e2e=$PREPARE_E2E"
}

run_alignment_script_step() {
    local label="$1"
    local script_name="$2"
    shift 2

    local script_path="$ALIGNMENT_SCRIPT_DIR/$script_name"
    if [[ ! -x "$script_path" ]]; then
        log_error "Alignment skill script not found or not executable: $script_name"
        exit 1
    fi
    log_info "Running: $label (.agents/skills/alignment/scripts/$script_name)"
    local start_time
    start_time=$(date +%s)
    "$script_path" "$@"
    local end_time
    end_time=$(date +%s)
    log_success "$label completed in $((end_time - start_time))s"
}

run_requested_stages() {
    phase_banner "Step 2: Local validation stages"
    if (( RUN_SCRIPT_AUDIT == 1 )); then
        run_alignment_script_step "Script entrypoint audit" "audit-script-entrypoints.sh"
    fi
    if (( RUN_TEMPLATE_AUDIT == 1 )); then
        run_alignment_script_step "Template readiness audit" "audit-template-readiness.sh"
    fi
    if (( RUN_NUMERIC_AUDIT == 1 )); then
        run_alignment_script_step "Numeric parsing audit" "audit-numeric-parsing.sh"
    fi
    if (( RUN_SIMULATOR_DETERMINISM_AUDIT == 1 )); then
        run_alignment_script_step "Simulator determinism audit" "audit-simulator-determinism.sh"
    fi
    if (( RUN_SIMULATOR_CONSISTENCY_AUDIT == 1 )); then
        run_alignment_script_step "Simulator suite mirror audit" "audit-simulator-consistency.sh"
    fi
    if (( RUN_CODE_SUPPRESSION_AUDIT == 1 )); then
        run_alignment_script_step "Code suppression audit" "audit-code-suppressions.sh"
    fi
    if (( RUN_BACKLOG_AUDIT == 1 )); then
        run_alignment_script_step "Backlog open-work audit" "audit-backlog-open-work.sh"
    fi
    if (( RUN_RELEASE_LINE_AUDIT == 1 )); then
        run_alignment_script_step "Release-line audit" "audit-release-line.sh"
    fi
    if (( RUN_PORTABILITY_AUDIT == 1 )); then
        run_alignment_script_step "Repository portability audit" "audit-repo-portability.sh"
    fi
    if (( RUN_DOMAIN_DAG_AUDIT == 1 )); then
        run_shell_step "Web-client domain DAG audit" "" "cd '$PROJECT_ROOT/web-client' && npm run validate:dag"
    fi
    if (( RUN_WIKI_TRUST_AUDIT == 1 )); then
        run_shell_step "Trusted wiki markdown audit" "" "cd '$PROJECT_ROOT/web-client' && npm run validate:wiki"
    fi
    if (( RUN_DEPENDENCY_AUDIT == 1 )); then
        run_alignment_script_step "Dependency posture audit" "audit-dependency-posture.sh"
    fi
    if (( RUN_CI == 1 )); then
        run_script_step "CI local workflow" "ci-local.sh"
    fi
    if (( RUN_BUILD == 1 )); then
        run_script_step "Runtime build" "03-build-runtime.sh"
    fi
    if (( RUN_E2E == 1 )); then
        run_e2e
    fi
}

run_e2e() {
    phase_banner "Step 3: E2E workflow"
    if (( PREPARE_E2E == 1 )); then
        run_script_step "Download binaries" "01-download-binaries.sh"
        run_script_step "Install tools" "02-install-tools.sh"
        run_script_step "Build runtime" "03-build-runtime.sh"
        run_script_step "Generate chain spec" "04-generate-chain-spec.sh"
    fi

    start_background_script "zombienet" "05-spawn-zombienet.sh" "$ZOMBIENET_LOG" ZOMBIENET_PID
    wait_for_chain_rpc "http://127.0.0.1:9988" "$RPC_READY_TIMEOUT_SEC" "Collator RPC" "$ZOMBIENET_PID" "$ZOMBIENET_LOG"
    run_script_step "Zombienet E2E" "06-zombienet-e2e.sh"
}

on_exit() {
    local exit_code=$?
    stop_background_process "$ZOMBIENET_PID" "$KEEP_NETWORK" "$ZOMBIENET_LOG" "zombienet"
    if (( exit_code != 0 )); then
        log_error "Local validation failed"
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Local validation completed successfully"
}

main() {
    parse_args "$@"
    phase_banner "DEOS local validation workflow"
    trap on_exit EXIT
    check_plan
    run_requested_stages
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
