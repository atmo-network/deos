# ABCd Context Protocols

Deep reference for the operating kernel in [SKILL.md](../SKILL.md). This document owns file resolution, full reconciliation, state transitions, consolidation, and local-overlay coexistence.

## File Resolution

### Human Entrypoints

Root and subtree `README.md` files form the human navigation plane.

- Root `README.md` should link to the durable protocol, canonical open work, completed history when present, and primary docs index.
- A subtree `README.md` becomes managed when the subtree acts as a real human entrypoint.
- Update the nearest relevant README when setup, topology, ownership, usage semantics, or same-domain operator/developer knowledge changes.
- Keep subtree entrypoints reachable from a parent README, root README, or docs index.

### Durable Protocol

Preferred file: `AGENTS.md`.

Accepted inherited fallbacks, in resolution order: `CLAUDE.md`, `CODEX.md`, `GEMINI.md`, `CONTEXT.md`.

Create `AGENTS.md` only when project convention permits it. If an inherited durable file contains useful material but mixes abstraction levels or state types, restructure it toward the appropriate template instead of appending another flat section.

### Canonical Open Work

Preferred file: `BACKLOG.md`.

Accepted inherited fallbacks: `TODO.md`, `PLAN.md`, `ROADMAP.md`.

- If exactly one exists and project behavior confirms it, use it.
- If several exist, identify the actively maintained canonical source before editing.
- Prefer eventual convergence toward one source, but do not rename established project files casually.
- Never mirror the same open item across multiple plans.

### Completed Delivery

Preferred file: `CHANGELOG.md`.

If the project already keeps canonical shipped history elsewhere, preserve that convention rather than inventing a parallel changelog. Completed delivery never belongs as rolling history in `AGENTS.md`.

### Documentation Plane

- `docs/README.md` indexes project documentation.
- `docs/*` owns subsystem, architecture, operations, decision, and public-contract detail.
- Root protocol files should link to docs rather than duplicate them.

## Adaptation and Restructuring

When writing managed files:

1. Read the current structure, tone, and ownership.
2. Decide whether the structure still represents reality honestly.
3. Match established form when it remains clear.
4. Restructure when one file mixes durable protocol, open work, completed history, or unrelated documentation.
5. Route new information only after ownership becomes clear.
6. Consolidate existing duplication in the same pass when safe.

Use [templates.md](./templates.md) for new/empty files and as target shapes for inherited drift. Choose the lean `AGENTS.md` starter for low coordination load and the layered starter only when real subsystem or governance complexity justifies it.

Do not preserve accidental formatting merely because it exists. Do not impose a mature hierarchy on a tiny project.

## Proportional Reconciliation

### Light Pass

Use after a small meaningful slice:

1. Reconcile the affected backlog item.
2. Update the nearest README or contract doc only if user-facing or operator-facing truth changed.
3. Record a changelog outcome only when delivery is meaningful.
4. Promote a durable lesson only when it will change future behavior.
5. Validate changed context and stop.

### Full Pass

Use for architecture changes, broad documentation refactors, migration of context ownership, or explicit audits:

1. Map root files, README entrypoints, docs index, and local overlays.
2. Identify canonical owners for durable protocol, open work, shipped history, and subsystem contracts.
3. Compare claims with repository reality.
4. Repair backlog truth first.
5. Repair README entrypoints and navigation.
6. Reconcile subsystem docs and docs index coverage.
7. Record meaningful delivered outcomes.
8. Promote only reusable constraints into durable protocol.
9. Merge duplicates and remove stale context.
10. Run validation, inspect warnings, and repeat only for concrete remaining drift.

## Backlog State Transitions

Use one of these explicit transitions:

- `Close`: Exit criteria are satisfied in reality; remove the item from open work.
- `Narrow`: Part landed; rewrite the item to describe only what remains.
- `Split`: One vague item became multiple independently executable slices.
- `Retarget`: The original wording no longer matches the real desired outcome.
- `Defer`: The item remains valid but no longer ranks as the next useful slice.
- `Gate`: Progress requires an external condition or approval.
- `Block`: Progress requires another unresolved internal or external dependency.

Rules:

- Active epics should expose at least one concrete next slice.
- If implementation completed work absent from the plan, update the nearest existing item or record delivery without fabricating retrospective backlog ceremony.
- If docs or architecture change exit criteria, update the item rather than preserving stale wording.
- Evergreen maintenance disciplines belong in durable protocol, not perpetual checkboxes.
- Prefer refining existing items over creating near-duplicates.

## Completed Delivery Sync

Record meaningful outcomes from reality, not intention.

A good entry names:

- The affected area.
- What changed for users, operators, developers, or system behavior.
- Material compatibility, migration, or safety impact when relevant.

Avoid version-bump-only notes, test bookkeeping, and duplicates of `AGENTS.md` rules.

## Consolidation

### Triggers

Consolidate when:

- Three or more entries describe one pattern.
- Two sections substantially overlap.
- A section grows beyond easy scanning because abstraction levels became mixed.
- A mistake repeats despite an existing rule.
- Docs contradict implementation.
- Two docs own the same contract.
- A docs file lacks index coverage.
- A README entrypoint becomes unreachable or stale.

Heuristics suggest inspection; they do not replace judgment.

### Procedure

1. Identify the authoritative home.
2. Preserve unique and still-true information.
3. Resolve contradictions against implementation and current project convention.
4. Merge the useful content.
5. Remove or explicitly deprecate the weaker source.
6. Repair links and index coverage.
7. Validate the resulting graph.

### Escalation Ladder

When a mistake repeats:

```text
Durable insight
→ Emphasized convention
→ Hard rule with validation
→ Structural tooling or automation
```

Escalate only after evidence shows the previous level failed.

## Local Overlay Coexistence

ABCd owns portable context hygiene:

- Root state separation.
- README and docs-index connectivity.
- Link health and basic Markdown shape.
- Generic backlog/changelog drift detection.
- Context bloat and freshness signals.

Local overlays own project-specific behavior:

- Release and deployment gates.
- Architecture/domain ownership rules.
- Stack-specific tests and security checks.
- Organization-specific conventions.

Run ABCd to verify the context surface, then run the stricter local overlay. Do not copy local rules into ABCd unless they become portable across projects. Tune style warnings through documented options or environment variables rather than forking the validator for one repository.

## Full Audit Checklist

After automated validation, manually verify what scripts cannot prove:

- [ ] Repository claims match implementation and current operations.
- [ ] One authoritative home owns each material fact.
- [ ] Canonical open work reflects real remaining work.
- [ ] Completed work no longer appears open.
- [ ] Durable protocol contains reusable constraints rather than delivery history.
- [ ] Relevant README entrypoints describe current setup, topology, usage, and ownership.
- [ ] Subtree entrypoints remain reachable.
- [ ] Docs index covers current docs without phantom entries.
- [ ] Active epics expose an executable next slice.
- [ ] Local overlays remain authoritative for project-specific gates.

## Related

- [SKILL.md](../SKILL.md) — operating kernel
- [templates.md](./templates.md) — starter and restructuring shapes
- [validation-design.md](./validation-design.md) — validator behavior and parity contract
