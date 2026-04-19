#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

LEDGER_DIR="$SKILL_DIR/ledgers"
LEDGER_KIND=""
TITLE=""
SUMMARY=""
SCOPE=""
STATUS="open"
FILES=""
TAGS=""
REMEDY=""
RELATED=""

usage() {
    cat <<'EOF'
Usage: log-ledger.sh --ledger KIND --title TEXT --summary TEXT [OPTIONS]

Append a high-order coordination-memory record to a DEOS alignment ledger.

Required:
  --ledger KIND         ambiguity | dead-end | dead_end | hallucination | boundary-drift | boundary_drift
  --title TEXT          Short record title
  --summary TEXT        Compact description of the pattern, ambiguity, or trap

Optional:
  --scope TEXT          Domain/surface affected (e.g. web-client/dev-flow)
  --status TEXT         open | resolved | mitigated | rejected (default: open)
  --files CSV           Comma-separated file paths or '*' when cross-cutting
  --tags CSV            Comma-separated tags
  --remedy TEXT         Preferred correction or avoidance pattern
  --related TEXT        Related doc, issue, command, or ledger reference
  -h, --help            Show this help

Examples:
  ./.agents/skills/alignment/scripts/log-ledger.sh \
    --ledger ambiguity \
    --title "Skill memory scope" \
    --summary "Per-run telemetry is not canonical memory" \
    --scope "alignment/ledgers" \
    --status resolved \
    --remedy "Capture only durable ambiguity, hallucination, and dead-end patterns"

  ./.agents/skills/alignment/scripts/log-ledger.sh \
    --ledger dead-end \
    --title "Foreground dev server in main flow" \
    --summary "Long-running dev servers block the conversation loop" \
    --scope "web-client/dev-flow" \
    --status resolved \
    --tags "blocking-flow,operator-trap" \
    --remedy "Prefer build/help/probe; background only when explicitly needed"

  ./.agents/skills/alignment/scripts/log-ledger.sh \
    --ledger boundary-drift \
    --title "Mechanism replaced by admin policy" \
    --summary "A core economic path drifted from deterministic routing to root-only mutation" \
    --scope "runtime/policy-surface" \
    --status open \
    --tags "mechanism-policy,boundary-drift" \
    --remedy "Restore mechanism-first contract and keep admin surfaces narrow"
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --ledger)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --ledger"; usage; exit 1; }
                LEDGER_KIND="$1"
                ;;
            --title)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --title"; usage; exit 1; }
                TITLE="$1"
                ;;
            --summary)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --summary"; usage; exit 1; }
                SUMMARY="$1"
                ;;
            --scope)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --scope"; usage; exit 1; }
                SCOPE="$1"
                ;;
            --status)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --status"; usage; exit 1; }
                STATUS="$1"
                ;;
            --files)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --files"; usage; exit 1; }
                FILES="$1"
                ;;
            --tags)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --tags"; usage; exit 1; }
                TAGS="$1"
                ;;
            --remedy)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --remedy"; usage; exit 1; }
                REMEDY="$1"
                ;;
            --related)
                shift
                [[ $# -gt 0 ]] || { log_error "Missing value for --related"; usage; exit 1; }
                RELATED="$1"
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

normalize_ledger_path() {
    case "$LEDGER_KIND" in
        ambiguity|ambiguities)
            echo "$LEDGER_DIR/ambiguities.jsonl"
            ;;
        dead-end|dead_end|dead-ends|dead_ends)
            echo "$LEDGER_DIR/dead_ends.jsonl"
            ;;
        boundary-drift|boundary_drift|boundary-drifts|boundary_drifts)
            echo "$LEDGER_DIR/boundary_drifts.jsonl"
            ;;
        hallucination|hallucinations)
            echo "$LEDGER_DIR/hallucinations.jsonl"
            ;;
        *)
            log_error "Unknown ledger kind: $LEDGER_KIND"
            usage
            exit 1
            ;;
    esac
}

escape_json() {
    local value="$1"
    value=${value//\\/\\\\}
    value=${value//"/\\"}
    value=${value//$'\n'/\\n}
    value=${value//$'\r'/\\r}
    value=${value//$'\t'/\\t}
    printf '%s' "$value"
}

is_banned_low_signal_text() {
    local value="$1"
    local normalized
    normalized="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]' | tr -s ' ' | sed 's/^ *//; s/ *$//')"
    case "$normalized" in
        "knowledge sync gate failed"|"architecture audit gate failed"|"compilation gate failed"|"shell syntax gate failed"|"simulation gate failed")
            return 0
            ;;
    esac
    return 1
}

validate_record_quality() {
    if is_banned_low_signal_text "$TITLE"; then
        log_error "Rejected low-signal ledger title: $TITLE"
        log_info "Use a scoped pattern title instead (what failed, where, and why it matters)"
        exit 1
    fi
    if is_banned_low_signal_text "$SUMMARY"; then
        log_error "Rejected low-signal ledger summary: $SUMMARY"
        log_info "Summaries must capture a reusable pattern, not only a gate symptom"
        exit 1
    fi
    if [[ ${#SUMMARY} -lt 24 ]]; then
        log_error "Ledger summary is too short to be durable memory"
        log_info "Describe the reusable pattern, affected surface, and preferred remedy"
        exit 1
    fi
    if [[ -z "$SCOPE" || -z "$REMEDY" ]]; then
        log_error "Ledger entries require both --scope and --remedy to stay useful"
        log_info "Canonical memory must say where the pattern lives and what to do differently"
        exit 1
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$PROJECT_ROOT/.git" "Git repository"
    require_directory "$SKILL_DIR" "Skill directory"
    require_commands date mkdir sed tr
    if [[ -z "$LEDGER_KIND" || -z "$TITLE" || -z "$SUMMARY" ]]; then
        log_error "--ledger, --title, and --summary are required"
        usage
        exit 1
    fi
    validate_record_quality
    mkdir -p "$LEDGER_DIR"
    log_success "Ledger prerequisites checked"
}

plan() {
    phase_banner "DEOS ledger append"
    log_info "Ledger: $LEDGER_KIND"
    log_info "Title: $TITLE"
    if [[ -n "$SCOPE" ]]; then
        log_info "Scope: $SCOPE"
    fi
    log_info "Status: $STATUS"
}

append_record() {
    local ledger_path
    local timestamp
    ledger_path="$(normalize_ledger_path)"
    timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    touch "$ledger_path"
    printf '{"timestamp":"%s","title":"%s","summary":"%s","scope":"%s","status":"%s","files":"%s","tags":"%s","remedy":"%s","related":"%s"}\n' \
        "$timestamp" \
        "$(escape_json "$TITLE")" \
        "$(escape_json "$SUMMARY")" \
        "$(escape_json "$SCOPE")" \
        "$(escape_json "$STATUS")" \
        "$(escape_json "$FILES")" \
        "$(escape_json "$TAGS")" \
        "$(escape_json "$REMEDY")" \
        "$(escape_json "$RELATED")" >> "$ledger_path"
    log_success "Appended record to $ledger_path"
}

main() {
    parse_args "$@"
    check_prerequisites
    plan
    append_record
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
