#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-markdown-tables.sh [MARKDOWN_FILE...]

Checks repository-owned Markdown table rows for compact readable syntax:
exactly one padding space inside every cell boundary, exactly three delimiter
hyphens, and only optional alignment colons.

With no file arguments, audits every existing tracked or unignored untracked
Markdown file in the repository; paths deleted by the current diff are ignored.

Options:
  -h, --help  Show this help message
EOF
}

parse_args() {
    FILES=()
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                exit 0
                ;;
            -* )
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
            *)
                FILES+=("$1")
                ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands git awk
    if (( ${#FILES[@]} == 0 )); then
        local candidates=()
        mapfile -d '' candidates < <(git -C "$PROJECT_ROOT" ls-files -co --exclude-standard -z -- '*.md')
        local candidate
        for candidate in "${candidates[@]}"; do
            [[ -f "$PROJECT_ROOT/$candidate" ]] && FILES+=("$candidate")
        done
    fi
    local file
    for file in "${FILES[@]}"; do
        if [[ "$file" = /* ]]; then
            [[ -f "$file" ]] || { log_error "Markdown file not found: $file"; exit 1; }
        else
            [[ -f "$PROJECT_ROOT/$file" ]] || { log_error "Markdown file not found: $file"; exit 1; }
        fi
        [[ "$file" == *.md ]] || { log_error "Expected a Markdown file: $file"; exit 1; }
    done
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Compact Markdown tables"
    local failed=0
    local file path
    for file in "${FILES[@]}"; do
        if [[ "$file" = /* ]]; then
            path="$file"
        else
            path="$PROJECT_ROOT/$file"
        fi
        if ! awk -v display="$file" '
            /^[[:space:]]*\|/ {
                line = $0
                if (line ~ /^[[:space:]]+\|/ || line !~ /^\| .* \|$/) {
                    print display ":" FNR ": non-compact table row: " line
                    bad = 1
                    next
                }
                count = split(line, cells, "|")
                for (i = 2; i < count; i++) {
                    cell = cells[i]
                    content = substr(cell, 2, length(cell) - 2)
                    if (cell !~ /^ .* $/ || content ~ /^ / || content ~ / $/) {
                        print display ":" FNR ": table cell must use exactly one padding space: " cell
                        bad = 1
                    }
                }
                probe = line
                gsub(/[|: -]/, "", probe)
                if (probe == "") {
                    for (i = 2; i < count; i++) {
                        content = substr(cells[i], 2, length(cells[i]) - 2)
                        if (content != "---" && content != ":---" && content != "---:" && content != ":---:") {
                            print display ":" FNR ": non-compact delimiter cell: " cells[i]
                            bad = 1
                        }
                    }
                }
            }
            END { exit bad }
        ' "$path"; then
            failed=1
        fi
    done
    if (( failed != 0 )); then
        log_error "Markdown table compaction audit failed"
        exit 1
    fi
    log_success "Markdown table compaction audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
