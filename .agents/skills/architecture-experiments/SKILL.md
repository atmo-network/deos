---
name: architecture-experiments
description: Preserves evidence-driven physical architecture experiments, bounded comparative research against sealed baselines, candidate decisions, explicit tradeoffs, negative results, exact baselines, cross-release lineage, and the next optimization gradient without allowing benchmarks to redefine semantics. Owns the Benchmark Reassessment Protocol — observations stay permanent while their evidence authority is reassessed, reused, qualified, superseded or invalidated.
---

# Architecture Experiments

Use this skill when implementation work must choose among physical architectures, measured geometry, resource allocations, lowering strategies, or other benchmark-sensitive mechanisms. It makes optimization cumulative across releases by preserving what was tried, against which baseline, with which artifacts, why a candidate won or lost, when evidence became stale, and what experiment should follow.

Experiments are decision instruments, not output. Open one only when a real implementation choice can materially affect a declared release objective, resource policy, or bounded comparative decision against a sealed baseline. Seek the highest decision-relevant performance among designs that preserve explicit functionality, safety, boundedness, and operability constraints. Do not create candidates or benchmarks to exercise the method, fill a portfolio, or accumulate evidence. Measure the smallest comparison that can select or reject a design, stop when the decision is supported, and delete candidate code that does not win.

## Ownership Boundary

This skill owns:

- Falsifiable architecture hypotheses and candidate boundaries.
- Exact baseline and controlled-comparison contracts.
- Durable track-local Experiment Records, status transitions, relations, and track indexes.
- First-class experiment tracks that partition stable physical research domains, baselines, gradients, and cross-track evidence flow.
- Multidimensional interpretation, Pareto classification, architectural decisions, outcome classification, rejected alternatives, invalidation, finite stopping, and next-gradient selection.
- Benchmark design, measurement hygiene, evidence classification, production-Weight handoff, Experimental Closure, and Architecture Provenance judgement.

It does not own:

- Protocol semantics or specification acceptance.
- Benchmark command implementations, generated Weight files, tests, release publication, or architecture-document truth.
- Open-work state, which remains in `BACKLOG.md`.

Root scripts and pallet harnesses mechanically execute measurements; this Skill is the single policy owner for why, what, and how to measure, interpret, decide, and retain architectural evidence. Routine regression checks may use the same method without creating an Experiment Record; open one when a material regression changes assumptions, needs architectural diagnosis, or introduces a candidate.

Experiment evidence is partitioned under [`tracks`](./tracks/). Each `tracks/<track>/experiments.md` owns its charter and canonical local index, and sibling `EXP-NNNN.md` files own evidence. This co-locates method, track direction, rejected alternatives, and cross-release lineage without diffusing history into project documentation. The canonical record template is [`templates/EXP-NNNN.md`](./templates/EXP-NNNN.md). Copy it to `tracks/<track>/EXP-NNNN.md`; canonical identity is `<track>/EXP-NNNN`, while the Markdown title carries the semantic name. Keep compact measurements, observations, interpretation, and decisions directly in that record by default. When justified raw artifacts would make inline Markdown materially worse, place them in the record's sibling directory `tracks/<track>/EXP-NNNN/`. Numeric IDs are globally unique across tracks; track-qualified paths retain explicit evidence ownership.

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

Use globally sequential IDs `EXP-NNNN`, zero-padded to four digits, across every track. A numeric ID has exactly one live owner and never encodes a track or release; the canonical ownership path remains `<track>/EXP-NNNN`. Ordinary allocation takes one plus the highest current ID or unqualified reserved former ID across all track indexes and records, including index-only Proposed questions. Never restart numbering for a new track or fill gaps through ordinary allocation. Resolve concurrent allocation collisions before merge; the track indexes remain the sole navigation owners, with no duplicate global registry.

Identity seals on the first Accepted, Rejected, Inconclusive, Superseded, or Invalidated decision. Sealed IDs MUST NOT be renumbered, including an Inconclusive record later reopened as Proposed. Proposed, Prepared, Measuring, and Measured identities may be normalized only through an explicit graph migration that improves proof/dependency ordering, atomically rewrites every repository reference, preserves former identity in `Former IDs`, prevents ambiguous reuse of vacated numbers, and retains an inspectable Git baseline. Interpreted records are not eligible for renumbering. An explicit provisional-number compaction may reuse a vacated navigation number only when every former occurrence is qualified as `<track>/EXP-NNNN@<full Git baseline commit>`, the manifest binds that historical identity to its current owner, and all live references resolve to the new canonical record. Unqualified former numbers remain reserved globally across tracks. Sealed identities are never eligible for this compaction. A renumber changes navigation, never evidence identity, semantics, numbers, or historical decisions. No duplicate alias files remain. Cross-track identity moves obey the same restrictions.

Before migration, record exact content hashes and modes of every existing file to be structurally migrated, its status and the Git baseline. During migration, a Skill-private manifest owns this provenance, not another graph or evidence archive. Record the former-to-current mapping and evidence extraction coverage. The validator checks the baseline identities, sealed-ID stability and absence of live former references; only explicit `Former IDs` fields or clearly labelled former-ID historical prose may retain those names. After the migration and manifest are committed and validated, retire the completed working-tree manifest when its exact full Git commit and repository path remain cited in the affected records or owning index, so hashes, mapping and extraction coverage stay retrievable. Git then owns the completed migration provenance; no live manifest or migration-only validator machinery is required for ordinary record maintenance. Preserve primary experimental evidence in its live record regardless of manifest retirement.

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

Every experiment record is copied from `templates/EXP-NNNN.md`. A Proposed question MAY exist only as a row in its track index; create the record before Prepared. Do not create empty track or artifact directories, semantic filename aliases, or a global evidence directory keyed only by unqualified numeric ID. Keep compact evidence in its Leaf. CSV and TSV are tabular evidence, not standalone artifacts: convert rows into Markdown tables in the owning Leaf and delete the temporary delimited file. Create a sibling `EXP-NNNN/` directory only for non-tabular raw evidence whose fidelity, machine consumption, or reviewability prevents faithful inline retention; filenames describe the workload or candidate without repeating the parent ID.

Each track index is concise navigation plus its stable charter, not experiment evidence. Each prepared-or-later `EXP-NNNN.md` follows the canonical template and links any ID-prefixed sibling artifact. Rejected, Superseded, Invalidated, and Inconclusive records remain permanent and discoverable after candidate code is deleted.

Each track's `experiments.md` is its sole graph projection and entrypoint, including its conditional portfolio. Do not create separate global maps, measurement archives, or duplicated full tables to conceal a compound record.

## Proof Graph Nodes

Every Prepared-or-later record declares `Record kind` as exactly `Leaf` or `Synthesis`; declare it for Proposed records when their boundary is known. Records are proof graph nodes, not research notebooks.

- `Leaf`: Own exactly one materially distinct falsifiable claim, one physical mechanism/owner, changed variable, evidence domain and acceptance criterion. Multiple workloads are permitted only as bounded witnesses of that same claim. Own primary measurements, preserve their limitations, and terminate when decided.
- `Synthesis`: Own a broader decision through a finite child-obligation map, child decisions, composition logic and final interpretation/decision. Never introduce a new candidate implementation, raw benchmark sweep, independently measured branch, or chronological debugging transcript. Measurements contains child-decision pointers only; no primary raw benchmark tables anywhere in the record.
- `Independence test`: Does new work test the same claim under the same changed mechanism, owner, domain and acceptance criterion? If yes, continue the Leaf; otherwise find or allocate the owning Leaf and transfer the question. A stricter witness stays only when all five boundaries are unchanged.
- `Mandatory fission`: A separately decidable or reusable claim requires its own Leaf: a distinct mechanism, Weight owner, production selector, reachable-state domain, host assumption, acceptance criterion, changed variable, or artifact-identity question whose result could independently be Accepted, Rejected, Inconclusive or Invalidated without deciding the parent. Additional fixtures, workload instances, parameter values, and stricter evidence classes for the same claim are bounded-witness growth, not automatic fission; they stay in the Leaf while the independence test's five boundaries are unchanged. A result reused by multiple downstream decisions normally needs its own identity.
- `No speculative nodes`: An ordinary correctness bug does not automatically earn an EXP. Allocate only when it exposes a physical choice, invalidates a load-bearing assumption, or creates a reusable independent proof question.

## Proof Obligation Freeze

Before Prepared, enumerate a finite `Proof Obligations` table: Obligation ID, Claim, Smallest falsifier, Evidence class, Owning Experiment, Required/conditional, Downstream consequence, Status. A Leaf has one owning claim with bounded witnesses; a Synthesis has the complete finite child-obligation map and every mandatory row names its experiment owner. Conditional rows name the condition that would make them necessary; they are not silent release gates. Satisfied is an evidence decision with scope, not a synonym for a passing fixture.

Record `Freeze`, `Review triggers`, and `Decomposition review` in that section. Freeze the obligation set at Measuring. Migration may reconstruct the previously implicit set only with explicit pre-fission provenance; it must not pretend the historical freeze occurred earlier. Discovering a materially independent obligation after freeze permits only the smallest evidence needed to classify independence, then find/create the child, add the graph edge, transfer the question and stop investigating it in the parent. Updating a child/status pointer preserves the frozen claim; adding an independent claim does not.

Mandatory decomposition review triggers are more than six independent third-level Measurement proof subsections, a new mandatory obligation after Measuring began, measuring a new Weight owner, investigating a new reachable-state domain beyond its smallest falsifier, introducing a new production selector, or reusing a result in multiple downstream decisions. Declare any such event in `Review triggers`, including events not inferable mechanically. `Decomposition review` records the boundary decision and transfer destination or why bounded witnesses still share exactly one claim. A review cannot waive mandatory fission for an independent claim. Large byte/line counts emit nonfatal diagnostics only; size never decides whether to split.

## Active Record Fission

For an active compound Measuring record, preserve exact evidence before editing, then apply this sequence:

1. Retain the original broad question as Synthesis and freeze its finite obligations with migration provenance.
2. Partition evidence by independently decidable claim and actual owner, not mechanically by subsection or benchmark name.
3. Allocate Leaf owners for those questions and move detailed decision evidence into them, without copying full tables into both parent and child.
4. Preserve every original measurement number, command, source/Wasm/output hash, rejection and evidence limitation. Each Leaf explicitly names the pre-fission parent subsection and baseline; exact extraction coverage stays inspectable.
5. Replace parent detail with compact provenance and child decisions. Add reciprocal decomposition, obligation ownership, hard dependencies where actually required, and evidence-flow relations.
6. Normalize eligible IDs only after partitioning, retain former-ID provenance and atomically rewrite repository navigation, then validate all three graph projections.
7. Continue only in the Leaf owning the current unanswered proof. Reuse extracted evidence without measurement when the measured implementation and exact identity remain unchanged and its evidence class still applies; otherwise mark the affected proof as requiring refresh.

Evidence relocation generates no new runtime artifact, production acceptance, or historical continuity. Do not reopen a frozen architectural decision or regenerate a frozen semantic oracle merely because the graph changes.

## Correction Boundary

Experiments preserve causal decision evidence; Git owns ordinary implementation chronology, tests own regression behavior, and BACKLOG owns unfinished work. Classify invalid setup, test expectation defects, clock mistakes, unsupported host assumptions and ordinary implementation bugs. If no reusable architectural knowledge results, fix code/test, rerun the smallest affected proof and keep only a compact note: `Invalid probe: assumption X failed because Y. No measurement retained. Corrected fixture uses Z.` Preserve any decision-relevant rejection, exact artifact identity and causal limitation during migration; compress routine repair chronology through inspectable Git/test provenance, not silent evidence deletion. A reusable failed host assumption may deserve a Leaf; an ordinary repair transcript does not.

## Record Normalization

[`templates/EXP-NNNN.md`](./templates/EXP-NNNN.md) is the executable normalization source for metadata field order and second-level section order. Every track record must match that shape exactly; record-specific third-level subsections remain permitted. The record's `Primary track` value must be a relative Markdown link whose label is the containing `<track>` and whose target is sibling `experiments.md`.

Run `./.agents/skills/architecture-experiments/scripts/validate-record-normalization.sh` after creating, moving, or restructuring any Experiment Record or changing the template. The validator discovers every `tracks/<track>/EXP-NNNN.md`, derives the canonical shape from the template, and fails on metadata, section, primary-track drift, or any retained CSV/TSV under `tracks/`. It is private Skill-method validation and must not become a dependency of project validation or the completion gate.

Relations follow the template's ordered fields and expose three distinct graphs:

- `Decomposition`: `Parent question` metadata and reciprocal `Decomposes into` relation connect a broad question to child proofs; `Satisfies obligation` identifies the parent's obligation ID. This graph has no self/cyclic decomposition and does not imply prerequisite ordering.
- `Hard dependencies`: `Depends on` names required accepted input to a downstream decision and remains a DAG. A child need not depend on its parent; a parent may depend on the children's completed proofs.
- `Evidence flow`: `Uses evidence from`, `Produces input for`, `Confirms` and `Invalidates` express scoped reuse or impact, not prerequisite ordering. `Refines`, `Supersedes` and `Contradicts` preserve decision scope; `Transfers question to` names the next owner. None of these silently creates a hard dependency.

Declare `Reopen trigger` explicitly. Every important claim leads through its owning experiment to exact evidence and source/artifact identity, and every decision exposes downstream consequences. Use None when absent rather than inventing causality.

The validator checks shape/kinds/obligation owners, reciprocal parent-child/obligation relations, hard and decomposition cycles, global numeric uniqueness (including index-only Proposed questions), sealed and former identity rules, prohibited Synthesis primary evidence and Leaf review conditions. It verifies visually separate decomposition, hard-dependency and evidence-flow projections inside the track index. After relation changes use `--write-index`, then ordinary validation; `--self-test` exercises positive and negative methodology fixtures. It remains Skill-private, never a runtime/project build dependency. Index-only Proposed IDs remain valid references without an inferred decision.

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

Cross-track hard dependencies must be directional and acyclic. Split a cyclic question at its evidence boundary or choose the track owning the changed mechanism. Update the index when track boundary, baseline, portfolio, lifecycle, status or relations change. Cross-track moves change identity and obey the provisional/sealed migration rules; sealed questions transfer through a new related record.

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

Benchmark evidence authority is a separate, revisable axis. A sealed record's current disposition may become `Qualified`, `Confirmed`, `Superseded` or `Invalidated` without mutating its historical status transition; record it in the record's `Benchmark Evidence Disposition` surface, name the reassessing owner there, and reflect the current disposition in the track index row.

Accepted may be scoped to physical architecture: state `Decision scope: physical architecture only` and distinguish Architecture Freeze from later production/release Geometry Freeze. One materially distinct hypothesis has one experiment owner. A scoped acceptance neither proves Weight soundness nor meets a throughput target. A frozen architecture reopens directly only under its declared evidence-backed invariant/impossibility trigger; a cost miss, stale artifact or unreachable fixture alone transfers to the relevant successor. A bounded comparative candidate may instead compete against the frozen baseline under the bounded comparative research rules below without reopening the decision or disputing its evidence; selecting it records a Superseded transition with preserved historical validity. Corrections preserving frozen invariants belong to that successor, and micro-optimizations require a measured production owner plus a dedicated falsifiable question.

## When to Open an Experiment

Open one when all are true:

- A conforming physical choice could materially change a release objective, resource bound, lifecycle dispatchability, state footprint, correctness simplicity, or scaling dependency, or a sealed-baseline comparison can decide an efficiency improvement, explicit tradeoff, or limitation.
- At least two candidates or one candidate plus an exact baseline can be controlled comparably.
- The result can change an implementation decision.
- The smallest falsifying workload and materiality threshold can be stated.

Do not open one for:

- A normative semantic choice.
- Routine regression tests, ordinary profiling with no decision, or generation of already-selected production Weight.
- A cosmetic refactor with no measurable architecture question.
- An idea already rejected under still-valid equivalent conditions.
- Measurement whose result cannot change the implementation.

Before opening, search the index by affected domain, mechanism, question, candidate, and relations. Read every linked Accepted, Rejected, Superseded, or Invalidated predecessor that shares the mechanism. Reuse a prior result only while its recorded identity dimensions still apply: workload and outcome semantics, semantic contract, resource policy, host and configuration, artifact identity, and measurement method. When a dimension must change, record either a labelled bridge comparison or the missing witness; never silently inherit inapplicable numbers. Either refine prior evidence or explain which assumption makes repetition necessary.

## Bounded Comparative Research

A sealed baseline remains historically valid and is not a permanently protected implementation. New research may admit a candidate that competes with the sealed baseline, including a frozen physical architecture such as C1, without proving the baseline wrong or reopening its decision. Comparative admission requires:

- One decision the comparison can change: an efficiency improvement, an explicit tradeoff, or an established limitation.
- The sealed baseline named with its exact record and artifact identity, held fixed as the reference.
- One smallest comparative claim under a matching workload, semantic contract, resource policy, host/configuration, and measurement method, or an explicitly labelled bridge when one dimension must change.
- Materiality declared from the decision and measurement method, not a universal percentage and not contingent on closing a numeric release target.
- A candidate set bounded by the owning campaign before measurement.

Target-closing-only admission is retired. A bounded comparison that decides an improvement, tradeoff, or limitation is admissible without reaching any release threshold, and a comparative candidate never has to invalidate the sealed baseline it competes with. A frozen decision reopens only through its declared invariant/impossibility trigger.

### Outcome Classes

When sealing a decision, name exactly one outcome class and its finite stopping basis:

- `Efficiency improvement`: comparable work and guarantees consume fewer resources or deliver better service under the same resource policy.
- `Explicit tradeoff`: an approved cost, capacity, latency, complexity, portability, or semantic change buys a stated benefit; it is not an unconditional improvement.
- `Research result`: a candidate is rejected or a limitation is established (a negative result); preserve it without claiming a performance record.

Cost moved into another resource domain, lifecycle phase, persistent state, or weaker guarantee is a tradeoff to assess, not a hidden efficiency gain.

Finite stopping is part of the decision, not an aspiration. Stop when the declared materiality is met, the named obligation is satisfied, the remaining delta is below materiality, the bottleneck has moved, or the frozen candidate set is exhausted. If every admitted candidate fails, the campaign closes with an explicit no-optimization research result. That disposition ends the campaign: it authorizes neither an endless search for a winner nor an automatic successor inside the same campaign. Admitting a new candidate requires a new explicit scope decision.

## Benchmark Reassessment Protocol

Benchmark numbers are observations produced by an imperfect host and procedure. Protocol semantics, storage topology, committed transitions, counters, ProofSize and database shapes may be exact; RefTime is an empirical observation. Treat these evidence classes differently.

`Observation identity is permanent; evidence authority is revisable.` Record source/tree, benchmark source, runtime/Wasm, generated Weight, command, parameters, toolchain, host facts, raw output and date once; never edit them to improve a later conclusion. Maintain a separate current disposition: `Authoritative`, `Qualified`, `Historical`, `Superseded`, `Invalidated`, `Inconclusive`. Use the record's `Benchmark Evidence Disposition` surface for the current disposition; `Not applicable` is sufficient for records without empirical benchmark evidence.

Reject both errors: that a generated result is authoritative forever, and that the newest measurement is automatically more authoritative. Authority = applicability + exact identity + measurement quality + reproducibility + comparative relevance. Reassess when the same source and command produce materially different RefTime fits, many untouched methods move together, raw storage or proof work stays identical while RefTime moves, a fitted coefficient becomes implausible relative to raw samples, baseline and candidate were measured under materially different host conditions, the benchmark tool, version or configuration changed, new repeated measurements contradict the previous ranking, the measured delta is of the same order as observed host variance, or a supposedly local change causes widespread unrelated drift. A trigger is not proof that the old result was wrong; it means its authority requires review.

### Sealed-Record Reassessment

Sealing an Experiment ID seals its historical decision identity, not the eternal authority of its empirical numbers. A sealed record may later be confirmed, qualified, superseded or invalidated by new evidence without renumbering or rewriting its original decision:

```text
original decision → new evidence → reassessment owner → current applicability
```

Record the reassessment in the owner's `Benchmark Evidence Disposition` surface and use the existing relations (`Confirms`, `Refines`, `Supersedes`, `Invalidates`, `Uses evidence from`) when decision scope changes. The track index carries the current disposition; the original status transition remains historical.

### Host-Constrained Comparative Protocol

A dedicated quiet benchmark host is not a release requirement. Prefer matched comparisons inside the host actually available:

```text
baseline A → candidate B → baseline A'   (or repeated A / B / A / B)
```

Use the smallest benchmark scope that can decide the candidate; do not repeat a full pallet generation when a focused comparison can establish environmental variance or candidate attribution. If `A ≈ A'` and `B` differs consistently, the comparative signal is stronger. If `A` and `A'` differ by roughly as much as `A` and `B`, the RefTime comparison is Inconclusive. Stop once the decision is robust.

For optimization decisions, a reproducible matched baseline/candidate delta may decide a candidate even when absolute RefTime varies between sessions, provided the execution paths are comparable, the sign of the delta is stable, the delta materially exceeds the observed comparison noise, structural metrics agree, and no protected resource or semantic regression occurs. This never means RefTime does not matter: it remains a production resource dimension.

### Noise Envelope and Dimension Separation

Never use an arbitrary universal percentage. Estimate a local noise envelope from repeated baseline runs, A/B/A drift, unchanged control methods, raw minimum variation, fit variation or whole-pallet unrelated movement. The minimum rule is: the candidate signal must be distinguishable from observed measurement noise. If it is not, the RefTime result is `Inconclusive` — not Accepted and not Rejected.

Keep authority separate per dimension: ProofSize, raw StorageProof bytes, database reads/writes, state bytes, candidate count, committed Steps, completed Cycles, latency blocks and RefTime. Equal ProofSize does not imply equal RefTime; unstable RefTime does not invalidate stable structural evidence.

### Production Weight Authority

Research comparison and production Weight follow different rules. For production Weight:

- `Case A — benchmarked execution owner unchanged`: an existing generated owner may remain authoritative only when the executed branch is benchmarked identically and the same storage reads/writes, bounded input domain, maximum branch geometry, host operations, state mutations and mandatory work hold. Any new selector, preflight or control-flow overhead outside that owner needs an explicit conservative Weight owner. No storage access or absent `meter.consume` is not a zero-RefTime proof: pure control flow still consumes execution time.
- `Case B — benchmarked execution path changed`: new storage or host work, a larger domain or an exceeded contract requires a new sound owner. Do not retain the old owner because fresh generation is noisy; if the host cannot produce a trustworthy binding, the production binding remains explicitly unresolved instead of hiding behind a stale coefficient.
- `Case C — fresh full generation environmentally unstable`: a generated complete file may be rejected when its movement is dominated by measurement instability. Rejection means this generation is inadmissible evidence, not that generation is optional. The previous owner remains current only under Case A or another separately proved containment argument.

Freshness is not evidence. Applicability, explicit ownership and soundness are evidence.

### Baseline Reassessment and Ranking

A released measurement stays the historical released value. A later experiment may establish a better estimate of the same implementation under a stronger protocol and record it as a `Reassessed comparison baseline`, keeping both; a candidate comparison may use the reassessed baseline when baseline and candidate share the method and the bridge to the released observation is explicit. Stronger evidence may reverse a candidate ranking: the old decision remains historical and is `Superseded` or `Invalidated` by the new owner. Reopening requires old evidence, new contradicting evidence, an applicability explanation and the smallest decision-relevant reassessment — never suspicion that another run might be faster.

### Anti-Cherry-Picking

Reassessment is not permission to rerun until a preferred number appears. Declare the comparison method before the decision run; retain or summarize every decision-relevant run; record an invalid-run reason before using its replacement; apply the same inclusion rule to baseline and candidate; and stop when the evidence is sufficient or Inconclusive. A noisy or Inconclusive result is a valid research outcome.

## Experiment Protocol

### 1. Lock the Decision Question

State one decision, record kind, owning implementation phase, governing specification sections, finite proof obligations and semantic invariants. Separate physical variables from semantic constants. A Synthesis declares its child decisions instead of preparing benchmark candidates; measurement protocol steps apply to the owning Leaves.

Declare materiality before measurement. Use a release objective, maximum RefTime/ProofSize envelope, lifecycle dispatchability, throughput or latency bound, eliminated read/write/scaling dependency, state-hold reduction, justified minimum percentage, or a comparative decision against a sealed baseline. A tiny numeric win without architectural significance is not material by default.

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

Match statistical method to evidence class. For noisy wall-clock work, declare warmup, repeated-run count and rationale, distribution or relevant percentiles, variance, and predeclared outlier handling; do not hide tails behind an average. Keep setup deterministic and outside the measured region, record host/cache conditions and competing processes, isolate only where the method can, and record random seed and contamination. Host, governor and load facts are explanatory context, not binary validity switches; authority and noise decisions follow the Benchmark Reassessment Protocol. For deterministic/model-generated FRAME Weight, use the required steps/repeats and generated model review rather than irrelevant statistical ceremony.

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

Keep every normalized or raw tabular dataset directly in the Markdown record as a compact table. CSV, TSV, and equivalent delimited-table files are temporary interchange only: integrate their complete decision-relevant rows into `EXP-NNNN.md` and delete them before the experiment checkpoint. Size or machine consumption does not justify a separate tabular artifact; split or summarize the inline table without discarding decision-relevant evidence. Retain a separate raw log, plot, binary, trace, or other non-tabular artifact only when fidelity or reviewability genuinely requires another file. Store such artifacts under the sibling track-qualified `tracks/<track>/EXP-NNNN/` directory and link specific files from the record; never use a global `evidence/EXP-NNNN/` path because evidence stays with its owning track and record. Retain full raw output only when small, uniquely valuable, or required to review/reproduce the decision. Large output may remain external or ephemeral only when the record preserves exact commands, hashes, parameters, environment, and sufficient normalized measurements.

Do not claim reproducibility when an essential artifact or condition was discarded. Never use one ambiguous `artifact_hash`; identify source tree, benchmark Wasm, production Wasm, generated Weight, and raw-output digest separately.

### 7. Separate Result, Interpretation, and Decision

- `Result`: What was measured, including uncertainty and deltas.
- `Interpretation`: What the evidence implies, which dimension binds, limitations, confounders, and Pareto relation.
- `Decision`: Which candidate is selected or why none is selected, against declared criteria; name the single outcome class and the finite stopping condition that applies.

A lower RefTime does not imply acceptance. Classify candidates as Pareto-improving, Pareto-dominated, a tradeoff, or a binding-dimension winner. Do not collapse dimensions into an arbitrary scalar unless the governing resource policy defines and justifies that objective.

Negative outcomes are valid: no material difference, regression, inconclusive, invalid experiment, rejected candidate, or falsified hypothesis. Record each rejected alternative with evidence and reason. A rejected candidate, a bounded limitation, or a no-optimization campaign disposition is a research result with the same evidentiary standing as a selected improvement; record it once and stop instead of searching for a substitute winner.

### 8. Update Baseline and Lineage

For Accepted decisions, name the new physical baseline and relate the record with `replaces`, `refines`, or `validates`. For later contrary evidence, use `supersedes`, `contradicts`, or `invalidates`. Relations are bidirectionally discoverable through index rows and record links.

Update the primary track's accepted baseline only after an Accepted decision changes that track's physical baseline. Preserve `Affected domain` and `Physical mechanism` inside the record as finer evidence labels; they do not replace track ownership. Release remains an index column rather than a release-local archive.

### 9. Derive the Next Gradient

Do not brainstorm from zero. Ask in order:

1. Which objective is unmet or furthest from its envelope?
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
- Every sealed decision names its outcome class and finite stopping basis; an exhausted campaign records the explicit no-optimization disposition as a research result.
- Every retained production Weight owner is sound for the segment it charges: applicable, explicitly owned, and free of placeholder or stale scope. Freshness of generation is not evidence.
- No decision exists only in chat, temporary output, commit messages, or developer memory.

### Architecture Provenance Gate

Domain Architecture may close only when every significant physical decision is traceable to at least one normative specification section, Accepted Experiment Record, production benchmark, or correctness/security invariant. Architecture states current truth and cites provenance compactly; it does not become an experiment log.

## Stop Rules

Stop the experiment loop when any applies:

- The declared objective or materiality is met.
- The named proof obligation is satisfied: stop measuring it.
- A new independent question is discovered: transfer it and stop the parent investigation.
- The next useful work belongs to another owner/domain: stop this experiment.
- A child result is sufficient for synthesis: do not seek more branch coverage for completeness.
- No candidate has a plausible decision-relevant advantage.
- The frozen candidate set is exhausted: record the no-optimization disposition; a new candidate is a new scope decision, not another iteration.
- Remaining delta is below declared materiality.
- The bottleneck moved to another architecture domain.
- Further progress requires semantic change and therefore Specification reopening.
- Evidence is insufficient; mark Inconclusive and state the exact missing evidence.
- The task contract or user says stop.

“There might be another corner case” is insufficient. A new corner case must name an uncovered owner, domain or acceptance claim. Proof complexity grows by adding earned graph nodes, not by extending one notebook indefinitely. Performance never outranks correctness, deterministic semantics, atomicity, FIFO, causal speed, ownership, rollback, runtime safety, or production Weight soundness.

## Track Registry

Track indexes are the canonical navigation surfaces for experiment status and lineage.

| Track | Scope | Index |
| --- | --- | --- |
| Actors | Actor storage, scheduling, control, lifecycle, and executor topology | [tracks/actors/experiments.md](./tracks/actors/experiments.md) |
| Adapters | Runtime adapter boundaries, lowering, and effect-resource evidence | [tracks/adapters/experiments.md](./tracks/adapters/experiments.md) |
| Router | Route search, quote, proof, and execution topology | [tracks/router/experiments.md](./tracks/router/experiments.md) |

Add a track only after its boundary, invariants, portfolio, dependencies, and entry/exit conditions are concrete. Create `tracks/<track>/experiments.md` first; do not reserve empty directories. Track-local indexes register globally allocated IDs, own statuses and relations, and preserve rejected or invalidated records.

## Handoff

Report:

- Experiment ID, status transition, decision question, and mechanism.
- Exact baseline and candidate artifact identities.
- Evidence class, workloads, controlled/changed variables, and validation.
- Result, Interpretation, and Decision as separate statements, with the outcome class and finite stopping basis named.
- Benchmark evidence disposition, reassessment trigger, compared observation identities, and the measured noise envelope or its Inconclusive disposition.
- Pareto class, binding dimension, rejected alternatives, and validity scope.
- New baseline or reason none changed.
- Next gradient or stop condition.
- Index, backlog, and eventual architecture provenance updates.
