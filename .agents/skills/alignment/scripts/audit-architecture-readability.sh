#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

MAX_BLOCK_CHARS="${DEOS_ARCHITECTURE_BLOCK_CHARS:-600}"

usage() {
    cat <<'EOF'
Usage: audit-architecture-readability.sh [ARCHITECTURE_FILE...]

Rejects oversized prose paragraphs, list items, and table rows in English
architecture Markdown. Fenced code, headings, and generated HTML are excluded.
The default maximum semantic block length is 600 characters.

With no arguments, audits docs/*.architecture.en.md.

Options:
  -h, --help  Show this help message

Environment:
  DEOS_ARCHITECTURE_BLOCK_CHARS=N
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
            -*)
                log_error "Unknown argument: $1"
                usage
                exit 2
                ;;
            *) FILES+=("$1") ;;
        esac
        shift
    done
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands awk find sort
    [[ "$MAX_BLOCK_CHARS" =~ ^[1-9][0-9]*$ ]] || {
        log_error "DEOS_ARCHITECTURE_BLOCK_CHARS must be a positive integer"
        exit 2
    }
    if (( ${#FILES[@]} == 0 )); then
        mapfile -t FILES < <(find "$PROJECT_ROOT/docs" -maxdepth 1 -type f -name '*.architecture.en.md' -print | sort)
    fi
    local file
    for file in "${FILES[@]}"; do
        [[ "$file" = /* ]] || file="$PROJECT_ROOT/$file"
        [[ -f "$file" ]] || { log_error "Architecture file not found: $file"; exit 1; }
    done
    log_success "Prerequisites checked"
}

run_audit() {
    phase_banner "Step 2: Architecture readability"
    local failed=0 file path display
    for file in "${FILES[@]}"; do
        if [[ "$file" = /* ]]; then
            path="$file"
            display="${file#"$PROJECT_ROOT/"}"
        else
            path="$PROJECT_ROOT/$file"
            display="$file"
        fi
        if ! awk -v max="$MAX_BLOCK_CHARS" -v display="$display" '
            function flush() {
                if (chars > max) {
                    printf "%s:%d: semantic prose block has %d characters (maximum %d)\n", display, start, chars, max
                    bad = 1
                }
                chars = 0
                start = 0
                kind = ""
            }
            function append(line) {
                if (chars == 0) start = FNR
                chars += length(line) + (chars > 0 ? 1 : 0)
            }
            /^```/ { flush(); fenced = !fenced; next }
            fenced { next }
            /^[[:space:]]*$/ { flush(); next }
            /^#/ || /^>/ || /^<!--/ || /^-->/ { flush(); next }
            /^[[:space:]]*\|/ {
                flush()
                if (length($0) > max) {
                    printf "%s:%d: table row has %d characters (maximum %d)\n", display, FNR, length($0), max
                    bad = 1
                }
                next
            }
            /^[[:space:]]*([-*+] |[0-9]+\. )/ {
                flush()
                kind = "list"
                append($0)
                next
            }
            {
                if (kind == "list" && $0 !~ /^[[:space:]]+/) flush()
                append($0)
            }
            END { flush(); exit bad }
        ' "$path"; then
            failed=1
        fi
    done
    if (( failed != 0 )); then
        log_error "Architecture readability audit failed"
        exit 1
    fi
    log_success "Architecture readability audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
