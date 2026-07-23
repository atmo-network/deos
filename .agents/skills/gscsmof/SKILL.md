---
name: gscsmof
description: Creates, evolves, consolidates, and retires bounded Skill-Meta-Organ-Features for epic feature delivery without becoming a shadow backlog or implementation owner.
---

# GSCSMOF

GSCSMOF is the **Genesis Skill Creator of Skill-Meta-Organ-Features**. Use it when a new or substantially reshaped epic may need a feature-specific implementation organ across multiple validated slices, sessions, or agents.

A **SMOF** is a project skill that maintains the reusable delivery method for one evolving epic. It connects repository reality to slice selection, evidence, gates, plan reconciliation, and handoff. It does not replace the feature, its implementation, or its canonical truth surfaces.

## Responsibility Split

| GSCSMOF owns | A generated SMOF owns |
| --- | --- |
| Decide whether an epic has earned an implementation organ | Maintain the feature-specific delivery feedback loop |
| Search for a reusable existing skill before creating one | Route changed feature scope to canonical scripts and checks |
| Generate the smallest valid SMOF contract | Preserve feature-specific gates, exclusions, and evidence interpretation |
| Enforce naming, ownership, DAG direction, and truth boundaries | Reconcile its method when real implementation friction exposes a reusable lesson |
| Review organs for merge, generalization, or retirement | Hand off plan transitions without copying plan contents |

GSCSMOF never owns feature implementation, feature decisions, backlog priority, release approval, or validation execution. A SMOF never creates sibling organs, governs GSCSMOF, rewrites unrelated plans, or claims ownership of code/specification truth.

## Canonical Truth Boundaries

| Surface | Owns |
| --- | --- |
| `BACKLOG.md` | Open outcomes, gates, and next deliverable boundaries |
| Specifications | Intended feature contract, invariants, and rationale |
| Architecture documents | Shipped topology, bindings, operational watchpoints, and accepted evidence |
| Code and tests | Executable behavior |
| Root `/scripts` | Shared deterministic project execution |
| Capability skills | Reusable cross-feature methods such as benchmarking or alignment |
| SMOF | Feature-specific growth method, routing, gates, and handoff |
| `CHANGELOG.md` | Meaningful completed outcomes |

A SMOF links to these owners. It does not mirror their task lists, implementation maps, commands, generated numbers, or history.

## Activation Contract

GSCSMOF activates only through an explicit request to create/evolve an epic organ or during a deliberate context/skill reconciliation that identifies a concrete candidate. File presence, branch names, labels, elapsed time, or periodic scans do not trigger hidden work.

Creation remains a local repository edit. GSCSMOF does not run a daemon, watch issues, open pull requests, schedule itself, mutate accounts, or cross publication/approval gates.

## Epic Qualification

Create a SMOF only when all applicable conditions hold:

- One named epic has a canonical open-work surface and objective stop conditions.
- Delivery spans multiple independently useful slices or repeated sessions.
- The epic has a distinct trigger, evidence contract, safety gate, or handoff boundary.
- Existing capability/delivery skills cannot express the method without growing feature-specific branches.
- The organ reduces repeated reasoning, context load, or coordination errors more than it adds surface area.
- Its scope can state explicit exclusions and a retirement condition.

Do not create one for a single coherent patch, a vague future idea, an arbitrary issue count, temporary telemetry, or folder symmetry. Large code size alone does not qualify an epic.

## Reuse Before Creation

Before generating a SMOF:

1. Read the project skill graph, canonical backlog scope, owning specification/architecture, and relevant delivery history.
2. Search existing skills by responsibility, trigger, evidence, and exclusion boundary rather than keyword similarity.
3. Extend an existing organ only when the new epic shares the same feedback loop and truth owners.
4. Compose an existing capability skill when only one reusable method is missing.
5. Create a new SMOF only when reuse would blur ownership or add feature policy to a lower capability.

Never use numerical semantic-match thresholds as architecture evidence. State the concrete overlap or separation.

## Creation Protocol

When a candidate qualifies:

1. Choose a compact role-first name, normally `<feature>-delivery`; avoid redundant project/runtime prefixes.
2. Create `/.agents/skills/<name>/SKILL.md` with valid `name` and concise `description` metadata.
3. Define the locked epic scope, canonical work surface, trigger, owners, exclusions, evidence ladder, route, safety gates, handoff, and retirement condition.
4. Reference shared root scripts and capability contracts instead of copying commands or sibling internals.
5. Add the organ to `/.agents/skills/README.md` with its owner, exclusions, and public route.
6. Narrow the canonical backlog only if creating the organ changes what remains; never duplicate backlog items in the SMOF.
7. Run skill metadata, link, context, and completion validation.
8. Report the created boundary and why an existing skill could not own it.

### Minimal SMOF Shape

```markdown
---
name: <feature>-delivery
description: <one-sentence feature delivery responsibility>
---

# <Feature> Delivery

Canonical work surface: <link or exact scope>

## Ownership Boundary
## Trigger and Scope
## Truth Owners
## Route
## Evidence and Gates
## Handoff
## Evolution and Retirement
```

Add sections only when real constraints require them. A SMOF with copied feature documentation or a large static checklist has failed compression.

## SMOF Operating Contract

A generated SMOF should guide this loop:

```text
verified repository reality
  → reconcile the canonical epic plan
    → select one bounded high-value slice
      → route to owned implementation and shared validation
        → interpret evidence and gates
          → hand off and refine only reusable method
```

The SMOF may improve after a slice when a recurring ambiguity, dead end, unsafe shortcut, missing route, or evidence mistake changes future delivery. It must not absorb raw logs, commit diaries, completed task lists, current hashes, or facts already owned elsewhere.

## Evolution Rules

- **Refine** when the same epic exposes a reusable missing decision rule.
- **Compose** when a cross-feature capability already owns the method.
- **Generalize** only after at least two real consumers share the same stable contract.
- **Split** when one organ contains independently triggered feedback loops with different owners or gates.
- **Merge** when two organs converge on the same trigger, evidence, and retirement condition.
- **Retire** when the epic closes and no durable feature-specific delivery method remains.

GSCSMOF proposes structural changes from evidence; it does not merge or delete organs merely from co-activation counts, age, file size, or speculative similarity.

## Release and Retirement

A release does not automatically preserve or destroy a SMOF. At epic completion:

1. Move implementation truth to code/tests and owning docs.
2. Move remaining work to `BACKLOG.md` and completed impact to `CHANGELOG.md` under project conventions.
3. Move reusable cross-feature method into the owning capability skill only when genuine reuse exists.
4. Keep the SMOF if the feature has an ongoing distinct delivery/operations loop.
5. Otherwise remove it and update the project skill graph in the same change.

Do not create parallel `.organs`, `wisdom`, sync ledgers, fractal knowledge trees, TTL deletion jobs, or shadow registries. The project skill graph and canonical ABC files already own those coordination boundaries.

## GSCSMOF Self-Boundary

GSCSMOF is a creator and gardener of implementation organs, not a super-organ over feature delivery. It may evolve its qualification, generation, and lifecycle method when real organ creation reveals reusable friction. It must not ingest child feature knowledge, coordinate their slices, override their owners, or treat itself as a SMOF.

## Completion Evidence

Report:

- epic and canonical work surface examined;
- reuse candidates considered;
- qualification decision and evidence;
- organ created, refined, merged, or retired;
- ownership and exclusion boundary;
- graph/context files updated;
- validation result;
- unresolved human or external gate.
