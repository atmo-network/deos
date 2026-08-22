# DEOS Backlog

> Open framework work only. Completed delivery history belongs in `CHANGELOG.md`; runtime semantics remain owned by the corresponding specifications, architecture documents, code, generated metadata, and production Weight.
>
> Pre-`1.0` boundary: no DEOS network will be launched before `1.0`. The `0.7.x` line remains fresh-genesis and may change storage, metadata, runtime APIs, validation topology, and release mechanics without deployed-lineage migration or live-network ceremony.

## DEOS 0.7.22 — Reactive Topology and Relevant-Work Scaling

> **Release framing:** `Semantic Activation + Heterogeneous Detection + Pre-1.0 Delivery Compression`.
>
> **Dependency:** Start from accepted `v0.7.21` Runtime Truth Closure. Preserve its canonical five-partition Actor state, strict FIFO, typed failure algebra, ledger-only fee collection, certified-movement atomicity, checked arithmetic, bounded Governance service, and production-generated Weight.
>
> **Scope:** Close the remaining `0.7.21` P0 dispatchability defect; make release identity unique; simplify CI around the actual pre-`1.0` risk boundary; add one useful declarative trigger family, `ObservationCrossing`; derive specialized physical detection indexes per trigger family; converge every detected condition through one activation latch and canonical FIFO; and reduce work from “all subscribers” toward “only semantically affected Actors”.
>
> **Governing topology rule:** `specialize detection → unify activation → unify execution`.
>
> Physical indexes, pages, cursors, batch sizes, scan geometry, and worker limits are runtime configuration and generated Weight concerns. They MUST NOT become Actor Contract fields or affect Actor Contract identity.
>
> **Delivery order:** (1) Compress CI and lock the fresh-genesis release boundary; (2) close signed Governance dispatchability and release-ref ambiguity; (3) freeze `ObservationCrossing` semantics and transition durability; (4) implement heterogeneous activation indexes; (5) converge detection through one bounded activation sink and FIFO; (6) enforce the System/User cycle policy; (7) eliminate avoidable canonical-state rereads; and (8) regenerate metadata, weights, client evidence, documentation, and release history.

## P0 — Canonical Release Identity

- [ ] `Release / One Version Ref`: Allow the dedicated `0.7.22` branch as a temporary development and review ref; require its pull request to pass `validation-gate` and merge into `main`; delete the remote plain branch after merge and before tagging; and establish `v0.7.22` on that accepted `main` commit as the sole persistent release ref identifying exactly one validated tree.
- [ ] `Release / One Version Ref`: Delete the obsolete remote `0.7.21` merged pull-request source branch; retain the published annotated `v0.7.21` tag on accepted `main` commit `997cb28d2a364c64e52dc394c366852d810431c5` as the sole canonical release ref, without moving the consumed tag.

## P3 — Documentation, Packaging, and Release Closure / Specifications

- [ ] `Wiki / Bilingual Approval`: Obtain independent bilingual `APPROVE` evidence for all 46 English/Russian mirrors, localized manifests, and WikiWidget strings after the incremental projection passes strict OKF, trust, graph, frontend-relation, consolidation, and native-Russian heuristic gates.

## P3 — Documentation, Packaging, and Release Closure / Validation

- [ ] `Validation / Pull Request`: Pass the single pull-request `validation-gate`.

## DEOS 0.7.22 Definition of Done

- [ ] `Definition of Done / Release Identity`: Make `v0.7.22` name exactly one release tree with no parallel plain version ref.
- [ ] `Definition of Done / Closure`: Return `BACKLOG.md` to `No open work.` only after the local full profile and pull-request gate pass on the final candidate.

## DEOS 0.7.22 Non-Goals

- `Non-Goal / Launch`: No network launch, `Live` preset, deployed-lineage migration, upgrade rehearsal, or `1.0` declaration.
- `Non-Goal / Remote Release`: No main-push CI, tag CI, network CI, publication pipeline, signed release attestation, or deployment-grade supply-chain ceremony.
- `Non-Goal / Trigger Language`: No generic trigger-expression language, nested Boolean trigger tree, user-authored debounce, polling interval, fanout page size, threshold bucket size, scan limit, batch size, or index selection.
- `Non-Goal / Universal Index`: No universal activation index shared by all trigger families.
- `Non-Goal / Scheduling`: No priority queue, System execution lane, threshold lane, cheap-Actor lane, FIFO bypass, or same-block actor-graph continuity.
- `Non-Goal / Off-Chain Truth`: No off-chain index as consensus truth and no event-log scan as an ingress/detection substitute.
- `Non-Goal / User Cycles`: No prohibition or generic graph analysis for User Actor cycles; their bound remains paid execution plus economic apoptosis.
- `Non-Goal / Open-World DAG`: No proof that external markets, Oracle publishers, users, or uncertified movements form an acyclic graph. The System DAG covers only explicit runtime-owned activation surfaces.
- `Non-Goal / Mechanism Expansion`: No new Task, amount mode, retry policy, Router route family, DEX aggregation, solver market, batch settlement, or arbitrary dispatch.
- `Non-Goal / Runtime Truth Regression`: No rollback of the `0.7.21` canonical Actor loader, checked arithmetic, atomicity boundaries, or production Weight ownership in exchange for throughput.
