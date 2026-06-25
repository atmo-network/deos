---
name: alignment
description: Focused DEOS alignment protocol for boundary discipline, durable failure memory, project-local audits, and diff-aware completion gates.
---

# DEOS Alignment Protocol

This skill keeps agents aligned with DEOS architecture without turning the skill into a second project manual. It should evolve **intensively**: compress rules, sharpen gates, and consolidate recurring drift classes instead of adding more phases, checklists, or prose inventory.

## 0. Operating Rule

Use this skill when work touches DEOS architecture, runtime semantics, project validation, backlog/context hygiene, or autonomous execution gates.

Default posture:

1. Preserve the project boundary: generic framework vs TMCTOL runtime topology.
2. Prefer bounded, deterministic, token-driven mechanics.
3. Validate the changed scope, not the whole repository by habit.
4. Record only reusable coordination lessons, not run telemetry.
5. When adding validation, first ask whether an existing audit can absorb the rule.

## 1. Alignment Lens

Before non-trivial design or runtime changes, classify the change with five questions:

- `Boundary`: L0/L1 physics, L2/tactical policy, frontend/read-model, tooling, or docs/context?
- `Trigger`: what deterministic event, schedule, balance ingress, or explicit call drives it?
- `Bound`: what keeps storage, iteration, weight, proof size, and history bounded?
- `Truth surface`: authoritative on-chain projection or external/materialized read model?
- `Rejected shortcut`: what lazy default would violate DEOS, and why is it not used?

Do not emit this matrix for tiny mechanical fixes unless it helps avoid confusion.

## 2. Durable Ledgers

Ledgers are high-order memory only:

- `ledgers/hallucinations.jsonl` — false architecture claims or contract-violating implementation moves
- `ledgers/ambiguities.jsonl` — unresolved competing meanings, owners, or truth surfaces
- `ledgers/dead_ends.jsonl` — tempting but globally wrong workflows or local traps
- `ledgers/boundary_drifts.jsonl` — membrane violations: L0/L1/L2, mechanism/policy, on-chain/indexed, generic/template vs ecosystem-specific

Reject low-signal entries such as bare `Gate Failed`, `Compilation Failed`, or `Audit Failed` notes. A durable entry must state:

1. What reusable pattern failed?
2. Which surface/boundary was affected?
3. What should the next agent do differently?

Use the helper only when the lesson meets that bar:

```bash
./.agents/skills/alignment/scripts/log-ledger.sh --help
```

## 3. Project Audit Layer

Project-specific audit leaves live here because they encode DEOS coordination memory. `/scripts` may orchestrate them, but durable project-audit knowledge should stay in this skill layer.

Routine fast path:

```bash
./scripts/validate-local.sh --audit-only
```

Network-backed dependency posture is opt-in:

```bash
./scripts/validate-local.sh --audit-only --dependency-audit
```

Narrow leaves are available under:

```bash
./.agents/skills/alignment/scripts/<audit-name>.sh --help
```

Current audit families cover architecture drift, economic-claim anchors/falsification inventory, script entrypoint and skill-metadata contracts, template readiness, numeric parsing, simulator determinism/mirror sync, code suppressions, backlog shape, release-line/package-marker consistency, repository portability, wiki trust/consolidation, dependency posture, and the while-true completion gate.

### Intensive Evolution Rule

When a new recurring drift appears:

1. Fix the concrete instance.
2. Decide whether it is a variant of an existing audit family.
3. Prefer tightening that existing audit over adding a new script or a new SKILL phase.
4. Add a new audit leaf only when the drift has a distinct source of truth, distinct failure mode, and independent useful invocation.
5. Keep `SKILL.md` as the router and rationale layer; put implementation detail in scripts and `--help` output.

## 4. Architecture Auditor

For Rust pallet/runtime architecture checks:

```bash
./.agents/skills/alignment/scripts/auditor.sh
```

Defaults are diff-aware: changed Rust lines plus full untracked files. Use a path or `--all` only when the task truly needs a broader sweep.

If the auditor emits a remedy path, read that referenced document before changing code.

## 5. While-True Gate

Autonomous loops must pass the gate before continuing:

```bash
./.agents/skills/alignment/scripts/while-true-gate.sh
```

The gate runs the smallest meaningful changed-scope set: architecture audit, shell syntax, simulator, cargo check, wiki trust, release-line audit, and knowledge sync as applicable.

Useful flags:

```bash
./.agents/skills/alignment/scripts/while-true-gate.sh --all-rust
./.agents/skills/alignment/scripts/while-true-gate.sh --skip-simulator
./.agents/skills/alignment/scripts/while-true-gate.sh --allow-no-context-sync
```
