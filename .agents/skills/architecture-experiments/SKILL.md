---
name: architecture-experiments
description: Preserves evidence-driven physical architecture experiments, candidate decisions, rejected alternatives, exact baselines, cross-release lineage, and the next optimization gradient without allowing benchmarks to redefine semantics.
---

# Architecture Experiments

Use this skill when implementation work must choose among physical architectures, measured geometry, resource allocations, lowering strategies, or other benchmark-sensitive mechanisms. It makes optimization cumulative across releases by preserving what was tried, against which baseline, with which artifacts, why a candidate won or lost, when evidence became stale, and what experiment should follow.

Experiments are decision instruments, not output. Open one only when a real implementation choice can materially affect a declared project target. Seek the highest decision-relevant performance among designs that preserve explicit functionality, safety, boundedness, and operability constraints. Do not create candidates or benchmarks to exercise the method, fill a portfolio, or accumulate evidence. Measure the smallest comparison that can select or reject a design, stop when the decision is supported, and delete candidate code that does not win.

## Ownership Boundary

This skill owns:

- Falsifiable architecture hypotheses and candidate boundaries.
- Exact baseline and controlled-comparison contracts.
- Durable track-local Experiment Records, status transitions, relations, and track indexes.
- First-class experiment tracks that partition stable physical research domains, baselines, gradients, and cross-track evidence flow.
- Multidimensional interpretation, Pareto classification, architectural decisions, rejected alternatives, invalidation, and next-gradient selection.
- Benchmark design, measurement hygiene, evidence classification, production-Weight handoff, Experimental Closure, and Architecture Provenance judgement.

It does not own:

- Protocol semantics or specification acceptance.
- Benchmark command implementations, generated Weight files, tests, release publication, or architecture-document truth.
- Open-work state, which remains in `BACKLOG.md`.

Root scripts and pallet harnesses mechanically execute measurements; this Skill is the single policy owner for why, what, and how to measure, interpret, decide, and retain architectural evidence. Routine regression checks may use the same method without creating an Experiment Record; open one when a material regression changes assumptions, needs architectural diagnosis, or introduces a candidate.

Experiment evidence is partitioned under [`tracks`](./tracks/). Each `tracks/<track>/experiments.md` owns its charter and canonical local index, and sibling `EXP-NNNN.md` files own evidence. This co-locates method, track direction, rejected alternatives, and cross-release lineage without diffusing history into project documentation. The canonical record template is [`templates/EXP-NNNN.md`](./templates/EXP-NNNN.md). Copy it to `tracks/<track>/EXP-NNNN.md`; canonical identity is `<track>/EXP-NNNN`, while the Markdown title carries the semantic name. Keep compact measurements, observations, interpretation, and decisions directly in that record by default. When justified raw artifacts would make inline Markdown materially worse, place them in the record's sibling directory `tracks/<track>/EXP-NNNN/`. Track qualification is mandatory because numeric IDs may repeat across tracks.

Project documentation may cite Experiment IDs as compact provenance, but it describes only accepted project truth and never owns raw measurements, candidate history, or experiment relations. `BACKLOG.md` owns remaining work. Deleting this Skill intentionally deletes its private experimental-memory capability but must not affect builds, tests, CI, release validation, or runtime behavior.

## Canonical Development Order

Experiments preserve this mandatory order:

```text
Specification
→ Implementation
  ├ candidate construction
  ├ controlled measurement
  ├ evidence recording
  ├ candidate comparison
  └ implementation convergence
→ Tests
→ Implementation Correction
→ Domain Architecture
```

Specification owns semantic architecture: behavior, ordering, atomicity, fairness, failure, causality, ownership, and safety. Experiment Records own why one conforming physical implementation was selected. Architecture documents own current accepted implementation truth after tests and correction converge.

An experiment MUST NOT silently redefine semantics. If evidence shows the specification is defective:

```text
finding → mark affected evidence invalidated
→ explicitly reopen Specification in BACKLOG.md
→ record the semantic conflict and rationale
→ change and accept Specification
→ repeat Implementation → Tests → Correction → Architecture
```

A faster candidate that changes externally observable semantics, deterministic behavior, FIFO, causal speed, atomicity, rollback, ownership, economic behavior, correctness, or production Weight soundness is not an optimization candidate. Reject it from the comparison and route it as a semantic proposal.

## Record Identity and Layout

Use track-local monotonic IDs `EXP-NNNN`, zero-padded to four digits. The canonical identity is `<track>/EXP-NNNN`; the numeric suffix may repeat across tracks. Within one track IDs never encode a release, are never reused, and remain stable across renames, supersession, or invalidation. Allocate the next ID from the highest ID in that track's `experiments.md`; concurrent branches touching the same track must resolve collisions before merge.

```text
.agents/skills/architecture-experiments/
├ SKILL.md
├ scripts/
│  └ validate-record-normalization.sh
├ templates/
│  └ EXP-NNNN.md
└ tracks/
   ├ actors/
   │  ├ experiments.md
   │  ├ EXP-NNNN.md
   │  └ EXP-NNNN/
   │     └ raw-artifact.ext
   ├ adapters/
   │  ├ experiments.md
   │  └ EXP-NNNN.md
   └ router/
      ├ experiments.md
      └ EXP-NNNN.md
```

Every experiment record is copied from `templates/EXP-NNNN.md`; its track-qualified path is stable identity. A Proposed question MAY exist only as a row in that track's `experiments.md`; create the record before transitioning to Prepared. Do not create empty track or artifact directories, semantic filename aliases, or a global evidence directory keyed only by unqualified numeric ID. Keep compact evidence in the record. CSV and TSV are tabular evidence, not standalone artifacts: convert their rows into Markdown tables inside `EXP-NNNN.md`, then delete the temporary delimited file. Never create or retain an `EXP-NNNN/` directory for CSV, TSV, or another delimited-table encoding. Create the sibling `EXP-NNNN/` artifact directory only for non-tabular raw evidence whose fidelity, machine consumption, or reviewability prevents faithful inline retention; artifact filenames describe their workload or candidate without repeating the parent ID.

Each track index is concise navigation plus its stable charter, not experiment evidence. Each prepared-or-later `EXP-NNNN.md` follows the canonical template and links any ID-prefixed sibling artifact. Rejected, Superseded, Invalidated, and Inconclusive records remain permanent and discoverable after candidate code is deleted.

When a compound record obscures independent decisions, retain its bounded decision and necessary evidence, and transfer active questions to explicit successors. Do not create retrospective archives by default. Each track's `experiments.md` is its sole entrypoint and shared metadata owner, including its dependency overview and conditional portfolio; do not split those into separate map or meta-information files.

### Record Normalization

[`templates/EXP-NNNN.md`](./templates/EXP-NNNN.md) is the executable normalization source for metadata field order and second-level section order. Every track record must match that shape exactly; record-specific third-level subsections remain permitted. The record's `Primary track` value must be a relative Markdown link whose label is the containing `<track>` and whose target is sibling `experiments.md`.

Run `./.agents/skills/architecture-experiments/scripts/validate-record-normalization.sh` after creating, moving, or restructuring any Experiment Record or changing the template. The validator discovers every `tracks/<track>/EXP-NNNN.md`, derives the canonical shape from the template, and fails on metadata, section, primary-track drift, or any retained CSV/TSV under `tracks/`. It is private Skill-method validation and must not become a dependency of project validation or the completion gate.

Relations follow the template's ordered fields. `Depends on` means required inputs and forms an acyclic graph; `Uses evidence from` may cite later negative findings without creating a prerequisite cycle. `Refines`, `Confirms`, `Invalidates` and `Supersedes` identify the exact claim and decision scope. `Transfers question to` assigns a question, while `Produces input for` names its downstream deliverable; neither is an automatic acceptance claim. Declare `Reopen trigger` explicitly. Every important claim must lead through its experiment to exact evidence and source/artifact identity, and every decision must expose downstream consequences. Use None when no relation exists rather than inventing causality.

The same validator checks relation shape, experiment references and dependency cycles, and verifies the dependency graph inside the Actors `experiments.md`. After relation changes use `--write-index`, then the ordinary validation route. The graph is an inline projection of record Relations, not another entrypoint. Index-only Proposed IDs remain valid references, but have no inferred decision.

## Experiment Tracks

A track is a stable physical research domain that can carry multiple experiment lineages across releases. It is not a release phase, backlog, folder label, or second experiment index. Create a track only when it has a distinct scope owner, invariant boundary, baseline lineage, reusable question portfolio, and entry/exit rule. Otherwise assign the question to an existing track. A portfolio names possible decision gradients only; it never authorizes an experiment without current implementation pressure.

Each track owns one `tracks/<stable-track-id>/experiments.md` charter/index and sibling Experiment Records. The charter/index owns only:

- Scope and explicit exclusions.
- Governing invariants shared by its experiments.
- Accepted physical baseline references.
- Research question families and next-gradient portfolio.
- Directional cross-track evidence dependencies.
- Entry, transfer, dormancy, and retirement conditions.
- Links to related Experiment IDs.

A track MUST NOT duplicate experiment status, measurements, interpretation, decisions, or open-work state. Those remain in the owning track index, Experiment Records, and `BACKLOG.md` respectively. Every Experiment Record belongs to its containing primary track and MAY link related tracks. The primary track owns the decision question; related tracks receive or provide evidence without becoming co-owners.

Cross-track dependencies must be directional and acyclic. When a proposed experiment would create a cycle, split the question at the evidence boundary or choose the track that owns the changed physical mechanism. Update `experiments.md` when its track boundary, accepted baseline, portfolio, dependency, lifecycle, experiment status, or relation changes. Moving an experiment between tracks changes canonical identity and is allowed only before a record reaches Prepared; otherwise supersede it with a related record in the receiving track.

## Lifecycle

| Status | Meaning | Permitted next states |
| --- | --- | --- |
| Proposed | A decision-relevant hypothesis exists; candidate or controls are not ready | Prepared, Rejected, Superseded |
| Prepared | Baseline, candidates, controls, workloads, criteria, commands, and artifact plan are ready | Measuring, Invalidated |
| Measuring | Controlled execution has begun but required samples or profiles are incomplete | Measured, Inconclusive, Invalidated |
| Measured | Raw or normalized evidence exists; no interpretation is accepted yet | Interpreted, Inconclusive, Invalidated |
| Interpreted | Measurement, uncertainty, Pareto shape, and validity have been analyzed separately from decision | Accepted, Rejected, Inconclusive, Invalidated |
| Accepted | Candidate becomes or materially informs the implementation baseline | Superseded, Invalidated |
| Rejected | Evidence is sufficient not to select the candidate | Superseded, Invalidated |
| Inconclusive | Evidence cannot distinguish candidates or support the decision | Proposed, Superseded, Invalidated |
| Superseded | Later stronger evidence replaces the old decision while preserving its historical validity | Invalidated |
| Invalidated | Changed assumptions mean the evidence no longer supports its prior current claim | Terminal; create or relate a replacement experiment |

Status describes evidence maturity, not code completion. Never jump from Measured to Accepted without an explicit Interpretation. Never rewrite an old decision to imitate later knowledge; append relations and transition it to Superseded or Invalidated with rationale.

Accepted may be scoped to physical architecture: state `Decision scope: physical architecture only` and distinguish Architecture Freeze from later production/release Geometry Freeze. One materially distinct hypothesis has one experiment owner. A scoped acceptance neither proves Weight soundness nor meets a throughput target. Frozen architecture may reopen only under its declared evidence-backed invariant/impossibility trigger; a cost miss, stale artifact or unreachable fixture transfers to the relevant successor. Corrections preserving frozen invariants belong to that successor, and micro-optimizations require a measured production owner plus a dedicated falsifiable question.

## When to Open an Experiment

Open one when all are true:

- A conforming physical choice could materially change a release target, resource bound, lifecycle dispatchability, state footprint, correctness simplicity, or scaling dependency.
- At least two candidates or one candidate plus an exact baseline can be controlled comparably.
- The result can change an implementation decision.
- The smallest falsifying workload and materiality threshold can be stated.

Do not open one for:

- A normative semantic choice.
- Routine regression tests, ordinary profiling with no decision, or generation of already-selected production Weight.
- A cosmetic refactor with no measurable architecture question.
- An idea already rejected under still-valid equivalent conditions.
- Measurement whose result cannot change the implementation.

Before opening, search the index by affected domain, mechanism, question, candidate, and relations. Read every linked Accepted, Rejected, Superseded, or Invalidated predecessor that shares the mechanism. Either refine prior evidence or explain which assumption makes repetition necessary.

## Experiment Protocol

### 1. Lock the Decision Question

State one decision, the owning implementation phase, governing specification sections, and semantic invariants candidates may not change. Separate physical variables from semantic constants.

Declare materiality before measurement. Use a release target, maximum RefTime/ProofSize envelope, lifecycle dispatchability, throughput or latency bound, eliminated read/write/scaling dependency, state-hold reduction, or justified minimum percentage. A tiny numeric win without architectural significance is not material by default.

### 2. Establish the Exact Baseline

Name:

- Baseline Experiment ID when one exists.
- Exact architecture and source commit/tree.
- Runtime configuration and relevant constants.
- Benchmark runtime Wasm hash and final production runtime Wasm hash as separate fields when applicable.
- Generated Weight identity, Rust toolchain, benchmark CLI/configuration, database backend, and workload.
- State population and warm/cold cache assumptions.

Never write “faster than before.” If exact identity is unavailable, narrow the claim or keep the experiment Proposed/Prepared.

### 3. Prepare Minimal Candidates

Construct the smallest candidate implementations capable of falsifying the hypothesis. Preserve one semantic workload and all governing invariants. Keep candidate-only code isolated enough to delete after decision without deleting the record.

List controlled variables and changed variables separately. If multiple variables differ, say so and do not attribute the result to one variable. Use the same toolchain, commands, repeats, state, runtime constants, Weight implementation, database configuration, and cache assumptions unless the changed variable explicitly requires otherwise.

### 4. Measure by Evidence Class

Classify every evidence source:

| Evidence | Authority |
| --- | --- |
| Synthetic microbenchmark | Isolated mechanism behavior; no runtime or release throughput claim |
| Pallet benchmark | Bounded pallet branch, storage model, and generated Weight inputs under benchmark setup |
| Native stress benchmark | Noisy native host/runtime latency, throughput, memory, or queue behavior in the declared environment |
| Integration benchmark | Cross-component behavior and contention for the measured composition |
| Production-Wasm benchmark | Production runtime execution and Weight evidence for measured cases |
| Full-runtime block profile | Composition, contention, throughput, and block-budget evidence |
| Exact release-tree validation profile | Strongest tree-bound release conclusion within recorded conditions |

Production evidence has stronger authority than exploratory evidence. Never let weaker evidence silently override stronger evidence or project a microbenchmark into production truth.

Match statistical method to evidence class. For noisy wall-clock work, declare warmup, repeated-run count and rationale, distribution or relevant percentiles, variance, and predeclared outlier handling; do not hide tails behind an average. Keep setup deterministic and outside the measured region, isolate competing processes, record host/cache conditions, random seed, and contamination. For deterministic/model-generated FRAME Weight, use the required steps/repeats and generated model review rather than irrelevant statistical ceremony.

Record relevant dimensions independently: RefTime, ProofSize, database reads/writes, encoded persistent state, lifecycle Weight, create/update/close cost, state hold, throughput, latency, queue pressure, fragmentation, memory, Wasm size, node wall-clock behavior, and TryRuntime cost. Omit irrelevant metrics explicitly rather than fabricating values.

For parameter geometry, choose candidates from measured boundaries and information value. Avoid blind sweeps; use one only when geometry is unknown, bounded sweep cost is justified, and the result can materially change architecture.

### 5. Execute and Integrate Runtime Benchmarks

For DEOS FRAME work, read the changed call path, benchmark, `WeightInfo`, runtime binding, governing specification, and open experiment before execution. Define bounded components, worst-case state, and postconditions; split branches when proof, reads/writes, cleanup, or failure topology differs. Construct the smallest state that reaches the real worst case, keep setup outside the measured block, assert the intended branch, and use measured ProofSize mode for storage-sensitive paths.

Use [`scripts/benchmarks.sh`](../../../scripts/benchmarks.sh) and its `--help` as the command owner. Run `--check` first, then one focused extrinsic or coherent same-runtime matrix. `--skip-build` is valid only while reusing the same freshly built benchmark runtime; rebuild after source, features, runtime configuration, or toolchain changes. Temporary focused output may support review but never replaces the complete generated pallet file.

For accepted production Weight:

- Keep RefTime and ProofSize separate and model independent stop conditions.
- Review maximum branch ownership, parameterized geometry, storage annotations, and generated database reads/writes against implementation reality.
- Distinguish minimum execution time, actual observed work, declared/charged Weight, and generated model; none substitutes for another.
- Verify benchmark name, `WeightInfo`, generic fallback, runtime implementation, and production binding agree; no placeholder runtime Weight remains.
- Rebuild production Wasm through [`scripts/03-build-runtime.sh`](../../../scripts/03-build-runtime.sh), retain benchmark-runtime and post-generation/final production Wasm identities separately, and run focused compile/check, formatting, Clippy, tests, and changed-scope completion.

A host timing, count ceiling, diagnostic run, or `Weight::MAX` test does not establish ordinary-block capacity. Production claims require runtime-bound generated methods plus production-Wasm or stronger composition evidence.

### 6. Preserve Evidence

Keep every normalized or raw tabular dataset directly in the Markdown record as a compact table. CSV, TSV, and equivalent delimited-table files are temporary interchange only: integrate their complete decision-relevant rows into `EXP-NNNN.md` and delete them before the experiment checkpoint. Size or machine consumption does not justify a separate tabular artifact; split or summarize the inline table without discarding decision-relevant evidence. Retain a separate raw log, plot, binary, trace, or other non-tabular artifact only when fidelity or reviewability genuinely requires another file. Store such artifacts under the sibling track-qualified `tracks/<track>/EXP-NNNN/` directory and link specific files from the record; never use a global `evidence/EXP-NNNN/` path because numeric IDs are track-local. Retain full raw output only when small, uniquely valuable, or required to review/reproduce the decision. Large output may remain external or ephemeral only when the record preserves exact commands, hashes, parameters, environment, and sufficient normalized measurements.

Do not claim reproducibility when an essential artifact or condition was discarded. Never use one ambiguous `artifact_hash`; identify source tree, benchmark Wasm, production Wasm, generated Weight, and raw-output digest separately.

### 7. Separate Result, Interpretation, and Decision

- `Result`: What was measured, including uncertainty and deltas.
- `Interpretation`: What the evidence implies, which dimension binds, limitations, confounders, and Pareto relation.
- `Decision`: Which candidate is selected or why none is selected, against declared criteria.

A lower RefTime does not imply acceptance. Classify candidates as Pareto-improving, Pareto-dominated, a tradeoff, or a binding-dimension winner. Do not collapse dimensions into an arbitrary scalar unless the governing resource policy defines and justifies that objective.

Negative outcomes are valid: no material difference, regression, inconclusive, invalid experiment, rejected candidate, or falsified hypothesis. Record each rejected alternative with evidence and reason.

### 8. Update Baseline and Lineage

For Accepted decisions, name the new physical baseline and relate the record with `replaces`, `refines`, or `validates`. For later contrary evidence, use `supersedes`, `contradicts`, or `invalidates`. Relations are bidirectionally discoverable through index rows and record links.

Update the primary track's accepted baseline only after an Accepted decision changes that track's physical baseline. Preserve `Affected domain` and `Physical mechanism` inside the record as finer evidence labels; they do not replace track ownership. Release remains an index column rather than a release-local archive.

### 9. Derive the Next Gradient

Do not brainstorm from zero. Ask in order:

1. Which target is failing or furthest from its envelope?
2. Which resource dimension binds?
3. Which measured component dominates that dimension?
4. Which physical architecture owns that component?
5. Which conforming change could reduce it?
6. What is the smallest experiment that can falsify the hypothesis?

Record:

```text
target → measured gap → binding dimension → dominant contributor
→ owning mechanism → next hypothesis → smallest falsifier
```

Also record eliminated hypotheses, remaining plausible hypotheses, and the successor Experiment ID when allocated.

### 10. Reconcile Project Truth

- Keep open work only in `BACKLOG.md`; link the Experiment ID rather than duplicating results.
- Keep current implementation truth in code and tests.
- After implementation, tests, and correction converge, cite significant accepted Experiment IDs from architecture prose without copying full tables.
- Keep completed causal history in Experiment Records, not the changelog.
- Update the owning track's `experiments.md` in the same change as every record status or decision transition.

## Validity and Invalidation

Review related records when any dependency changes materially:

- Storage layout or state population geometry.
- Database backend or cache assumptions.
- MaxBlockWeight, ProofSize limit, Actor resource policy, or relevant runtime constants.
- Compiler/toolchain, benchmark method, or Wasm build.
- Task semantics, host adapter, underlying pallet Weight, or production Weight implementation.
- Workload, fairness, queueing, causality, or lifecycle contract.

Use Superseded when stronger evidence replaces a still-historically-valid decision. Use Invalidated when changed assumptions remove support for the old claim. State whether qualitative insight remains useful; never delete the record.

## Closure Gates

### Experimental Closure Gate

Implementation alternatives are converged enough to enter the main Test phase only when:

- Every architecture-affecting alternative is Accepted, Rejected, Inconclusive, or explicitly deferred in `BACKLOG.md` with rationale.
- Every accepted benchmark-sensitive physical choice has an Experiment Record.
- Every rejected candidate remains indexed and discoverable.
- Baseline, artifacts, workloads, measurements, interpretation, decision, and validity are explicit.
- No decision exists only in chat, temporary output, commit messages, or developer memory.

### Architecture Provenance Gate

Domain Architecture may close only when every significant physical decision is traceable to at least one normative specification section, Accepted Experiment Record, production benchmark, or correctness/security invariant. Architecture states current truth and cites provenance compactly; it does not become an experiment log.

## Stop Rules

Stop the experiment loop when any applies:

- The acceptance target is met.
- No candidate has a plausible decision-relevant advantage.
- Remaining delta is below declared materiality.
- The bottleneck moved to another architecture domain.
- Further progress requires semantic change and therefore Specification reopening.
- Evidence is insufficient; mark Inconclusive and state the exact missing evidence.
- The task contract or user says stop.

Do not optimize indefinitely because another microbenchmark improvement is imaginable. Performance never outranks correctness, deterministic semantics, atomicity, FIFO, causal speed, ownership, rollback, runtime safety, or production Weight soundness.

## Track Registry

Track indexes are the canonical navigation surfaces for experiment status and lineage.

| Track | Scope | Index |
| --- | --- | --- |
| Actors | Actor storage, scheduling, control, lifecycle, and executor topology | [tracks/actors/experiments.md](./tracks/actors/experiments.md) |
| Adapters | Runtime adapter boundaries, lowering, and effect-resource evidence | [tracks/adapters/experiments.md](./tracks/adapters/experiments.md) |
| Router | Route search, quote, proof, and execution topology | [tracks/router/experiments.md](./tracks/router/experiments.md) |

Add a track only after its boundary, invariants, portfolio, dependencies, and entry/exit conditions are concrete. Create `tracks/<track>/experiments.md` first; do not reserve empty directories. Track-local indexes allocate IDs, own statuses and relations, and preserve rejected or invalidated records.

## Handoff

Report:

- Experiment ID, status transition, decision question, and mechanism.
- Exact baseline and candidate artifact identities.
- Evidence class, workloads, controlled/changed variables, and validation.
- Result, Interpretation, and Decision as separate statements.
- Pareto class, binding dimension, rejected alternatives, and validity scope.
- New baseline or reason none changed.
- Next gradient or stop condition.
- Index, backlog, and eventual architecture provenance updates.
