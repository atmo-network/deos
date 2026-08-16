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

Use repository-relative paths, and garbage-collect repeated bare failures instead of preserving operator-local diagnostics as durable memory.

Use the helper only when the lesson meets that bar:

```bash
./.agents/skills/alignment/scripts/log-ledger.sh --help
```

## 3. Project Audit Layer

Project-specific audit leaves live here because they encode DEOS coordination memory. `/scripts` may orchestrate them, but durable project-audit knowledge should stay in this skill layer.

Default changed-scope route:

```bash
./.agents/skills/alignment/scripts/completion-gate.sh
```

Select additional leaves only when the touched contract requires them. Routing follows progressive enhancement and graceful degradation: begin with the smallest sufficient canonical route, escalate through declared layers, and represent unavailable optional evidence as an explicit narrower result. Never replace a missing required layer with a weaker semantic claim.

| Changed scope | Targeted route | Excluded by default | Escalation trigger |
| --- | --- | --- | --- |
| Docs or context | Completion gate | Cargo, simulator, client, network | Owning code/math/wiki also changed |
| One Rust package | `ci-local.sh --only CHECK --package NAME` for each required check | Other packages, Wasm, network | Cross-package/runtime boundary changed |
| One Rust test family | Add `--test-filter NAME` to scoped tests | Unrelated tests | Shared state or integration behavior changed |
| Actors scheduler slice | `actors-assurance.sh --quick`, then completion gate | Full stress and occupancy profile | Capacity, fairness, liveness, or release gate changed |
| Benchmark code | `benchmarks.sh --check`, then one exact extrinsic or owning pallet | Other pallets and runtime release build | Production weights or Wasm accepted |
| Runtime integration | Scoped `deos-runtime` tests, then completion gate | Full workspace, E2E, client | Runtime metadata/Wasm or network behavior changed |
| Staking delivery | `staking-delivery` readiness ladder through the shared bootstrap script | Signing, funds, governance execution | Explicit target state and mutation approval exist |
| Wiki only | Wiki trust/consolidation leaves | Client build and Cargo | Renderer/client contract changed |
| Release validation | `validate-local.sh [--fresh] PROFILE` | Mutable external/network/release evidence and all substage reuse | Explicit `fast`, `heavy`, or `full` boundary |

Network-backed dependency posture remains opt-in:

```bash
./.agents/skills/alignment/scripts/audit-dependency-posture.sh
```

Narrow leaves are available under:

```bash
./.agents/skills/alignment/scripts/<audit-name>.sh --help
```

Current audit families cover Rust architecture drift, architecture-document readability, economic-claim anchors/falsification inventory, script entrypoint and skill-metadata contracts, template readiness, numeric parsing, simulator determinism/mirror sync, code suppressions, backlog shape, release-line/package-marker consistency, strategic-governance ingress and shortcut absence, protocol-coherence semantic-owner regressions, fail-closed Error Narrowness discovery and typed witnesses, repository portability, wiki trust/consolidation, dependency posture, runtime-source test gating, and the repo-local completion gate.

The Error Narrowness evidence owner recursively checks the five pallet source roots, the complete runtime source root, and the client library root. It snapshots path/exclusion, source, public-result-boundary, exact result-expression, witness/test-file, and explicit-command identities, then verifies only typed-signature, public-root execution, exhaustive-classification, and conversion-edge witnesses. The completion gate executes every declared command and every cited executable anchor. It makes no inferred constructor, universal reachability, semantic-duplication, or closure claim:

```bash
node ./.agents/skills/alignment/scripts/audit-semantic-surface.mjs --check .agents/skills/alignment/semantic-surface.v1.json
node ./.agents/skills/alignment/scripts/audit-semantic-surface.mjs --inventory
```

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

## 5. Completion Gate

Local delivery slices should pass the repo-local completion gate before continuing or committing:

```bash
./.agents/skills/alignment/scripts/completion-gate.sh
```

The gate runs the smallest meaningful changed-scope set: architecture audit, shell syntax, simulator, Cargo checks, runtime unit tests for runtime-source changes, Markdown table/readability checks, wiki trust, release-line audit, and knowledge sync as applicable. It is a project validation entrypoint, not a dependency on any one operator's local execution-loop skill.

Before checkpointing a candidate for reusable profile evidence, run canonical formatting and the intended profile; changed-scope completion does not establish full-profile readiness.

Whole-profile local reuse remains support logic beneath `scripts/validate-local.sh`, which alone owns stages, order, proof-required repetitions, and pass/fail. Only an exact clean immutable-input identity—including every effective Cargo/npm configuration not proven to be an exact candidate-tree regular file—may reuse a Git-common-dir v2 success record, and the complete identity is rechecked immediately before return; dirty local `fast|heavy` runs execute uncached, while CI and `full` fail closed. Required repetition boundaries must be reported in exact reached order after their commands succeed. Missing, empty, invalid, or drifted `full` artifacts cause an ordinary miss and canonical rerun, and network, mutable refs/freshness, release, or publication evidence remains outside this cache. Inherited semantic controls fail before lookup, while test-only numeric controls accept complete unsigned safe-integer literals only. GitHub PR/main reuse remains a narrow provenance layer over the same exact local identity: the stable `validation-gate` accepts only API-confirmed same-repository PR success under unchanged workflow authority and otherwise runs canonical `--fresh fast`; release-candidate `full` stays separately fresh and ref-sensitive.

`auditor.sh` and `completion-gate.sh` use compact orchestration output by default: successful runs report only the step, duration, and result. Failures retain complete temporary logs and print a bounded tail; `DEOS_VERBOSE=1` restores the full nested protocol for diagnosis.

Useful flags:

```bash
./.agents/skills/alignment/scripts/completion-gate.sh --all-rust
./.agents/skills/alignment/scripts/completion-gate.sh --skip-simulator
./.agents/skills/alignment/scripts/completion-gate.sh --skip-runtime-tests
./.agents/skills/alignment/scripts/completion-gate.sh --allow-no-context-sync
```
