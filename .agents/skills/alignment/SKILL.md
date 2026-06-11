---
name: alignment
description: Advanced strict alignment protocol enforcing the Token-Driven Economic Automaton architecture. Integrates durable ledgers for hallucinations, ambiguities, dead ends, and boundary drifts, plus contextual remediation and a diff-aware while-true gate.
---

# DEOS 10x Alignment Protocol

This skill enforces the fundamental mathematical and architectural laws of the DEOS repository. It functions as the project's immune system and your architectural mentor.

## Phase 0: The Hallucination Ledger

Before proposing any design, read the latest mistakes caught by the skill:

```bash
tail -n 5 .agents/skills/alignment/ledgers/hallucinations.jsonl
```

If entries exist, explicitly state which failure mode you will avoid in this pass.

Ledger layout:

- `./ledgers/hallucinations.jsonl` — architectural falsehoods, rule violations, and gate failures worth remembering
- `./ledgers/ambiguities.jsonl` — discovered ambiguities in intent, terminology, source-of-truth, or boundary classification, with their current resolution state
- `./ledgers/dead_ends.jsonl` — locally tempting but globally wrong paths: blocking commands, false progress, interface traps, and other operator cul-de-sacs
- `./ledgers/boundary_drifts.jsonl` — cross-layer leakage: Physics/Tactics confusion, on-chain/indexed confusion, mechanism/policy substitution, and template/business-logic contamination

These ledgers are meant for **durable coordination memory**, not low-order telemetry. Per-run traces are not canonical memory unless they crystallize into a reusable failure pattern.

### Ledger quality gate

Do **not** record low-information symptoms as canonical memory.
The ledger MUST reject entries that only restate a gate or tool failure without reusable context.
Examples of banned low-signal entries:

- `Knowledge Sync Gate Failed`
- `Architecture Audit Gate Failed`
- `Compilation Gate Failed`
- `Shell Syntax Gate Failed`
- any other bare pass/fail label with no scoped pattern, no cause, and no remedy

A durable ledger entry must answer at least these three questions:

1. `What pattern failed?`
2. `On which surface/boundary did it fail?`
3. `What should the next agent do differently?`

If you cannot answer those questions yet, do **not** write to a ledger; keep the failure in transient command output only

### Memory taxonomy

- `Hallucination` = a false claim about the architecture or an implementation move that violates the contract
- `Ambiguity` = the contract is not yet crisp enough: multiple meanings, owners, or truth-surfaces still compete
- `Dead End` = a path that feels productive locally but is globally wrong, blocking, or anti-coordination
- `Boundary Drift` = a category error across system membranes: L0/L1/L2, mechanism/policy, on-chain/indexed, generic/template vs ecosystem-specific

Use the ledgers as a cognitive immune system:

- If the pattern is **false**, it belongs in `hallucinations`
- If the pattern is **unclear**, it belongs in `ambiguities`
- If the pattern is **tempting but wrong**, it belongs in `dead_ends`
- If the pattern is **crossing the wrong membrane**, it belongs in `boundary_drifts`

When you need to append a high-order memory item explicitly, use:

```bash
./.agents/skills/alignment/scripts/log-ledger.sh --help
```

The helper now enforces the same quality bar and rejects banned low-signal titles/summaries

## Phase 1: Architectural Forcing Function

Before writing code, output this design matrix:

1. **L0/L1 vs L2 Boundary:** Does the change affect Physics or Tactics?
2. **Ingress Trigger:** What balance ingress or deterministic trigger drives the state change?
3. **O(1) Rule:** Can the design avoid unbounded iteration?
4. **Read-Model Contract:** Is any data silently being pushed on-chain instead of into an indexed/materialized read model?
5. **Anti-Pattern Confession:** What is the lazy DAO/default-substrate version of this solution, and why is DEOS rejecting it?

## Phase 2: The Enforcer & Mentor

Run the architecture auditor after code changes:

```bash
./.agents/skills/alignment/scripts/auditor.sh
```

Behavior:

- By default, it audits **changed Rust lines only** (plus full untracked files) so legacy repo debt inside touched files does not block new work
- Pass a path or `--all` when you intentionally want a full-scope sweep
- If the auditor emits a remedy path, use the `read` tool on that document before fixing code
- Every caught violation is appended to the Hallucination Ledger
- The auditor also hints which higher-order memory surface fits the pattern best (`hallucinations` vs `boundary_drifts` today)

Examples:

```bash
./.agents/skills/alignment/scripts/auditor.sh
./.agents/skills/alignment/scripts/auditor.sh template/pallets/governance/src/lib.rs
./.agents/skills/alignment/scripts/auditor.sh --all
```

## Phase 3: Project Validation Immune System

Fast local audit leaves live in this skill because they encode project-specific coordination memory rather than generic operator automation. Treat this as a hard layering rule: when DEOS needs validation for a new recurring drift class, evolve this skill and wire orchestrators to call it instead of expanding `/scripts` with project-audit knowledge. `/scripts` is for operator workflows; this skill is the project validation immune system.

Prefer the root orchestrator for routine use:

```bash
./scripts/validate-local.sh --audit-only
./scripts/validate-local.sh --audit-only --dependency-audit
```

Direct leaves are available when a task needs a narrow gate:

```bash
./.agents/skills/alignment/scripts/audit-script-entrypoints.sh
./.agents/skills/alignment/scripts/audit-template-readiness.sh
./.agents/skills/alignment/scripts/audit-numeric-parsing.sh
./.agents/skills/alignment/scripts/audit-simulator-determinism.sh
./.agents/skills/alignment/scripts/audit-simulator-consistency.sh
./.agents/skills/alignment/scripts/audit-code-suppressions.sh
./.agents/skills/alignment/scripts/audit-backlog-open-work.sh
./.agents/skills/alignment/scripts/audit-dependency-posture.sh
```

These checks protect lessons discovered during proactive hardening: entrypoint contracts, validation-leaf ownership, launch-readiness smells, complete-literal numeric boundaries, deterministic simulator correctness, simulator suite/documentation mirror consistency, broad-suppression discipline, backlog-as-open-work shape without command inventories, and non-regressive dependency posture.

## Phase 4: The While-True Gatekeeper

If you are operating autonomously, you must pass the gate before the next loop iteration:

```bash
./.agents/skills/alignment/scripts/while-true-gate.sh
```

Behavior:

- Uses the **smallest meaningful validation set** for the changed scope
- Runs architecture audit only when pallet Rust changed
- Runs `bash -n` against changed shell scripts when shell entrypoints changed
- Runs simulator tests only when the touched scope is math-coupled
- Runs `cargo check --workspace` only when Rust workspace files changed
- Requires context sync (`CHANGELOG.md`, `AGENTS.md`, `BACKLOG.md`, or `docs/`) before the next loop by default

Useful flags:

```bash
./.agents/skills/alignment/scripts/while-true-gate.sh --all-rust
./.agents/skills/alignment/scripts/while-true-gate.sh --skip-simulator
./.agents/skills/alignment/scripts/while-true-gate.sh --allow-no-context-sync
```
