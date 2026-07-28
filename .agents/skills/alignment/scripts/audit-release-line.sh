#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-release-line.sh [OPTIONS]

Checks release-line consistency across CHANGELOG.md, the current framework
boundary, package metadata, and AAA package-owned documentation. The audit
prevents release fragmentation and stale specification identity/navigation.

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

extract_heading_title() {
    sed -E 's/^## [0-9]+\.[0-9]+\.[0-9]+: //'
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

check_exact_line() {
    local path="$1"
    local expected="$2"
    local drift_label="$3"
    if ! rg -Fqx -- "$expected" "$path"; then
        log_error "${drift_label}: ${path#$PROJECT_ROOT/} must contain ${expected}"
        exit 1
    fi
}

check_fixed_reference() {
    local path="$1"
    local expected="$2"
    local drift_label="$3"
    if ! rg -Fq -- "$expected" "$path"; then
        log_error "${drift_label}: ${path#$PROJECT_ROOT/} must reference ${expected}"
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
    local latest_title
    latest_version="$(printf '%s' "$heading" | extract_heading_version)"
    latest_title="$(printf '%s' "$heading" | extract_heading_title)"
    local duplicate_headings
    duplicate_headings="$(rg '^## [0-9]+\.[0-9]+\.[0-9]+:' "$PROJECT_ROOT/CHANGELOG.md" | sed -E 's/^## ([0-9]+\.[0-9]+\.[0-9]+):.*/\1/' | sort | uniq -d || true)"
    if [[ -n "$duplicate_headings" ]]; then
        log_error "Duplicate release headings found in CHANGELOG.md"
        echo "$duplicate_headings"
        exit 1
    fi
    check_changelog_order

    check_template_workspace_versions "$latest_version"
    check_exact_line "$TEMPLATE_DIR/pallets/router/Cargo.toml" "name = \"pallet-deos-router\"" "DEOS Router Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/pallets/asset-registry/Cargo.toml" "name = \"pallet-deos-asset-registry\"" "DEOS Asset Registry Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/primitives/Cargo.toml" "name = \"deos-primitives\"" "DEOS primitives Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/pallets/asset-registry/Cargo.toml" "name = \"pallet_asset_registry\"" "Asset Registry Rust-crate identity drift"
    check_exact_line "$TEMPLATE_DIR/primitives/Cargo.toml" "name = \"primitives\"" "Primitives Rust-crate identity drift"
    if rg -q 'pallet-deus-router' "$TEMPLATE_DIR" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/README.md" "$PROJECT_ROOT/AGENTS.md" "$PROJECT_ROOT/BACKLOG.md" --glob '!target/**'; then
        log_error "DEOS Router Cargo-package identity drift: legacy deus spelling remains"
        exit 1
    fi

    local aaa_spec="$TEMPLATE_DIR/pallets/aaa/docs/specification.en.md"
    check_markdown_release_marker "$aaa_spec" "Specification line" "$latest_version"
    check_exact_line "$aaa_spec" "- **Release focus**: ${latest_title}" "AAA release-focus drift"
    check_exact_line "$aaa_spec" "- **Source basis**: This accepted specification and the verified \`${latest_version}\` ${latest_title} implementation, generated evidence, and release-validation baseline." "AAA source-basis drift"
    check_markdown_release_marker "$TEMPLATE_DIR/pallets/aaa/docs/embedding.md" "Release line" "$latest_version"
    local package
    local integration_label
    for package in aaa oracle; do
        case "$package" in
            aaa) integration_label="AAA Integration in DEOS" ;;
            oracle) integration_label="DEOS Oracle Integration" ;;
        esac
        if [[ ! -f "$TEMPLATE_DIR/pallets/$package/docs/embedding.md" ]]; then
            log_error "Package embedding drift: missing canonical pallets/$package/docs/embedding.md"
            exit 1
        fi
        if [[ -e "$TEMPLATE_DIR/pallets/$package/EMBEDDING.md" ]]; then
            log_error "Package embedding drift: uppercase pallets/$package/EMBEDDING.md alias exists"
            exit 1
        fi
        if [[ ! -f "$PROJECT_ROOT/docs/$package.integration.en.md" ]]; then
            log_error "Integration-document drift: missing docs/$package.integration.en.md"
            exit 1
        fi
        check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[$integration_label](./$package.integration.en.md)" "$package integration-navigation drift"
    done

    local forbidden
    for forbidden in TmctolGenesisSystemAaas TmctolAssetOps AaaSystemExecutionReserve "Canonical Address Catalog" Production-Wasm FEE_SINK_AAA_ID MAX_SYSTEM_REFERENCE_AGE_BLOCKS; do
        if grep -Fq "$forbidden" "$TEMPLATE_DIR/pallets/aaa/docs/architecture.en.md"; then
            log_error "AAA package-purity drift: concrete integration marker remains: $forbidden"
            exit 1
        fi
    done
    for forbidden in "pallet index \`52\`" AaaObservationChangeIngress "Production Weight Evidence" "DEOS Router publishes" "Axial Router publishes"; do
        if grep -Fq "$forbidden" "$TEMPLATE_DIR/pallets/oracle/docs/architecture.en.md"; then
            log_error "Oracle package-purity drift: concrete integration marker remains: $forbidden"
            exit 1
        fi
    done
    for forbidden in "System AAA #" "| Role | aaa_id |" "AAA #0" "AAA #2"; do
        if grep -Fq "$forbidden" "$PROJECT_ROOT/docs/core.architecture.en.md"; then
            log_error "Core-architecture ownership drift: concrete System topology remains: $forbidden"
            exit 1
        fi
    done
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/aaa-system.en.md" "../../docs/aaa.integration.en.md" "AAA wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/aaa-system.en.md" "../../docs/oracle.integration.en.md" "Oracle wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.en.md" "../../docs/oracle.integration.en.md" "Oracle wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.en.md" "canonical_page_id: router" "DEOS Router canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.ru.md" "canonical_page_id: router" "DEOS Router canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/index.en.md" "[DEOS Router](overview/router.en.md)" "DEOS Router wiki-entrypoint drift"
    if [[ -e "$PROJECT_ROOT/wiki/overview/axial-router.en.md" || -e "$PROJECT_ROOT/wiki/overview/axial-router.ru.md" ]]; then
        log_error "DEOS Router wiki-identity drift: legacy Axial Router page exists"
        exit 1
    fi
    if rg -q 'Axial Router|axial-router' "$PROJECT_ROOT/wiki"; then
        log_error "DEOS Router wiki-identity drift: stale terminology remains"
        exit 1
    fi
    if rg -q 'Axial Router|axial-router' "$PROJECT_ROOT/README.md" "$PROJECT_ROOT/docs" "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client" --glob '*.md' --glob '*.rs' --glob '*.ts' --glob '*.svelte' --glob '*.mjs'; then
        log_error "DEOS Router public-terminology drift: stale Axial Router prose remains"
        exit 1
    fi
    if [[ ! -f "$PROJECT_ROOT/wiki/overview/governance.en.md" || ! -f "$PROJECT_ROOT/wiki/overview/governance.ru.md" || -e "$PROJECT_ROOT/wiki/overview/deos-governance.en.md" || -e "$PROJECT_ROOT/wiki/overview/governance-overview.en.md" ]]; then
        log_error "Governance wiki-owner drift"
        exit 1
    fi
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/governance.en.md" "canonical_page_id: governance" "Governance canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/_meta/locales.json" '"governance"' "Governance locale-metadata drift"
    if [[ ! -f "$PROJECT_ROOT/wiki/overview/staking.en.md" || ! -f "$PROJECT_ROOT/wiki/overview/staking.ru.md" || -e "$PROJECT_ROOT/wiki/overview/deos-staking.en.md" || -e "$PROJECT_ROOT/wiki/concepts/staking-pools.en.md" ]]; then
        log_error "Staking wiki-owner drift"
        exit 1
    fi
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/staking.en.md" "canonical_page_id: staking" "Staking canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/_meta/locales.json" '"staking"' "Staking locale-metadata drift"
    if [[ ! -f "$PROJECT_ROOT/wiki/overview/typed-observations.en.md" || ! -f "$PROJECT_ROOT/wiki/overview/typed-observations.ru.md" || -e "$PROJECT_ROOT/wiki/overview/deos-oracle.en.md" ]]; then
        log_error "Typed-observations wiki-owner drift"
        exit 1
    fi
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/typed-observations.en.md" "canonical_page_id: typed-observations" "Typed-observations canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/_meta/locales.json" '"typed-observations"' "Typed-observations locale-metadata drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/index.en.md" "[Typed Observations](overview/typed-observations.en.md)" "Typed-observations wiki-entrypoint drift"
    for retired_wiki_page in ui-kit-and-domain-dag.en.md ui-kit-and-domain-dag.ru.md what-deos-is-not.en.md what-deos-is-not.ru.md; do
        if [[ -e "$PROJECT_ROOT/wiki/concepts/$retired_wiki_page" ]]; then
            log_error "Wiki focus drift: retired concept page exists: $retired_wiki_page"
            exit 1
        fi
    done
    if rg -q '"(ui-kit-and-domain-dag|what-deos-is-not)"' "$PROJECT_ROOT/wiki/_meta" --glob '!aliases.json'; then
        log_error "Wiki focus drift: retired concept id remains in active metadata"
        exit 1
    fi
    check_fixed_reference "$TEMPLATE_DIR/pallets/oracle/README.md" "# pallet-deos-oracle" "DEOS Oracle package-title drift"
    check_fixed_reference "$TEMPLATE_DIR/pallets/oracle/docs/specification.en.md" "# DEOS Oracle Specification" "DEOS Oracle specification-title drift"
    check_fixed_reference "$TEMPLATE_DIR/pallets/staking/README.md" "# pallet-deos-staking" "DEOS Staking package-title drift"
    check_fixed_reference "$TEMPLATE_DIR/pallets/staking/docs/specification.en.md" "# DEOS Staking Specification:" "DEOS Staking specification-title drift"
    check_fixed_reference "$PROJECT_ROOT/web-client/src/lib/widgets/StakingWidget.svelte" 'title="DEOS Staking"' "DEOS Staking client-title drift"
    if rg -q 'Typed Observation Oracle|Oracle Integration in DEOS|\[Staking (Specification|Architecture|README)|title="Staking"|# pallet-oracle|# pallet-staking|# Staking Specification|# Staking:' "$PROJECT_ROOT/README.md" "$PROJECT_ROOT/docs" "$TEMPLATE_DIR" "$PROJECT_ROOT/web-client" --glob '*.md' --glob '*.svelte'; then
        log_error "Generic subsystem public-terminology drift"
        exit 1
    fi

    check_exact_line "$PROJECT_ROOT/BACKLOG.md" "> Release boundary: \`DEOS ${latest_version} — ${latest_title}\` is the current framework line. Completed semantics and release evidence live in \`CHANGELOG.md\` and the owning DEOS Oracle, DEOS Router, AAA, control-plane, and architecture documents." "Current framework-boundary drift"
    check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[AAA Specification](../template/pallets/aaa/docs/specification.en.md)" "AAA package-navigation drift"
    check_fixed_reference "$TEMPLATE_DIR/pallets/aaa/README.md" "[AAA Specification](./docs/specification.en.md)" "AAA package-navigation drift"
    check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[Web Client Architecture](../web-client/docs/architecture.en.md)" "Web-client architecture-navigation drift"
    check_fixed_reference "$PROJECT_ROOT/web-client/README.md" "[\`docs/architecture.en.md\`](./docs/architecture.en.md)" "Web-client architecture-navigation drift"
    if [[ ! -f "$PROJECT_ROOT/web-client/docs/architecture.en.md" || -e "$PROJECT_ROOT/docs/web-client.architecture.en.md" ]]; then
        log_error "Web-client architecture ownership drift"
        exit 1
    fi
    if [[ -e "$PROJECT_ROOT/docs/aaa.specification.en.md" ]]; then
        log_error "AAA package-navigation drift: obsolete root specification path exists"
        exit 1
    fi
    log_success "Release-line audit passed"
}

main() {
    parse_args "$@"
    check_prerequisites
    run_audit
}

main "$@"
