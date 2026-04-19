#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

RUN_BUILD="${RUN_BUILD:-0}"
CHUNK_WARN_KB="${CHUNK_WARN_KB:-500}"
TOP_N="${TOP_N:-15}"
CLIENT_OUTPUT_DIR="${CLIENT_OUTPUT_DIR:-$PROJECT_ROOT/web-client/.svelte-kit/output/client}"
MANIFEST_PATH="${MANIFEST_PATH:-$CLIENT_OUTPUT_DIR/.vite/manifest.json}"

usage() {
    cat <<'EOF'
Usage: 10-web-client-bundle-report.sh [OPTIONS]

Reports the largest generated web-client bundle artifacts and highlights chunks
that exceed a warning threshold.

Options:
  --build     Run `npm run build` before reporting
  -h, --help  Show this help message

Environment:
  RUN_BUILD=0|1
  CHUNK_WARN_KB=500
  TOP_N=15
  CLIENT_OUTPUT_DIR=<path>
  MANIFEST_PATH=<path>
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --build)
                RUN_BUILD=1
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
    phase_banner "Step 1: Prerequisites"
    require_directory "$PROJECT_ROOT/web-client" "web-client workspace"
    require_commands find sort awk numfmt python3
    log_success "Bundle report prerequisites checked"
}

build_if_requested() {
    if (( RUN_BUILD == 0 )); then
        log_warning "Skipping build step"
        return
    fi
    phase_banner "Step 2: Build web-client"
    (cd "$PROJECT_ROOT/web-client" && npm run build)
}

annotate_top_assets() {
    local report_file="$1"
    if [[ ! -f "$MANIFEST_PATH" ]]; then
        log_warning "Skipping manifest annotations because manifest was not found at $MANIFEST_PATH"
        return
    fi

    echo
    log_info "Manifest annotations for top $TOP_N assets"
    REPORT_FILE="$report_file" MANIFEST_PATH="$MANIFEST_PATH" TOP_N="$TOP_N" python3 - <<'PY'
import json
import os
from pathlib import Path

manifest = json.loads(Path(os.environ["MANIFEST_PATH"]).read_text())
entries_by_file = {}
importers_by_file = {}
dynamic_importers_by_file = {}
for key, value in manifest.items():
    file_name = value.get("file")
    if not file_name:
        continue
    entries_by_file.setdefault(file_name, []).append((key, value))
for importer_key, importer_value in manifest.items():
    for imported_key in importer_value.get("imports") or []:
        imported_file = manifest.get(imported_key, {}).get("file")
        if not imported_file:
            continue
        importers_by_file.setdefault(imported_file, []).append((importer_key, importer_value))
    for imported_key in importer_value.get("dynamicImports") or []:
        imported_file = manifest.get(imported_key, {}).get("file")
        if not imported_file:
            continue
        dynamic_importers_by_file.setdefault(imported_file, []).append((importer_key, importer_value))

with Path(os.environ["REPORT_FILE"]).open() as handle:
    for index, raw_line in enumerate(handle):
        if index >= int(os.environ["TOP_N"]):
            break
        _, path = raw_line.strip().split(" ", 1)
        annotations = entries_by_file.get(path)
        if not annotations:
            continue
        print(f"  {path}")
        for key, value in annotations:
            flags = []
            if value.get("isEntry"):
                flags.append("entry")
            if value.get("isDynamicEntry"):
                flags.append("dynamic")
            flag_suffix = f" [{' '.join(flags)}]" if flags else ""
            src = value.get("src") or "—"
            name = value.get("name") or key
            print(f"    -> {name}{flag_suffix}")
            print(f"       key: {key}")
            print(f"       src: {src}")
            static_imports = value.get("imports") or []
            if static_imports:
                print(f"       imports: {len(static_imports)}")
                for imported_key in static_imports[:5]:
                    imported_value = manifest.get(imported_key, {})
                    imported_name = imported_value.get('name') or imported_key
                    imported_src = imported_value.get('src') or '—'
                    imported_file = imported_value.get('file') or '—'
                    print(f"         - {imported_name} :: {imported_src} :: {imported_file}")
                if len(static_imports) > 5:
                    print(f"         - ... and {len(static_imports) - 5} more")
            dynamic_imports = value.get("dynamicImports") or []
            if dynamic_imports:
                print(f"       dynamically imports: {len(dynamic_imports)}")
                for imported_key in dynamic_imports[:5]:
                    imported_value = manifest.get(imported_key, {})
                    imported_name = imported_value.get('name') or imported_key
                    imported_src = imported_value.get('src') or '—'
                    imported_file = imported_value.get('file') or '—'
                    print(f"         - {imported_name} :: {imported_src} :: {imported_file}")
                if len(dynamic_imports) > 5:
                    print(f"         - ... and {len(dynamic_imports) - 5} more")
            importers = importers_by_file.get(path, [])
            if importers:
                print(f"       imported by: {len(importers)}")
                for importer_key, importer_value in importers[:5]:
                    importer_name = importer_value.get('name') or importer_key
                    importer_src = importer_value.get('src') or '—'
                    print(f"         - {importer_name} :: {importer_src}")
                if len(importers) > 5:
                    print(f"         - ... and {len(importers) - 5} more")
            dynamic_importers = dynamic_importers_by_file.get(path, [])
            if dynamic_importers:
                print(f"       dynamically imported by: {len(dynamic_importers)}")
                for importer_key, importer_value in dynamic_importers[:5]:
                    importer_name = importer_value.get('name') or importer_key
                    importer_src = importer_value.get('src') or '—'
                    print(f"         - {importer_name} :: {importer_src}")
                if len(dynamic_importers) > 5:
                    print(f"         - ... and {len(dynamic_importers) - 5} more")
PY
}

report_bundle() {
    phase_banner "Step 3: Bundle report"
    require_directory "$CLIENT_OUTPUT_DIR" "Client output directory"

    local report_file
    report_file="$(mktemp)"

    find "$CLIENT_OUTPUT_DIR" -type f \( -name '*.js' -o -name '*.css' \) -printf '%s %P\n' \
        | sort -nr > "$report_file"

    log_info "Top $TOP_N emitted client assets"
    head -n "$TOP_N" "$report_file" | while read -r size path; do
        printf '  %8s  %s\n' "$(numfmt --to=iec --suffix=B "$size")" "$path"
    done

    annotate_top_assets "$report_file"

    echo
    log_info "Chunks above ${CHUNK_WARN_KB} KiB"
    local warned=0
    while read -r size path; do
        local kib
        kib=$(( (size + 1023) / 1024 ))
        if (( kib > CHUNK_WARN_KB )); then
            warned=1
            printf '  %8s  %s\n' "$(numfmt --to=iec --suffix=B "$size")" "$path"
        fi
    done < "$report_file"

    if (( warned == 0 )); then
        log_success "No client assets exceed ${CHUNK_WARN_KB} KiB"
    else
        log_warning "Some client assets exceed ${CHUNK_WARN_KB} KiB"
    fi

    rm -f "$report_file"
}

main() {
    parse_args "$@"
    phase_banner "DEOS web-client bundle report"
    log_info "Plan: run_build=$RUN_BUILD chunk_warn_kb=$CHUNK_WARN_KB top_n=$TOP_N"
    check_prerequisites
    build_if_requested
    report_bundle
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
