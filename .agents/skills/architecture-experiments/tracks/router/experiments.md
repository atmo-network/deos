# Router Experiment Track

| Field | Value |
| --- | --- |
| Track ID | router |
| Status | Active |
| Scope owner | DEOS Router physical route, quote, and execution architecture |
| Depends on tracks | None |

## Scope and Boundary

- `Owns`: Candidate enumeration geometry, bounded Native-anchored routing, quote/execution representation, route proof topology, and Router lifecycle/throughput tradeoffs.
- `Excludes`: Maximum-recipient-output semantics, downstream adapter wrappers, Actor scheduling, open work, and experiment results.

## Governing Invariants

- Preserve maximum recipient output among viable candidates, deterministic bounded search, fee-burning behavior, typed quote metadata, transactional execution, reserve safety, and production Weight soundness.

## Accepted Physical Baseline

- None established by an Accepted Experiment Record. Current Router implementation and generated Weight are implementation evidence only.

## Research Portfolio

- Direct versus bounded Native-anchored route geometry.
- Candidate quote representation and proof/read sharing.
- Quote-to-execution commitment, failure branches, and throughput limits.

## Cross-Track Dependencies

- Router produces bounded route/effect evidence that may be consumed by adapter and [Actors](../actors/experiments.md) research.
- Router experiments must not depend on adapter wrappers or Actor queue topology for correctness.

## Entry and Exit Conditions

- `Entry`: The decision changes Router-owned physical search, quote, route, or execution geometry without changing routing semantics.
- `Exit`: Wrapper/lowering questions transfer to adapter research; Actor service composition transfers to Actors; this track becomes Dormant when no Router decision remains.

## Experiment Index

No Router experiment has been opened or measured. The portfolio above is directional only and does not allocate IDs.

| ID | Release | Status | Question | Baseline | Result / decision | Successor |
| --- | --- | --- | --- | --- | --- | --- |
| None | — | — | No experiment opened | — | — | — |

## Maintenance

- Allocate `EXP-0001` only when the first real Router hypothesis has a comparable baseline/candidate, materiality threshold, and smallest falsifier.
- Create `EXP-NNNN.md` before Proposed becomes Prepared, and update this index with every lifecycle or relation change.
- Keep measurements, interpretation, decisions, and artifacts in the record rather than this index.
