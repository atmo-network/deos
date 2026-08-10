#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: audit-release-line.sh [OPTIONS]

Checks release-line consistency across bounded CHANGELOG.md records, package
metadata, the current framework boundary, and package-owned documentation. The audit prevents release fragmentation while preserving
standalone normative specification authority and stable package navigation.

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
    if [[ ! -f "$PROJECT_ROOT/scripts/03-build-runtime.sh" ]]; then
        log_error "Canonical runtime-build atom not found"
        exit 1
    fi
    require_commands rg awk grep head sed sort uniq jq
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

extract_workspace_package_field() {
    local field="$1"
    awk -v field="$field" '
        $0 == "[workspace.package]" { in_workspace_package = 1; next }
        /^\[/ && in_workspace_package { exit }
        in_workspace_package && $1 == field && $2 == "=" {
            value = $3
            gsub(/"/, "", value)
            print value
            exit
        }
    ' "$TEMPLATE_DIR/Cargo.toml"
}

extract_cargo_version() {
    local file="$1"
    if rg -Fqx 'version.workspace = true' "$file"; then
        extract_workspace_package_field "version"
        return
    fi
    extract_cargo_field "$file" "version"
}

extract_cargo_name() {
    local file="$1"
    extract_cargo_field "$file" "name"
}

extract_lock_package_versions() {
    local package="$1"
    awk -v package="$package" '
        $0 == "[[package]]" { in_package = 1; name = ""; next }
        in_package && /^name = / {
            name = $3
            gsub(/"/, "", name)
            next
        }
        in_package && /^version = / && name == package {
            version = $3
            gsub(/"/, "", version)
            print version
        }
    ' "$TEMPLATE_DIR/Cargo.lock"
}

extract_lock_package_version() {
    extract_lock_package_versions "$1" | head -n 1
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
    if ! rg -Fqx 'version.workspace = true' "$cargo_file"; then
        log_error "Template package version must inherit the workspace release authority"
        echo "Package: $cargo_path"
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

check_changelog_shape() {
    local violations
    violations="$(awk '
        function finish_section() {
            if (section != "" && records > 8) {
                printf "%s has %d records; maximum is 8\n", section, records
            }
        }
        /^## / {
            finish_section()
            section = $0
            records = 0
            next
        }
        section == "" { next }
        /^- / {
            records++
            if (length($0) > 512) {
                printf "%s line %d has %d characters; maximum is 512\n", section, NR, length($0)
            }
            next
        }
        /^[[:space:]]*$/ { next }
        {
            printf "%s line %d is not a single-line outcome record\n", section, NR
        }
        END { finish_section() }
    ' "$PROJECT_ROOT/CHANGELOG.md")"
    if [[ -n "$violations" ]]; then
        log_error "CHANGELOG.md exceeds the bounded history shape"
        printf '%s\n' "$violations"
        exit 1
    fi
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
    if [[ "$(grep -c '^\[\[relaychain\.nodes\]\]$' "$TEMPLATE_DIR/zombienet.toml")" -ne 2 || "$(grep -c '^\[\[parachains\.collators\]\]$' "$TEMPLATE_DIR/zombienet.toml")" -ne 2 ]]; then
        log_error "Assurance topology must declare exactly two relay validators and two collators"
        exit 1
    fi
    local runtime_build="$PROJECT_ROOT/scripts/03-build-runtime.sh"
    check_exact_line "$runtime_build" 'CANONICAL_BUILD_ROOT="/tmp/deos-runtime-production-source"' "Canonical runtime physical-root drift"
    check_exact_line "$runtime_build" '    export WASM_BUILD_TYPE=production' "Production Wasm-profile drift"
    check_exact_line "$runtime_build" '    export CARGO_INCREMENTAL=0' "Runtime incremental-build drift"
    check_exact_line "$runtime_build" '    trap cleanup_build_stage EXIT' "Canonical runtime cleanup-ownership drift"
    check_exact_line "$runtime_build" '    if ! mkdir "$CANONICAL_BUILD_ROOT"; then' "Canonical runtime concurrent-build guard drift"
    check_exact_line "$runtime_build" '    export WASM_BUILD_RUSTFLAGS="--remap-path-prefix=$BUILD_PROJECT_ROOT=/deos/source --remap-path-prefix=$cargo_home=/deos/cargo --remap-path-prefix=$rustup_home=/deos/rustup"' "Canonical runtime virtual-path drift"
    check_exact_line "$runtime_build" '    mv "$temporary_output" "$OUTPUT_WASM_PATH"' "Canonical runtime atomic-publication drift"
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
    check_changelog_shape

    local workspace_version
    workspace_version="$(extract_workspace_package_field version)"
    if [[ -z "$workspace_version" ]]; then
        log_error "Template workspace release authority is missing"
        exit 1
    fi
    local prepared_version="$workspace_version"
    local unreleased_count
    unreleased_count="$(grep -c '^## Unreleased$' "$PROJECT_ROOT/CHANGELOG.md" || true)"
    if [[ "$unreleased_count" -gt 1 ]]; then
        log_error "CHANGELOG.md contains more than one Unreleased section"
        exit 1
    fi
    if [[ "$unreleased_count" == "1" ]]; then
        local prepared_key latest_key
        prepared_key="$(version_key "$prepared_version")"
        latest_key="$(version_key "$latest_version")"
        if [[ "$prepared_key" == "$latest_key" || "$prepared_key" < "$latest_key" ]]; then
            log_error "Unreleased workspace version must be newer than the latest historical release"
            echo "Workspace: $prepared_version"
            echo "Historical: $latest_version"
            exit 1
        fi
    elif [[ "$prepared_version" != "$latest_version" ]]; then
        log_error "Finalized workspace release authority does not match latest changelog release"
        echo "Workspace: $prepared_version"
        echo "CHANGELOG: $latest_version"
        exit 1
    fi
    check_template_workspace_versions "$prepared_version"
    check_exact_line "$PROJECT_ROOT/web-client/package.json" "  \"version\": \"${prepared_version}\"," "Web-client package-version drift"
    check_exact_line "$PROJECT_ROOT/web-client/package-lock.json" "  \"version\": \"${prepared_version}\"," "Web-client lockfile-version drift"
    check_exact_line "$PROJECT_ROOT/web-client/package-lock.json" "      \"version\": \"${prepared_version}\"," "Web-client root-package lockfile drift"
    check_exact_line "$TEMPLATE_DIR/pallets/router/Cargo.toml" "name = \"pallet-deos-router\"" "DEOS Router Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/pallets/asset-registry/Cargo.toml" "name = \"pallet-deos-asset-registry\"" "DEOS Asset Registry Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/primitives/Cargo.toml" "name = \"deos-primitives\"" "DEOS primitives Cargo-package identity drift"
    check_exact_line "$TEMPLATE_DIR/pallets/asset-registry/Cargo.toml" "name = \"pallet_asset_registry\"" "Asset Registry Rust-crate identity drift"
    check_exact_line "$TEMPLATE_DIR/primitives/Cargo.toml" "name = \"primitives\"" "Primitives Rust-crate identity drift"
    if rg -q 'pallet-deus-router' "$TEMPLATE_DIR" "$PROJECT_ROOT/docs" "$PROJECT_ROOT/README.md" "$PROJECT_ROOT/AGENTS.md" "$PROJECT_ROOT/BACKLOG.md" --glob '!target/**'; then
        log_error "DEOS Router Cargo-package identity drift: legacy deus spelling remains"
        exit 1
    fi

    local actor_spec="$TEMPLATE_DIR/pallets/actors/docs/specification.en.md"
    check_exact_line "$actor_spec" "- **Scope**: Bounded economic actor runtime contract" "Actors specification-scope drift"
    check_exact_line "$actor_spec" "- **Status**: Normative" "Actors specification-status drift"
    if rg -q '^- \*\*(Specification line|Release focus|Source basis)\*\*:' "$actor_spec"; then
        log_error "Actors specification purity drift: implementation markers must remain outside the normative specification"
        exit 1
    fi
    if tail -n +2 "$actor_spec" | rg -q '\b(DEOS|TMCTOL)\b'; then
        log_error "Actors specification purity drift: DEOS product/runtime framing remains in the standalone contract"
        exit 1
    fi
    check_markdown_release_marker "$TEMPLATE_DIR/pallets/actors/docs/embedding.md" "Release line" "$prepared_version"
    local package
    local integration_doc
    local integration_label
    for package in actors oracle; do
        case "$package" in
            actors)
                integration_doc="actors"
                integration_label="DEOS Actors Integration"
                ;;
            oracle)
                integration_doc="oracle"
                integration_label="DEOS Oracle Integration"
                ;;
        esac
        if [[ ! -f "$TEMPLATE_DIR/pallets/$package/docs/embedding.md" ]]; then
            log_error "Package embedding drift: missing canonical pallets/$package/docs/embedding.md"
            exit 1
        fi
        if [[ -e "$TEMPLATE_DIR/pallets/$package/EMBEDDING.md" ]]; then
            log_error "Package embedding drift: uppercase pallets/$package/EMBEDDING.md alias exists"
            exit 1
        fi
        if [[ ! -f "$PROJECT_ROOT/docs/$integration_doc.integration.en.md" ]]; then
            log_error "Integration-document drift: missing docs/$integration_doc.integration.en.md"
            exit 1
        fi
        check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[$integration_label](./$integration_doc.integration.en.md)" "$package integration-navigation drift"
    done

    local forbidden
    for forbidden in TmctolGenesisSystemActors TmctolAssetOps ActorSystemExecutionReserve "Canonical Address Catalog" Production-Wasm FEE_SINK_ACTORS_ID MAX_SYSTEM_REFERENCE_AGE_BLOCKS; do
        if grep -Fq "$forbidden" "$TEMPLATE_DIR/pallets/actors/docs/architecture.en.md"; then
            log_error "Actors package-purity drift: concrete integration marker remains: $forbidden"
            exit 1
        fi
    done
    for forbidden in "pallet index \`52\`" ActorObservationChangeIngress "Production Weight Evidence" "DEOS Router publishes" "DEOS Router publishes"; do
        if grep -Fq "$forbidden" "$TEMPLATE_DIR/pallets/oracle/docs/architecture.en.md"; then
            log_error "Oracle package-purity drift: concrete integration marker remains: $forbidden"
            exit 1
        fi
    done
    for forbidden in "System Actors #" "| Role | actor_id |" "Actors #0" "Actors #2"; do
        if grep -Fq "$forbidden" "$PROJECT_ROOT/docs/core.architecture.en.md"; then
            log_error "Core-architecture ownership drift: concrete System topology remains: $forbidden"
            exit 1
        fi
    done
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/actor-system.en.md" "../../docs/actors.integration.en.md" "Actors wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/actor-system.en.md" "../../docs/oracle.integration.en.md" "Oracle wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.en.md" "../../docs/oracle.integration.en.md" "Oracle wiki-provenance drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.en.md" "canonical_page_id: router" "DEOS Router canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/overview/router.ru.md" "canonical_page_id: router" "DEOS Router canonical-id drift"
    check_fixed_reference "$PROJECT_ROOT/wiki/index.en.md" "[DEOS Router](overview/router.en.md)" "DEOS Router wiki-entrypoint drift"
    if ! "$SCRIPT_DIR/audit-router-identity.sh"; then
        log_error "DEOS Router identity drift"
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

    check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[DEOS Actors Specification](../template/pallets/actors/docs/specification.en.md)" "Actors package-navigation drift"
    check_fixed_reference "$TEMPLATE_DIR/pallets/actors/README.md" "[DEOS Actors Specification](./docs/specification.en.md)" "Actors package-navigation drift"
    check_fixed_reference "$PROJECT_ROOT/docs/README.md" "[Web Client Architecture](../web-client/docs/architecture.en.md)" "Web-client architecture-navigation drift"
    check_fixed_reference "$PROJECT_ROOT/web-client/README.md" "[\`docs/architecture.en.md\`](./docs/architecture.en.md)" "Web-client architecture-navigation drift"
    if [[ ! -f "$PROJECT_ROOT/web-client/docs/architecture.en.md" || -e "$PROJECT_ROOT/docs/web-client.architecture.en.md" ]]; then
        log_error "Web-client architecture ownership drift"
        exit 1
    fi
    if [[ -e "$PROJECT_ROOT/docs/actor.specification.en.md" ]]; then
        log_error "Actors package-navigation drift: obsolete root specification path exists"
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
