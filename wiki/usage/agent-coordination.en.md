---
page_type: usage
title: Agent Coordination
summary: How DEOS uses repository-local agent skills, ABC context files, wiki sync, and changed-scope completion gates to keep human and agent work aligned.
locale: en
canonical_page_id: agent-coordination
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../docs/README.md
  - ../../.agents/skills/wiki-sync/SKILL.md
  - ../../.agents/skills/alignment/SKILL.md
status: active
audience: developer
tags:
  - usage
  - agents
  - coordination
  - validation
related:
  - Contributing Guidelines
  - Generated Wiki
  - Three-Layer Validation
last_compiled: 2026-07-20
confidence: 0.85
---

# Agent Coordination

## Summary

DEOS treats agent coordination as part of the architecture. The repository is dense enough that humans and agents need shared context files, repeatable validation gates, and local skills that encode project-specific rules.

The goal is not automation for its own sake. The goal is to keep changes aligned with the framework contract, backlog state, and wiki/docs truth.

## Coordination Surfaces

The main surfaces are:

- `AGENTS.md`: durable project protocol and architecture rules;
- `BACKLOG.md`: closable open work and external gates;
- `CHANGELOG.md`: completed delivery history;
- `/docs`: intended subsystem contracts and shipped architecture maps that give the wiki its provenance;
- Code and tests: executable implementation truth;
- `/.agents/skills/`: repo-local skills for alignment and wiki work;
- `/wiki`: generated domain graph used by humans, agents, and the reference client;
- Validation scripts and the completion gate that check changed scope.

`/docs` is not just another prose folder, but it does not override executable behavior. Before changing `/template`, `/web-client`, `/scripts`, or `/wiki`, find the relevant spec and architecture context, inspect code/tests when implementation truth matters, and do not replace either owner with the more convenient wiki summary.

This is why context updates are part of “done” rather than optional documentation cleanup.

## How Agent Work Should Flow

1. Classify the touched surface: docs, template, web-client, scripts, simulator, or wiki.
2. Read the owner context before editing.
3. Make the smallest coherent change.
4. Run the smallest meaningful validation.
5. Update backlog/changelog/wiki/context if project truth changed.
6. Run the repo-local completion gate when operating autonomously.

If a task discovers a new in-scope slice, it should be represented in backlog or closed in the same pass. Evergreen rules belong in `AGENTS.md`, not as immortal backlog items.

## Skill Boundaries

- `Wiki-sync` owns wiki trust, semantic projection, and generated-wiki rules.
- `Alignment` owns project-local audits, hallucination/boundary-drift memory, and completion discipline.
- Generic coding work still follows the coding contract, but DEOS-specific architecture rules come from this repository context.

Skills are cognitive infrastructure. If they encode a durable project rule, they should be treated like part of the system, not like a disposable helper script.

## Related

- [Contributing Guidelines](../community/contributing.en.md)
- [Generated Wiki](../concepts/generated-wiki.en.md)
- [Three-Layer Validation](../development/three-layer-validation.en.md)
