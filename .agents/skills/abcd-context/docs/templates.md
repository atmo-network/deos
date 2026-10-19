# ABCd Context Templates

Starter and restructuring shapes for files managed by ABCd. Existing honest project conventions win. Use only the files and sections the project actually needs.

## Root README

```markdown
# [Project Name]

[One-sentence project purpose.]

## Start Here

- [Project Context](./AGENTS.md)
- [Open Backlog](./BACKLOG.md)
- [Changelog](./CHANGELOG.md)
- [Documentation](./docs/README.md)
```

Omit links to surfaces the project deliberately does not maintain. Do not create empty control-plane files merely to satisfy the template.

## AGENTS — Lean

Use for early projects with low coordination load and few durable constraints.

```markdown
# Project Context

## Meta-Protocol Principles

- `Constraint-Driven Evolution`: Add structure when real constraints justify it.
- `Single Source of Truth`: Keep each fact in one authoritative layer.
- `Context Hygiene`: Consolidate and remove stale context before it becomes drag.
- `Boundary Clarity`: Separate durable protocol, open work, completed history, and docs.

## Concept

[One-sentence project purpose and boundary.]

## Topology

- `/[directory]/`: [Responsibility.]

## Durable Conventions

- `[Label]`: [Constraint or rule.]
  - Trigger: [When it applies.]
  - Action: [Required behavior.]
```

## AGENTS — Layered

Use only when multiple subsystems, stronger contracts, or autonomous coordination pressure justify a durable hierarchy. This also serves as a restructuring target for overgrown flat files.

```markdown
# Project Context

## 0. Meta-Protocol Principles

- `Constraint-Driven Evolution`: Add complexity only when discovered constraints justify it.
- `Single Source of Truth`: Keep durable protocol, open work, completed history, and subsystem docs distinct.
- `Decreasing Abstraction`: Organize context from mental model to execution protocol.
- `Context Optimization`: Consolidate and remove stale structure before context becomes entropy.
- `Validation Infrastructure`: Pair structural rules with explicit checks.
- `Human + Agent Coherence`: Keep the same graph useful to future humans and agents.

## 1. Concept

[Project identity, problem, and product boundary.]

## 2. Identity and Naming

- [Canonical terms and naming boundaries.]

## 3. Topology

- `/[directory]/`: [Responsibility.]

## 4. Core Entities

- [Durable domain atoms.]

## 5. Architectural Decisions

- [Stable boundaries and design decisions.]

## 6. Engineering Conventions

- [Validation, implementation, and code standards.]

## 7. Operational Conventions

- [Documentation, tooling, coordination, and release rules.]

## 8. Integration Protocols

- [Runtime, upstream, network, and service seams.]

## 9. Pre-Task Protocol

- [Preparation required by this project.]

## 10. Completion Protocol

- [Validation, context sync, and delivery gates.]
```

## BACKLOG

```markdown
# Project Backlog

## Open Backlog

- [ ] `[Slice]` [Concrete remaining work with truthful exit criteria.]
```

Keep only open, gated, or blocked work. Remove completed items after recording meaningful delivery in the canonical history surface.

## CHANGELOG

```markdown
# Changelog

## [Version or Current]

- `[Area]` [Delivered outcome]. Impact: [Meaningful user/operator/developer effect.]
```

## Durable Convention Entry

```markdown
- `[Label]`: [Reusable constraint or insight.]
  - Trigger: [Observed condition.]
  - Action: [Future behavior.]
```

## Subtree README

```markdown
# [Area Name]

[One-sentence responsibility.]

## What This Area Owns

- [Responsibility.]

## Key Entry Points

- [Relevant file or directory](./path)
- [Parent or related documentation](../README.md)
```

## Docs Index

```markdown
# Documentation Index

Living index of project documentation.

## Documents

- [`filename.md`](./filename.md): [Purpose.]
```

## Project Document

```markdown
# [Document Title]

## Overview

[What this document owns and why it exists.]

## [Main Section]

[Content organized from general contract to specific behavior.]

## Related

- [Related document](./related.md)
```

## README Status Update

Use when local entrypoint truth changed:

```markdown
## Current Status

- [What this area now owns or exposes.]
- [Changed entrypoint, workflow, compatibility, or boundary.]
```
