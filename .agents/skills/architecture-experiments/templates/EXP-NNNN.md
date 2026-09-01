# EXP-NNNN — Short Decision Title

| Field | Value |
| --- | --- |
| Status | Proposed / Prepared / Measuring / Measured / Interpreted / Accepted / Rejected / Inconclusive / Superseded / Invalidated |
| Architecture release / experiment campaign | Measured X.Y.Z baseline / campaign X.Y.Z |
| Date | YYYY-MM-DD or Not recorded |
| Affected domain | Domain |
| Physical mechanism | Stable cross-release mechanism name |
| Primary track | Replace with a link to sibling `./experiments.md` |
| Related tracks | None or links to `../<track>/experiments.md` |
| Baseline experiment | EXP-NNNN or None |
| Successor experiment | EXP-NNNN or None |

## Decision Question

State the single implementation choice this evidence can decide.

## Governing Specification

- `Exact section`: Path and heading.
- `Invariant`: Candidate-invariant semantic rule.
- `Reopen trigger`: Finding that would require Specification reopening rather than candidate selection.

## Context and Hypothesis

- `Current architecture`: Exact physical behavior at the baseline.
- `Reason`: Measured gap or decision pressure.
- `Hypothesis`: Falsifiable candidate claim.
- `Materiality`: Minimum decision-relevant change or release envelope.

## Baseline and Candidates

| ID | Architecture | Source tree | Runtime/config identity |
| --- | --- | --- | --- |
| Baseline | Exact baseline | Commit and tree | Relevant constants |
| A | Candidate A | Commit and tree | Relevant constants |

## Controls

- `Controlled variables`: Toolchain, Wasm build, Weight implementation, database, command, repeats, state, workload, cache assumptions, and constants held equal.
- `Changed variables`: Exact intended differences.
- `Confounders`: Known uncontrolled differences; state None when none are known.

## Benchmark Class, Workloads, and Criteria

- `Benchmark class`: Microbenchmark / pallet / native stress / integration / production-Wasm / full-runtime block / release-tree validation.

| Workload | State/population geometry | Hot/cold assumptions | Purpose |
| --- | --- | --- | --- |
| W1 | Exact bounded setup | Declared | Decision witness |

- `Metrics`: Relevant dimensions; omit only with rationale.
- `Statistical method`: Warmup, samples, repeats, variance/distribution, percentiles, and outlier policy where applicable; explain when deterministic Weight methodology makes them irrelevant. Integrate every CSV/TSV-style dataset below as Markdown rather than retaining a delimited artifact.
- `Acceptance criteria`: Candidate-selection rule declared before measurement.
- `Rejection criteria`: Semantic, correctness, multidimensional, or materiality failure.

## Environment and Artifact Identity

| Artifact | Identity |
| --- | --- |
| Source commit | Hash |
| Source tree | Tree hash |
| Rust toolchain | Exact version |
| Benchmark runtime Wasm | Hash or Not applicable |
| Post-generation production runtime Wasm | Hash or Not measured |
| Final release production runtime Wasm | Hash or Not measured |
| Generated Weight | File/hash/method identity or Not applicable |
| Database/backend | Exact configuration |
| Benchmark command/config | Exact command and parameters |
| Raw evidence | Inline tables below; repository path/digest only for justified non-tabular artifacts, or Not retained with reason |

## Measurements

| Candidate | Workload | RefTime | ProofSize | Reads | Writes | Other decision metrics |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Baseline | W1 | Not measured | Not measured | Not measured | Not measured | Not measured |

## Derived Metrics

| Candidate | Workload | RefTime delta | ProofSize delta | Throughput / latency | Marginal or amortized cost | Pareto class |
| --- | --- | ---: | ---: | --- | --- | --- |
| A | W1 | Not derived | Not derived | Not derived | Not derived | Not interpreted |

## Statistical / Variance Notes

Report warmup, distribution, variance, tails, outliers, contamination, and sample adequacy where applicable. For deterministic generated Weight, record model/repeat review instead.

## Result

Measurement only. Report observations, uncertainty, and negative results without selecting a candidate.

## Interpretation

Explain binding dimension, dominant contributor, Pareto relation, confounders, and what the evidence does or does not imply.

## Decision

State Accepted, Rejected, Inconclusive, Superseded, or Invalidated; identify the selected/new baseline or explain why none changed. Never merge this section with Result.

## Rejected Alternatives and Tradeoffs

| Candidate | Decision | Evidence-backed reason |
| --- | --- | --- |
| Candidate | Pending | Pending measurement |

## Validity

- `Establishes`: Exact supported claim.
- `Does not establish`: Explicit non-claims.
- `Known limitations`: Missing profiles, uncertainty, or scope restrictions.
- `Invalidation triggers`: Layout, runtime, toolchain, policy, adapter, Weight, or methodology changes that require review.
- `Residual qualitative insight`: Useful intuition if numeric authority later becomes invalid.

## Follow-Up / Next Gradient

```text
target → measured gap → binding dimension → dominant contributor
→ owning mechanism → next hypothesis → smallest falsifier
```

- `Eliminated hypotheses`: None yet.
- `Remaining hypotheses`: Candidate list.
- `Stop condition`: Exact target/materiality boundary.

## Architecture Impact

- `Affected physical truth`: Domain/mechanism.
- `Architecture update`: Required only after implementation, tests, and correction converge.
- `Provenance citation`: Suggested compact `EXP-NNNN` reference.

## Relations

- `Replaces`: None.
- `Supersedes`: None.
- `Refines`: None.
- `Contradicts`: None.
- `Depends on`: Governing specification and baseline IDs.
- `Baseline of`: None.
- `Confirms`: None.
- `Invalidates`: None.
- `Uses evidence from`: Exact historical experiment/evidence links; distinguish borrowed observations from required accepted inputs.
- `Transfers question to`: Experiment links and the specific transferred question, or None.
- `Produces input for`: Downstream experiment links and deliverable, or None.
- `Reopen trigger`: Exact failed assumption/invariant and required evidence; cost alone does not reopen a frozen physical baseline.
