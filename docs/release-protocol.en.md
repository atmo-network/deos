# DEOS Release Protocol

> Operational contract for pre-release acceptance, evidence identity, review order, and guarded delivery. Runtime semantics remain owned by specifications and executable code.

## Scope and Authority

- `BACKLOG.md` owns unfinished release gates.
- `CHANGELOG.md` owns completed delivery outcomes.
- Subsystem specifications own normative behavior.
- Generated metadata, weights, manifests, vectors, and Wasm own their exact artifact identities.
- This document owns release choreography and evidence interpretation, not implementation semantics.

## Candidate Discipline

- Bind each candidate to one accepted specification hash before implementation acceptance.
- Treat pre-`1.0.0` contract corrections as canonical replacement rather than compatibility layering.
- Keep unrelated future release lines outside the active candidate scope.
- Preserve explicit non-goals; a release cannot silently acquire a new capability to satisfy a gate.

## Review Order

- Review the accepted specification and canonical backlog first.
- Review authored runtime and package behavior before generated output.
- Review tests and independent embedding evidence before client projections.
- Review metadata, descriptors, manifests, vectors, weights, and Wasm only after authored semantics stabilize.
- Review documentation, wiki projection, backlog closure, and changelog truth last.

## Churn and Contraction Accounting

- Compare authored additions and deletions against the named baseline separately from generated artifacts.
- Count runtime, tests, scripts, client source, and documentation as authored surfaces.
- Account for generated metadata, descriptors, weights, manifests, vectors, Wasm, and lockfiles separately.
- A contraction claim requires net authored deletion and reduced semantic ownership, not merely renamed or generated churn.
- Any unavoidable public addition must implement an accepted requirement and remove a larger competing surface.

## Evidence Identity

- Record SHA-256 identities only for current reproducible candidate artifacts.
- Regenerate metadata and client descriptors after every public ABI change.
- Regenerate production weights after measured paths or storage topology change.
- Rebuild production Wasm after runtime code or accepted production weights change.
- Refresh dependent manifests, vectors, and observation evidence after their bound identities change.
- Never present stale, provisional, diagnostic, or locally inferred evidence as current production evidence.

## Validation Escalation

- Run focused changed-scope tests first.
- Run package and embedding checks when a reusable boundary changes.
- Run runtime integration checks when composition, adapters, weights, or metadata change.
- Run client automation and type checks when public projections or evidence identities change.
- Run wiki trust and consolidation checks after wiki projection changes.
- Run `./.agents/skills/alignment/scripts/completion-gate.sh` after knowledge synchronization.
- A failed required gate keeps the corresponding backlog item open.

## Benchmark Acceptance

- Benchmark compilation proves registration only.
- Focused runs provide diagnostic evidence only.
- Production weights require the production benchmark runtime, accepted steps and repeats, reviewed storage annotations, and successful generation of the complete pallet weight file.
- Runtime capacity claims require the runtime-bound generated method and rebuilt production Wasm.

## External Boundaries

- Do not publish, tag, deploy, submit transactions, sign payloads, or mutate external accounts without explicit authorization.
- Do not describe an external CI, governance, audit, or deployment gate as locally verified.
- Keep exact unblockers in `BACKLOG.md` when acceptance depends on external evidence.

## Acceptance and Handoff

- Close backlog entries immediately when their complete local contract passes.
- Add one changelog outcome for meaningful delivered impact; omit intermediate implementation diary entries.
- Confirm accepted specification, public ABI, storage schema, generated evidence, client projections, and documentation agree.
- Confirm release invariants and exit-state checks from repository reality rather than intent.
- Report changed paths, validation evidence, remaining gates, and exact external unblockers.
