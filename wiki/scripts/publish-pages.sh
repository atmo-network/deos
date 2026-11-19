#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SITE_DIST="$WIKI_ROOT/site/dist"
BRANCH="gh-pages"
REMOTE="origin"
PUSH=0
SELF_TEST=0
EXPECT_BASE=""
CNAME=""
STAGE=""

usage() {
    cat <<'EOF'
Usage: publish-pages.sh [--push] [--expect-base PATH] [--cname DOMAIN]
                        [--branch NAME] [--remote NAME] | --self-test | --help

Publish the already built static Wiki (wiki/site/dist) to a static-hosting
branch, gh-pages by default, for GitHub Pages "deploy from a branch".

The artifact is re-validated, then committed through a temporary index in the
project's object store. The working tree, index and HEAD are untouched. The
new commit sits on top of the current remote branch tip, so branch history is
kept and no force-push is needed. An unchanged site publishes nothing.
.nojekyll is added so GitHub Pages serves the artifact verbatim.

Options:
  --push               Push the commit (default: dry run, report only).
  --expect-base PATH   Refuse an artifact built for another WIKI_BASE_PATH.
  --cname DOMAIN       Add a CNAME file for a custom Pages domain.
  --branch NAME        Target branch (default: gh-pages).
  --remote NAME        Target remote (default: origin).
  --self-test          Exercise publication against a temporary bare remote;
                       no network, build or project mutation.
  -h, --help           Show this help.

Inputs: site/dist from build-site.sh, the project Git repository and remote.
Outputs: at most one new commit on the remote branch (with --push only).
Side effects: writes Git objects to the project repository; with --push,
updates the remote branch. No build, DNS or hosting-settings change.
Environment: WIKI_PAGES_AUTHOR_NAME / WIKI_PAGES_AUTHOR_EMAIL set the commit
identity (default: the GitHub Actions bot).
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --push) PUSH=1 ;;
            --self-test) SELF_TEST=1 ;;
            --expect-base|--cname|--branch|--remote)
                [[ $# -ge 2 && -n "$2" ]] || { log_error "Missing value for $1"; exit 1; }
                case "$1" in
                    --expect-base) EXPECT_BASE="$2" ;;
                    --cname) CNAME="$2" ;;
                    --branch) BRANCH="$2" ;;
                    --remote) REMOTE="$2" ;;
                esac
                shift ;;
            -h|--help) usage; exit 0 ;;
            *) log_error "Unknown publication option: $1"; usage; exit 1 ;;
        esac
        shift
    done
    if [[ "$SELF_TEST" == 1 && "$PUSH" == 1 ]]; then log_error "--self-test never pushes"; exit 1; fi
    [[ -z "$CNAME" || "$CNAME" =~ ^([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}$ ]] || { log_error "Invalid --cname domain"; exit 1; }
    git check-ref-format --branch "$BRANCH" >/dev/null 2>&1 || { log_error "Invalid --branch name"; exit 1; }
}

cleanup() {
    local status=$?
    if [[ -n "$STAGE" && -d "$STAGE" ]]; then rm -rf "$STAGE"; fi
    exit "$status"
}

# publish GIT_DIR DIST REMOTE BRANCH PUSH: prints the published commit or "unchanged".
publish() {
    local git_dir="$1" dist="$2" remote="$3" branch="$4" push="$5"
    local base tip="" parent_args=() tree commit source author_name author_email index
    [[ -d "$dist" && ! -L "$dist" ]] || { log_error "Built site missing: $dist (run build-site.sh first)"; return 1; }
    node "$WIKI_ROOT/site/scripts/check-output.mjs" "$dist" >/dev/null || { log_error "Built site failed output validation"; return 1; }
    base="$(node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).base_path ?? "/")' "$dist/graph.json")"
    if [[ -n "$EXPECT_BASE" && "$base" != "$EXPECT_BASE" ]]; then
        log_error "Site was built for base path $base, expected $EXPECT_BASE"; return 1
    fi

    local work="$STAGE/publish.$RANDOM"
    mkdir -p "$work"
    cp -R "$dist/." "$work/"
    : >"$work/.nojekyll"
    if [[ -n "$CNAME" ]]; then printf '%s\n' "$CNAME" >"$work/CNAME"; fi

    tip="$(git --git-dir="$git_dir" ls-remote --heads "$remote" "refs/heads/$branch" | awk '{print $1}')"
    if [[ -n "$tip" ]]; then
        # A shallow fetch would mark a full developer clone shallow; fetch the tip only when absent.
        git --git-dir="$git_dir" cat-file -e "$tip^{commit}" 2>/dev/null ||
            git --git-dir="$git_dir" fetch --quiet --no-tags "$remote" "refs/heads/$branch" >/dev/null 2>&1
        parent_args=(-p "$tip")
    fi
    index="$STAGE/index.$RANDOM"
    GIT_INDEX_FILE="$index" git --git-dir="$git_dir" --work-tree="$work" add --all --force .
    tree="$(GIT_INDEX_FILE="$index" git --git-dir="$git_dir" write-tree)"
    if [[ -n "$tip" && "$(git --git-dir="$git_dir" rev-parse "$tip^{tree}")" == "$tree" ]]; then
        echo unchanged
        return 0
    fi
    source="$(git --git-dir="$git_dir" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
    author_name="${WIKI_PAGES_AUTHOR_NAME:-github-actions[bot]}"
    author_email="${WIKI_PAGES_AUTHOR_EMAIL:-41898299+github-actions[bot]@users.noreply.github.com}"
    commit="$(GIT_AUTHOR_NAME="$author_name" GIT_AUTHOR_EMAIL="$author_email" \
        GIT_COMMITTER_NAME="$author_name" GIT_COMMITTER_EMAIL="$author_email" \
        git --git-dir="$git_dir" commit-tree "$tree" "${parent_args[@]}" -m "Publish wiki from $source (base $base)")"
    if [[ "$push" == 1 ]]; then
        git --git-dir="$git_dir" push --quiet --force-with-lease="refs/heads/$branch:$tip" \
            "$remote" "$commit:refs/heads/$branch" >/dev/null
    fi
    echo "$commit"
}

self_test() {
    local fixture git_dir result first second
    fixture="$STAGE/fixture"
    mkdir -p "$fixture/dist/en" "$fixture/project"
    git init --quiet --bare "$fixture/remote.git"
    git -C "$fixture/project" init --quiet
    git -C "$fixture/project" -c user.name=t -c user.email=t@invalid commit --quiet --allow-empty -m source
    git -C "$fixture/project" remote add origin "$fixture/remote.git"
    git_dir="$fixture/project/.git"
    printf '<!DOCTYPE html><a href="en/index.html">en</a>\n' >"$fixture/dist/index.html"
    printf '<!DOCTYPE html><a href="/site/">home</a>\n' >"$fixture/dist/en/index.html"
    printf '{"base_path":"/site/","nodes":[],"edges":[],"linear_order":[]}\n' >"$fixture/dist/graph.json"
    local head_before
    head_before="$(git --git-dir="$git_dir" rev-parse HEAD)"

    EXPECT_BASE="/other/"
    if publish "$git_dir" "$fixture/dist" origin gh-pages 1 >/dev/null 2>&1; then log_error "Mismatched base path was published"; exit 1; fi
    EXPECT_BASE="/site/"
    if publish "$git_dir" "$fixture/missing" origin gh-pages 1 >/dev/null 2>&1; then log_error "Missing site was published"; exit 1; fi

    result="$(publish "$git_dir" "$fixture/dist" origin gh-pages 0)"
    [[ -z "$(git -C "$fixture/remote.git" branch --list gh-pages)" ]] || { log_error "Dry run pushed"; exit 1; }
    first="$(publish "$git_dir" "$fixture/dist" origin gh-pages 1)"
    [[ "$(git -C "$fixture/remote.git" rev-parse gh-pages)" == "$first" ]] || { log_error "First publication missing"; exit 1; }
    git -C "$fixture/remote.git" cat-file -e "gh-pages:.nojekyll" || { log_error ".nojekyll missing"; exit 1; }
    git -C "$fixture/remote.git" cat-file -e "gh-pages:en/index.html" || { log_error "Site content missing"; exit 1; }
    [[ "$(publish "$git_dir" "$fixture/dist" origin gh-pages 1)" == unchanged ]] || { log_error "Unchanged site published again"; exit 1; }

    printf '<!DOCTYPE html><a href="/site/">home, changed</a>\n' >"$fixture/dist/en/index.html"
    second="$(publish "$git_dir" "$fixture/dist" origin gh-pages 1)"
    [[ "$(git -C "$fixture/remote.git" rev-parse gh-pages^)" == "$first" ]] || { log_error "History was not preserved"; exit 1; }
    [[ "$(git -C "$fixture/remote.git" rev-parse gh-pages)" == "$second" ]] || { log_error "Second publication missing"; exit 1; }

    printf '<!DOCTYPE html><a href="/elsewhere/">escape</a>\n' >"$fixture/dist/en/index.html"
    if publish "$git_dir" "$fixture/dist" origin gh-pages 1 >/dev/null 2>&1; then log_error "Link outside the base path was published"; exit 1; fi

    [[ "$(git --git-dir="$git_dir" rev-parse HEAD)" == "$head_before" && -z "$(git -C "$fixture/project" status --porcelain)" ]] || {
        log_error "Publication mutated the project checkout"; exit 1;
    }
    [[ -n "$result" ]] || { log_error "Dry run produced no commit"; exit 1; }
    log_success "Pages publication self-test passed (dry run, first/unchanged/history, refusals)"
}

main() {
    parse_args "$@"
    require_commands git node
    trap cleanup EXIT
    STAGE="$(mktemp -d "${TMPDIR:-/tmp}/wiki-pages.XXXXXX")"
    if [[ "$SELF_TEST" == 1 ]]; then self_test; return; fi
    local result
    result="$(publish "$PROJECT_ROOT/.git" "$SITE_DIST" "$REMOTE" "$BRANCH" "$PUSH")"
    if [[ "$result" == unchanged ]]; then log_success "Wiki unchanged; $REMOTE/$BRANCH already serves this artifact"
    elif [[ "$PUSH" == 1 ]]; then log_success "Published wiki commit $result to $REMOTE/$BRANCH"
    else log_info "Dry run: prepared wiki commit $result for $REMOTE/$BRANCH; pass --push to publish"
    fi
}

main "$@"
