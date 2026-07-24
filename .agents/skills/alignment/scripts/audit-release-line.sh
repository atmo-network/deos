#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-release-line.sh [OPTIONS]

Checks release-line consistency across CHANGELOG.md and template package metadata.
The audit prevents accidental release fragmentation such as adding a new
changelog heading while package markers still describe the prior active line.

Options:
  -h, --help  Show this help message
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
    if [[ ! -f "$PROJECT_ROOT/CHANGELOG.md" ]]; then
        log_error "CHANGELOG.md not found"
        exit 1
    fi
    if [[ ! -f "$TEMPLATE_DIR/runtime/Cargo.toml" ]]; then
        log_error "template runtime Cargo.toml not found"
        exit 1
    fi
    if [[ ! -f "$TEMPLATE_DIR/Cargo.lock" ]]; then
        log_error "template Cargo.lock not found"
        exit 1
    fi
    require_commands rg awk grep sed sort uniq
    log_success "Prerequisites checked"
}

latest_changelog_heading() {
    rg -m 1 '^## [0-9]+\.[0-9]+\.[0-9]+:' "$PROJECT_ROOT/CHANGELOG.md"
}

extract_heading_version() {
    sed -E 's/^## ([0-9]+\.[0-9]+\.[0-9]+):.*/\1/'
}

extract_cargo_field() {
    local file="$1"
    local field="$2"
    awk -v field="$field" '
        $0 == "[package]" { in_package = 1; next }
        /^\[/ && in_package { exit }
        in_package && $1 == field && $2 == "=" {
            value = $3
            gsub(/"/, "", value)
            print value
            exit
        }
    ' "$file"
}

extract_cargo_version() {
    local file="$1"
    extract_cargo_field "$file" "version"
}

extract_cargo_name() {
    local file="$1"
    extract_cargo_field "$file" "name"
}

extract_lock_package_version() {
    local package="$1"
    awk -v package="$package" '
        $0 == "[[package]]" { in_package = 1; name = ""; version = ""; next }
        in_package && /^name = / {
            name = $3
            gsub(/"/, "", name)
            next
        }
        in_package && /^version = / {
            version = $3
            gsub(/"/, "", version)
            if (name == package) {
                print version
                exit
            }
        }
    ' "$TEMPLATE_DIR/Cargo.lock"
}

check_template_package_version() {
    local latest_version="$1"
    local cargo_path="$2"
    local cargo_file="$TEMPLATE_DIR/$cargo_path"
    if [[ ! -f "$cargo_file" ]]; then
        log_error "Template package Cargo.toml not found: $cargo_path"
        exit 1
    fi
    local cargo_name
    local cargo_version
    local lock_version
    cargo_name="$(extract_cargo_name "$cargo_file")"
    cargo_version="$(extract_cargo_version "$cargo_file")"
    if [[ -z "$cargo_name" ]]; then
        log_error "Template package name missing from Cargo.toml: $cargo_path"
        exit 1
    fi
    lock_version="$(extract_lock_package_version "$cargo_name")"
    if [[ -z "$lock_version" ]]; then
        log_error "Template package missing from Cargo.lock: $cargo_name"
        exit 1
    fi
    if [[ "$cargo_version" != "$latest_version" ]]; then
        log_error "Template package version does not match latest changelog release"
        echo "Package: $cargo_path"
        echo "CHANGELOG: $latest_version"
        echo "Cargo.toml: $cargo_version"
        exit 1
    fi
    if [[ "$lock_version" != "$cargo_version" ]]; then
        log_error "Template package Cargo.lock version does not match Cargo.toml"
        echo "Package: $cargo_name"
        echo "Cargo.toml: $cargo_version"
        echo "Cargo.lock: $lock_version"
        exit 1
    fi
}

list_template_workspace_cargo_paths() {
    awk '
        /^members = \[/ { in_members = 1; line = $0 }
        in_members && $0 !~ /^members = \[/ { line = line " " $0 }
        in_members && /\]/ {
            sub(/^[^[]*\[/, "", line)
            sub(/\].*$/, "", line)
            count = split(line, members, ",")
            for (i = 1; i <= count; i++) {
                gsub(/[ \t"]/, "", members[i])
                if (members[i] != "") {
                    print members[i] "/Cargo.toml"
                }
            }
            exit
        }
    ' "$TEMPLATE_DIR/Cargo.toml"
}

check_template_workspace_versions() {
    local latest_version="$1"
    local cargo_paths
    cargo_paths="$(list_template_workspace_cargo_paths)"
    if [[ -z "$cargo_paths" ]]; then
        log_error "No template workspace members found in template/Cargo.toml"
        exit 1
    fi
    local cargo_path
    while IFS= read -r cargo_path; do
        [[ -z "$cargo_path" ]] && continue
        check_template_package_version "$latest_version" "$cargo_path"
    done <<< "$cargo_paths"
}

version_key() {
    local version="$1"
    local major minor patch
    IFS=. read -r major minor patch <<< "$version"
    printf '%09d%09d%09d' "$major" "$minor" "$patch"
}

check_changelog_order() {
    local previous_version=""
    local previous_key=""
    local line
    while IFS= read -r line; do
        local version
        local key
        version="$(printf '%s' "$line" | extract_heading_version)"
        key="$(version_key "$version")"
        if [[ -n "$previous_key" && "$key" > "$previous_key" ]]; then
            log_error "CHANGELOG.md release headings are not in descending order"
            echo "Previous: $previous_version"
            echo "Found later: $version"
            exit 1
        fi
        previous_version="$version"
        previous_key="$key"
    done < <(rg '^## [0-9]+\.[0-9]+\.[0-9]+:' "$PROJECT_ROOT/CHANGELOG.md")
}

check_markdown_release_marker() {
    local path="$1"
    local label="$2"
    local expected_version="$3"
    local expected="- **${label}**: \`${expected_version}\`"
    if ! rg -Fqx -- "$expected" "$path"; then
        log_error "Release marker drift: ${path#$PROJECT_ROOT/} must contain ${expected}"
        exit 1
    fi
}

run_audit() {
    phase_banner "Step 2: Release-line consistency"
    local heading
    heading="$(latest_changelog_heading)"
    if [[ -z "$heading" ]]; then
        log_error "No release heading found in CHANGELOG.md"
        exit 1
    fi
    local latest_version
    latest_version="$(printf '%s' "$heading" | extract_heading_version)"
    local duplicate_headings
    duplicate_headings="$(rg '^## [0-9]+\.[0-9]+\.[0-9]+:' "$PROJECT_ROOT/CHANGELOG.md" | sed -E 's/^## ([0-9]+\.[0-9]+\.[0-9]+):.*/\1/' | sort | uniq -d || true)"
    if [[ -n "$duplicate_headings" ]]; then
        log_error "Duplicate release headings found in CHANGELOG.md"
        echo "$duplicate_headings"
        exit 1
    fi
    check_changelog_order

    check_template_workspace_versions "$latest_version"
    check_markdown_release_marker "$PROJECT_ROOT/docs/aaa.specification.en.md" "Specification line" "$latest_version"
    check_markdown_release_marker "$TEMPLATE_DIR/pallets/aaa/EMBEDDING.md" "Release line" "$latest_version"
    log_success "Release-line audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
