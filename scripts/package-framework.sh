#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

usage() {
    cat <<'EOF'
Usage: package-framework.sh --source COMMIT --output NEW_DIRECTORY

Package the selected committed framework source. No working-tree, generated
Wasm/metadata, installed tools, caches or Git state enter the payload. Run
validate-local.sh framework and actual-Wasm export against a fresh unpacking;
the manifest and checksums establish identity, not acceptance.

Inputs: Git checkout, a source commit, pinned Node, GNU tar/gzip/coreutils.
Outputs: source archive, contents.json and SHA256SUMS in a new directory.
Side effects: writes only beneath the requested new output directory.
EOF
}

SOURCE=""
OUTPUT=""
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --source|--output)
                [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || { usage; return 1; }
                case "$1" in
                    --source) [[ -z "$SOURCE" ]] || return 1; SOURCE="$2" ;;
                    --output) [[ -z "$OUTPUT" ]] || return 1; OUTPUT="$2" ;;
                esac
                shift 2 ;;
            -h|--help) usage; exit 0 ;;
            *) usage; return 1 ;;
        esac
    done
    [[ -n "$SOURCE" && -n "$OUTPUT" ]] || { usage; return 1; }
}

main() {
    parse_args "$@"
    require_commands git tar gzip sha256sum awk node
    local commit tree version name epoch node_pin
    commit="$(git -C "$PROJECT_ROOT" rev-parse --verify --end-of-options "$SOURCE^{commit}")"
    node_pin="$(git -C "$PROJECT_ROOT" show "$commit:.node-version")"
    [[ "$(node --version)" == "v$node_pin" ]] || { log_error "Select Node $node_pin before packaging"; return 1; }
    tree="$(git -C "$PROJECT_ROOT" rev-parse "$commit^{tree}")"
    version="$(git -C "$PROJECT_ROOT" show "$commit:template/Cargo.toml" | awk -F'"' '/^version = / {print $2}')"
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { log_error "Invalid framework version"; return 1; }
    name="deos-$version-framework"
    epoch="$(git -C "$PROJECT_ROOT" show -s --format=%ct "$commit")"
    # Cargo sources and fixtures are included as complete trees: examples,
    # workspace targets and patch packages are not inferred from crate names.
    local -a include=(
        .node-version .npm-version .gitignore LICENSE README.md CHANGELOG.md notices
        docs/README.md docs/deos
        template/README.md template/Cargo.toml template/Cargo.lock template/rust-toolchain.toml template/rustfmt.toml
        template/primitives template/pallets template/runtime template/vendor template/tools/README.md simulators
        scripts/README.md scripts/_common.sh scripts/inputs scripts/01-download-binaries.sh
        scripts/02-install-tools.sh scripts/03-build-runtime.sh scripts/04-generate-chain-spec.sh
        scripts/audit-asset-conversion-boundaries.sh scripts/build-corrected-node.sh scripts/build-corrected-node.mjs
        scripts/ci-local.sh scripts/export-papi-metadata.sh scripts/package-framework.sh scripts/framework-contents.mjs
        scripts/setup-environment.sh scripts/validate-local.sh
    )
    mkdir -- "$OUTPUT"
    OUTPUT="$(cd -- "$OUTPUT" && pwd -P)"
    (
        cd "$OUTPUT"
        trap 'rm -rf -- stage' EXIT
        mkdir stage
        git -C "$PROJECT_ROOT" archive --format=tar --prefix="$name/" "$commit" -- "${include[@]}" | tar -xf - -C stage
        node "stage/$name/scripts/framework-contents.mjs" "stage/$name" "$version" "$commit" "$tree" >contents.json
        tar --format=gnu --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner -cf - -C stage "$name" |
            gzip -n >"$name.tar.gz"
        sha256sum "$name.tar.gz" contents.json >SHA256SUMS
        sha256sum --check SHA256SUMS
    )
    log_success "Framework source payload: $OUTPUT/$name.tar.gz ($commit, tree $tree)"
}

main "$@"
