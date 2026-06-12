#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-release-line.sh [OPTIONS]

Checks release-line consistency across CHANGELOG.md and package metadata.
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
    if [[ ! -f "$TEMPLATE_DIR/pallets/aaa/Cargo.toml" ]]; then
        log_error "pallet-aaa Cargo.toml not found"
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

extract_cargo_version() {
    local file="$1"
    awk '
        $0 == "[package]" { in_package = 1; next }
        /^\[/ && in_package { exit }
        in_package && /^version = / {
            gsub(/"/, "", $3)
            print $3
            exit
        }
    ' "$file"
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

    if [[ "$heading" == *AAA* || "$heading" == *aaa* ]]; then
        local aaa_cargo_version
        local aaa_lock_version
        aaa_cargo_version="$(extract_cargo_version "$TEMPLATE_DIR/pallets/aaa/Cargo.toml")"
        aaa_lock_version="$(extract_lock_package_version "pallet-aaa")"
        if [[ "$aaa_cargo_version" != "$latest_version" ]]; then
            log_error "pallet-aaa Cargo.toml version does not match latest AAA changelog release"
            echo "CHANGELOG: $latest_version"
            echo "Cargo.toml: $aaa_cargo_version"
            exit 1
        fi
        if [[ "$aaa_lock_version" != "$aaa_cargo_version" ]]; then
            log_error "pallet-aaa Cargo.lock version does not match Cargo.toml"
            echo "Cargo.toml: $aaa_cargo_version"
            echo "Cargo.lock: $aaa_lock_version"
            exit 1
        fi
    fi
    log_success "Release-line audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
