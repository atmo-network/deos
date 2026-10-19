#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SITE_DIR="$SKILL_DIR/site"
TOOL_DIR="$SITE_DIR/target/bin"
MODE=""
STAGE=""
MDBOOK="${MDBOOK:-$TOOL_DIR/mdbook}"
VERSION=""

usage() {
    cat <<'EOF'
Usage: build-site.sh [--prepare | --build | --validate | --browser-test | --self-test]

Build this Skill's optional standalone Wiki artifact with pinned mdBook.
The published static website needs no Skill runtime; project build/CI never calls this pipeline.
Default: --build. Run from any working directory.

Modes:
  --prepare    Install locked website npm dependencies and checksum-pinned mdBook
               under site/target/bin (network; Linux x86_64/aarch64, macOS x86_64/arm64).
  --build      Compile reviewed OKF/graph data to Markdown/SUMMARY, then render
               with mdBook and prepare native Unicode/URL/layer search in site/dist.
               No download or deployment.
  --validate   Run deterministic Wiki/projection/URL/context/locale tests, then build.
  --browser-test  Build, then check scoped native search data and real EN/RU
                  search/keyboard/graph/locale/layer and native-control journeys,
                  including doubled-default-text-size reflow (not browser zoom).
                  CHROME_BIN may select an existing Chrome; no security bypass flags.
  --self-test  Run projection/tool regression tests and fail-closed CLI checks;
               no download, mdBook build, server or publication.
  -h, --help   Show this help.

Inputs: project wiki/, site/mdbook.json, site source/theme and the Skill npm lockfile.
Outputs: ignored site/target and site/dist; --prepare installs site/target/bin/mdbook.
A failed build retains the previous dist. No wallet, chain, account, DNS or hosting mutation.
Environment: MDBOOK selects an existing executable with the exact pinned version;
DEOS_VERBOSE=1 shows child output.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --prepare|--build|--validate|--browser-test|--self-test)
                [[ -z "$MODE" ]] || { log_error "Choose exactly one Wiki mode"; exit 1; }
                MODE="${1#--}" ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown Wiki option: $1"; usage; exit 1 ;;
        esac
        shift
    done
    MODE="${MODE:-build}"
}

cleanup() {
    local status=$?
    if [[ -n "$STAGE" && -d "$STAGE" ]]; then
        if [[ -d "$STAGE/previous" && ! -e "$SITE_DIR/dist" ]]; then
            if ! mv "$STAGE/previous" "$SITE_DIR/dist"; then
                log_error "Previous Wiki output retained at $STAGE/previous; automatic restore failed"
                exit 1
            fi
        fi
        rm -rf "$STAGE"
    fi
    exit "$status"
}

check_prerequisites() {
    require_commands node npm
    require_directory "$SITE_DIR" "Wiki website"
    VERSION="$(node "$SITE_DIR/scripts/toolchain.mjs" --version)"
    if [[ "$MODE" == prepare ]]; then
        require_commands curl tar uname
        return
    fi
    [[ -d "$SKILL_DIR/node_modules/yaml" ]] || { log_error "Wiki dependencies missing; run npm run site:prepare --prefix .agents/skills/wiki-sync"; exit 1; }
    if [[ "$MODE" == self-test ]]; then return; fi
    [[ -x "$MDBOOK" ]] || { log_error "mdBook $VERSION missing; run this Skill's site:prepare or set MDBOOK"; exit 1; }
    [[ "$("$MDBOOK" --version)" == "mdbook v$VERSION" ]] || { log_error "mdBook version must be exactly $VERSION"; exit 1; }
    [[ ! -L "$SITE_DIR/dist" && ( ! -e "$SITE_DIR/dist" || -d "$SITE_DIR/dist" ) ]] || { log_error "Refusing a symlink or non-directory dist"; exit 1; }
}

prepare() {
    local platform url expected actual
    platform="$(uname -s)-$(uname -m)"
    url="$(node "$SITE_DIR/scripts/toolchain.mjs" --url "$platform")"
    expected="$(node "$SITE_DIR/scripts/toolchain.mjs" --sha256 "$platform")"
    run_command_step "Locked Wiki dependencies" "" npm ci --prefix "$SKILL_DIR" --ignore-scripts --no-audit --no-fund
    if [[ -x "$TOOL_DIR/mdbook" && "$("$TOOL_DIR/mdbook" --version)" == "mdbook v$VERSION" ]]; then
        log_success "Pinned mdBook $VERSION already installed"
        return
    fi
    [[ ! -L "$TOOL_DIR/mdbook" ]] || { log_error "Refusing a symlink tool destination"; exit 1; }
    STAGE="$(mktemp -d "${TMPDIR:-/tmp}/deos-mdbook.XXXXXX")"
    run_command_step "Download pinned mdBook $VERSION" "" curl --fail --silent --show-error --location --max-time 120 --output "$STAGE/mdbook.tar.gz" "$url"
    actual="$(deos_sha256_file "$STAGE/mdbook.tar.gz")"
    [[ "$actual" == "$expected" ]] || { log_error "mdBook archive SHA-256 mismatch"; exit 1; }
    tar -xzf "$STAGE/mdbook.tar.gz" -C "$STAGE" mdbook
    [[ "$("$STAGE/mdbook" --version)" == "mdbook v$VERSION" ]] || { log_error "Downloaded mdBook identity mismatch"; exit 1; }
    mkdir -p "$TOOL_DIR"
    chmod 0755 "$STAGE/mdbook"
    mv "$STAGE/mdbook" "$TOOL_DIR/mdbook"
    log_success "Installed checksum-verified mdBook $VERSION"
}

check_cli_refusals() {
    if bash "$SCRIPT_DIR/build-site.sh" --unknown >/dev/null 2>&1; then log_error "Unknown option was accepted"; exit 1; fi
    if bash "$SCRIPT_DIR/build-site.sh" --build --prepare >/dev/null 2>&1; then log_error "Conflicting modes were accepted"; exit 1; fi
    if MDBOOK=/nonexistent/deos-mdbook bash "$SCRIPT_DIR/build-site.sh" --build >/dev/null 2>&1; then log_error "Missing required renderer was accepted"; exit 1; fi
}

self_test() {
    run_command_step "Wiki projection and tool tests" "" npm run site:test --prefix "$SKILL_DIR"
    check_cli_refusals
    log_success "Wiki entrypoint self-test passed"
}

build() {
    local locales locale
    mkdir -p "$SITE_DIR/target"
    STAGE="$(mktemp -d "$SITE_DIR/target/build.XXXXXX")"
    run_command_step "Compile unified Wiki graph to Markdown" "" node "$SITE_DIR/scripts/compile.mjs" --out "$STAGE"
    locales="$(node "$SITE_DIR/scripts/compile.mjs" --locales "$STAGE/projection.json")"
    run_command_step "Render Wiki language entrypoint" "" "$MDBOOK" build "$STAGE/books/root" --dest-dir "$STAGE/html"
    while IFS= read -r locale; do
        run_command_step "Render Wiki locale $locale" "" "$MDBOOK" build "$STAGE/books/$locale" --dest-dir "$STAGE/html/$locale"
        run_command_step "Prepare native Unicode/layer search $locale" "" node "$SITE_DIR/scripts/prepare-search.mjs" "$STAGE/html/$locale" "$STAGE/projection.json" "$locale"
    done <<< "$locales"
    cp "$STAGE/projection.json" "$STAGE/html/graph.json"
    run_command_step "Validate native mdBook destinations" "" node "$SITE_DIR/scripts/check-output.mjs" "$STAGE/html"
    if [[ -d "$SITE_DIR/dist" ]]; then mv "$SITE_DIR/dist" "$STAGE/previous"; fi
    mv "$STAGE/html" "$SITE_DIR/dist"
    log_success "Static mdBook website: $SITE_DIR/dist"
}

main() {
    parse_args "$@"
    check_prerequisites
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    case "$MODE" in
        prepare) prepare ;;
        self-test) self_test ;;
        validate) run_command_step "Wiki and projection regressions" "" npm test --prefix "$SKILL_DIR"; check_cli_refusals; build ;;
        browser-test) build; run_command_step "Native search and mdBook browser journeys" "" node --test "$SITE_DIR/test/native-search.test.mjs" "$SITE_DIR/test/browser.test.mjs" "$SITE_DIR/test/layer-browser.test.mjs" "$SITE_DIR/test/reader-browser.test.mjs" ;;
        build) build ;;
    esac
}

main "$@"
