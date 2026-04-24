#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WIKI_DIR="$PROJECT_ROOT/wiki"

usage() {
    cat <<'EOF'
Usage: validate-wiki-trust.sh [OPTIONS]

Validate the trusted wiki-markdown contract used by the browser-side renderer.

Checks:
  - no raw HTML tag blocks in wiki markdown
  - no dangerous URL schemes such as javascript: or data:text/html
  - no inline DOM event handler attributes in markdown
  - no extra value-side colons in wiki frontmatter key-value lines

Options:
  --wiki-dir <path>  Override the wiki directory (default: ./wiki)
  -h, --help         Show this help message
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --wiki-dir)
                if [[ $# -lt 2 ]]; then
                    log_error "Missing value for --wiki-dir"
                    usage
                    exit 1
                fi
                WIKI_DIR="$2"
                shift
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
    phase_banner "Step 1: Trusted wiki validation prerequisites"
    hydrate_local_tool_paths
    require_commands rg python3
    if [[ ! -d "$WIKI_DIR" ]]; then
        log_error "Wiki directory not found: $WIKI_DIR"
        exit 1
    fi
    log_info "Wiki directory: $WIKI_DIR"
}

check_raw_html() {
    if rg -n '^\s*<([A-Za-z][A-Za-z0-9-]*)(\s|>|/)' "$WIKI_DIR" --glob '*.md'; then
        log_error "Raw HTML tag blocks are not allowed in trusted wiki markdown"
        exit 1
    fi
    if rg -n '<(script|iframe|object|embed|link|style)\b' "$WIKI_DIR" --glob '*.md'; then
        log_error "Executable or embedded HTML tags are not allowed in trusted wiki markdown"
        exit 1
    fi
}

check_dangerous_urls() {
    if rg -ni 'javascript:|vbscript:|data:text/html' "$WIKI_DIR" --glob '*.md'; then
        log_error "Dangerous URL schemes are not allowed in trusted wiki markdown"
        exit 1
    fi
}

check_inline_handlers() {
    if rg -ni '\bon[a-z]+\s*=' "$WIKI_DIR" --glob '*.md'; then
        log_error "Inline DOM event handler attributes are not allowed in trusted wiki markdown"
        exit 1
    fi
}

check_frontmatter_colons() {
    if ! python3 - "$WIKI_DIR" <<'PY'
from pathlib import Path
import sys

wiki_dir = Path(sys.argv[1])
violations = []
for path in sorted(wiki_dir.rglob("*.md")):
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0].strip() != "---":
        continue
    for index, line in enumerate(lines[1:], 2):
        if line.strip() == "---":
            break
        if not line.strip() or line.lstrip().startswith("-"):
            continue
        if ":" in line and line.count(":") > 1:
            violations.append(f"{path}:{index}:{line}")
if violations:
    print("\n".join(violations))
    sys.exit(1)
PY
    then
        log_error "Wiki frontmatter key-value lines must not contain extra value-side colons"
        exit 1
    fi
}

main() {
    parse_args "$@"
    phase_banner "DEOS trusted wiki markdown validation"
    check_prerequisites
    phase_banner "Step 2: Markdown trust contract"
    check_raw_html
    check_dangerous_urls
    check_inline_handlers
    check_frontmatter_colons
    log_success "Trusted wiki markdown validation passed"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
