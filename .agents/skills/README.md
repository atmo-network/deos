# Project Skill Graph

Project skills form a small Domain DAG for agent-facing workflow ownership. They complement deterministic root scripts; they do not duplicate subsystem specifications, architecture documents, or executable behavior.

| Skill | Owns | Explicitly excludes | Public route |
| --- | --- | --- | --- |
| `alignment` | Changed-scope validation routing, DEOS audits, completion gate, durable failure memory | Subsystem implementation policy; release publication | `alignment/SKILL.md`; `alignment/scripts/completion-gate.sh` |
| `aaa-delivery` | AAA validation profile selection, stress/occupancy evidence policy, benchmark handoff | AAA runtime semantics; shared gate execution | `aaa-delivery/SKILL.md`; shared `scripts/aaa-release-gate.sh` implementation |
| `domain-dag` | Generic ownership/DAG review and validator | DEOS subsystem policy; workflow-specific delivery gates | `domain-dag/SKILL.md`; `domain-dag/scripts/validate-domain-dag.sh` |
| `gscsmof` | Qualification, creation, evolution, consolidation, and retirement of epic implementation organs | Feature implementation; backlog priority; child-organ delivery; hidden automation | `gscsmof/SKILL.md` |
| `benchmarking` | Benchmark design, evidence classification, interpretation, and integration; currently FRAME runtime measurement | Runtime semantics; shared command execution; scheduler stress; release publication; frontend benchmarking without an adopted route | `benchmarking/SKILL.md`; shared `scripts/benchmarks.sh` and `scripts/03-build-runtime.sh` implementations |
| `upgrade-delivery` | Upgrade preparation sequence, evidence rungs, relay approval boundary, and post-upgrade handoff | Governance authorization decisions; version/migration semantics; credentials; shared commands | `upgrade-delivery/SKILL.md`; shared runtime-build, try-runtime, and authorized-upgrade scripts |
| `staking-delivery` | Native-staking bootstrap sequence, readiness interpretation, authority boundary, and activation handoff | Staking/AAA semantics; signing; funds; shared commands | `staking-delivery/SKILL.md`; shared `scripts/bootstrap-native-staking-local.sh` implementation |
| `wiki-sync` | Generated wiki projection, provenance, trust, and consolidation workflow | Source specification ownership; browser implementation | `wiki-sync/SKILL.md` and its documented scripts |

## Cognitive Scaffolding

Skills do not compete with `BACKLOG.md`. The backlog owns **what remains**; a skill owns a reusable method for **how work in one domain grows safely** through routing, evidence, gates, interpretation, and handoff.

Some delivery skills become implementation organs for an evolving feature: they maintain its feedback loop from repository reality through the next slice, evidence, gates, and plan reconciliation. They may emerge after repeated implementation friction, improve as real failures expose missing guidance, and help later slices reproduce the stronger method without expanding every backlog item into procedural prose. Capability skills remain reusable instruments rather than feature organs.

Scaffolding must not become permanent by inertia. Consolidate, generalize, or delete a skill when the subsystem stabilizes, its method moves into deterministic tooling, or it no longer has a distinct reusable decision boundary. `gscsmof` owns this creation/lifecycle method but never the child organ's feature loop. Keep implementation truth in code/docs, open outcomes in the backlog, and completed outcomes in the changelog.

## Dependency Direction

```text
human / agent request
  → delivery or alignment skill
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
