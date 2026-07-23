#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-script-entrypoints.sh [OPTIONS]

Checks repository shell/Node entrypoints for syntax validity, --help availability,
shared shell-step status propagation, declared root-to-skill bridges, and compact
skill metadata shape. Also enforces that project-specific audit leaves live in the
alignment skill, not in the root operator scripts directory.

Options:
  -h, --help        Show this help message

Scope:
  root scripts/*.sh except _common.sh
  web-client/scripts/*.mjs
  .agents/skills/*/scripts/*.sh except _common.sh
  .agents/skills/*/SKILL.md frontmatter description lines
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
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
    require_directory "$ROOT_SCRIPT_DIR" "Root scripts directory"
    require_directory "$SCRIPT_DIR" "Alignment skill scripts directory"
    require_commands bash find sort basename node awk grep
    log_success "Prerequisites checked"
}

audit_shell_entrypoints() {
    local script
    while IFS= read -r script; do
        if [[ "$(basename "$script")" == "_common.sh" ]]; then
            continue
        fi

        if [[ "$DEOS_VERBOSE" == "1" ]]; then
            log_info "Checking ${script#$PROJECT_ROOT/}"
        fi
        if ! bash -n "$script"; then
            log_error "Syntax check failed: $script"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
            continue
        fi

        if ! bash "$script" --help >/dev/null; then
            log_error "--help failed: $script"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
        fi
    done < <(
        {
            find "$ROOT_SCRIPT_DIR" -maxdepth 1 -type f -name '*.sh'
            if [[ -d "$PROJECT_ROOT/.agents/skills" ]]; then
                find "$PROJECT_ROOT/.agents/skills" -path '*/scripts/*.sh' -type f
            fi
        } | sort -u
    )
}

audit_node_entrypoints() {
    local script
    if [[ ! -d "$PROJECT_ROOT/web-client/scripts" ]]; then
        return
    fi
    while IFS= read -r script; do
        if [[ "$DEOS_VERBOSE" == "1" ]]; then
            log_info "Checking ${script#$PROJECT_ROOT/}"
        fi
        if ! node --check "$script" >/dev/null; then
            log_error "Syntax check failed: $script"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
            continue
        fi

        if ! node "$script" --help >/dev/null; then
            log_error "--help failed: $script"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
        fi
    done < <(find "$PROJECT_ROOT/web-client/scripts" -maxdepth 1 -type f -name '*.mjs' | sort)
}

audit_audit_leaf_ownership() {
    local root_audit_scripts
    root_audit_scripts="$(find "$ROOT_SCRIPT_DIR" -maxdepth 1 -type f -name 'audit-*.sh' | sort || true)"
    if [[ -n "$root_audit_scripts" ]]; then
        log_error "Project-specific audit leaves must live in .agents/skills/alignment/scripts, not root scripts/"
        echo "$root_audit_scripts"
        AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
    fi
}

audit_shell_step_status_propagation() {
    local output
    local status
    if output="$(run_shell_step "Expected failure probe" "" "exit 23" 2>&1)"; then
        log_error "run_shell_step converted a failing command into success"
        AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
        return
    else
        status=$?
    fi
    if [[ "$status" -ne 23 ]]; then
        log_error "run_shell_step returned status $status instead of the command status 23"
        printf '%s\n' "$output"
        AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
    fi
    if ! run_shell_step "Expected success probe" "" "exit 0" >/dev/null; then
        log_error "run_shell_step rejected a successful command"
        AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
    fi
}

audit_skill_bridges() {
    local bridge owner entrypoint implementation
    while IFS= read -r bridge; do
        owner="$(awk -F': ' '/^# Skill-Owner: / { print $2; exit }' "$bridge")"
        entrypoint="$(awk -F': ' '/^# Skill-Entrypoint: / { print $2; exit }' "$bridge")"
        if [[ -z "$entrypoint" ]]; then
            log_error "Skill bridge lacks Skill-Entrypoint metadata: ${bridge#$PROJECT_ROOT/}"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
            continue
        fi
        implementation="$PROJECT_ROOT/.agents/skills/$owner/$entrypoint"
        if [[ ! -x "$implementation" || ! -f "$PROJECT_ROOT/.agents/skills/$owner/SKILL.md" ]]; then
            log_error "Skill bridge owner or executable is missing: ${bridge#$PROJECT_ROOT/} -> $owner/$entrypoint"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
            continue
        fi
        if ! grep -Fq ".agents/skills/$owner/$entrypoint" "$bridge"; then
            log_error "Skill bridge does not delegate to its declared entrypoint: ${bridge#$PROJECT_ROOT/}"
            AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
        fi
    done < <(grep -l '^# Skill-Owner: ' "$ROOT_SCRIPT_DIR"/*.sh || true)
}

audit_skill_metadata_descriptions() {
    local skill_file
    local matches=""
    if [[ ! -d "$PROJECT_ROOT/.agents/skills" ]]; then
        return
    fi
    while IFS= read -r skill_file; do
        local file_matches
        file_matches="$(awk -v file="$skill_file" '
            NR == 1 && $0 == "---" { in_frontmatter = 1; next }
            in_frontmatter && $0 == "---" { exit }
            in_frontmatter && /^description:[^:]*:/ {
                print file ":" NR ":" $0
            }
        ' "$skill_file")"
        if [[ -n "$file_matches" ]]; then
            matches+="$file_matches"$'\n'
        fi
    done < <(find "$PROJECT_ROOT/.agents/skills" -name SKILL.md -type f | sort)
    if [[ -n "$matches" ]]; then
        log_error "Skill description metadata must not contain extra value-side colons"
        printf '%s' "$matches"
        AUDIT_FAILURES=$((AUDIT_FAILURES + 1))
    fi
}

audit_entrypoints() {
    phase_banner "Step 2: Script entrypoints"

    AUDIT_FAILURES=0
    audit_shell_entrypoints
    audit_node_entrypoints
    audit_audit_leaf_ownership
    audit_shell_step_status_propagation
    audit_skill_bridges
    audit_skill_metadata_descriptions

    if [[ "$AUDIT_FAILURES" -gt 0 ]]; then
        log_error "Script entrypoint audit failed with $AUDIT_FAILURES failure(s)"
        exit 1
    fi

    log_success "All script entrypoints passed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS script entrypoint audit"
    check_prerequisites
    audit_entrypoints
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
