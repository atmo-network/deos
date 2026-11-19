#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SITE_DIR="$WIKI_ROOT/site"
TOOL_DIR="$SITE_DIR/target/bin"
MODE=""
STAGE=""
MDBOOK="${MDBOOK:-$TOOL_DIR/mdbook}"
VERSION=""

usage() {
    cat <<'EOF'
Usage: build-site.sh [--prepare | --build | --validate | --self-test]

Build the optional standalone Wiki artifact with pinned mdBook.
This pipeline serves the project Wiki validation profile and Pages workflow; framework source
validation does not call it. The static output requires no build tooling at runtime.
Default: --build. Run from any working directory.

Modes:
  --prepare    Install locked website npm dependencies and checksum-pinned mdBook
               under site/target/bin (network; Linux x86_64/aarch64, macOS x86_64/arm64).
  --build      Compile reviewed OKF/graph data to Markdown/SUMMARY, then render
               with mdBook and prepare native Unicode/URL/layer search in site/dist.
               No download or deployment.
  --validate   Validate OKF, trust and tool regressions, build once with structural output
               checks, then verify the shipped native search data and destinations on the built
               site. Runs on Node only: no browser, server or network. This is the gate that
               publication depends on.
  --self-test  Run projection/tool regression tests and fail-closed CLI checks;
               no download, mdBook build, server or publication.
  -h, --help   Show this help.

Inputs: wiki/layers/, site/mdbook.json, site source/theme and the Wiki npm lockfile.
Outputs: ignored site/target and site/dist; --prepare installs site/target/bin/mdbook.
A failed build retains the previous dist. No wallet, chain, account, DNS or hosting mutation.
Environment: MDBOOK selects an existing executable with the exact pinned version;
WIKI_BASE_PATH sets the public URL prefix the site is served under (default /,
e.g. /repository/ for a GitHub Pages project site); WIKI_SITE_URL, the absolute
public URL of that prefix, adds canonical/hreflang links and sitemap.xml;
DEOS_VERBOSE=1 shows child output.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --prepare|--build|--validate|--self-test)
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

check_base_path() {
    WIKI_BASE_PATH="${WIKI_BASE_PATH:-/}"
    [[ "$WIKI_BASE_PATH" =~ ^/([A-Za-z0-9_~-][A-Za-z0-9._~-]*/)*$ ]] || {
        log_error "WIKI_BASE_PATH must be / or an absolute /segment/ prefix ending in /"; exit 1;
    }
    export WIKI_BASE_PATH
}

check_prerequisites() {
    check_base_path
    if [[ "$MODE" == prepare ]]; then
        activate_pinned_node
        activate_pinned_npm
    fi
    require_commands node npm
    require_directory "$SITE_DIR" "Wiki website"
    VERSION="$(node "$SITE_DIR/scripts/toolchain.mjs" --version)"
    if [[ "$MODE" == prepare ]]; then
        require_commands curl tar uname
        return
    fi
    [[ -d "$WIKI_ROOT/node_modules/yaml" ]] || { log_error "Wiki dependencies missing; run wiki/scripts/build-site.sh --prepare"; exit 1; }
    if [[ "$MODE" == self-test ]]; then return; fi
    [[ -x "$MDBOOK" ]] || { log_error "mdBook $VERSION missing; run wiki/scripts/build-site.sh --prepare or set MDBOOK"; exit 1; }
    [[ "$("$MDBOOK" --version)" == "mdbook v$VERSION" ]] || { log_error "mdBook version must be exactly $VERSION"; exit 1; }
    [[ ! -L "$SITE_DIR/dist" && ( ! -e "$SITE_DIR/dist" || -d "$SITE_DIR/dist" ) ]] || { log_error "Refusing a symlink or non-directory dist"; exit 1; }
}

prepare() {
    local platform url expected actual
    platform="$(uname -s)-$(uname -m)"
    url="$(node "$SITE_DIR/scripts/toolchain.mjs" --url "$platform")"
    expected="$(node "$SITE_DIR/scripts/toolchain.mjs" --sha256 "$platform")"
    run_command_step "Locked Wiki dependencies" "" npm ci --prefix "$WIKI_ROOT" --ignore-scripts --no-audit --no-fund
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
    local invalid
    for invalid in deos deos/ /deos //deos/ /../ /a/./ '/a b/'; do
        if WIKI_BASE_PATH="$invalid" bash "$SCRIPT_DIR/build-site.sh" --build >/dev/null 2>&1; then log_error "Invalid base path accepted: $invalid"; exit 1; fi
    done
    for invalid in 'ftp://example.org/' 'https://example.org/other/' 'https://example.org/?x=1' 'not a url'; do
        if (cd "$SITE_DIR" && WIKI_SITE_URL="$invalid" node --input-type=module -e "import { siteUrlFor } from './scripts/prepare-meta.mjs'; siteUrlFor(process.env.WIKI_SITE_URL, '/');") >/dev/null 2>&1; then
            log_error "Invalid site URL accepted: $invalid"; exit 1
        fi
    done
}

self_test() {
    run_command_step "Wiki projection and tool tests" "" npm run site:test --prefix "$WIKI_ROOT"
    run_command_step "Pages publication self-test" "" bash "$SCRIPT_DIR/publish-pages.sh" --self-test
    check_cli_refusals
    log_success "Wiki entrypoint self-test passed"
}

build() {
    local locales locale
    mkdir -p "$SITE_DIR/target"
    STAGE="$(mktemp -d "$SITE_DIR/target/build.XXXXXX")"
    run_command_step "Compile unified Wiki graph to Markdown" "" node "$SITE_DIR/scripts/compile.mjs" --out "$STAGE"
    locales="$(node "$SITE_DIR/scripts/compile.mjs" --locales "$STAGE/projection.json")"
    run_command_step "Render Wiki site root and 404" "" "$MDBOOK" build "$STAGE/books/root" --dest-dir "$STAGE/html"
    # The site root opens the reader's language directly; it is not a language chooser.
    cp "$STAGE/site-index.html" "$STAGE/html/index.html"
    while IFS= read -r locale; do
        run_command_step "Render Wiki locale $locale" "" "$MDBOOK" build "$STAGE/books/$locale" --dest-dir "$STAGE/html/$locale"
        run_command_step "Prepare native Unicode/layer search $locale" "" node "$SITE_DIR/scripts/prepare-search.mjs" "$STAGE/html/$locale" "$STAGE/projection.json" "$locale"
    done <<< "$locales"
    cp "$STAGE/projection.json" "$STAGE/html/graph.json"
    run_command_step "Prepare crawler metadata" "" node "$SITE_DIR/scripts/prepare-meta.mjs" "$STAGE/html" "$STAGE/projection.json"
    run_command_step "Validate native mdBook destinations" "" node "$SITE_DIR/scripts/check-output.mjs" "$STAGE/html"
    if [[ -d "$SITE_DIR/dist" ]]; then mv "$SITE_DIR/dist" "$STAGE/previous"; fi
    mv "$STAGE/html" "$SITE_DIR/dist"
    log_success "Static mdBook website for $WIKI_BASE_PATH: $SITE_DIR/dist"
}

static_test() {
    run_command_step "Native search data and destinations at $WIKI_BASE_PATH" "" node --test "$SITE_DIR/tests/native-search.test.mjs"
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
        validate)
            run_command_step "Wiki corpus, trust and tool regressions" "" bash "$SCRIPT_DIR/validate-wiki-trust.sh"
            check_cli_refusals
            build
            static_test
            ;;
        build) build ;;
    esac
}

main "$@"
