---
name: architecture-experiments
description: Preserves evidence-driven physical architecture decisions, bounded comparisons against exact baselines, negative results and the next optimization gradient without letting benchmarks redefine semantics. Keeps only decision records that current work consumes; release tags preserve earlier research. Owns the Benchmark Reassessment Protocol — observations stay permanent while their evidence authority is reassessed.
---

# Architecture Experiments

Use this skill when implementation must choose among physical architectures, measured geometry, resource allocations, lowering strategies or other benchmark-sensitive mechanisms. It makes optimization cumulative: what was tried, against which baseline, why it won or lost, when its evidence became stale, and what should follow.

Experiments are decision instruments, not output. Open one only when a real implementation choice can materially change a release objective, resource bound or bounded comparison against an exact baseline. Measure the smallest comparison that can select or reject a design, stop when the decision is supported, and delete candidate code that does not win.

## Ownership Boundary

This skill owns falsifiable physical hypotheses, exact baseline and controlled-comparison contracts, live Experiment Records, track indexes, interpretation and decisions, benchmark evidence authority, and next-gradient selection.

It does not own protocol semantics, benchmark command implementations, generated Weight, tests, architecture-document truth or open work. `BACKLOG.md` owns remaining work; specifications own semantics; code, tests and generated artifacts own implementation truth. Project documentation may cite a live Experiment ID as compact provenance. Deleting this skill must not affect builds, tests, CI, release validation or runtime behavior.

## Canonical Development Order

```text
Specification → Implementation (candidates, controlled measurement, decision) → Tests → Correction → Domain Architecture
```

An experiment MUST NOT silently redefine semantics. If evidence shows the specification is defective, mark the affected evidence invalidated, reopen the specification in `BACKLOG.md`, and repeat the order after the specification changes. A faster candidate that changes observable semantics, determinism, FIFO, causal speed, atomicity, rollback, ownership, economic behavior or production Weight soundness is a semantic proposal, not an optimization candidate.

## Records and Layout

```text
.agents/skills/architecture-experiments/
├ SKILL.md
├ scripts/validate-record-normalization.sh
├ templates/EXP-NNN.md
└ tracks/<track>/
   ├ experiments.md      charter, Current Records, hazards, principles, history
   └ EXP-NNN.md          live records only
```

- `Identity`: IDs are `EXP-NNN`, globally unique across tracks and never reused within the current numbering; allocate one plus the highest live ID or the highest ID deleted since the restart.
- `Live set`: A record stays in the tree only while it is a track's accepted physical baseline or a current `BACKLOG.md` owner consumes it. Each live record has one `Current Records` row naming its consumers (`Baseline` or backticked backlog owners).
- `Deletion`: A record without a consumer is deleted at the next release boundary; the release tag that contained it preserves its text. Never cite a deleted record by ID — restate a still-useful general finding in `Retained Principles` without numbers. Records must not cite commits of an unreleased branch, which release squashing erases; cite content hashes, release tags or file paths.
- `One record per mechanism family`: Each record owns one physical mechanism family (unique `Physical mechanism`). A new design question about that mechanism becomes a new obligation row in its record, never a new record; open a new record only for a mechanism family no record owns. A track should hold a handful of live records; the validator warns above six.
- `Shape`: Copy [`templates/EXP-NNN.md`](./templates/EXP-NNN.md); metadata fields, second-level sections and relation fields must match it. Keep evidence inline as Markdown tables; CSV/TSV files are forbidden. A sibling `EXP-NNN/` directory is allowed only for non-tabular raw evidence that cannot be retained faithfully inline.
- `Size`: Keep only decision-relevant numbers, identities and limitations. Chronology, repaired probes and superseded diagnostics belong to Git history, not the record.

## Release-Boundary Consolidation

At every release boundary and whenever a mechanism is deleted:

1. Delete every record without a current consumer.
2. Merge records that share a mechanism family into one record, carrying forward decisions, binding numbers, open falsifiers and limitations.
3. Restate reusable general findings from deleted records in the index's `Retained Principles`, without IDs or numbers.
4. Repoint repository references to live IDs, then run the validator.

When the source tree deletes a mechanism a record measured, delete the record; reintroducing the mechanism requires a new record. A record that only used a retired workload to witness a surviving owner stays, and its consumer must re-witness that owner on the current tree.

## What Is Not an Experiment

These never become records or obligation rows: whether a generated Weight owner bounds a selected geometry (benchmark coverage; witnesses go to the owning `BACKLOG.md` item and then to the benchmark suite), workload and measurement contracts (the performance-assurance specification), fixture defects, diagnostics, regression checks and production Weight generation. A record is justified only by a physical design choice between named candidates whose outcome changes the architecture.

## Proof Obligations

Before Prepared, enumerate a finite `Proof Obligations` table (ID, claim, smallest falsifier, evidence class, status, consumer) and freeze it at Measuring. A later question about the same mechanism becomes a new obligation row; do not extend one notebook with diagnostics. Satisfied is an evidence decision with scope, not a passing fixture. An ordinary correctness bug earns nothing unless it exposes a physical choice or invalidates a load-bearing assumption.

## Correction Boundary

Experiments preserve causal decision evidence; Git owns implementation chronology, tests own regressions, and `BACKLOG.md` owns unfinished work. For an invalid setup or ordinary bug, fix code or test, rerun the smallest affected proof, and keep at most one line: `Invalid probe: assumption X failed because Y; corrected fixture uses Z.`

## Validation

Run `./.agents/skills/architecture-experiments/scripts/validate-record-normalization.sh` after changing any record, index or the template, and `--self-test` after changing the validator. It checks template shape, one record per mechanism family, obligations, freeze, disposition fields, Current Records rows with live backlog consumers, ID uniqueness, the absence of pre-restart four-digit IDs, relation targets, the hard-dependency DAG and links. It proves reference integrity only, never semantic equivalence or Weight sufficiency. It is Skill-private and never a project build or completion-gate dependency.

## Experiment Tracks

A track is a stable physical research domain with its own scope, invariants, baseline and entry/exit rule; it is not a release phase or a second backlog. Each `tracks/<track>/experiments.md` owns only the charter, current baseline, Current Records, Retained Principles, research portfolio and a one-paragraph history. Cross-track hard dependencies stay directional and acyclic.

| Track | Scope | Index |
| --- | --- | --- |
| Actors | Actor storage, scheduling, control, lifecycle and executor topology | [tracks/actors/experiments.md](./tracks/actors/experiments.md) |
| Adapters | Runtime adapter boundaries, lowering and effect-resource evidence | [tracks/adapters/experiments.md](./tracks/adapters/experiments.md) |
| Router | Route search, quote, proof and execution topology | [tracks/router/experiments.md](./tracks/router/experiments.md) |

## Lifecycle

| Status | Meaning | Next |
| --- | --- | --- |
| Proposed | Decision-relevant hypothesis; candidates or controls not ready | Prepared, Rejected, Superseded |
| Prepared | Baseline, candidates, controls, workloads, criteria and commands ready | Measuring, Invalidated |
| Measuring | Execution begun; samples incomplete | Measured, Inconclusive, Invalidated |
| Measured | Evidence exists; no accepted interpretation | Interpreted, Inconclusive, Invalidated |
| Interpreted | Uncertainty, Pareto shape and validity analyzed | Accepted, Rejected, Inconclusive, Invalidated |
| Accepted | Candidate becomes or informs the implementation baseline | Superseded, Invalidated |
| Rejected | Evidence suffices not to select the candidate | Superseded, Invalidated |
| Inconclusive | Evidence cannot decide | Proposed, Superseded, Invalidated |
| Superseded | Stronger evidence replaces a historically valid decision | Invalidated |
| Invalidated | Changed assumptions remove support for the claim | Terminal |

Status is evidence maturity, not code completion. Never jump from Measured to Accepted without Interpretation, and never rewrite an old decision to imitate later knowledge. Accepted may be scoped (`Decision scope: physical architecture only`); scoped acceptance proves neither Weight soundness nor throughput. A frozen architecture reopens only through a structural/soundness falsifier, or admits one bounded same-semantics comparison after a predeclared missed criterion with one causally implicated owner and one concrete alternative. A stale artifact, unreachable fixture, provisional Weight or unrelated old coefficient opens neither route.

## When to Open an Experiment

Open one only when a conforming physical choice could materially change a release objective, resource bound, dispatchability, state footprint or scaling dependency; at least two candidates, or one candidate plus an exact baseline, can be compared under control; the result can change an implementation decision; and the smallest falsifying workload and materiality are statable. Do not open one for semantic choices, routine regression or profiling, generation of already-selected Weight, cosmetic refactors, or ideas rejected under still-valid conditions. Search live records and Retained Principles first (and earlier release tags when needed), and reuse a prior result only while workload, semantics, resource policy, host, artifact identity and method still apply.

## Bounded Comparative Research

A sealed baseline stays historically valid but is not a protected implementation. A comparison against it needs one decision it can change, the exact baseline identity, one smallest comparative claim under matching conditions (or a labelled bridge), materiality declared from the decision, and a candidate set bounded before measurement. Name exactly one outcome class — `Efficiency improvement`, `Explicit tradeoff` or `Research result` — and a finite stopping basis. Cost moved to another resource, phase, state or weaker guarantee is a tradeoff, not an efficiency gain. When every admitted candidate fails, close with a no-optimization research result; a new candidate needs a new scope decision.

## Benchmark Reassessment Protocol

Protocol semantics, storage topology, counters, ProofSize and database shapes may be exact; RefTime is an empirical observation. `Observation identity is permanent; evidence authority is revisable.` Record source, Wasm, Weight, command, parameters, toolchain and date once; keep the current disposition (`Authoritative`, `Qualified`, `Historical`, `Superseded`, `Invalidated`, `Inconclusive`, `Not applicable`) in the record's `Benchmark Evidence Disposition`.

- `Triggers`: Reassess when the same source and command give materially different RefTime, untouched methods move together, structural work is identical while RefTime moves, host conditions differ between baseline and candidate, tooling changes, or a delta is of the order of observed noise. A trigger requires review, not rejection.
- `Matched comparison`: Prefer `A → B → A'` or repeated A/B on the available host. If `A` and `A'` differ as much as `A` and `B`, the RefTime comparison is Inconclusive. A stable-sign matched delta that clearly exceeds observed noise, with agreeing structural metrics, may decide an optimization.
- `Dimensions`: Keep ProofSize, storage proof bytes, reads/writes, state bytes, committed Steps, latency and RefTime separate; stable structural evidence survives unstable RefTime.
- `Production Weight`: An existing owner stays authoritative only when the executed branch, storage access, input domain, maximum geometry and mandatory work are unchanged; any new selector or control flow needs its own conservative owner, because no storage access is not zero RefTime. A changed path needs a new sound owner. An environmentally unstable generation is inadmissible evidence, not a reason to keep a stale owner. Freshness is not evidence; applicability, ownership and soundness are.
- `Anti-cherry-picking`: Declare the method before the decision run, apply one inclusion rule to baseline and candidate, record invalid-run reasons, and stop when evidence is sufficient or Inconclusive.

## Experiment Protocol

1. `Lock the question`: One decision, governing specification sections, finite obligations and materiality declared before measurement.
2. `Exact baseline`: Commit/tree, runtime constants, benchmark and production Wasm, generated Weight, toolchain, command, database, workload, population and cache assumptions. Never write "faster than before".
3. `Minimal candidates`: The smallest implementations able to falsify the hypothesis; list controlled and changed variables; keep candidate-only code deletable.
4. `Measure by evidence class`: Microbenchmark, pallet benchmark, native stress, integration, production-Wasm, full-runtime block and release-tree validation have increasing authority; weaker evidence never silently overrides stronger. Use warmup, repeats and percentiles for wall-clock work, and generated-model review for deterministic FRAME Weight.
5. `Runtime benchmarks`: Read the call path, benchmark, `WeightInfo`, binding and specification first; construct the smallest state reaching the real worst case with setup outside the measured block; assert the branch; use measured ProofSize for storage-sensitive paths. [`scripts/benchmarks.sh`](../../../scripts/benchmarks.sh) owns commands (`--check` first; `--skip-build` only with the same fresh runtime). Production binding rebuilds Wasm through [`scripts/03-build-runtime.sh`](../../../scripts/03-build-runtime.sh) and keeps benchmark and production Wasm identities separate. A host timing or `Weight::MAX` test never establishes ordinary-block capacity.
6. `Result, Interpretation, Decision`: Keep them separate. Classify candidates as Pareto-improving, dominated, tradeoff or binding-dimension winner; never collapse dimensions into an unjustified scalar. Negative results are first-class; record them once and stop.
7. `Next gradient`: `target → measured gap → binding dimension → dominant contributor → owning mechanism → next hypothesis → smallest falsifier`.
8. `Reconcile`: Update the track index with every status change, keep open work in `BACKLOG.md`, and cite accepted IDs from architecture prose only after tests and correction converge.

## Validity and Stop Rules

Review affected records when storage layout, population geometry, database or cache assumptions, block limits or resource policy, toolchain or benchmark method, Task semantics, adapters, Weight implementation, or workload/fairness/lifecycle contracts change. Use Superseded for stronger replacement evidence and Invalidated when assumptions no longer hold.

Stop when the objective or materiality is met, the obligation is satisfied, a new question appears (add an obligation row to its mechanism record, or a new record only for an unowned mechanism family), the next work belongs to another owner, no candidate has a plausible advantage, the frozen candidate set is exhausted, the remaining delta is below materiality, the bottleneck moved, progress needs a semantic change, evidence is insufficient (mark Inconclusive and name the missing evidence), or the user says stop. "There might be another corner case" is insufficient; name the uncovered owner or domain. Performance never outranks correctness, determinism, atomicity, FIFO, causal speed, ownership, rollback, runtime safety or production Weight soundness.

## Closure Gates

- `Experimental closure`: Every architecture-affecting alternative is decided or explicitly deferred in `BACKLOG.md`; every accepted benchmark-sensitive choice has a live record or a Retained Principle; every retained production Weight owner is sound for the segment it charges; no decision exists only in chat, temporary output or commit messages.
- `Architecture provenance`: Every significant physical decision traces to a specification section, a live Experiment Record, a production benchmark or a correctness invariant; architecture cites provenance compactly and never becomes an experiment log.

## Handoff

Report the Experiment ID and status transition, question and mechanism, baseline and candidate identities, evidence class and workloads, Result/Interpretation/Decision with outcome class and stopping basis, benchmark disposition and noise envelope, binding dimension and rejected alternatives, new baseline or why none changed, the next gradient, and index/backlog/architecture updates.
