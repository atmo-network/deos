# Adapters Experiment Track

| Field | Value |
| --- | --- |
| Track ID | adapters |
| Status | Active |
| Scope owner | Runtime adapter physical boundaries, lowering, and effect-resource evidence |
| Depends on tracks | [router](../router/experiments.md) |

## Scope and Boundary

- `Owns`: Host adapter interface geometry, effect-Weight ports, actual-versus-maximum accounting, failure-class plumbing, generic lowering, and portability costs.
- `Excludes`: Protocol semantics, Actor queue/control topology, Router route-selection mechanics, open work, and experiment results.

## Governing Invariants

- Preserve host-defined semantics, transactional effects, typed failure classes, bounded resource ownership, exact non-overlapping Weight attribution, generic pallet portability, and fail-closed unsupported paths.

## Accepted Physical Baseline

- None established by an Accepted Experiment Record. Current code and production Weight remain implementation evidence only.

## Research Portfolio

- Effect-resource envelope representation and adapter-returned actual Weight.
- Generic versus task-family lowering and interface-surface pressure.
- Cross-pallet proof ownership, portability, and failure-path geometry.

## Cross-Track Dependencies

- Adapters consumes Router-owned route and execution envelopes when exposing swap effects.
- [Actors](../actors/experiments.md) may consume adapter evidence but does not determine adapter internals.
- Dependency direction is `Router → Adapters → Actors`; shared questions receive one primary owner instead of duplicate experiments.

## Entry and Exit Conditions

- `Entry`: The decision changes a runtime adapter boundary or effect-resource realization while preserving caller semantics.
- `Exit`: Actor scheduling questions transfer to Actors; route selection and quote topology transfer to Router; this track becomes Dormant when adapter geometry has no decision pressure.

## Experiment Index

No experiment is allocated. Add the first record only when a decision-relevant hypothesis, comparable baseline/candidate, materiality threshold, and smallest falsifier are known.

| ID | Release | Status | Question | Baseline | Result / decision | Successor |
| --- | --- | --- | --- | --- | --- | --- |
| None | — | — | No experiment opened | — | — | — |

## Maintenance

- Allocate `EXP-0001` when the first real Adapters experiment is opened; IDs are monotonic within this track.
- Create `EXP-NNNN.md` before Proposed becomes Prepared, and update this index with every lifecycle or relation change.
- Keep measurements, interpretation, decisions, and artifacts in the record rather than this index.
