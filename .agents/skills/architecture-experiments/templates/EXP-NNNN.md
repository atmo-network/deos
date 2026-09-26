# EXP-NNNN — Short Decision Title

| Field | Value |
| --- | --- |
| Status | Proposed / Prepared / Measuring / Measured / Interpreted / Accepted / Rejected / Inconclusive / Superseded / Invalidated |
| Record kind | Leaf / Consolidated |
| Absorbs | None, or archived IDs whose decisions this record carries forward |
| Release | X.Y.Z |
| Date | YYYY-MM-DD |
| Physical mechanism | Stable cross-release mechanism name |
| Primary track | Replace with a link to sibling `./experiments.md` |
| Governing specification | Path and section |

## Decision Question

The single physical choice or bounded comparison this evidence decides. A Consolidated record states one umbrella question whose obligations share one mechanism family.

## Context and Candidates

- `Current architecture`: Exact baseline behavior.
- `Hypothesis`: Falsifiable claim.
- `Materiality`: Decision-relevant threshold declared before measurement.
- `Controlled / changed variables`: What is held equal and what differs; name known confounders.

| ID | Architecture | Source/runtime identity |
| --- | --- | --- |
| Baseline | Exact baseline | Commit, tree or constants |

## Workloads and Criteria

- `Evidence class`: Microbenchmark / pallet / native stress / integration / production-Wasm / full-runtime block / release-tree validation.
- `Acceptance criteria`: Selection rule declared before measurement.
- `Rejection criteria`: Semantic, correctness, multidimensional or materiality failure.

| Workload | Geometry | Purpose |
| --- | --- | --- |
| W1 | Exact bounded setup | Decision witness |

## Artifact Identity

Source commit/tree, toolchain, benchmark and production Wasm, generated Weight, command and parameters, database backend. A Consolidated record may cite its archived source records for full identities.

## Proof Obligations

- `Freeze`: Not frozen until Measuring; then `Frozen <date or commit>`.

| Obligation ID | Claim | Smallest falsifier | Evidence class | Status | Consumer |
| --- | --- | --- | --- | --- | --- |
| O1 | One falsifiable claim | Smallest deciding witness | Exact class | Open / Satisfied / Rejected / Inconclusive / Accepted / Requires refresh | Backlog owner, record, or None |

A Leaf owns exactly one obligation. A Consolidated record owns two or more obligations of one mechanism family, each with its own status.

## Measurements

Decision-relevant numbers only, as Markdown tables; keep RefTime, ProofSize, reads/writes and other dimensions separate.

### Benchmark Evidence Disposition

- `Benchmark Evidence Status`: Authoritative / Qualified / Historical / Superseded / Invalidated / Inconclusive / Not applicable.
- `Reassessment Trigger`: None, or the exact trigger.
- `Compared Observation IDs`: None, or the compared run identities.
- `Noise / Stability Evidence`: None, or the measured envelope.
- `Current Authority`: What these numbers still decide, for whom.

## Result

Observations, uncertainty and negative results, without selecting a candidate.

## Interpretation

Binding dimension, dominant contributor, Pareto relation, confounders and non-implications.

## Decision

Decision, new baseline or why none changed, one outcome class (efficiency improvement, explicit tradeoff, research result) and the finite stopping basis.

## Validity

- `Establishes`: Exact supported claim.
- `Does not establish`: Explicit non-claims.
- `Invalidation triggers`: Changes that require review.

## Next Gradient

`target → measured gap → binding dimension → dominant contributor → owning mechanism → next hypothesis → smallest falsifier`, or the stop condition.

## Relations

- `Depends on`: Required accepted inputs, or None.
- `Uses evidence from`: Borrowed observations, or None.
- `Produces input for`: Downstream records or backlog owners, or None.
- `Supersedes`: None.
- `Invalidates`: None.
- `Transfers question to`: None.
- `Reopen trigger`: Exact failed assumption and required evidence.
