# Project Skill Graph

Project skills form a small Domain DAG for agent-facing workflow ownership. They complement deterministic root scripts; they do not duplicate subsystem specifications, architecture documents, or executable behavior.

| Skill | Owns | Explicitly excludes | Public route |
| --- | --- | --- | --- |
| `alignment` | Changed-scope validation routing, DEOS audits, completion gate, durable failure memory | Subsystem implementation policy; release publication | `alignment/SKILL.md`; `alignment/scripts/completion-gate.sh` |
| `aaa-delivery` | AAA validation profile selection, stress/occupancy evidence policy, benchmark handoff | AAA runtime semantics; generic release mechanics | `aaa-delivery/SKILL.md`; root `scripts/aaa-release-gate.sh` bridge |
| `domain-dag` | Generic ownership/DAG review and validator | DEOS subsystem policy; workflow-specific delivery gates | `domain-dag/SKILL.md`; `domain-dag/scripts/validate-domain-dag.sh` |
| `wiki-sync` | Generated wiki projection, provenance, trust, and consolidation workflow | Source specification ownership; browser implementation | `wiki-sync/SKILL.md` and its documented scripts |

## Dependency Direction

```text
human / agent request
  → delivery or alignment skill
    → documented capability contract
      → co-located deterministic script leaf
        → root atomic/operator command where required
```

A capability skill does not call a sibling's internal scripts. Cross-domain composition stays in the requesting delivery skill or a stable root bridge, and each dependency uses the documented public route.

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
