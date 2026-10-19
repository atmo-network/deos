---
name: abcd-context
description: ABCd context protocol across ABC root files (AGENTS.md, BACKLOG.md, CHANGELOG.md), human entrypoint README.md files tree, and /docs. Use after meaningful project changes, backlog drift, documentation refactors, self-evolving context maintenance, or a forced context reconciliation pass.
---

# ABCd Context

Keep project context truthful, separated, connected, and small enough to use. Treat `AGENTS.md`, `BACKLOG.md`, `CHANGELOG.md`, the README tree, and `/docs` as one context graph with distinct ownership.

## Core Guarantee

Durable rules stay durable. Open work stays open. Completed delivery moves to history. README entrypoints remain trustworthy. Docs remain discoverable. Each fact has one authoritative home.

## Activation Modes

- `POST_TASK` (default): Reconcile context after meaningful reality changed. Do not add pre-task ceremony retroactively.
- `ON_REQUEST`: Run a full context audit and reconciliation when the user explicitly asks.
- `ALWAYS_ON` (opt-in): Monitor context continuously only when explicitly requested; otherwise its attention cost exceeds its value.

Skip the skill for conversational work, trivial changes, or tasks that produced no durable context change.

## Context Model

- `README.md`: Human entrypoint, setup, usage, topology, and navigation.
- Subtree `README.md`: Local human entrypoint for a real subsystem, workspace, package, or tool.
- `AGENTS.md`: Durable protocol, constraints, conventions, architecture memory, and recurring operating rules.
- `BACKLOG.md`: Canonical remaining open, gated, or blocked work. Accepted aliases: `TODO.md`, `PLAN.md`, `ROADMAP.md` when project convention already owns them.
- `CHANGELOG.md`: Completed delivery and impact, unless the project already has another canonical shipped-history surface.
- `docs/README.md`: Documentation index.
- `docs/*`: Product, subsystem, architecture, operations, and contract documentation.

Detailed resolution, adaptation, consolidation, and overlay rules live in [`docs/protocols.md`](./docs/protocols.md). Templates live in [`docs/templates.md`](./docs/templates.md).

## Hierarchical Backlog Batching

For non-trivial work, model the plan as `outcome or epic → acceptance checkpoint → owned work item`, not one long flat sequence.

- Create a checkpoint only when several compatible changes share one meaningful integration boundary and can form a compiling, bisectable candidate. Keep one-shot work flat.
- Group by coupled invariant, ownership boundary, or acceptance evidence rather than file proximity or arbitrary item count. Keep conflicting transitions, unrelated domains, and externally gated work in separate checkpoints.
- Give every work item one focused falsification route: a named test family, filter, audit, or explicit evidence owner. A shared test names the smallest item set whose interaction it proves.
- During edits, prefer focused item validation. Run aggregate package/runtime checks once when the checkpoint candidate forms; reserve full release validation for the highest boundary that actually needs it.
- Keep dependency order inside a checkpoint only where correctness requires it. Independent items may proceed in parallel without turning the backlog into a false linear pipeline.
- Treat the checkpoint as an acceptance and context boundary, not a second source of task truth. Child items own concrete remaining work; the checkpoint owns only the shared exit condition.
- Close or narrow children as reality changes. Remove the checkpoint when its aggregate exit condition passes, and move meaningful impact to delivery history.

## Invariants

1. `Single source of truth`: Do not track the same state across durable protocol, open work, completed history, and docs.
2. `Reality before prose`: Inspect the implementation, diff, plans, and existing conventions before updating context.
3. `Constraint-driven structure`: Add hierarchy only when discovered constraints justify it; restructure inherited context when accidental shape hides truth.
4. `Hierarchical execution`: Group compatible non-trivial work under acceptance checkpoints with attributable item-level evidence; do not force independent work into a false sequence or replay broad gates after every edit.
5. `Human entrypoint continuity`: Keep root and relevant subtree README files reachable and current when setup, usage, topology, or ownership changes.
6. `Backlog truth`: A completed or narrowed slice must not remain falsely open.
7. `Impact-oriented history`: Record meaningful delivered outcomes, not bookkeeping or duplicated commit logs.
8. `Local convention compatibility`: Cooperate with stricter project overlays; do not copy project-specific gates into this portable protocol.
9. `Farmville guard`: If an edit would not preserve durable wisdom, correct open-work truth, record meaningful delivery, or repair discoverability, skip it. Silence is valid.

## POST_TASK Flow

Run one proportional reconciliation pass:

1. `Assess`: Did the task materially change behavior, architecture, public contracts, open-work truth, setup, topology, or reusable operating knowledge? If not, stop.
2. `Inspect`: Read the touched context surfaces, relevant diff, canonical plan, nearest README entrypoints, and existing shipped-history convention.
3. `Reconcile open work`: Close, narrow, split, retarget, defer, gate, or block affected backlog items so only real remaining work stays open.
4. `Route knowledge`:
   - Setup, usage, topology, navigation, ownership → nearest `README.md`.
   - Durable reusable constraint → `AGENTS.md`.
   - Remaining executable work → canonical backlog.
   - Meaningful completed outcome → `CHANGELOG.md` or established equivalent.
   - Product/subsystem/architecture contract → indexed `/docs` document.
5. `Consolidate and connect`: Merge duplicates, remove stale statements, preserve one authoritative home, update navigation, and keep docs index coverage honest.
6. `Validate`: When context files changed, run the project convention first, otherwise run `validate-context`. Review warnings rather than treating exit code alone as proof.
7. `Report`: Name changed context files, validation result, and any unresolved drift. If no update passed the Farmville guard, report that no context mutation was needed.

Use a light pass for small scoped changes. Use the full protocol in [`docs/protocols.md`](./docs/protocols.md) for broad context refactors, architecture changes, or multi-surface drift.

## ON_REQUEST Flow

1. Map the current context graph and canonical files.
2. Run both automated validation and manual truth checks.
3. Compare claims against repository reality.
4. Reconcile state ownership, stale content, duplicates, missing entrypoints, links, backlog drift, and docs coverage.
5. Restructure only where current shape obstructs navigation or truth.
6. Rerun validation and report remaining uncertainty.

## Mutation Rules

- Read the existing format before writing and match it when it remains honest.
- Prefer targeted reconciliation over broad rewrites.
- Use templates for new/empty files and as restructuring targets for drifted files, not as mandatory ceremony.
- Keep prose unwrapped unless project tooling requires wrapping.
- Prefer label bullets over definition tables.
- Do not create missing root files automatically when project convention is ambiguous; ask or preserve the established equivalent.
- Do not claim an item completed from documentation alone; verify reality.
- Do not let generated context exceed the mistakes or navigation cost it prevents.

## Validation

Run the Node validator from the project root or pass an explicit root:

```bash
node "${SKILL_DIR}/scripts/validate-context.mjs" /path/to/project
```

Output is human-readable. Markdown list items must remain contiguous: blank lines inside one list fail validation, including between multiline or nested items, while fenced examples are ignored. Markdown table checks always require compact delimiter cells such as `| --- | ---: | :--- |`; non-compact delimiters fail validation, while rows longer than 120 characters only warn without imposing a maximum length.

Useful environment controls:

- `VALIDATE_CONTEXT_ROOT=/path` as an environment fallback.
- `NO_COLOR=1` for logs consumed by CI or agents.

Exit `0` means no errors; warnings may still require judgment. Exit `1` means validation errors or invalid invocation. Validator design and check semantics live in [`docs/validation-design.md`](./docs/validation-design.md).

## Completion Checklist

- [ ] Canonical open work matches reality.
- [ ] Completed delivery left the backlog and reached the canonical history surface when meaningful.
- [ ] Durable constraints live in `AGENTS.md`, not delivery history.
- [ ] Relevant README entrypoints still explain and connect the changed area.
- [ ] Docs describe current contracts and remain indexed.
- [ ] Duplicate or stale context was removed rather than layered over.
- [ ] Automated validation ran after context mutation, and warnings were reviewed.
