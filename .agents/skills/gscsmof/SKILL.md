---
name: gscsmof
description: Creates, evolves, consolidates, and retires bounded Skill-Meta-Organ-Features for epic feature delivery without becoming a shadow backlog or implementation owner.
---

# GSCSMOF

GSCSMOF is the **Genesis Skill Creator of Skill-Meta-Organ-Features**. Its presence grants standing project permission to create and evolve feature-specific implementation organs when new or substantially reshaped epics appear across validated slices, sessions, or agents.

A **SMOF** is a feature-specialist project organ for one evolving epic. It connects repository reality to a richer feature working model, slice selection, intermediate insights, evidence, gates, plan reconciliation, completion pressure, and handoff. It acts as an expanded operational lens over the canonical backlog without replacing the feature, implementation, or truth surfaces.

## Responsibility Split

| GSCSMOF owns | A generated SMOF owns |
| --- | --- |
| Decide whether an epic has earned an implementation organ | Maintain the feature-specific delivery feedback loop and completion pull |
| Search for a reusable existing skill before creating one | Route changed feature scope to canonical scripts and checks |
| Generate the smallest valid SMOF contract | Preserve feature-specific gates, exclusions, and evidence interpretation |
| Enforce naming, ownership, DAG direction, and truth boundaries | Reconcile its method when real implementation friction exposes a reusable lesson |
| Review organs for merge, generalization, or retirement | Hand off plan transitions and bounded useful insights without cloning the backlog |

GSCSMOF never owns feature implementation, feature decisions, backlog priority, release approval, or validation semantics. It may invoke canonical validation while creating or evolving an organ. A SMOF never creates sibling organs, governs GSCSMOF, rewrites unrelated plans, or claims ownership of code/specification truth.

## Canonical Truth Boundaries

| Surface | Owns |
| --- | --- |
| `BACKLOG.md` | Open outcomes, gates, and next deliverable boundaries |
| Specifications | Intended feature contract, invariants, and rationale |
| Architecture documents | Shipped topology, bindings, operational watchpoints, and accepted evidence |
| Code and tests | Executable behavior |
| Root `/scripts` | Shared deterministic project execution |
| Capability skills | Reusable cross-feature methods such as benchmarking or alignment |
| SMOF `SKILL.md` | Feature-specific growth method, routing, gates, completion pull, and handoff |
| SMOF `INSIGHTS.md` | Bounded pre-consolidation feature findings with provenance and destination |
| `CHANGELOG.md` | Meaningful completed outcomes |

A SMOF links to these owners. It does not mirror their task lists, implementation maps, commands, generated numbers, or history.

## Activation Contract

Keeping GSCSMOF in the repository is an explicit standing opt-in to a proactive protocol. Whenever an agent receives or discovers a high-level EPIC Feature task, it should evaluate whether the feature needs a SMOF before deep implementation. The agent may create, refine, merge, or retire that organ automatically without a separate request or confirmation for each local, reversible skill edit.

The user does not need to say “create a skill.” Epic-level intent is the trigger. If evidence is incomplete but a multi-slice feature organ would cheaply improve continuity, GSCSMOF may create a minimal SMOF and let repository reality evolve it. If maintainers do not want proactive organ creation, they should remove GSCSMOF; its presence is the permission.

This is proactive agent behavior, not a hidden process. GSCSMOF runs only while an agent is handling repository work; it does not claim a daemon, timer, watcher, or independent execution channel. External publication, credentials, accounts, and irreversible actions retain their normal approval gates.

## Project Adaptation

GSCSMOF learns the local project shape before generating anything. It reads the project protocol, skill graph, canonical work surface, nearby successful skills, script ownership rules, naming style, and validation routes. It copies proven local patterns where they fit and changes shape when the epic's real constraints differ.

The templates and heuristics below are defaults, not a universal framework. Project conventions outrank GSCSMOF preferences unless they would violate a higher safety or truth boundary.

## Epic Qualification

Treat these as positive signals rather than a rigid checklist:

- The project or user names an EPIC Feature, high-level feature outcome, release-sized capability, or repository reality reveals one coherent multi-slice objective.
- Work will likely cross sessions, agents, subsystems, or independently validated slices.
- The epic has feature-specific routing, evidence, safety, sequencing, or handoff pressure.
- An organ would preserve useful intermediate insight, compress repeated reasoning, and keep procedural detail out of the backlog.
- Existing skills do not already provide a clear home for the same feedback loop.

GSCSMOF may create a minimal SMOF early and let evidence grow it. It should usually skip a clearly one-shot patch, an idea with no actionable scope, temporary telemetry, or folder symmetry, but uncertainty alone does not require user confirmation. Cheap retirement keeps early creation reversible.

## Reuse Before Creation

Before generating a SMOF:

1. Read the project skill graph, canonical backlog scope, owning specification/architecture, and relevant delivery history.
2. Search existing skills by responsibility, trigger, evidence, and exclusion boundary rather than keyword similarity.
3. Extend an existing organ when the new epic shares the same feedback loop and truth owners.
4. Compose an existing capability skill when only one reusable method is missing.
5. Create a new SMOF when a separate organ gives the feature a clearer home.

Reuse is a compression preference, not a veto on creation. Never use numerical semantic-match thresholds as architecture evidence; state the concrete overlap or separation.

## Creation Protocol

When GSCSMOF selects a candidate, no additional creation confirmation is required:

1. Choose a compact role-first name following nearby project conventions, often `<feature>-delivery`; avoid redundant prefixes.
2. Create `/.agents/skills/<name>/SKILL.md` with valid `name` and concise `description` metadata.
3. Define the locked epic scope, canonical work surface, trigger, owners, exclusions, evidence ladder, route, safety gates, handoff, and retirement condition.
4. Reference shared root scripts and capability contracts instead of copying commands or sibling internals.
5. Add the organ to `/.agents/skills/README.md` with its owner, exclusions, and public route.
6. Narrow the canonical backlog only if creating the organ changes what remains; represent detailed feature guidance through links and feature-local insight rather than copied backlog items.
7. Add a bounded `INSIGHTS.md` only when the epic needs temporary feature-local working memory beyond the protocol itself.
8. Run skill metadata, link, context, and completion validation.
9. Report the created boundary, reuse decision, and validation evidence.

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

Adapt this shape to the project and epic. Add sections only when real constraints require them; copied feature documentation or a large static checklist has failed compression.

## Feature Working Memory

A SMOF may keep `INSIGHTS.md` as bounded pre-consolidation memory for useful feature-local findings that have not yet earned a stable owner. This makes the organ richer than a backlog item without turning it into another backlog.

Good entries capture a finding, its evidence/provenance, why it changes future delivery, and its intended destination or retirement condition. Suitable material includes unresolved design pressure, recurring implementation traps, cross-slice dependencies, falsified assumptions, evidence gaps, and decisions awaiting specification or architecture promotion.

Do not store raw logs, command transcripts, commit diaries, copied tasks, current generated hashes, or stable facts already owned by code/docs. At each meaningful slice, promote mature truth to its owner, convert actionable outcomes into the canonical backlog, and delete stale or resolved insight. Keep the file small enough to read as one feature checkpoint.

## Completion Pull

A SMOF should actively pull its epic toward completion:

- Keep the epic's objective stop conditions and highest-value remaining slice visible.
- At relevant checkpoints, report what closed, what remains, and the exact next actionable or gated transition.
- When continuation intent permits, prefer advancing the unfinished epic over unrelated available polish.
- Ask the user to resume or unblock the feature when a human decision or external gate is the only remaining path.
- Recognize completion, consolidate durable knowledge, and stop generating momentum when the objective closes.

Completion pull never overrides a user's latest direction, manufactures urgency, or continues through approval and safety gates.

## SMOF Operating Contract

A generated SMOF should guide this loop:

```text
verified repository reality
  → refresh and prune the feature working model
    → reconcile the canonical epic plan
      → select one bounded high-value slice
        → route to owned implementation and shared validation
          → interpret evidence and gates
            → promote useful insight and pull toward completion
```

The SMOF may improve after a slice when a recurring ambiguity, dead end, unsafe shortcut, missing route, or evidence mistake changes future delivery. It must not absorb raw logs, commit diaries, completed task lists, current hashes, or facts already owned elsewhere.

## Evolution Rules

- **Refine** when the same epic exposes a reusable missing decision rule.
- **Compose** when a cross-feature capability already owns the method.
- **Generalize** when concrete reuse pressure shows that the method belongs above one feature; multiple real consumers provide the strongest signal.
- **Split** when one organ contains independently triggered feedback loops with different owners or gates.
- **Merge** when two organs converge on the same trigger, evidence, and retirement condition.
- **Retire** when the epic closes and no durable feature-specific delivery method remains.

GSCSMOF makes local structural changes from evidence; it does not merge or delete organs merely from co-activation counts, age, file size, or speculative similarity.

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
