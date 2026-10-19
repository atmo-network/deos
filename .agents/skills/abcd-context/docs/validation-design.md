# `validate-context` Design

## Purpose

`validate-context` supplies structural evidence for the ABCd graph. It checks ownership surfaces, navigation, links, drift signals, and Markdown shape. It cannot prove that documentation claims match implementation; the full audit keeps that manual responsibility explicit.

## Runtime

`scripts/validate-context.mjs` is the single supported implementation. It runs on the supported Node runtime and prints classic human-readable validation logs by default.

## Root Resolution

Resolution order:

1. Explicit `project-root` argument.
2. `VALIDATE_CONTEXT_ROOT`.
3. Current working directory.

A missing or non-directory root fails before validation.

## Checks

1. `Durable file detection — Error`: Finds `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `GEMINI.md`, or `CONTEXT.md`.
2. `Root README connectivity — Warning`: Checks references to the durable file, canonical open work, completed history, and docs index when present.
3. `Core durable structure — Warning`: Accepts numbered mature-project sections or compact skill-style meta/operating sections.
4. `Root state split — Warning`: Detects the canonical plan, completed-history surface, delivery history inside the durable file, and obvious backlog/changelog label drift.
5. `Relative links and anchors — Error`: Validates Markdown links outside fenced code blocks, including heading anchors and GitHub-style line-reference bounds.
6. `README reachability — Warning`: Finds subtree README files with no inbound Markdown link.
7. `Meta-protocol presence — Warning`: Checks the durable file for `Meta-Protocol Principles`.
8. `Bloat signals — Warning`: Reports low information density or sparse structure in the durable file.
9. `Markdown lists — Error`: Fails blank lines within one continuous list, including multiline and nested items, while ignoring fenced examples and files above the bounded Markdown scan limit.
10. `Markdown tables — Error/Warning`: Always fails non-compact delimiter rows and warns about table rows longer than 120 characters.
11. `Freshness — Warning`: Reports durable files older than 30 days.
12. `Docs directory — Warning`: Checks for `/docs`.
13. `Docs index coverage — Warning`: Detects docs missing from `docs/README.md` and indexed files that do not exist.

## Severity Contract

- `Pass`: The structural check found no issue.
- `Info`: Evidence or an intentionally skipped bounded check.
- `Warning`: Potential drift requiring judgment; warnings do not change exit status.
- `Error`: Broken structural invariant or invalid invocation; errors return exit `1`.

Exit `0` never means the context is factually correct. It means automated checks found no errors.

## Link Validation

The validator scans Markdown under the project root while excluding common generated, dependency, cache, and vendor directories. It ignores links and table syntax inside fenced code blocks, including backtick or tilde fences with CommonMark-compatible indentation and matching closing-fence length.

It handles anchor-only, relative-file, file-plus-anchor, and GitHub line-reference links. Heading anchors use GitHub-style normalization while preserving underscores. Line references must point to existing lines.

Files larger than `ABCD_MARKDOWN_LINK_SCAN_MAX_BYTES`, default `262144`, skip link scanning and emit an info item instead of spending unbounded time on generated or reference dumps.

## Root State Drift

The validator prefers `BACKLOG.md` but accepts `TODO.md`, `PLAN.md`, and `ROADMAP.md` with a warning.

It warns when:

- The durable file contains a `Change History` section while `CHANGELOG.md` exists.
- An unchecked backtick-labelled backlog slice also appears in `CHANGELOG.md`.

These checks identify suspicious duplication; they do not prove semantic completion.

## Bloat Signals

The validator avoids a hard file-length limit. It checks independent signals:

- `Low information density`: Structural elements make up less than 40% of nonblank lines.
- `Sparse structure`: The file averages more than 15 lines per heading.

Signals suggest consolidation; they do not replace judgment.

## Markdown Lists

List checks apply to Markdown within the bounded scan size and outside fenced code blocks. A blank line fails when it separates sibling list items, a list item from its nested list, nested sibling items, or a multiline item's continuation from its next item. Paragraphs and independent blocks remain separated by blank lines normally.

## Markdown Tables

Table checks always run and have no enabling option.

- Delimiter cells use exactly three hyphens, optional alignment colons, and one space inside each pipe: for example, `| --- | ---: | :--- |`.
- Delimiters without inner spaces or with padded hyphen runs fail validation.
- Table row length has no maximum; each contiguous table emits one warning when one or more rows exceed 120 characters.

LaTeX is allowed and is not validated.

## Human-Readable Output

Validation prints each check and a summary by default. `NO_COLOR=1` disables ANSI color for CI or captured logs. There is no JSON output mode.

## Regression Contract

`scripts/_self-test.mjs` verifies the Node validator against:

1. `fixtures/abcd-project`, through both environment and explicit-root resolution.
2. The `abcd-context` skill root.
3. A missing path, which must fail clearly.
4. The removed `--json` option, which must fail clearly.
5. A temporary fixture with an out-of-range line reference, which must fail clearly.
6. Temporary fixtures proving that contiguous multiline/nested lists and fenced examples pass while blank separators inside lists fail.
7. Temporary fixtures proving that LaTeX and compact table delimiters pass, non-compact delimiters fail, and rows over 120 characters warn.

The fixture remains linked from [its README](../fixtures/abcd-project/README.md).

## Usage

```bash
# Current project
node /path/to/skill/scripts/validate-context.mjs

# Explicit root
node /path/to/skill/scripts/validate-context.mjs /path/to/project

# Skill regression suite
node /path/to/skill/scripts/_self-test.mjs
```

## Related

- [SKILL.md](../SKILL.md) — operating kernel
- [protocols.md](./protocols.md) — reconciliation and consolidation rules
- [templates.md](./templates.md) — starter and restructuring shapes
