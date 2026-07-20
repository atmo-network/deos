#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WIKI_DIR="$PROJECT_ROOT/wiki"
MIN_BODY_LINES="${MIN_BODY_LINES:-18}"
LOW_CONFIDENCE_THRESHOLD="${LOW_CONFIDENCE_THRESHOLD:-0.85}"

usage() {
    cat <<'EOF'
Usage: audit-wiki-consolidation.sh [OPTIONS]

Audits generated wiki pages for consolidation pressure, confidence drift, and
low-signal leaflet decay. Hard failures cover missing or inconsistent metadata,
missing/dead provenance, invalid confidence bands, missing locale mirrors, or no
navigation/graph path. Short, low-confidence, source-stale, and graph-leaf pages
are reported as candidates only.

Options:
  --wiki-dir <path>  Override the wiki directory (default: ./wiki)
  --wiki-dir=<path> Override the wiki directory (default: ./wiki)
  --min-body-lines <n>  Candidate threshold for short pages (default: 18)
  --low-confidence-threshold <n>  Inclusive candidate threshold for low-confidence pages (default: 0.85)
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
            --wiki-dir=*)
                WIKI_DIR="${1#--wiki-dir=}"
                if [[ -z "$WIKI_DIR" ]]; then
                    log_error "Missing value for --wiki-dir"
                    usage
                    exit 1
                fi
                ;;
            --min-body-lines)
                if [[ $# -lt 2 ]]; then
                    log_error "Missing value for --min-body-lines"
                    usage
                    exit 1
                fi
                MIN_BODY_LINES="$2"
                shift
                ;;
            --low-confidence-threshold)
                if [[ $# -lt 2 ]]; then
                    log_error "Missing value for --low-confidence-threshold"
                    usage
                    exit 1
                fi
                LOW_CONFIDENCE_THRESHOLD="$2"
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
    phase_banner "Step 1: Wiki consolidation prerequisites"
    hydrate_local_tool_paths
    require_commands git python3
    if [[ ! -d "$WIKI_DIR" ]]; then
        log_error "Wiki directory not found: $WIKI_DIR"
        exit 1
    fi
    log_info "Wiki directory: $WIKI_DIR"
    log_info "Short-page candidate threshold: $MIN_BODY_LINES non-empty body lines"
    log_info "Low-confidence candidate threshold: $LOW_CONFIDENCE_THRESHOLD"
}

run_audit() {
    phase_banner "Step 2: Wiki consolidation guard"
    if ! python3 - "$WIKI_DIR" "$MIN_BODY_LINES" "$LOW_CONFIDENCE_THRESHOLD" "$PROJECT_ROOT" <<'PY'
from datetime import date
from pathlib import Path
import json
import re
import subprocess
import sys

wiki_dir = Path(sys.argv[1]).resolve()
min_body_lines = int(sys.argv[2])
low_confidence_threshold = float(sys.argv[3])
project_root = Path(sys.argv[4]).resolve()
failures = []
warnings = []
pages = {}
locales_by_id = {}
confidence_by_id = {}
confidence_by_locale = {}
source_date_cache = {}

required_fields = [
    "page_type",
    "title",
    "summary",
    "locale",
    "canonical_page_id",
    "last_compiled",
    "confidence",
]

def list_field(frontmatter, field):
    values = []
    active = False
    for line in frontmatter.splitlines():
        if line == f"{field}:":
            active = True
            continue
        if not active:
            continue
        if line.startswith("  - "):
            values.append(line[4:].strip())
            continue
        if line and not line.startswith(" "):
            break
    return values


def source_commit_date(source_path):
    source_path = source_path.resolve()
    if source_path in source_date_cache:
        return source_date_cache[source_path]
    try:
        relative = source_path.relative_to(project_root)
    except ValueError:
        source_date_cache[source_path] = None
        return None
    result = subprocess.run(
        ["git", "-C", str(project_root), "log", "-1", "--format=%cs", "--", str(relative)],
        check=False,
        capture_output=True,
        text=True,
    )
    raw = result.stdout.strip()
    if result.returncode != 0 or not raw:
        source_date_cache[source_path] = None
        return None
    try:
        value = date.fromisoformat(raw)
    except ValueError:
        value = None
    source_date_cache[source_path] = value
    return value

for path in sorted(wiki_dir.rglob("*.md")):
    rel = path.relative_to(wiki_dir).as_posix()
    text = path.read_text(encoding="utf-8")
    if not rel.endswith(('.en.md', '.ru.md')):
        failures.append(f"{rel}: missing explicit locale suffix")
    if not text.startswith("---\n"):
        failures.append(f"{rel}: missing frontmatter")
        continue
    parts = text.split("---", 2)
    if len(parts) < 3:
        failures.append(f"{rel}: unterminated frontmatter")
        continue
    frontmatter = parts[1]
    body = parts[2]
    meta = {}
    for line in frontmatter.splitlines():
        if not line.strip() or line.lstrip().startswith('-'):
            continue
        if ':' in line:
            key, value = line.split(':', 1)
            meta[key.strip()] = value.strip()
    missing = [field for field in required_fields if not meta.get(field)]
    if missing:
        failures.append(f"{rel}: missing required metadata {', '.join(missing)}")
    cid = meta.get("canonical_page_id")
    locale = meta.get("locale")
    if not cid or not locale:
        continue
    page_key = (cid, locale)
    if page_key in pages:
        failures.append(f"{rel}: duplicate canonical page id + locale {cid}/{locale}")
    pages[page_key] = rel
    locales_by_id.setdefault(cid, set()).add(locale)

    source_values = list_field(frontmatter, "sources")
    if not source_values:
        failures.append(f"{rel}: missing or empty sources block")
    if cid != "index" and "\nrelated:" not in text:
        failures.append(f"{rel}: missing related block")

    compiled_raw = meta.get("last_compiled", "")
    try:
        compiled_date = date.fromisoformat(compiled_raw)
    except ValueError:
        compiled_date = None
        if compiled_raw:
            failures.append(f"{rel}: invalid last_compiled date {compiled_raw}")

    latest_source_date = None
    latest_source = None
    for source_value in source_values:
        source_path = Path(source_value)
        if not source_path.is_absolute():
            source_path = path.parent / source_path
        source_path = source_path.resolve()
        if not source_path.exists():
            failures.append(f"{rel}: missing source path {source_value}")
            continue
        commit_date = source_commit_date(source_path)
        if commit_date and (latest_source_date is None or commit_date > latest_source_date):
            latest_source_date = commit_date
            latest_source = source_value
    if compiled_date and latest_source_date and latest_source_date > compiled_date:
        warnings.append(
            f"{rel}: source-newer-than-page confidence review candidate "
            f"({latest_source} committed {latest_source_date.isoformat()} > {compiled_date.isoformat()})"
        )

    non_empty_body_lines = [line for line in body.splitlines() if line.strip()]
    if cid != "index" and len(non_empty_body_lines) < min_body_lines:
        warnings.append(f"{rel}: short-page consolidation candidate ({len(non_empty_body_lines)} body lines)")

    confidence = meta.get("confidence")
    if confidence:
        try:
            confidence_value = float(confidence)
        except ValueError:
            failures.append(f"{rel}: invalid confidence value {confidence}")
        else:
            if not 0.0 <= confidence_value <= 1.0:
                failures.append(f"{rel}: confidence must be between 0 and 1 ({confidence_value})")
            if abs(confidence_value * 20 - round(confidence_value * 20)) > 1e-9:
                failures.append(
                    f"{rel}: confidence must use conservative 0.05 bands ({confidence_value})"
                )
            confidence_by_locale.setdefault(cid, {})[locale] = confidence_value
            confidence_by_id[cid] = min(confidence_by_id.get(cid, confidence_value), confidence_value)
            if cid != "index" and confidence_value <= low_confidence_threshold:
                warnings.append(
                    f"{rel}: low-confidence consolidation candidate "
                    f"({confidence_value:.2f} <= {low_confidence_threshold:.2f})"
                )

for cid, locales in sorted(locales_by_id.items()):
    if cid == "index":
        continue
    missing = {"en", "ru"} - locales
    if missing:
        failures.append(f"{cid}: missing locale mirror(s): {', '.join(sorted(missing))}")

state_path = wiki_dir / "_meta" / "state.json"
if state_path.exists():
    state_pages = json.loads(state_path.read_text(encoding="utf-8")).get("pages", {})
    for cid, locale_confidences in sorted(confidence_by_locale.items()):
        if cid not in state_pages:
            failures.append(f"_meta/state.json: missing page {cid}")
            continue
        state_confidence = state_pages[cid].get("confidence")
        if not isinstance(state_confidence, (int, float)):
            failures.append(f"_meta/state.json: missing or invalid confidence for {cid}")
            continue
        expected_confidence = min(locale_confidences.values())
        if abs(float(state_confidence) - expected_confidence) > 1e-9:
            failures.append(
                f"_meta/state.json: confidence drift for {cid} "
                f"({float(state_confidence):.2f} != locale minimum {expected_confidence:.2f})"
            )
    extra_state_ids = set(state_pages) - set(locales_by_id)
    for cid in sorted(extra_state_ids):
        failures.append(f"_meta/state.json: stale page entry {cid}")
else:
    failures.append("_meta/state.json: missing page state metadata")

nav_ids = set()
nav_path = wiki_dir / "_meta" / "navigation.json"
if nav_path.exists():
    def walk(value):
        if isinstance(value, dict):
            if "id" in value and "path" in value:
                nav_ids.add(value["id"])
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)
    walk(json.loads(nav_path.read_text(encoding="utf-8")))
else:
    failures.append("_meta/navigation.json: missing navigation metadata")

graph_inbound = set()
graph_outbound = set()
graph_edges = []
graph_path = wiki_dir / "_meta" / "graph.json"
if graph_path.exists():
    graph = json.loads(graph_path.read_text(encoding="utf-8"))
    for edge in graph.get("edges", []):
        if "to" in edge:
            graph_inbound.add(edge["to"])
        if "from" in edge:
            graph_outbound.add(edge["from"])
        if "from" in edge and "to" in edge:
            graph_edges.append((edge["from"], edge["to"]))
else:
    failures.append("_meta/graph.json: missing graph metadata")

for cid in sorted(locales_by_id):
    if cid == "index":
        continue
    has_inbound = cid in nav_ids or cid in graph_inbound
    has_outbound = cid in graph_outbound
    if not has_inbound:
        failures.append(f"{cid}: no navigation item or graph inbound edge")
    if not has_outbound:
        warnings.append(f"{cid}: graph leaf candidate with no outbound edge")

low_confidence_nodes = {
    cid for cid, confidence in confidence_by_id.items()
    if confidence <= low_confidence_threshold and cid != "index"
}
adjacency = {cid: set() for cid in low_confidence_nodes}
for source, target in graph_edges:
    if source in low_confidence_nodes and target in low_confidence_nodes:
        adjacency[source].add(target)
        adjacency[target].add(source)
seen = set()
for cid in sorted(low_confidence_nodes):
    if cid in seen:
        continue
    stack = [cid]
    component = []
    seen.add(cid)
    while stack:
        current = stack.pop()
        component.append(current)
        for next_id in sorted(adjacency.get(current, set())):
            if next_id not in seen:
                seen.add(next_id)
                stack.append(next_id)
    if len(component) >= 2:
        names = ", ".join(sorted(component))
        warnings.append(f"low-confidence graph cluster candidate: {names}")

if warnings:
    print("Consolidation candidates:")
    for warning in warnings:
        print(f"[WARN] {warning}")
if failures:
    print("Structural wiki consolidation failures:")
    for failure in failures:
        print(f"[FAIL] {failure}")
    sys.exit(1)
print("Wiki consolidation guard passed")
PY
    then
        log_error "Wiki consolidation audit failed"
        exit 1
    fi
    log_success "Wiki consolidation audit passed"
}

main() {
    parse_args "$@"
    phase_banner "DEOS wiki consolidation audit"
    check_prerequisites
    run_audit
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
