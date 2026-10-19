# ABCd Context Project Context

## Meta-Protocol Principles

- `Self-Reference`: This skill validates its own context graph with its own runtimes and regression fixture.
- `Root State Separation`: Durable protocol, open work, completed delivery, README entrypoints, and subsystem docs keep distinct ownership.
- `Constraint-Driven Evolution`: Structure grows from observed coordination pressure, not template ceremony.
- `Project Neutrality`: Portable rules remain independent of repository stacks and local overlays.
- `Single Runtime`: The Node validator provides one authoritative behavior and supported execution path.
- `Cross-Platform`: Validator behavior runs wherever the supported Node runtime runs.

## Operating Principles

- Keep `SKILL.md` as the operating kernel; place resolution, lifecycle, and consolidation detail in `docs/protocols.md`.
- Keep `BACKLOG.md` limited to real open, gated, or blocked work and `CHANGELOG.md` limited to completed outcomes.
- Keep root `README.md` connected to `AGENTS.md`, `BACKLOG.md`, `CHANGELOG.md`, and `docs/README.md` when those surfaces exist.
- Keep subtree README entrypoints reachable once they become real human starting points.
- Use `scripts/validate-context.mjs` for audits and `scripts/_self-test.mjs` for regression.
- Treat warnings as evidence requiring judgment, not as automatic failure or automatic approval.

## Protocol Constraints

- `Farmville Guard`: Skip context mutation when it would not preserve truth, prevent drift, record meaningful delivery, or repair discoverability.
- `Activation Symmetry`: Default post-task activation performs post-task reconciliation; it must not imply retroactive pre-task ceremony.
- `Template Proportionality`: Lean projects receive lean structure; mature hierarchy requires real complexity.
- `State Ownership`: The same reality must not remain simultaneously durable, open, and completed.
- `README Continuity`: Setup, usage, topology, ownership, and same-domain entrypoint knowledge belong in the nearest relevant README.
- `Impact-Oriented History`: Delivery history records outcomes and impact rather than iteration bookkeeping.
- `Natural-Language Operation`: Do not introduce fictional YAML tracking or formalize obvious senior-engineer behavior without a non-obvious contract.
- `Local Overlay Boundary`: Project-specific release, architecture, security, and stack gates remain in local overlays.

## Validator Constraints

- `Single Runtime`: Keep `validate-context.mjs` as the only supported validator implementation.
- `Anchor Validation`: Preserve underscores while normalizing GitHub-style heading anchors and validate line-reference bounds.
- `Path Portability`: Resolve paths through Node rather than platform-specific shell utilities.
- `Human Output`: Emit concise readable logs and a summary; `NO_COLOR=1` suppresses color.
- `Bounded Scanning`: Skip link scanning above the configured byte threshold while validating the surrounding graph.
- `Contiguous Lists`: Fail blank lines inside one Markdown list; list items are contiguous structural peers, not paragraphs. Ignore fenced examples and bounded-scan exclusions.
- `Compact Tables`: Fail non-compact delimiter cells; require exactly three hyphens with one space inside each pipe, while table rows over 120 characters only warn.
- `Core Shape Flexibility`: Accept both numbered mature-project sections and compact skill-style durable sections.
