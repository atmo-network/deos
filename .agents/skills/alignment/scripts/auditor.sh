#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MODE="${MODE:-changed}"
SINCE_REF="${SINCE_REF:-HEAD}"
TARGET_PATH=""
LEDGER_DIR="$SKILL_DIR/ledgers"
HALLUCINATIONS_FILE="$LEDGER_DIR/hallucinations.jsonl"
SKIP_AUDIT=0

declare -a AUDIT_TARGETS=()

audit_errors=0
audit_warnings=0

usage() {
    cat <<'EOF'
Usage: auditor.sh [PATH] [OPTIONS]

Comprehensive DEOS architecture auditor with contextual remediation and a hallucination ledger.

Options:
  --changed         Audit changed Rust lines only (default)
  --all             Audit the full pallet tree (defaults to ./template/pallets)
  --since REF       Compare changed files against git REF (default: HEAD)
  -h, --help        Show this help message

Environment:
  MODE=changed|all
  SINCE_REF=HEAD|<git-ref>
  DEOS_VERBOSE=0|1
  DEOS_FAILURE_TAIL_LINES=N
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --changed)
                MODE="changed"
                ;;
            --all)
                MODE="all"
                ;;
            --since)
                shift
                if [[ $# -eq 0 ]]; then
                    log_error "Missing value for --since"
                    usage
                    exit 1
                fi
                SINCE_REF="$1"
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            -*)
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
            *)
                TARGET_PATH="$1"
                ;;
        esac
        shift
    done
}

resolve_path() {
    local value="$1"
    if [[ -z "$value" ]]; then
        return 1
    fi
    if [[ -e "$value" ]]; then
        local dir
        dir="$(cd "$(dirname "$value")" && pwd)"
        echo "$dir/$(basename "$value")"
        return 0
    fi
    if [[ -e "$PROJECT_ROOT/$value" ]]; then
        local dir
        dir="$(cd "$(dirname "$PROJECT_ROOT/$value")" && pwd)"
        echo "$dir/$(basename "$value")"
        return 0
    fi
    echo "$PROJECT_ROOT/$value"
}

collect_changed_rust_files() {
    local tracked
    local untracked
    tracked="$(git -C "$PROJECT_ROOT" diff --name-only --diff-filter=ACMRTUXB "$SINCE_REF" -- '*.rs' || true)"
    untracked="$(git -C "$PROJECT_ROOT" ls-files --others --exclude-standard -- '*.rs' || true)"
    printf '%s\n%s\n' "$tracked" "$untracked" | awk 'NF' | sort -u
}

collect_audit_targets() {
    AUDIT_TARGETS=()
    if [[ "$MODE" == "all" ]]; then
        local target
        if [[ -n "$TARGET_PATH" ]]; then
            target="$(resolve_path "$TARGET_PATH")"
        else
            target="$TEMPLATE_DIR/pallets"
        fi
        AUDIT_TARGETS+=("$target")
        return 0
    fi
    local changed
    if [[ -n "$TARGET_PATH" ]]; then
        local resolved_target
        resolved_target="$(resolve_path "$TARGET_PATH")"
        if [[ -d "$resolved_target" ]]; then
            while IFS= read -r changed; do
                [[ -z "$changed" ]] && continue
                if [[ "$PROJECT_ROOT/$changed" == "$resolved_target"/* ]]; then
                    AUDIT_TARGETS+=("$PROJECT_ROOT/$changed")
                fi
            done < <(collect_changed_rust_files)
        else
            AUDIT_TARGETS+=("$resolved_target")
        fi
        return 0
    fi
    while IFS= read -r changed; do
        [[ -z "$changed" ]] && continue
        AUDIT_TARGETS+=("$PROJECT_ROOT/$changed")
    done < <(collect_changed_rust_files)
}

append_ledger_entry() {
    local severity="$1"
    local rule_name="$2"
    local matches="$3"
    local files
    local timestamp
    files="$(printf '%s\n' "$matches" | awk -F: 'NF { print $1 }' | sort -u | paste -sd ',' -)"
    timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf '{"timestamp":"%s","severity":"%s","rule":"%s","files":"%s"}\n' "$timestamp" "$severity" "$rule_name" "$files" >> "$HALLUCINATIONS_FILE"
}


is_git_tracked() {
    local abs_path="$1"
    local rel_path="${abs_path#$PROJECT_ROOT/}"
    git -C "$PROJECT_ROOT" ls-files --error-unmatch "$rel_path" >/dev/null 2>&1
}

emit_diff_records() {
    local tracked_rel=()
    local target
    local rel_path
    for target in "${AUDIT_TARGETS[@]}"; do
        [[ -f "$target" ]] || continue
        if is_git_tracked "$target"; then
            rel_path="${target#$PROJECT_ROOT/}"
            tracked_rel+=("$rel_path")
        fi
    done
    if [[ ${#tracked_rel[@]} -eq 0 ]]; then
        return 0
    fi
    git -C "$PROJECT_ROOT" diff -U0 "$SINCE_REF" -- "${tracked_rel[@]}" | awk -v root="$PROJECT_ROOT" '
        /^\+\+\+ b\// {
            file = substr($0, 7)
            next
        }
        /^@@ / {
            split($0, parts, " ")
            new_range = parts[3]
            sub(/^\+/, "", new_range)
            split(new_range, coords, ",")
            line = coords[1] + 0
            next
        }
        /^\+/ && !/^\+\+\+/ {
            print root "/" file ":" line ":" substr($0, 2)
            line++
            next
        }
    '
}

emit_untracked_records() {
    local target
    for target in "${AUDIT_TARGETS[@]}"; do
        [[ -f "$target" ]] || continue
        if ! is_git_tracked "$target"; then
            awk -v file="$target" '{ print file ":" NR ":" $0 }' "$target"
        fi
    done
}

search_targets() {
    local pattern="$1"
    local exclude_pattern="$2"
    local matches=""
    if [[ "$MODE" == "all" ]]; then
        matches="$(grep -R -n -E --include='*.rs' "$pattern" "${AUDIT_TARGETS[@]}" 2>/dev/null || true)"
    else
        local records=""
        records+="$(emit_diff_records)"
        if [[ -n "$records" ]]; then
            records+=$'\n'
        fi
        records+="$(emit_untracked_records)"
        matches="$(printf '%s\n' "$records" | awk 'NF' | grep -E "$pattern" || true)"
    fi
    if [[ -n "$exclude_pattern" ]]; then
        matches="$(printf '%s\n' "$matches" | grep -v -E "$exclude_pattern" || true)"
    fi
    printf '%s' "$matches"
}

memory_hint() {
    local ledger_kind="$1"
    local reason="$2"
    local ledger_file=""
    case "$ledger_kind" in
        hallucination)
            ledger_file="ledgers/hallucinations.jsonl"
            ;;
        ambiguity)
            ledger_file="ledgers/ambiguities.jsonl"
            ;;
        dead-end)
            ledger_file="ledgers/dead_ends.jsonl"
            ;;
        boundary-drift)
            ledger_file="ledgers/boundary_drifts.jsonl"
            ;;
        *)
            return 0
            ;;
    esac
    log_info "Memory surface: $ledger_file"
    if [[ -n "$reason" ]]; then
        log_info "Why: $reason"
    fi
    log_info "If this reveals a reusable pattern beyond the raw violation, capture it via ./.agents/skills/alignment/scripts/log-ledger.sh --ledger $ledger_kind"
}

check_rule() {
    local pattern="$1"
    local message="$2"
    local exclude_pattern="$3"
    local rule_name="$4"
    local docs_path="$5"
    local severity="$6"
    local memory_surface="${7:-}"
    local memory_reason="${8:-}"
    local path_filter="${9:-}"
    local matches
    matches="$(search_targets "$pattern" "$exclude_pattern")"
    if [[ -n "$path_filter" ]]; then
        matches="$(printf '%s\n' "$matches" | grep -E "$path_filter" || true)"
    fi
    if [[ -z "$matches" ]]; then
        return 0
    fi
    append_ledger_entry "$severity" "$rule_name" "$matches"
    if [[ "$severity" == "FATAL" ]]; then
        log_error "$message"
        audit_errors=$((audit_errors + 1))
    else
        log_warning "$message"
        audit_warnings=$((audit_warnings + 1))
    fi
    if [[ -n "$docs_path" ]]; then
        log_info "Remedy: read $docs_path"
    fi
    memory_hint "$memory_surface" "$memory_reason"
    printf '%s\n\n' "$matches"
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    hydrate_local_tool_paths
    require_directory "$PROJECT_ROOT/.git" "Git repository"
    require_commands git grep awk sort paste date node
    mkdir -p "$LEDGER_DIR"
    touch "$HALLUCINATIONS_FILE"
    collect_audit_targets
    if [[ ${#AUDIT_TARGETS[@]} -eq 0 ]]; then
        log_warning "No changed Rust files detected; skipping architecture audit"
        SKIP_AUDIT=1
        return 0
    fi
    local target
    for target in "${AUDIT_TARGETS[@]}"; do
        if [[ ! -d "$target" && ! -f "$target" ]]; then
            log_error "Audit target not found: $target"
            exit 1
        fi
    done
    log_success "Auditor prerequisites checked"
}

plan() {
    phase_banner "DEOS architecture auditor"
    log_info "Mode: $MODE"
    log_info "Since ref: $SINCE_REF"
    if [[ ${#AUDIT_TARGETS[@]} -gt 0 ]]; then
        log_info "Targets: ${AUDIT_TARGETS[*]}"
    fi
}

inventory_patterns() {
    local rule_id="$1"
    local inventory_file="${2:-$PROJECT_ROOT/.agents/skills/alignment/rules/actors-identity-rules.json}"
    node -e '
const fs = require("node:fs");
const inventory = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const rule = inventory.rules.find((candidate) => candidate.id === process.argv[2]);
if (!rule || !Array.isArray(rule.patterns)) process.exit(1);
process.stdout.write(rule.patterns.join("|"));
' "$inventory_file" "$rule_id"
}

run_audit() {
    if [[ "$SKIP_AUDIT" == "1" ]]; then
        return 0
    fi
    phase_banner "Step 2: Architecture rules"
    local bounded_iteration_allowlist
    local actors_execution_panic_allowlist
    local actors_pallet_panic_allowlist
    local runtime_panic_allowlist
    bounded_iteration_allowlist="$(inventory_patterns actors-bounded-iteration-allowlist)"
    actors_execution_panic_allowlist="$(inventory_patterns actors-execution-panic-owner-allowlist)"
    actors_pallet_panic_allowlist="$(inventory_patterns actors-pallet-panic-owner-allowlist)"
    runtime_panic_allowlist="$(inventory_patterns runtime-panic-owner-allowlist "$PROJECT_ROOT/.agents/skills/alignment/rules/runtime-panic-rules.json")"
    check_rule "\\.iter\\(\\)|for .* in .*::iter\\(\\)" "Potential unbounded state iteration detected. DEOS prefers bounded/O(1) mechanics." "tests.rs|/tests/|/vendor/|benchmarking.rs|mock.rs|deos-bypass: bounded-iter|legs\\.iter\\(|normalized_transfers\\.iter\\(|tracked\\.iter\\(|${bounded_iteration_allowlist}" "O(N) Iteration Anti-Pattern" "template/pallets/staking/docs/architecture.en.md" "FATAL" "hallucination" "The violation is a concrete false move against the O(1) contract"
    check_rule "StorageValue<.*, Vec<|StorageMap<.*, Vec<" "Unbounded Vec found in storage. Use BoundedVec for consensus state." "tests.rs|/tests/|mock.rs|deos-bypass: vec" "Unbounded Vector Anti-Pattern" "docs/read-model.contract.en.md" "WARNING" "hallucination" "Consensus-state unboundedness is a direct architecture falsehood"
    check_rule "sudo_|ensure_root" "Admin/root policy detected. DEOS prefers mechanism over policy for core flows." "tests.rs|/tests/|mock.rs|pallet-governance|deos-bypass: admin" "Sudo Root Policy Anti-Pattern" "docs/manifesto.en.md" "FATAL" "boundary-drift" "This usually signals a mechanism-to-policy drift across a constitutional boundary"
    check_rule "pub fn claim.*\\(origin" "Traditional claim extrinsic detected. Prefer token-driven balance ingress where possible." "tests.rs|/tests/|mock.rs|staking|deos-bypass: claim" "Empty Extrinsic Anti-Pattern" "docs/core.architecture.en.md" "WARNING" "boundary-drift" "This usually means token-driven ingress is drifting back into user-pulled extrinsic control"
    check_rule "remove_liquidity|burn_native|withdraw_reserve" "TMC reserve extraction pattern detected. The minting curve must remain unidirectional." "tests.rs|/tests/|mock.rs" "TMC Reserve Extraction Anti-Pattern" "template/pallets/tmc/docs/architecture.en.md" "FATAL" "boundary-drift" "This breaches the mint-only physics boundary by reintroducing extraction semantics" "/template/pallets/tmc/"
    check_rule "\\* [A-Za-z_0-9]+ *: *u128" "Direct u128 multiplication detected. Check intermediate overflow and prefer U256 where needed." "tests.rs|/tests/|mock.rs|weights.rs|deos-bypass: math" "u128 Math Overflow Risk" "template/pallets/tmc/docs/architecture.en.md" "WARNING" "hallucination" "Arithmetic safety claims are false until overflow boundaries are proven"
    check_rule "\\+=" "Unchecked Actors accumulation detected. Use checked_add, saturating arithmetic only for bounded telemetry, or an exact same-line owner." "tests.rs|/tests/|mock.rs|benchmarking.rs|weights.rs|deos-bypass: checked-count" "Unchecked Actors Arithmetic" "template/pallets/actors/docs/architecture.en.md" "FATAL" "hallucination" "Consensus counts, revisions, generations, positions, and economic totals require explicit overflow semantics" "/template/pallets/actors/src/"
    check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "Unowned Actors execution panic surface detected. Return a typed failure or register an exact mechanically precluded owner." "$actors_execution_panic_allowlist" "Unowned Actors Execution Panic Surface" "template/pallets/actors/docs/architecture.en.md" "FATAL" "hallucination" "Actors execution assertions require canonical-loader, admission, type, integrity, or transactional-finalization ownership" "/template/pallets/actors/src/execution\\.rs"
    check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "Unowned Actors pallet panic surface detected. Return a typed failure or register an exact genesis, integrity, or admission owner." "$actors_pallet_panic_allowlist" "Unowned Actors Pallet Panic Surface" "template/pallets/actors/docs/architecture.en.md" "FATAL" "hallucination" "Actors pallet assertions require explicit genesis, integrity, or admitted-helper ownership" "/template/pallets/actors/src/(lib|crossing|scheduler|reactions|subscriptions|types/(contract|lifecycle|observation|scheduler))\\.rs"
    if [[ "$MODE" == "all" ]]; then
        local original_targets=("${AUDIT_TARGETS[@]}")
        AUDIT_TARGETS=("$TEMPLATE_DIR/runtime/src")
        check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "Unowned reference-runtime panic surface detected. Return a typed failure or register an exact construction, test, benchmark, TryRuntime, or bounded-catalog owner." "$runtime_panic_allowlist|/tests/" "Unowned Reference Runtime Panic Surface" "BACKLOG.md" "FATAL" "hallucination" "Runtime assertions require explicit construction, test, benchmark, TryRuntime, or bounded-catalog ownership" "/template/runtime/src/"
        AUDIT_TARGETS=("${original_targets[@]}")
    else
        check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "Unowned reference-runtime panic surface detected. Return a typed failure or register an exact construction, test, benchmark, TryRuntime, or bounded-catalog owner." "$runtime_panic_allowlist|/tests/" "Unowned Reference Runtime Panic Surface" "BACKLOG.md" "FATAL" "hallucination" "Runtime assertions require explicit construction, test, benchmark, TryRuntime, or bounded-catalog ownership" "/template/runtime/src/"
    fi
    check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "Unowned production pallet panic surface detected. Return a typed failure or mark a mechanically precluded same-line owner." "tests.rs|mock.rs|benchmarking.rs|deos-bypass: panic-owner" "Unowned Production Pallet Panic Surface" "BACKLOG.md" "FATAL" "hallucination" "Production pallet panic assumptions require an explicit owner and falsifiable preclusion evidence" "/template/pallets/(asset-registry|governance|oracle|router|staking|tmc)/src/"
    if [[ "$MODE" == "changed" ]]; then
        check_rule "panic!|unreachable!|\\.expect\\(|\\.unwrap\\(" "New unowned Rust panic surface detected. Return a typed failure, prove the site mechanically precluded, or mark the same line with 'deos-bypass: panic-owner' and its owner." "tests.rs|/tests/|/vendor/|mock.rs|benchmarking.rs|/examples/|genesis_config_presets.rs|embedding-runtime|deos-bypass: panic-owner" "Unowned Runtime Panic Surface" "BACKLOG.md" "FATAL" "hallucination" "Reachable panic assumptions require an explicit owner and falsifiable preclusion evidence"
    fi
}

print_summary() {
    phase_banner "Summary"
    log_info "Errors: $audit_errors | Warnings: $audit_warnings"
    if [[ "$SKIP_AUDIT" == "1" ]]; then
        log_success "Architecture audit skipped because no changed Rust files were detected"
        return 0
    fi
    if [[ $audit_errors -gt 0 ]]; then
        log_error "Audit failed"
        exit 1
    fi
    log_success "Audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    plan
    run_audit
    print_summary
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
    run_command_step "DEOS architecture audit" "" "$script_path" --internal "$@"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_entrypoint "$@"
fi
