---
page_type: usage
title: Validation Troubleshooting
summary: What to do when DEOS validation fails, including wiki trust, context, web-client, Domain DAG, simulator, Rust, and script-layer failures.
locale: en
canonical_page_id: validation-troubleshooting
translation_status: source
available_locales:
  - en
  - ru
sources:
  - ../../AGENTS.md
  - ../../web-client/README.md
  - ../../scripts/README.md
  - ../../.agents/skills/wiki-sync/SKILL.md
  - ../../.agents/skills/alignment/SKILL.md
  - ../../scripts/validate-local.sh
status: active
audience: developer
tags:
  - usage
  - validation
  - troubleshooting
  - development
related:
  - Three-Layer Validation
  - Scripts Layer
  - Tech Stack
  - Development Status
last_compiled: 2026-06-13
confidence: 0.86
---

# Validation Troubleshooting

## Summary

When validation fails, first identify the layer. Do not jump to full-tree fixes before knowing whether the failure is documentation shape, wiki trust, frontend ownership, runtime behavior, math, or local tooling.

The rule is: fix the smallest truthful surface, then rerun the gate that failed.

## Quick Triage

- **Wiki trust fails**: inspect the reported file/line. Common causes are raw HTML, dangerous links, inline DOM handlers, or extra colons in frontmatter scalar lines.
- **Context validation fails**: check `AGENTS.md`, `BACKLOG.md`, `CHANGELOG.md`, README links, docs index coverage, and broken markdown links.
- **Domain DAG fails**: find the import or ownership boundary. Usually the fix is moving code to the owner slice, not adding a generic shared bucket.
- **Web-client check/build fails**: separate Svelte syntax, TypeScript contracts, generated descriptors, adapter boundaries, and formatting issues.
- **Simulator fails**: treat it as economic truth feedback. Recheck formulas, thresholds, precision, and invariant assumptions before touching runtime code.
- **Rust tests or clippy fail**: prefer targeted crate fixes first. Escalate to workspace checks when the diff crosses runtime or pallet boundaries.
- **Script smoke fails**: run `--help`, check prerequisites, confirm environment variables, and inspect `_common.sh` usage.
- **While-true gate fails**: treat the pass as not done. Fix the failing layer, update backlog/context/changelog if the failure exposed durable drift, then rerun the gate.

## Recovery Pattern

1. Copy the exact failing command and first actionable error.
2. Classify the failure layer.
3. Fix the owner surface, not the symptom consumer.
4. Rerun the smallest failing command.
5. Rerun the relevant aggregate gate only after the local failure is gone.
6. For release/wiki/context work, finish with `./.agents/skills/alignment/scripts/while-true-gate.sh --skip-simulator` unless math/runtime behavior changed.
7. Update wiki, backlog, or changelog if the failure revealed a durable contract gap.

## Avoid These Mistakes

- Do not hide a Domain DAG boundary issue by creating `shared/`.
- Do not change runtime math to satisfy a broken simulator expectation without proving the formula.
- Do not make indexers a silent dependency to avoid building bounded read surfaces.
- Do not call a pass complete if the while-true gate fails after local checks.

## Related

- [Three-Layer Validation](../development/three-layer-validation.en.md)
- [Scripts Layer](scripts-layer.en.md)
- [Tech Stack](../implementation/tech-stack.en.md)
- [Development Status](../development/status.en.md)
