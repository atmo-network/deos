# Project Skill Graph

Project skills form a small Domain DAG for agent-facing workflow ownership. They complement deterministic root scripts; they do not duplicate subsystem specifications, architecture documents, or executable behavior.

| Skill | Owns | Explicitly excludes | Public route |
| --- | --- | --- | --- |
| `alignment` | Changed-scope DEOS audits, completion judgement, and durable failure memory | Project CI/release validation; subsystem implementation policy | `alignment/SKILL.md`; `alignment/scripts/completion-gate.sh` |
| `architecture-experiments` | Physical architecture hypotheses, controlled benchmark design, production-Weight evidence, durable rejected alternatives, artifact-bound lineage, invalidation, and optimization gradients | Protocol semantics; benchmark command execution; open-work and architecture truth | `architecture-experiments/SKILL.md` |
| `domain-dag` | Independent generic ownership/DAG review and validator | DEOS subsystem policy; project acceptance | `domain-dag/SKILL.md`; `domain-dag/scripts/validate-domain-dag.sh` |
| `release-assurance` | Agent-led dependency provenance, Weight delta, threat-boundary, artifact-identity, and attestation review | Project validation, CI, runtime semantics, publication, signing, and history mutation | `release-assurance/SKILL.md` and its private evidence scripts |
| `wiki-sync` | Independent generated Wiki projection, provenance, trust, localization, and consolidation | Source specification ownership; browser implementation; project acceptance | `wiki-sync/SKILL.md` and its private scripts |

## Validation Ownership

- `Package validation` lives with the package or workspace it verifies and may be composed by project validation.
- `Project validation` lives in root scripts and workflows and separates CI, development, domain, integration, artifact, and release checks while retaining one project-owned comprehensive entrypoint.
- `Skill validation` lives inside its owning Skill only when it verifies that Skill's method or agent-facing evidence contract. Any code/domain/project check needed by project acceptance must move to the owning project surface rather than being invoked inside the Skill.
- Evidence may flow upward: a Skill may invoke public package or project validation and interpret its result. Execution dependencies never flow downward from project or package surfaces into Skills.
- Independent Skills neither invoke nor validate each other. A coordinator may run them separately and integrate their evidence without creating a combined executable gate.
- Keep narrow owner-specific gates and one comprehensive project gate. The comprehensive gate composes only lower project-owned surfaces; it never turns independent Skill checks into project dependencies.

## Cognitive Scaffolding

Skills do not compete with `BACKLOG.md`. The backlog owns **what remains**; a skill owns a reusable method for **how work in one domain grows safely** through routing, evidence, gates, interpretation, and handoff. Feature-local temporary insight must be promoted, reconciled, or pruned instead of cloning open work.

Some delivery skills become continuous feature entry points: during active work they maintain the feedback loop from repository reality through the next slice, evidence, gates, and plan reconciliation. After stabilization they drop stale delivery wording and task routing while preserving compact feature ownership, support guidance, and decisive validation boundaries. Renewed feature pressure regrows delivery guidance in the same skill. Capability skills remain reusable instruments rather than feature owners.

Stabilization changes a feature skill's emphasis; it does not by itself justify deletion. Retire a feature skill only when the feature disappears, merges into another owned domain, or loses a distinct reusable knowledge or support boundary. Consolidate or generalize duplicated method when a stronger canonical owner exists, but preserve the mature feature entry point while its identity remains useful. Keep implementation truth in code/docs, open outcomes in the backlog, and completed outcomes in the changelog.

## Dependency Direction

```text
human / agent request
  → feature, delivery, or alignment skill
    → documented capability contract
      → shared root script when humans, CI, or multiple skills consume it
      → co-located skill leaf only for agent-specific execution
```

A capability skill does not call a sibling's internal scripts. Cross-domain composition stays in the requesting delivery skill, and every executable shared with humans, GitHub Actions, CI, root compositions, or multiple skills lives under root `/scripts` as the documented public route.

## Split Test

Add a skill only when all applicable answers are concrete:

- It owns a durable responsibility not already owned elsewhere.
- It has a distinct trigger or user/agent decision boundary.
- It can state what it excludes.
- Its public route is smaller than its internal workflow.
- Extraction reduces context, interface pressure, or duplicated coordination.

Keep work in the current owner when a proposed child would wrap one call, repeat most parent context, require sibling internals, or obscure locally valuable control flow. Large file size alone never justifies a split.

## Evolution

Update this map when adding, deleting, renaming, or changing the public responsibility of a project skill. Prefer consolidation when two skills converge on the same trigger and evidence contract. Use the `domain-dag` review lens before introducing a new orchestration layer.
